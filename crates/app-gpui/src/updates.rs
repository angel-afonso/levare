//! Keeps Levare up to date: looks for a newer release shortly after start-up
//! and every few hours (unless the user turns that off), downloads it in the
//! background, and swaps it in when the user restarts. Checking, verifying
//! and installing live in `levare-update`; this is the app's side: the
//! schedule, the status bar and the restart.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use gpui::{Context, Timer};
use levare_update::{Installation, Update, Updater};
use serde::{Deserialize, Serialize};

use crate::AppView;
use crate::app_paths::data_path;
use crate::autosave::autosave_now;

/// The key the release workflow signs updates with; its secret half is the
/// repository's UPDATE_SIGNING_KEY (see docs/releasing.md).
const PUBLIC_KEY: &str = include_str!("../assets/update.pub");
/// Long enough after start-up not to slow it down.
const FIRST_CHECK: Duration = Duration::from_secs(10);
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);
const VERSION: &str = env!("CARGO_PKG_VERSION");

enum Status {
    Idle,
    Checking,
    Downloading {
        version: String,
        percent: Option<u8>,
    },
    Ready {
        update: Update,
        archive: PathBuf,
    },
    Installing,
    /// Swapped in, but something had to finish before restarting.
    Installed {
        path: PathBuf,
    },
}

pub struct Updates {
    status: Status,
    /// Why updates are unavailable, when they are (a development build, an
    /// app not running from Levare.app, a folder Levare cannot write).
    updater: Result<(Arc<Updater>, Installation), String>,
    automatic: bool,
}

/// `actualizaciones.json` in the data folder.
#[derive(Serialize, Deserialize)]
struct Settings {
    automaticas: bool,
}

/// `reabrir.json`: the saved score to open again after restarting into an
/// update. Unsaved work comes back through the autosave instead.
#[derive(Serialize, Deserialize)]
struct Reopen {
    partitura: PathBuf,
}

impl Updates {
    pub fn new() -> Self {
        Self {
            status: Status::Idle,
            updater: updater(),
            automatic: data_path("actualizaciones.json")
                .as_deref()
                .is_none_or(load_automatic),
        }
    }

    /// The status bar's update item, when an update is under way, and
    /// whether clicking it restarts into the update.
    pub fn status_item(&self) -> Option<(String, bool)> {
        let restarts = matches!(self.status, Status::Ready { .. } | Status::Installed { .. });
        status_text(&self.status).map(|text| (text, restarts))
    }
}

fn updater() -> Result<(Arc<Updater>, Installation), String> {
    if cfg!(debug_assertions) {
        return Err("Las actualizaciones no funcionan en las compilaciones de desarrollo.".into());
    }
    let platform =
        levare_update::platform().ok_or("No se publican actualizaciones para este ordenador.")?;
    let installation = Installation::current()
        .map_err(|error| format!("Levare no puede actualizarse solo: {error}"))?;
    let updater = Updater::new(levare_update::MANIFEST_URL, PUBLIC_KEY, VERSION, platform)
        .map_err(|error| error.to_string())?;
    Ok((Arc::new(updater), installation))
}

/// Starts the schedule and clears what an interrupted update left. Called
/// while the view is being built.
pub fn start(updates: &Updates, cx: &mut Context<AppView>) {
    let Ok((_, installation)) = &updates.updater else {
        return;
    };
    let installation = installation.clone();
    // The archive an earlier update installed is no longer needed.
    let downloads = data_path("actualizaciones");
    cx.background_executor()
        .spawn(async move {
            installation.clean_up();
            if let Some(downloads) = downloads {
                let _ = std::fs::remove_dir_all(downloads);
            }
        })
        .detach();
    cx.spawn(async move |view, cx| {
        Timer::after(FIRST_CHECK).await;
        loop {
            let alive = view.update(cx, |view, cx| {
                if view.updates.automatic {
                    view.check_for_updates(false, cx);
                }
            });
            if alive.is_err() {
                return;
            }
            Timer::after(EVERY).await;
        }
    })
    .detach();
}

impl AppView {
    /// Looks for a newer release and downloads it. `manual` checks (the
    /// command) say what they found; automatic ones stay quiet unless an
    /// update is ready.
    pub(crate) fn check_for_updates(&mut self, manual: bool, cx: &mut Context<Self>) {
        let updater = match &self.updates.updater {
            Ok((updater, _)) => updater.clone(),
            Err(why) => {
                if manual {
                    self.status = why.clone();
                    cx.notify();
                }
                return;
            }
        };
        match &self.updates.status {
            Status::Idle => {}
            Status::Ready { update, .. } => {
                if manual {
                    self.status = ready_message(&update.version.to_string());
                    cx.notify();
                }
                return;
            }
            Status::Checking
            | Status::Downloading { .. }
            | Status::Installing
            | Status::Installed { .. } => {
                if manual {
                    self.status = "Levare ya se está actualizando.".into();
                    cx.notify();
                }
                return;
            }
        }
        self.set_update_status(Status::Checking, cx);
        cx.spawn(async move |view, cx| {
            let found = {
                let updater = updater.clone();
                cx.background_executor()
                    .spawn(async move { updater.check() })
                    .await
            };
            let _ = view.update(cx, |view, cx| match found {
                Ok(Some(update)) => view.download_update(updater, update, manual, cx),
                Ok(None) => {
                    view.set_update_status(Status::Idle, cx);
                    if manual {
                        view.status = format!("Levare {VERSION} es la última versión.");
                    }
                }
                Err(error) => {
                    view.set_update_status(Status::Idle, cx);
                    view.report_update_failure(
                        manual,
                        format!("No se pudo buscar actualizaciones: {error}"),
                    );
                }
            });
        })
        .detach();
    }

    /// Downloads and verifies `update` into the cache, showing progress in
    /// the status bar, then offers the restart.
    fn download_update(
        &mut self,
        updater: Arc<Updater>,
        update: Update,
        manual: bool,
        cx: &mut Context<Self>,
    ) {
        let version = update.version.to_string();
        let Some(dir) = data_path("actualizaciones") else {
            self.set_update_status(Status::Idle, cx);
            self.report_update_failure(
                manual,
                format!("No se pudo descargar Levare {version}: no hay carpeta de datos."),
            );
            return;
        };
        self.set_update_status(
            Status::Downloading {
                version: version.clone(),
                percent: None,
            },
            cx,
        );
        let (progress, mut received) = futures::channel::mpsc::unbounded();
        let task = cx.background_executor().spawn(async move {
            // Only the latest download is kept.
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir)?;
            let mut last = None;
            let archive = updater.download(&update, &dir, |download| {
                let now = percent(download.received, download.total);
                if now != last {
                    last = now;
                    let _ = progress.unbounded_send(now);
                }
            })?;
            Ok::<_, levare_update::Error>((update, archive))
        });
        cx.spawn(async move |view, cx| {
            while let Some(percent) = received.next().await {
                let version = version.clone();
                let _ = view.update(cx, |view, cx| {
                    view.set_update_status(Status::Downloading { version, percent }, cx)
                });
            }
            let result = task.await;
            let _ = view.update(cx, |view, cx| match result {
                Ok((update, archive)) => {
                    view.status = ready_message(&update.version.to_string());
                    view.set_update_status(Status::Ready { update, archive }, cx);
                }
                Err(error) => {
                    view.set_update_status(Status::Idle, cx);
                    view.report_update_failure(
                        manual,
                        format!("No se pudo descargar Levare {version}: {error}"),
                    );
                }
            });
        })
        .detach();
    }

    /// Installs the downloaded update and restarts into it. Unsaved work is
    /// kept by the autosave and comes back on start-up; a saved score is
    /// opened again.
    pub(crate) fn restart_to_update(&mut self, cx: &mut Context<Self>) {
        if self.agent_blocks(cx) {
            return;
        }
        if let Some(why) = self.restart_blocker() {
            self.status = why.into();
            cx.notify();
            return;
        }
        let (archive, installation) = match (&self.updates.status, &self.updates.updater) {
            (Status::Installed { path }, _) => {
                let path = path.clone();
                self.restart_now(path, cx);
                return;
            }
            (Status::Ready { archive, .. }, Ok((_, installation))) => {
                (archive.clone(), installation.clone())
            }
            (Status::Installing, _) => return,
            _ => {
                self.status = "Todavía no hay ninguna actualización lista. «Buscar \
                               actualizaciones…» busca una."
                    .into();
                cx.notify();
                return;
            }
        };
        self.playback.stop();
        let ready = std::mem::replace(&mut self.updates.status, Status::Installing);
        cx.notify();
        cx.spawn(async move |view, cx| {
            let installed = cx
                .background_executor()
                .spawn(async move { installation.install(&archive) })
                .await;
            let _ = view.update(cx, |view, cx| match installed {
                // Something may have started while installing; the new app is
                // in place either way and runs from the next start.
                Ok(path) => match view.restart_blocker() {
                    Some(why) => {
                        view.status = format!("Actualización instalada. {why}");
                        view.set_update_status(Status::Installed { path }, cx);
                    }
                    None => view.restart_now(path, cx),
                },
                Err(error) => {
                    view.updates.status = ready;
                    view.status = format!(
                        "No se pudo instalar la actualización: {error}. Puedes descargarla \
                         desde la página de versiones."
                    );
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Why Levare cannot restart right now, if it cannot: something under
    /// way would be cut short, or unsaved work has no autosave to come back
    /// from.
    fn restart_blocker(&self) -> Option<&'static str> {
        if self.agent_locked() {
            return Some("Para al agente antes de reiniciar.");
        }
        if self.busy || self.confirming {
            return Some("Espera a que termine lo que está en curso antes de reiniciar.");
        }
        #[cfg(feature = "vsco")]
        if self.store.job.is_some() {
            return Some("Espera a que termine la instalación de sonidos antes de reiniciar.");
        }
        let autosaved = self
            .quit_snapshot
            .as_ref()
            .is_some_and(|recovery| recovery.last_error().is_none());
        if self.document.is_dirty() && !autosaved {
            return Some(
                "Guarda la partitura antes de reiniciar: el autoguardado no está disponible.",
            );
        }
        None
    }

    fn restart_now(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.playback.stop();
        // Unsaved work comes back only through the autosave, so its last
        // write must have reached the disk before Levare goes away.
        if let Err(error) = autosave_now(self.quit_snapshot.as_ref(), self.unsaved_snapshot()) {
            self.status = format!(
                "Actualización instalada, pero Levare no se reinicia: no se pudo autoguardar la \
                 partitura ({error}). Guárdala y vuelve a intentarlo."
            );
            self.set_update_status(Status::Installed { path }, cx);
            return;
        }
        if !self.document.is_dirty()
            && let (Some(score), Some(marker)) = (self.path.clone(), data_path("reabrir.json"))
        {
            let _ = save_reopen(&marker, &score);
        }
        // macOS reopens the bundle through GPUI's restart.
        #[cfg(target_os = "macos")]
        {
            cx.set_restart_path(path);
            cx.restart();
        }
        // GPUI's restart would paste the path into a script unquoted; see
        // `levare_update::relaunch_after_exit`.
        #[cfg(not(target_os = "macos"))]
        match levare_update::relaunch_after_exit(&path) {
            Ok(()) => cx.quit(),
            Err(error) => {
                // The update is in place; it runs from the next start.
                self.status = format!(
                    "Actualización instalada, pero no se pudo reiniciar Levare: {error}. \
                     Ciérralo y vuelve a abrirlo."
                );
                self.set_update_status(Status::Installed { path }, cx);
            }
        }
    }

    pub(crate) fn toggle_automatic_updates(&mut self, cx: &mut Context<Self>) {
        let on = !self.updates.automatic;
        let saved = data_path("actualizaciones.json")
            .ok_or_else(|| "no hay carpeta de datos".to_owned())
            .and_then(|path| save_automatic(&path, on).map_err(|error| error.to_string()));
        match saved {
            Ok(()) => {
                self.updates.automatic = on;
                self.status = if let (true, Err(why)) = (on, &self.updates.updater) {
                    // On, but nothing will come of it; say why now.
                    format!("Búsqueda automática de actualizaciones activada. {why}")
                } else if on {
                    "Levare buscará actualizaciones automáticamente.".into()
                } else {
                    "Búsqueda automática de actualizaciones desactivada. «Buscar \
                     actualizaciones…» sigue funcionando."
                        .into()
                };
                if on {
                    self.check_for_updates(false, cx);
                }
            }
            Err(error) => self.status = format!("No se pudo guardar el ajuste: {error}"),
        }
        cx.notify();
    }

    fn set_update_status(&mut self, status: Status, cx: &mut Context<Self>) {
        self.updates.status = status;
        cx.notify();
    }

    /// A failure the user asked about is shown; one from a background check
    /// only goes to the log, since nothing is wrong with what they are doing.
    fn report_update_failure(&mut self, manual: bool, message: String) {
        if manual {
            self.status = message;
        } else {
            crate::diagnostics::log(message);
        }
    }
}

fn ready_message(version: &str) -> String {
    format!("Levare {version} está listo: reinicia para actualizar.")
}

fn status_text(status: &Status) -> Option<String> {
    match status {
        Status::Idle => None,
        Status::Checking => Some("Buscando actualizaciones…".into()),
        Status::Downloading {
            version,
            percent: Some(percent),
        } => Some(format!("Descargando Levare {version} · {percent} %")),
        Status::Downloading { version, .. } => Some(format!("Descargando Levare {version}")),
        Status::Ready { update, .. } => {
            Some(format!("Reiniciar para actualizar a {}", update.version))
        }
        Status::Installing => Some("Instalando la actualización…".into()),
        Status::Installed { .. } => Some("Reiniciar para terminar la actualización".into()),
    }
}

/// How much of a download has arrived, when its size is known.
fn percent(received: u64, total: Option<u64>) -> Option<u8> {
    let total = total.filter(|total| *total > 0)?;
    Some((received.min(total) * 100 / total) as u8)
}

/// On unless the file says otherwise; a missing or damaged file keeps the
/// default.
fn load_automatic(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Settings>(&text).ok())
        .is_none_or(|settings| settings.automaticas)
}

fn save_automatic(path: &Path, on: bool) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(&Settings { automaticas: on })?;
    atomic_write::write(path, (text + "\n").as_bytes())
}

fn save_reopen(path: &Path, score: &Path) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(&Reopen {
        partitura: score.to_path_buf(),
    })?;
    atomic_write::write(path, (text + "\n").as_bytes())
}

/// The score to open again after an update's restart, once: the marker is
/// removed as it is read.
pub fn take_reopen() -> Option<PathBuf> {
    take_reopen_at(&data_path("reabrir.json")?)
}

fn take_reopen_at(path: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(path).ok()?;
    let _ = std::fs::remove_file(path);
    serde_json::from_str::<Reopen>(&text)
        .ok()
        .map(|reopen| reopen.partitura)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_of_a_download() {
        assert_eq!(percent(0, Some(200)), Some(0));
        assert_eq!(percent(50, Some(200)), Some(25));
        assert_eq!(percent(200, Some(200)), Some(100));
        assert_eq!(percent(300, Some(200)), Some(100));
        assert_eq!(percent(50, None), None);
        assert_eq!(percent(50, Some(0)), None);
    }

    #[test]
    fn the_status_bar_shows_each_update_step_and_nothing_at_rest() {
        let downloading = |percent| Status::Downloading {
            version: "0.3.0".into(),
            percent,
        };
        assert_eq!(status_text(&Status::Idle), None);
        assert_eq!(
            status_text(&Status::Checking).as_deref(),
            Some("Buscando actualizaciones…")
        );
        assert_eq!(
            status_text(&downloading(None)).as_deref(),
            Some("Descargando Levare 0.3.0")
        );
        assert_eq!(
            status_text(&downloading(Some(42))).as_deref(),
            Some("Descargando Levare 0.3.0 · 42 %")
        );
        assert_eq!(
            status_text(&ready()).as_deref(),
            Some("Reiniciar para actualizar a 0.3.0")
        );
        assert_eq!(
            status_text(&Status::Installing).as_deref(),
            Some("Instalando la actualización…")
        );
        assert_eq!(
            status_text(&Status::Installed {
                path: PathBuf::from("/Applications/Levare.app")
            })
            .as_deref(),
            Some("Reiniciar para terminar la actualización")
        );
    }

    #[test]
    fn only_an_update_to_restart_into_is_clickable() {
        let updates = |status| Updates {
            status,
            updater: Err(String::new()),
            automatic: true,
        };
        assert_eq!(updates(Status::Idle).status_item(), None);
        assert_eq!(
            updates(Status::Checking)
                .status_item()
                .map(|(_, ready)| ready),
            Some(false)
        );
        assert_eq!(
            updates(ready()).status_item().map(|(_, ready)| ready),
            Some(true)
        );
        let installed = Status::Installed {
            path: PathBuf::from("/Applications/Levare.app"),
        };
        assert_eq!(
            updates(installed).status_item().map(|(_, ready)| ready),
            Some(true)
        );
    }

    fn ready() -> Status {
        Status::Ready {
            update: Update {
                version: "0.3.0".parse().unwrap(),
                notes_url: String::new(),
                asset: levare_update::Asset {
                    url: String::new(),
                    sha256: String::new(),
                    signature: String::new(),
                },
                file_name: "levare.zip".into(),
            },
            archive: PathBuf::from("/datos/actualizaciones/levare.zip"),
        }
    }

    #[test]
    fn automatic_checks_are_on_unless_turned_off() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("actualizaciones.json");
        assert!(load_automatic(&path), "no file");
        std::fs::write(&path, "{ roto").unwrap();
        assert!(load_automatic(&path), "damaged file");
        save_automatic(&path, false).unwrap();
        assert!(!load_automatic(&path));
        save_automatic(&path, true).unwrap();
        assert!(load_automatic(&path));
    }

    #[test]
    fn a_read_only_settings_file_is_kept_and_explained_in_spanish() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("actualizaciones.json");
        save_automatic(&path, false).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&path, permissions.clone()).unwrap();
        let error = save_automatic(&path, true).unwrap_err().to_string();
        assert!(
            error.starts_with("«actualizaciones.json» es de solo lectura"),
            "{error}"
        );
        assert!(!load_automatic(&path));
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        std::fs::set_permissions(&path, permissions).unwrap();
    }

    #[test]
    fn the_score_to_reopen_is_read_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reabrir.json");
        assert_eq!(take_reopen_at(&path), None);
        save_reopen(&path, Path::new("/obras/Sinfonía.partitura")).unwrap();
        assert_eq!(
            take_reopen_at(&path).as_deref(),
            Some(Path::new("/obras/Sinfonía.partitura"))
        );
        assert!(!path.exists());
        assert_eq!(take_reopen_at(&path), None);
    }

    #[test]
    fn the_public_key_is_a_minisign_key() {
        assert!(PUBLIC_KEY.starts_with("untrusted comment: minisign public key"));
        assert_eq!(PUBLIC_KEY.lines().count(), 2);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn development_builds_do_not_update() {
        // Tests are debug builds, like `cargo run`.
        assert!(matches!(updater(), Err(why) if why.contains("desarrollo")));
    }
}
