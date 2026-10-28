//! The autosave Levare relies on when it goes away without asking: restarting
//! into an update, or quitting from outside the window.

use std::path::{Path, PathBuf};

use score_core::Score;
use score_io::{RecoveredScore, Recovery};

use crate::AppView;

impl AppView {
    /// GPUI 0.2.2 cannot cancel a quit that comes from outside the window
    /// (the Dock, logging out): `on_app_quit` runs once it is decided, with no
    /// way to ask about unsaved changes. So the work is kept instead.
    pub(crate) fn keep_unsaved_on_quit(&mut self) {
        let folder = crate::recovery_path();
        let kept = keep_on_quit(
            self.quit_snapshot.as_ref(),
            self.document
                .is_dirty()
                .then(|| self.document.editor.score()),
            self.path.clone(),
            folder.as_deref().and_then(Path::parent),
        );
        match kept {
            Ok(None) => {}
            Ok(Some(copy)) => {
                crate::diagnostics::log(format_args!(
                    "Copia de emergencia guardada en {}",
                    copy.display()
                ));
                // «Abrir reciente» leads to it on the next start.
                self.recent.touch(&copy);
                if let Some(path) = &self.recent_path
                    && let Err(error) = self.recent.save(path)
                {
                    crate::diagnostics::log(format_args!(
                        "No se pudo guardar la lista de recientes: {error}"
                    ));
                }
            }
            Err(error) => crate::diagnostics::log(format_args!(
                "No se pudo conservar la partitura sin guardar: {error}"
            )),
        }
    }
}

/// Autosaves `unsaved` one last time; when that fails, saves it as an
/// emergency copy in `folder` and returns where.
pub(crate) fn keep_on_quit(
    recovery: Option<&Recovery>,
    unsaved: Option<&Score>,
    original_path: Option<PathBuf>,
    folder: Option<&Path>,
) -> Result<Option<PathBuf>, String> {
    let snapshot = unsaved.map(|score| RecoveredScore {
        score: score.clone(),
        original_path,
    });
    let Err(autosave) = autosave_now(recovery, snapshot) else {
        return Ok(None);
    };
    crate::diagnostics::log(format_args!(
        "No se pudo guardar la recuperación: {autosave}"
    ));
    let (Some(score), Some(folder)) = (unsaved, folder) else {
        return Err("no se encontró la carpeta de datos de Levare".into());
    };
    score_io::save_emergency_copy(folder, score)
        .map(Some)
        .map_err(|error| error.to_string())
}

/// Writes `unsaved` as the autosave right away and says whether it reached the
/// disk. A write can only fail after it was queued, so `Recovery::last_error`
/// beforehand cannot tell.
pub(crate) fn autosave_now(
    recovery: Option<&Recovery>,
    unsaved: Option<RecoveredScore>,
) -> Result<(), String> {
    let Some(recovery) = recovery else {
        return match unsaved {
            Some(_) => Err("el autoguardado no está disponible".into()),
            None => Ok(()),
        };
    };
    let dirty = unsaved.is_some();
    recovery.update(unsaved);
    match recovery.flush() {
        Err(error) if dirty => Err(error.to_string()),
        // Nothing unsaved: an old autosave left behind is only offered again.
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unsaved(score: Score) -> Option<RecoveredScore> {
        Some(RecoveredScore {
            score,
            original_path: None,
        })
    }

    #[test]
    fn a_failing_last_autosave_is_reported_though_the_previous_one_worked() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recovery.partitura");
        let recovery = Recovery::open(&path).unwrap();
        assert_eq!(
            autosave_now(Some(&recovery), unsaved(Score::default())),
            Ok(())
        );
        assert_eq!(recovery.last_error(), None);
        // Too long a title to save, like a score over the size limit.
        let mut invalid = serde_json::to_value(Score::default()).unwrap();
        invalid["title"] = serde_json::Value::String("x".repeat(2048));
        let invalid = serde_json::from_value(invalid).unwrap();
        assert!(autosave_now(Some(&recovery), unsaved(invalid)).is_err());
        assert_eq!(
            score_io::load_recovery(&path).unwrap().unwrap().score,
            Score::default()
        );
    }

    #[test]
    fn unsaved_work_without_an_autosave_is_reported() {
        assert!(autosave_now(None, unsaved(Score::default())).is_err());
        assert_eq!(autosave_now(None, None), Ok(()));
    }

    #[test]
    fn unsaved_work_is_kept_as_a_copy_when_the_last_autosave_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recovery.partitura");
        let recovery = Recovery::open(&path).unwrap();
        let mut score = Score::default();
        score.append_measure().unwrap();
        assert_eq!(
            keep_on_quit(Some(&recovery), Some(&score), None, Some(dir.path())),
            Ok(None)
        );
        // A folder in the autosave's place makes its write fail on every system.
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("ocupado"), b"").unwrap();
        let copy = keep_on_quit(Some(&recovery), Some(&score), None, Some(dir.path()))
            .unwrap()
            .expect("an emergency copy");
        assert_eq!(copy.parent(), Some(dir.path()));
        assert_eq!(score_io::open(&copy).unwrap().score, score);
    }

    #[test]
    fn unsaved_work_without_an_autosave_is_kept_as_a_copy() {
        let dir = tempfile::tempdir().unwrap();
        let score = Score::default();
        let copy = keep_on_quit(None, Some(&score), None, Some(dir.path()))
            .unwrap()
            .expect("an emergency copy");
        assert_eq!(score_io::open(&copy).unwrap().score, score);
        assert_eq!(keep_on_quit(None, None, None, Some(dir.path())), Ok(None));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
