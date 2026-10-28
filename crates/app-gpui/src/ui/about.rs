//! Acerca de Levare: the version, the license, who made what Levare ships
//! with, and whether it looks for updates on its own.

use super::*;
use std::path::{Path, PathBuf};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What Levare ships with: name, what it is and whose, and its license. The
/// license texts are in the licenses folder (`licenses_folder`).
const CREDITS: &[(&str, &str, &str)] = &[
    (
        "Bravura",
        "Fuente musical de Steinberg Media Technologies",
        "SIL Open Font License",
    ),
    (
        "GeneralUser GS",
        "Sonidos de S. Christian Collins",
        "licencia de GeneralUser GS",
    ),
    (
        "Liberation",
        "Fuentes de texto de Red Hat",
        "SIL Open Font License",
    ),
    (
        "Cantarell",
        "Fuente de la interfaz, del proyecto GNOME",
        "SIL Open Font License",
    ),
    (
        "sfizz",
        "Motor de sonidos SFZ",
        "licencia BSD de 2 cláusulas",
    ),
];

impl AppView {
    pub(crate) fn open_about(&mut self, cx: &mut Context<Self>) {
        self.ui.file_menu = false;
        self.ui.open_group = None;
        self.about = true;
        // Like the export dialog: nothing else runs while it is open.
        self.confirming = true;
        cx.notify();
    }

    pub(crate) fn close_about(&mut self, cx: &mut Context<Self>) {
        if self.about {
            self.about = false;
            self.confirming = false;
            cx.notify();
        }
    }

    fn show_licenses(&mut self, cx: &mut Context<Self>) {
        match licenses_folder() {
            Some(folder) => cx.open_with_system(&folder),
            None => {
                self.status = "No se encuentra la carpeta de licencias.".into();
                cx.notify();
            }
        }
    }
}

/// The licenses folder a release ships (see `.github/workflows/release.yml`)
/// or, in a development build, the repository's `assets`, which hold the
/// same texts.
fn licenses_folder() -> Option<PathBuf> {
    let shipped = std::env::current_exe()
        .ok()
        .and_then(|executable| shipped_licenses(&executable))
        .filter(|folder| folder.is_dir());
    shipped.or_else(|| {
        cfg!(debug_assertions)
            .then(|| Path::new(env!("CARGO_MANIFEST_DIR")).parent()?.parent())
            .flatten()
            .map(|repository| repository.join("assets"))
            .filter(|folder| folder.is_dir())
    })
}

/// Inside the bundle on macOS (`Levare.app/Contents/Resources/licenses`),
/// next to the executable on Windows and Linux.
fn shipped_licenses(executable: &Path) -> Option<PathBuf> {
    let folder = executable.parent()?;
    Some(if cfg!(target_os = "macos") {
        folder.parent()?.join("Resources").join("licenses")
    } else {
        folder.join("licenses")
    })
}

pub fn dialog(view: &AppView, cx: &Context<AppView>) -> impl IntoElement {
    let licenses = licenses_folder().is_some();
    let updates = &view.updates;
    div()
        .id("about-dialog")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgba(SCRIM))
        .occlude()
        .child(
            div()
                .w(px(440.))
                .max_w(relative(0.94))
                .max_h(relative(0.92))
                .flex()
                .flex_col()
                .bg(rgb(PANEL))
                .border_1()
                .border_color(rgb(POPOVER_BORDER))
                .rounded_lg()
                .shadow_lg()
                .overflow_hidden()
                .child(
                    div()
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .border_b_1()
                        .border_color(rgb(BORDER))
                        .child(
                            div()
                                .text_size(px(20.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Levare"),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(SECONDARY))
                                .child(format!("Versión {VERSION} · Licencia MIT")),
                        ),
                )
                .child(
                    div()
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(caption("Incluye"))
                        .children(CREDITS.iter().map(|(name, what, license)| {
                            div()
                                .flex()
                                .flex_col()
                                .child(div().text_size(px(12.)).child(*name))
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(rgb(MUTED))
                                        .child(format!("{what} · {license}")),
                                )
                        }))
                        .child(div().mt_2().child(caption("Actualizaciones")))
                        .child(div().text_size(px(12.)).child(if updates.automatic() {
                            "La búsqueda automática de actualizaciones está activada."
                        } else {
                            "La búsqueda automática de actualizaciones está desactivada."
                        }))
                        .when_some(updates.unavailable(), |section, why| {
                            section.child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(MUTED))
                                    .child(why.to_owned()),
                            )
                        }),
                )
                .child(
                    div()
                        .p_4()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .border_t_1()
                        .border_color(rgb(BORDER))
                        .when(licenses, |footer| {
                            footer.child(
                                button(
                                    view,
                                    "about-licenses",
                                    "Ver licencias",
                                    false,
                                    cx,
                                    |v, _, cx| v.show_licenses(cx),
                                )
                                .px_3(),
                            )
                        })
                        .child(
                            button(view, "about-close", "Cerrar", true, cx, |v, _, cx| {
                                v.close_about(cx)
                            })
                            .px_3(),
                        ),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::{licenses_folder, shipped_licenses};
    use std::path::{Path, PathBuf};

    #[test]
    fn the_licenses_ship_inside_the_bundle_on_macos_and_beside_levare_elsewhere() {
        let app = Path::new("/Applications/Levare.app/Contents/MacOS/levare");
        let folder = Path::new("/home/ana/levare/levare");
        if cfg!(target_os = "macos") {
            assert_eq!(
                shipped_licenses(app),
                Some(PathBuf::from(
                    "/Applications/Levare.app/Contents/Resources/licenses"
                ))
            );
        } else {
            assert_eq!(
                shipped_licenses(folder),
                Some(Path::new("/home/ana/levare").join("licenses"))
            );
        }
    }

    #[cfg(debug_assertions)]
    #[test]
    fn development_builds_show_the_repository_licenses() {
        let folder = licenses_folder().expect("the repository's assets");
        assert!(folder.join("fonts").join("OFL.txt").is_file());
        assert!(folder.join("sounds").join("LICENSE.txt").is_file());
    }
}
