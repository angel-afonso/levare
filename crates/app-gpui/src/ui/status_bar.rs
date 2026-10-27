//! What the bottom bar says, apart from how it is drawn.

/// The page on screen, in Page view.
#[derive(Clone, Copy)]
pub struct PageFacts {
    pub index: usize,
    pub count: usize,
    pub first_measure: usize,
    pub last_measure: usize,
}

pub struct Facts<'a> {
    pub busy: bool,
    pub audio_error: Option<&'a str>,
    pub status: &'a str,
    /// `None` outside Page view.
    pub page: Option<PageFacts>,
    pub zoom_percent: u16,
    pub recovery_ready: bool,
    pub dirty: bool,
}

pub struct Pages {
    /// «Página 2 de 5».
    pub label: String,
    /// «Compases 9–16», for the hint on the label.
    pub bars: String,
    /// The page each arrow goes to; `None` at either end.
    pub previous: Option<usize>,
    pub next: Option<usize>,
}

/// The three things the bar keeps: the message, the pages and the saving.
pub struct StatusBar {
    pub message: String,
    pub pages: Option<Pages>,
    /// «70 %».
    pub zoom: String,
    pub autosave: &'static str,
    pub autosave_ok: bool,
    pub document: &'static str,
    pub unsaved: bool,
}

pub fn status_bar(facts: &Facts) -> StatusBar {
    let message = if facts.busy {
        "Procesando archivo…".to_owned()
    } else if let Some(error) = facts.audio_error {
        error.to_owned()
    } else {
        facts.status.to_owned()
    };
    let pages = facts.page.map(|page| Pages {
        label: format!("Página {} de {}", page.index + 1, page.count),
        bars: format!(
            "Compases {}–{}",
            page.first_measure + 1,
            page.last_measure + 1
        ),
        previous: page.index.checked_sub(1),
        next: (page.index + 1 < page.count).then_some(page.index + 1),
    });
    StatusBar {
        message,
        pages,
        zoom: format!("{} %", facts.zoom_percent),
        autosave: if facts.recovery_ready {
            "Autoguardado activo"
        } else {
            "Autoguardado no disponible"
        },
        autosave_ok: facts.recovery_ready,
        document: if facts.dirty {
            "Cambios sin guardar"
        } else {
            "Guardado"
        },
        unsaved: facts.dirty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> Facts<'static> {
        Facts {
            busy: false,
            audio_error: None,
            status: "Listo",
            page: None,
            zoom_percent: 70,
            recovery_ready: true,
            dirty: false,
        }
    }

    #[test]
    fn status_bar_says_only_status_pages_and_saving() {
        // Naming every field keeps voice, key, metre, instruments and view
        // mode from coming back without this test noticing.
        let StatusBar {
            message,
            pages,
            zoom,
            autosave,
            autosave_ok,
            document,
            unsaved,
        } = status_bar(&facts());
        assert_eq!(message, "Listo");
        assert!(pages.is_none(), "page navigation only in Page view");
        assert_eq!(zoom, "70 %");
        assert_eq!(autosave, "Autoguardado activo");
        assert!(autosave_ok);
        assert_eq!(document, "Guardado");
        assert!(!unsaved);

        let bar = status_bar(&Facts {
            dirty: true,
            recovery_ready: false,
            ..facts()
        });
        assert_eq!(bar.autosave, "Autoguardado no disponible");
        assert!(!bar.autosave_ok);
        assert_eq!(bar.document, "Cambios sin guardar");
        assert!(bar.unsaved);
    }

    #[test]
    fn message_prefers_busy_then_audio_error_then_status() {
        let both = Facts {
            audio_error: Some("Sin salida de audio"),
            ..facts()
        };
        assert_eq!(status_bar(&both).message, "Sin salida de audio");
        let busy = Facts { busy: true, ..both };
        assert_eq!(status_bar(&busy).message, "Procesando archivo…");
    }

    #[test]
    fn pages_name_the_page_its_bars_and_the_ends() {
        let page = |index| PageFacts {
            index,
            count: 3,
            first_measure: index * 8,
            last_measure: index * 8 + 7,
        };
        let pages = |index| {
            status_bar(&Facts {
                page: Some(page(index)),
                ..facts()
            })
            .pages
            .unwrap()
        };
        let first = pages(0);
        assert_eq!(first.label, "Página 1 de 3");
        assert_eq!(first.bars, "Compases 1–8");
        assert_eq!((first.previous, first.next), (None, Some(1)));
        let middle = pages(1);
        assert_eq!(middle.bars, "Compases 9–16");
        assert_eq!((middle.previous, middle.next), (Some(0), Some(2)));
        assert_eq!((pages(2).previous, pages(2).next), (Some(1), None));
    }
}
