//! Sonidos › Más sonidos: the catalog, one install at a time in the
//! background, and removal of packs installed from here.
use super::*;
use score_sound_store::{Catalog, Entry, Installed, Progress, Stage, size_label};
use std::sync::Arc;

pub(crate) struct Store {
    pub catalog: Catalog,
    pub installed: Vec<Installed>,
    pub job: Option<Job>,
    /// The pack whose license is shown, waiting to be accepted.
    pub accepting: Option<String>,
    /// The installed pack waiting for its removal to be confirmed.
    pub removing: Option<String>,
    pub error: Option<String>,
    /// Last progress drawn, so the screen refreshes only when it moves.
    shown: Option<(Stage, u64)>,
}

pub(crate) struct Job {
    pub id: String,
    pub progress: Arc<Progress>,
}

pub(crate) enum State<'a> {
    Available,
    Accepting,
    Installing(&'a Progress),
    Installed { outdated: bool },
    Removing,
}

impl Store {
    pub fn new() -> Self {
        Self {
            catalog: Catalog::builtin(),
            installed: root().map_or_else(Vec::new, |root| score_sound_store::installed(&root)),
            job: None,
            accepting: None,
            removing: None,
            error: None,
            shown: None,
        }
    }

    pub fn state(&self, entry: &Entry) -> State<'_> {
        if let Some(job) = self.job.as_ref().filter(|job| job.id == entry.id) {
            return State::Installing(&job.progress);
        }
        if let Some(installed) = self.installed.iter().find(|pack| pack.id == entry.id) {
            if self.removing.as_deref() == Some(&entry.id) {
                return State::Removing;
            }
            return State::Installed {
                outdated: installed.version < entry.version,
            };
        }
        if self.accepting.as_deref() == Some(&entry.id) {
            State::Accepting
        } else {
            State::Available
        }
    }

    /// Whether the progress shown on screen is stale.
    pub fn moved(&mut self) -> bool {
        let now = self.job.as_ref().map(|job| {
            let (done, total) = job.progress.bytes();
            (job.progress.stage(), done * 100 / total.max(1))
        });
        now != std::mem::replace(&mut self.shown, now)
    }
}

/// "Descargando 45 % · 210 MB de 461 MB".
pub(crate) fn progress_label(progress: &Progress) -> String {
    match progress.stage() {
        Stage::Downloading => {
            let (done, total) = progress.bytes();
            format!(
                "Descargando {} % · {} de {}",
                done * 100 / total.max(1),
                size_label(done),
                size_label(total)
            )
        }
        Stage::Unpacking => "Descomprimiendo…".into(),
        Stage::Checking => "Comprobando el paquete…".into(),
    }
}

fn root() -> Option<PathBuf> {
    crate::data_path("soundpacks")
}

impl AppView {
    pub(crate) fn install_pack(&mut self, id: &str, cx: &mut Context<Self>) {
        self.store.accepting = None;
        if self.store.job.is_some() {
            self.status = "Espera a que termine la instalación en curso".into();
            cx.notify();
            return;
        }
        let Some(entry) = self.store.catalog.entry(id).cloned() else {
            return;
        };
        let (Some(root), Some(engine)) = (root(), score_sound_vsco::bundled_engine()) else {
            self.store.error =
                Some("Falta el motor SFZ de Levare (sfizz); no se pueden instalar sonidos".into());
            cx.notify();
            return;
        };
        let progress = Arc::new(Progress::default());
        self.store.error = None;
        self.store.job = Some(Job {
            id: entry.id.clone(),
            progress: progress.clone(),
        });
        self.status = format!("Instalando {}…", entry.name);
        let id = entry.id.clone();
        let task = cx.background_executor().spawn(async move {
            score_sound_store::prepare_install(&entry, &root, &engine, &progress)
        });
        cx.spawn(async move |view, cx| {
            // The old version plays on during the download; it is let go
            // only for the swap, which Windows refuses while files are open.
            let chosen = match task.await {
                Ok(prepared) => Ok((
                    prepared,
                    view.update(cx, |view, _| view.release_pack(&id))
                        .ok()
                        .flatten(),
                )),
                Err(error) => Err(error),
            };
            let result = match chosen {
                Ok((prepared, chosen)) => cx
                    .background_executor()
                    .spawn(async move { prepared.finish() })
                    .await
                    .map(|_| chosen),
                Err(error) => Err(error),
            };
            let _ = view.update(cx, |view, cx| {
                let name = view
                    .store
                    .job
                    .take()
                    .and_then(|job| view.store.catalog.entry(&job.id).map(|e| e.name.clone()))
                    .unwrap_or_default();
                match result {
                    Ok(chosen) => {
                        view.refresh_packs();
                        // The bank that was playing the old version comes back.
                        if let Some(index) = chosen.and_then(|id| {
                            crate::sound_banks::index_by_id(&view.playback.banks, &id)
                        }) {
                            view.choose_sound(Some(index), cx);
                        }
                        view.status = format!("{name} instalado · elígelo en Fuente de sonido");
                    }
                    Err(error) => {
                        view.status = error.0.clone();
                        view.store.error = Some(error.0);
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn cancel_install(&mut self) {
        if let Some(job) = &self.store.job {
            job.progress.cancel();
        }
    }

    pub(crate) fn uninstall_pack(&mut self, id: &str, cx: &mut Context<Self>) {
        self.store.removing = None;
        let Some(root) = root() else {
            return;
        };
        self.playback.stop();
        self.release_pack(id);
        let name = self
            .store
            .catalog
            .entry(id)
            .map_or_else(|| id.to_owned(), |entry| entry.name.clone());
        self.status = match score_sound_store::uninstall(id, &root) {
            Ok(()) => {
                self.refresh_packs();
                format!("{name} quitado")
            }
            Err(error) => error.0,
        };
        cx.notify();
    }

    /// Samples stream from disk: before pack `id` is moved or deleted, the
    /// bank playing from it gives way to the built-in sounds. Returns that
    /// bank's id, to choose it again after a reinstall.
    fn release_pack(&mut self, id: &str) -> Option<String> {
        if !self.playback.uses_pack(id) {
            return None;
        }
        let chosen = self
            .playback
            .selected_bank
            .and_then(|index| self.playback.banks.get(index))
            .map(|bank| bank.id().to_owned());
        let score = self.document.editor.score();
        let part = self.document.editor.cursor().part;
        if let Err(error) = self.playback.choose_sound(None, score, part) {
            self.playback.sound_error = Some(error);
        }
        chosen
    }

    /// Reread the installed packs and the banks they provide.
    fn refresh_packs(&mut self) {
        self.store.installed =
            root().map_or_else(Vec::new, |root| score_sound_store::installed(&root));
        let score = self.document.editor.score();
        if let Err(error) = self
            .playback
            .rediscover(score, self.document.editor.cursor().part)
        {
            self.playback.sound_error = Some(error);
        }
    }
}
