//! The catalog of notation marks. Each kind is described once here: name,
//! family, glyph and where it is written. Commands, the ribbon, the
//! inspector and layout read these facts instead of repeating them.
//!
//! A mark in the score is a `MarkRef`, so it can be selected, changed or
//! deleted without touching its note. Point marks are reached through their
//! note; span marks through their connection.
use super::*;

/// One kind of mark a user can write.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MarkKind {
    Articulation(Articulation),
    Fermata,
    Dynamic(Dynamic),
    Technique(PlayingTechnique),
    Connection(ConnectionKind),
}

/// Marks of one family replace each other on a note; a note holds at most
/// one of each point family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MarkFamily {
    Articulation,
    Fermata,
    Dynamic,
    Technique,
    Tie,
    Slur,
    Hairpin,
}

/// Which side of the staff a mark is written on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Above,
    Below,
}

impl MarkKind {
    pub const ALL: [Self; 19] = [
        Self::Articulation(Articulation::Staccato),
        Self::Articulation(Articulation::Tenuto),
        Self::Articulation(Articulation::Accent),
        Self::Articulation(Articulation::Marcato),
        Self::Fermata,
        Self::Dynamic(Dynamic::Ppp),
        Self::Dynamic(Dynamic::Pp),
        Self::Dynamic(Dynamic::P),
        Self::Dynamic(Dynamic::Mp),
        Self::Dynamic(Dynamic::Mf),
        Self::Dynamic(Dynamic::F),
        Self::Dynamic(Dynamic::Ff),
        Self::Dynamic(Dynamic::Fff),
        Self::Technique(PlayingTechnique::Pizzicato),
        Self::Technique(PlayingTechnique::Arco),
        Self::Connection(ConnectionKind::Tie),
        Self::Connection(ConnectionKind::Slur),
        Self::Connection(ConnectionKind::Crescendo),
        Self::Connection(ConnectionKind::Diminuendo),
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Articulation(mark) => mark.label(),
            Self::Fermata => "Calderón",
            Self::Dynamic(mark) => mark.label(),
            Self::Technique(mark) => mark.label(),
            Self::Connection(kind) => kind.label(),
        }
    }
    /// Full name for buttons and tooltips ("Pianissimo", "Pizzicato"…).
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dynamic(mark) => match mark {
                Dynamic::Ppp => "Pianississimo",
                Dynamic::Pp => "Pianissimo",
                Dynamic::P => "Piano",
                Dynamic::Mp => "Mezzopiano",
                Dynamic::Mf => "Mezzoforte",
                Dynamic::F => "Forte",
                Dynamic::Ff => "Fortissimo",
                Dynamic::Fff => "Fortississimo",
            },
            Self::Technique(PlayingTechnique::Pizzicato) => "Pizzicato",
            Self::Technique(PlayingTechnique::Arco) => "Arco",
            _ => self.label(),
        }
    }
    /// Music-font symbol for buttons and menus: the mark's own glyph, or a
    /// hairpin's. Ties and slurs use drawn icons instead.
    pub const fn symbol(self) -> Option<&'static str> {
        match self {
            Self::Connection(ConnectionKind::Crescendo) => Some("\u{e53e}"),
            Self::Connection(ConnectionKind::Diminuendo) => Some("\u{e53f}"),
            _ => self.glyph(),
        }
    }
    pub const fn family(self) -> MarkFamily {
        match self {
            Self::Articulation(_) => MarkFamily::Articulation,
            Self::Fermata => MarkFamily::Fermata,
            Self::Dynamic(_) => MarkFamily::Dynamic,
            Self::Technique(_) => MarkFamily::Technique,
            Self::Connection(ConnectionKind::Tie) => MarkFamily::Tie,
            Self::Connection(ConnectionKind::Slur) => MarkFamily::Slur,
            Self::Connection(_) => MarkFamily::Hairpin,
        }
    }
    /// The music-font glyph; connections are drawn as curves and wedges.
    pub const fn glyph(self) -> Option<&'static str> {
        match self {
            Self::Articulation(mark) => Some(mark.glyph()),
            Self::Fermata => Some("\u{e4c0}"),
            Self::Dynamic(mark) => Some(mark.glyph()),
            Self::Technique(_) | Self::Connection(_) => None,
        }
    }
    pub const fn placement(self) -> Placement {
        match self {
            Self::Dynamic(_) | Self::Connection(ConnectionKind::Crescendo) => Placement::Below,
            Self::Connection(ConnectionKind::Diminuendo) => Placement::Below,
            _ => Placement::Above,
        }
    }
    /// Written on one note, as opposed to joining two.
    pub const fn is_point(self) -> bool {
        !matches!(self, Self::Connection(_))
    }
    /// Bowed-string techniques only apply to violin, viola, cello and bass.
    pub const fn requires_strings(self) -> bool {
        matches!(self, Self::Technique(_))
    }
    /// Rests take a fermata, dynamic or technique, but no articulation.
    pub const fn allowed_on_rest(self) -> bool {
        !matches!(self, Self::Articulation(_) | Self::Connection(_))
    }
}

impl MarkFamily {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Articulation => "Articulación",
            Self::Fermata => "Calderón",
            Self::Dynamic => "Dinámica",
            Self::Technique => "Técnica",
            Self::Tie => "Ligadura de unión",
            Self::Slur => "Ligadura de expresión",
            Self::Hairpin => "Regulador",
        }
    }
    pub fn kinds(self) -> impl Iterator<Item = MarkKind> {
        MarkKind::ALL
            .into_iter()
            .filter(move |kind| kind.family() == self)
    }
}

/// A mark written in the score.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MarkRef {
    Articulation(ElementId),
    Fermata(ElementId),
    Dynamic(ElementId),
    Technique(ElementId),
    Connection(Connection),
}

impl MarkRef {
    /// The note the mark is written on; a connection's first note.
    pub fn anchor(self) -> ElementId {
        match self {
            Self::Articulation(id)
            | Self::Fermata(id)
            | Self::Dynamic(id)
            | Self::Technique(id) => id,
            Self::Connection(connection) => connection.start,
        }
    }
    /// The point mark of `family` on `id`, whether or not it is written yet.
    pub fn point(family: MarkFamily, id: ElementId) -> Option<Self> {
        Some(match family {
            MarkFamily::Articulation => Self::Articulation(id),
            MarkFamily::Fermata => Self::Fermata(id),
            MarkFamily::Dynamic => Self::Dynamic(id),
            MarkFamily::Technique => Self::Technique(id),
            MarkFamily::Tie | MarkFamily::Slur | MarkFamily::Hairpin => return None,
        })
    }
}

impl Score {
    /// What a written mark currently is, or `None` once it is gone.
    pub fn mark_kind(&self, mark: MarkRef) -> Option<MarkKind> {
        let notation = |id| self.event(id).map(|event| event.notation);
        match mark {
            MarkRef::Articulation(id) => notation(id)?.articulation.map(MarkKind::Articulation),
            MarkRef::Fermata(id) => notation(id)?.fermata.then_some(MarkKind::Fermata),
            MarkRef::Dynamic(id) => notation(id)?.dynamic.map(MarkKind::Dynamic),
            MarkRef::Technique(id) => notation(id)?.technique.map(MarkKind::Technique),
            MarkRef::Connection(connection) => self
                .connections()
                .contains(&connection)
                .then_some(MarkKind::Connection(connection.kind)),
        }
    }
    pub fn has_mark(&self, mark: MarkRef) -> bool {
        self.mark_kind(mark).is_some()
    }
    /// The point mark of `family` written on `id`.
    pub fn point_mark(&self, id: ElementId, family: MarkFamily) -> Option<MarkKind> {
        self.mark_kind(MarkRef::point(family, id)?)
    }
    /// Name shown when the mark is selected ("Staccato", "Dinámica f"…).
    pub fn mark_label(&self, mark: MarkRef) -> Option<String> {
        let kind = self.mark_kind(mark)?;
        Some(match kind.family() {
            MarkFamily::Dynamic | MarkFamily::Technique => {
                format!("{} {}", kind.family().label(), kind.label())
            }
            _ => kind.label().into(),
        })
    }
    /// Write a point mark on `id`, replacing its family's current one, or
    /// clear that family with `None`.
    pub fn set_point_mark(
        &mut self,
        id: ElementId,
        family: MarkFamily,
        kind: Option<MarkKind>,
    ) -> Result<(), ScoreError> {
        if kind.is_some_and(|kind| kind.family() != family) {
            return Err(ScoreError("La marca no pertenece a esa familia"));
        }
        match (family, kind) {
            (MarkFamily::Articulation, Some(MarkKind::Articulation(mark))) => {
                self.set_articulation(id, Some(mark))
            }
            (MarkFamily::Articulation, _) => self.set_articulation(id, None),
            (MarkFamily::Fermata, kind) => self.set_fermata(id, kind.is_some()),
            (MarkFamily::Dynamic, Some(MarkKind::Dynamic(mark))) => {
                self.set_dynamic(id, Some(mark))
            }
            (MarkFamily::Dynamic, _) => self.set_dynamic(id, None),
            (MarkFamily::Technique, Some(MarkKind::Technique(mark))) => {
                self.set_technique(id, Some(mark))
            }
            (MarkFamily::Technique, _) => self.set_technique(id, None),
            (MarkFamily::Tie | MarkFamily::Slur | MarkFamily::Hairpin, _) => {
                Err(ScoreError("Las ligaduras y los reguladores unen dos notas"))
            }
        }
    }
    pub fn remove_mark(&mut self, mark: MarkRef) -> Result<(), ScoreError> {
        let kind = self
            .mark_kind(mark)
            .ok_or(ScoreError("La marca ya no está en la partitura"))?;
        match mark {
            MarkRef::Connection(connection) => {
                self.remove_connection(connection);
                Ok(())
            }
            _ => self.set_point_mark(mark.anchor(), kind.family(), None),
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
    fn the_catalog_lists_every_kind_once_with_its_family_and_glyph() {
        let mut seen = std::collections::HashSet::new();
        for kind in MarkKind::ALL {
            assert!(seen.insert(kind), "{kind:?}");
            assert!(!kind.label().is_empty());
            assert_eq!(
                kind.glyph().is_some(),
                kind.is_point() && kind.family() != MarkFamily::Technique
            );
            assert!(kind.family().kinds().any(|k| k == kind));
        }
        assert_eq!(MarkFamily::Dynamic.kinds().count(), Dynamic::ALL.len());
        assert_eq!(
            MarkFamily::Articulation.kinds().count(),
            Articulation::ALL.len()
        );
        assert_eq!(MarkKind::Dynamic(Dynamic::P).placement(), Placement::Below);
        assert_eq!(MarkKind::Fermata.placement(), Placement::Above);
        assert!(MarkKind::Fermata.allowed_on_rest());
        assert!(!MarkKind::Articulation(Articulation::Accent).allowed_on_rest());
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
        score.set_fermata(a, true).unwrap();
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
        assert_eq!(
            score.point_mark(a, MarkFamily::Fermata),
            Some(MarkKind::Fermata)
        );
        assert_eq!(MarkRef::Connection(slur).anchor(), a);

        score.remove_mark(MarkRef::Articulation(a)).unwrap();
        let event = score.event(a).unwrap();
        assert_eq!(event.notation.articulation, None);
        assert_eq!(event.notation.dynamic, Some(Dynamic::F));
        assert!(event.notation.fermata);
        assert!(score.has_mark(MarkRef::Connection(slur)));

        score.remove_mark(MarkRef::Fermata(a)).unwrap();
        assert!(!score.event(a).unwrap().notation.fermata);
        score.remove_mark(MarkRef::Connection(slur)).unwrap();
        assert!(score.connections().is_empty());
        assert_eq!(score.event(a).unwrap().pitches, note(Step::C).pitches);
        assert!(!score.has_mark(MarkRef::Technique(a)));
        assert!(score.remove_mark(MarkRef::Articulation(a)).is_err());
        assert_eq!(score.mark_label(MarkRef::Articulation(a)), None);
    }

    #[test]
    fn point_marks_replace_within_their_family_and_check_it() {
        let mut score = Score::default();
        let a = score.insert(0, 0, note(Step::C)).unwrap();
        let accent = MarkKind::Articulation(Articulation::Accent);
        score
            .set_point_mark(a, MarkFamily::Articulation, Some(accent))
            .unwrap();
        score
            .set_point_mark(
                a,
                MarkFamily::Articulation,
                Some(MarkKind::Articulation(Articulation::Tenuto)),
            )
            .unwrap();
        assert_eq!(
            score.event(a).unwrap().notation.articulation,
            Some(Articulation::Tenuto)
        );
        assert!(
            score
                .set_point_mark(a, MarkFamily::Dynamic, Some(accent))
                .is_err()
        );
        assert!(score.set_point_mark(a, MarkFamily::Slur, None).is_err());
        // A fermata may also sit on a rest.
        let rest = score
            .insert(
                0,
                1,
                Event {
                    pitches: Pitches::default(),
                    ..note(Step::C)
                },
            )
            .unwrap();
        score
            .set_point_mark(rest, MarkFamily::Fermata, Some(MarkKind::Fermata))
            .unwrap();
        assert!(score.event(rest).unwrap().notation.fermata);
    }
}
