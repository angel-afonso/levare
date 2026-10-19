//! Notation marks as objects of their own, so the score can select, change
//! or delete one without touching its note. Point marks are reached through
//! their note; span marks through their connection.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MarkRef {
    Articulation(ElementId),
    Dynamic(ElementId),
    Technique(ElementId),
    Connection(Connection),
}

impl MarkRef {
    /// The note the mark is written on; a connection's first note.
    pub fn anchor(self) -> ElementId {
        match self {
            Self::Articulation(id) | Self::Dynamic(id) | Self::Technique(id) => id,
            Self::Connection(connection) => connection.start,
        }
    }
}

impl Score {
    pub fn has_mark(&self, mark: MarkRef) -> bool {
        let notation = |id| self.event(id).map(|event| event.notation);
        match mark {
            MarkRef::Articulation(id) => notation(id).is_some_and(|n| n.articulation.is_some()),
            MarkRef::Dynamic(id) => notation(id).is_some_and(|n| n.dynamic.is_some()),
            MarkRef::Technique(id) => notation(id).is_some_and(|n| n.technique.is_some()),
            MarkRef::Connection(connection) => self.connections().contains(&connection),
        }
    }
    /// Name shown when the mark is selected ("Staccato", "Dinámica f"…).
    pub fn mark_label(&self, mark: MarkRef) -> Option<String> {
        let notation = |id| self.event(id).map(|event| event.notation);
        Some(match mark {
            MarkRef::Articulation(id) => notation(id)?.articulation?.label().into(),
            MarkRef::Dynamic(id) => format!("Dinámica {}", notation(id)?.dynamic?.label()),
            MarkRef::Technique(id) => format!("Técnica {}", notation(id)?.technique?.label()),
            MarkRef::Connection(connection) => {
                if !self.has_mark(mark) {
                    return None;
                }
                connection.kind.label().into()
            }
        })
    }
    pub fn remove_mark(&mut self, mark: MarkRef) -> Result<(), ScoreError> {
        if !self.has_mark(mark) {
            return Err(ScoreError("La marca ya no está en la partitura"));
        }
        match mark {
            MarkRef::Articulation(id) => self.set_articulation(id, None),
            MarkRef::Dynamic(id) => self.set_dynamic(id, None),
            MarkRef::Technique(id) => self.set_technique(id, None),
            MarkRef::Connection(connection) => {
                self.remove_connection(connection);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(step: Step) -> Event {
        Event {
            pitches: Pitches::single(Pitch::new(step, 4)),
            duration: Duration::Quarter,
            notation: Notation::default(),
        }
    }

    #[test]
    fn each_mark_is_removed_alone_and_keeps_its_note_and_the_other_marks() {
        let mut score = Score::default();
        let a = score.insert(0, 0, note(Step::C)).unwrap();
        let b = score.insert(0, 1, note(Step::D)).unwrap();
        score
            .set_articulation(a, Some(Articulation::Staccato))
            .unwrap();
        score.set_dynamic(a, Some(Dynamic::F)).unwrap();
        let slur = Connection {
            start: a,
            end: b,
            kind: ConnectionKind::Slur,
        };
        score.add_connection(slur).unwrap();
        assert_eq!(
            score.mark_label(MarkRef::Dynamic(a)).as_deref(),
            Some("Dinámica f")
        );
        assert_eq!(MarkRef::Connection(slur).anchor(), a);

        score.remove_mark(MarkRef::Articulation(a)).unwrap();
        let event = score.event(a).unwrap();
        assert_eq!(event.notation.articulation, None);
        assert_eq!(event.notation.dynamic, Some(Dynamic::F));
        assert!(score.has_mark(MarkRef::Connection(slur)));

        score.remove_mark(MarkRef::Connection(slur)).unwrap();
        assert!(score.connections().is_empty());
        assert_eq!(score.event(a).unwrap().pitches, note(Step::C).pitches);
        assert!(!score.has_mark(MarkRef::Technique(a)));
        assert!(score.remove_mark(MarkRef::Articulation(a)).is_err());
        assert_eq!(score.mark_label(MarkRef::Articulation(a)), None);
    }
}
