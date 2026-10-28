//! The autosave Levare relies on when it goes away without asking: restarting
//! into an update, or quitting from outside the window.

use score_io::{RecoveredScore, Recovery};

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
    use score_core::Score;

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
}
