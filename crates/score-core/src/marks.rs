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
    Division(Division),
    Arpeggio(Arpeggio),
    Articulation(Articulation),
    Fermata,
    Tremolo(Tremolo),
    Dynamic(Dynamic),
    Technique(PlayingTechnique),
    Harmonic(Harmonic),
    Grace(GraceKind),
    /// Words written at the note; the words themselves live in its notation.
    Text(TextKind),
    /// A sung syllable; its verse is in the reference.
    Lyric,
    Chord,
    Connection(ConnectionKind),
}

/// Marks of one family replace each other on a note; a note holds at most
/// one of each point family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MarkFamily {
    Division,
    Arpeggio,
    Articulation,
    Fermata,
    Tremolo,
    Dynamic,
    Technique,
    Harmonic,
    Grace,
    ExpressionText,
    TechniqueText,
    Lyric,
    Chord,
    Tie,
    Slur,
    Hairpin,
    Line,
}

/// Which side of the staff a mark is written on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Left,
    Above,
    Below,
    Stem,
}

impl MarkKind {
    pub const ALL: [Self; 40] = [
        Self::Division(Division::Divisi),
        Self::Division(Division::Unison),
        Self::Arpeggio(Arpeggio::Normal),
        Self::Arpeggio(Arpeggio::Up),
        Self::Arpeggio(Arpeggio::Down),
        Self::Articulation(Articulation::Staccato),
        Self::Articulation(Articulation::Tenuto),
        Self::Articulation(Articulation::Accent),
        Self::Articulation(Articulation::Marcato),
        Self::Fermata,
        Self::Tremolo(Tremolo::One),
        Self::Tremolo(Tremolo::Two),
        Self::Tremolo(Tremolo::Three),
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
        Self::Harmonic(Harmonic::Natural),
        Self::Harmonic(Harmonic::Artificial),
        Self::Grace(GraceKind::Acciaccatura),
        Self::Grace(GraceKind::Appoggiatura),
        Self::Text(TextKind::Expression),
        Self::Text(TextKind::Technique),
        Self::Lyric,
        Self::Chord,
        Self::Connection(ConnectionKind::Tie),
        Self::Connection(ConnectionKind::Slur),
        Self::Connection(ConnectionKind::Crescendo),
        Self::Connection(ConnectionKind::Diminuendo),
        Self::Connection(ConnectionKind::OttavaAlta),
        Self::Connection(ConnectionKind::OttavaBassa),
        Self::Connection(ConnectionKind::Pedal),
        Self::Connection(ConnectionKind::Trill),
        Self::Connection(ConnectionKind::Glissando),
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Division(mark) => mark.label(),
            Self::Arpeggio(mark) => mark.label(),
            Self::Articulation(mark) => mark.label(),
            Self::Fermata => "Calderón",
            Self::Tremolo(mark) => mark.label(),
            Self::Dynamic(mark) => mark.label(),
            Self::Technique(mark) => mark.label(),
            Self::Harmonic(mark) => mark.label(),
            Self::Grace(kind) => kind.label(),
            Self::Text(kind) => kind.label(),
            Self::Lyric => "Letra",
            Self::Chord => "Cifrado",
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
    /// hairpin's. The UI composes ties and slurs from music-font glyphs.
    pub const fn symbol(self) -> Option<&'static str> {
        match self {
            Self::Connection(ConnectionKind::Crescendo) => Some("\u{e53e}"),
            Self::Connection(ConnectionKind::Diminuendo) => Some("\u{e53f}"),
            Self::Connection(ConnectionKind::OttavaAlta) => Some("\u{e511}"),
            Self::Connection(ConnectionKind::OttavaBassa) => Some("\u{e51c}"),
            Self::Connection(ConnectionKind::Pedal) => Some("\u{e650}"),
            Self::Connection(ConnectionKind::Trill) => Some("\u{e566}"),
            Self::Connection(ConnectionKind::Glissando) => Some("\u{e585}"),
            Self::Grace(GraceKind::Acciaccatura) => Some("\u{e560}"),
            Self::Grace(GraceKind::Appoggiatura) => Some("\u{e562}"),
            _ => self.glyph(),
        }
    }
    pub const fn family(self) -> MarkFamily {
        match self {
            Self::Division(_) => MarkFamily::Division,
            Self::Arpeggio(_) => MarkFamily::Arpeggio,
            Self::Articulation(_) => MarkFamily::Articulation,
            Self::Fermata => MarkFamily::Fermata,
            Self::Tremolo(_) => MarkFamily::Tremolo,
            Self::Dynamic(_) => MarkFamily::Dynamic,
            Self::Technique(_) => MarkFamily::Technique,
            Self::Harmonic(_) => MarkFamily::Harmonic,
            Self::Grace(_) => MarkFamily::Grace,
            Self::Text(TextKind::Expression) => MarkFamily::ExpressionText,
            Self::Text(TextKind::Technique) => MarkFamily::TechniqueText,
            Self::Lyric => MarkFamily::Lyric,
            Self::Chord => MarkFamily::Chord,
            Self::Connection(ConnectionKind::Tie) => MarkFamily::Tie,
            Self::Connection(ConnectionKind::Slur) => MarkFamily::Slur,
            Self::Connection(kind) if kind.is_line() => MarkFamily::Line,
            Self::Connection(_) => MarkFamily::Hairpin,
        }
    }
    /// The music-font glyph; connections are drawn as curves and wedges.
    pub const fn glyph(self) -> Option<&'static str> {
        match self {
            Self::Arpeggio(mark) => Some(mark.glyph()),
            Self::Harmonic(mark) => Some(mark.glyph()),
            Self::Articulation(mark) => Some(mark.glyph()),
            Self::Fermata => Some("\u{e4c0}"),
            Self::Tremolo(mark) => Some(mark.glyph()),
            Self::Dynamic(mark) => Some(mark.glyph()),
            // Grace notes are engraved note by note; they have only a symbol.
            Self::Division(_)
            | Self::Technique(_)
            | Self::Grace(_)
            | Self::Text(_)
            | Self::Lyric
            | Self::Chord
            | Self::Connection(_) => None,
        }
    }
    pub const fn placement(self) -> Placement {
        match self {
            Self::Arpeggio(_) | Self::Grace(_) => Placement::Left,
            Self::Tremolo(_) => Placement::Stem,
            Self::Dynamic(_)
            | Self::Text(TextKind::Expression)
            | Self::Lyric
            | Self::Connection(ConnectionKind::Crescendo) => Placement::Below,
            Self::Connection(
                ConnectionKind::Diminuendo | ConnectionKind::OttavaBassa | ConnectionKind::Pedal,
            ) => Placement::Below,
            _ => Placement::Above,
        }
    }
    /// Written on one note, as opposed to joining two.
    pub const fn is_point(self) -> bool {
        !matches!(self, Self::Connection(_))
    }
    /// Bowed-string techniques only apply to violin, viola, cello and bass.
    pub const fn requires_strings(self) -> bool {
        matches!(self, Self::Technique(_) | Self::Harmonic(_))
    }
    /// Rests take a fermata, dynamic or technique, but no articulation.
    pub const fn allowed_on_rest(self) -> bool {
        !matches!(
            self,
            Self::Arpeggio(_)
                | Self::Articulation(_)
                | Self::Harmonic(_)
                | Self::Tremolo(_)
                | Self::Grace(_)
                | Self::Connection(_)
        )
    }
}

impl MarkFamily {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Division => "Divisi y unísono",
            Self::Arpeggio => "Arpegio",
            Self::Articulation => "Articulación",
            Self::Fermata => "Calderón",
            Self::Tremolo => "Trémolo",
            Self::Dynamic => "Dinámica",
            Self::Technique => "Técnica",
            Self::Harmonic => "Armónico",
            Self::Grace => "Notas de adorno",
            Self::ExpressionText => "Texto de expresión",
            Self::TechniqueText => "Texto de técnica",
            Self::Lyric => "Letra",
            Self::Chord => "Cifrado",
            Self::Tie => "Ligadura de unión",
            Self::Slur => "Ligadura de expresión",
            Self::Hairpin => "Regulador",
            Self::Line => "Líneas",
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
    Division(ElementId),
    Arpeggio(ElementId),
    Articulation(ElementId),
    Fermata(ElementId),
    Tremolo(ElementId),
    Dynamic(ElementId),
    Technique(ElementId),
    Harmonic(ElementId),
    /// One grace note of a note: its index in the group.
    Grace(ElementId, u8),
    Text(ElementId, TextKind),
    /// The syllable of one verse (from 0) of a note.
    Lyric(ElementId, u8),
    Chord(ElementId),
    Connection(Connection),
}

impl MarkRef {
    /// The note the mark is written on; a connection's first note.
    pub fn anchor(self) -> ElementId {
        match self {
            Self::Division(id)
            | Self::Arpeggio(id)
            | Self::Articulation(id)
            | Self::Fermata(id)
            | Self::Tremolo(id)
            | Self::Dynamic(id)
            | Self::Technique(id)
            | Self::Harmonic(id)
            | Self::Grace(id, _)
            | Self::Text(id, _)
            | Self::Lyric(id, _)
            | Self::Chord(id) => id,
            Self::Connection(connection) => connection.start,
        }
    }
    /// Whether the mark's place in time is musical: dragged sideways, it goes
    /// to another note and sounds there.
    pub fn follows_time(self) -> bool {
        matches!(
            self,
            Self::Dynamic(_)
                | Self::Technique(_)
                | Self::Fermata(_)
                | Self::Text(..)
                | Self::Chord(_)
        ) || matches!(self, Self::Connection(connection) if connection.kind.is_hairpin())
    }
    /// The point mark of `family` on `id`, whether or not it is written yet.
    pub fn point(family: MarkFamily, id: ElementId) -> Option<Self> {
        Some(match family {
            MarkFamily::Division => Self::Division(id),
            MarkFamily::Arpeggio => Self::Arpeggio(id),
            MarkFamily::Articulation => Self::Articulation(id),
            MarkFamily::Fermata => Self::Fermata(id),
            MarkFamily::Tremolo => Self::Tremolo(id),
            MarkFamily::Dynamic => Self::Dynamic(id),
            MarkFamily::Technique => Self::Technique(id),
            MarkFamily::Harmonic => Self::Harmonic(id),
            MarkFamily::Grace => Self::Grace(id, 0),
            MarkFamily::ExpressionText => Self::Text(id, TextKind::Expression),
            MarkFamily::TechniqueText => Self::Text(id, TextKind::Technique),
            MarkFamily::Lyric => Self::Lyric(id, 0),
            MarkFamily::Chord => Self::Chord(id),
            MarkFamily::Tie | MarkFamily::Slur | MarkFamily::Hairpin | MarkFamily::Line => {
                return None;
            }
        })
    }
}

impl Score {
    /// What a written mark currently is, or `None` once it is gone.
    pub fn mark_kind(&self, mark: MarkRef) -> Option<MarkKind> {
        let notation = |id| self.event(id).map(|event| event.notation);
        match mark {
            MarkRef::Division(id) => {
                let part = self.part(id.part)?;
                if part.has_passage_divisi() {
                    part.divisi_change_at_tick(
                        id.measure,
                        (self.event_tick(id)? - self.measure_start_tick(id.measure)) as u16,
                    )
                    .map(MarkKind::Division)
                } else {
                    notation(id)?.division.map(MarkKind::Division)
                }
            }
            MarkRef::Arpeggio(id) => self
                .arpeggio_at(id)
                .map(|mark| MarkKind::Arpeggio(mark.kind)),
            MarkRef::Articulation(id) => notation(id)?.articulation.map(MarkKind::Articulation),
            MarkRef::Fermata(id) => notation(id)?.fermata.then_some(MarkKind::Fermata),
            MarkRef::Tremolo(id) => notation(id)?.tremolo.map(MarkKind::Tremolo),
            MarkRef::Dynamic(id) => notation(id)?.dynamic.map(MarkKind::Dynamic),
            MarkRef::Technique(id) => notation(id)?.technique.map(MarkKind::Technique),
            MarkRef::Harmonic(id) => notation(id)?.harmonic.map(MarkKind::Harmonic),
            MarkRef::Grace(id, index) => notation(id)?
                .grace
                .filter(|grace| grace.note(usize::from(index)).is_some())
                .map(|grace| MarkKind::Grace(grace.kind)),
            MarkRef::Text(id, kind) => self.text(id, kind).map(|_| MarkKind::Text(kind)),
            MarkRef::Lyric(id, verse) => notation(id)?
                .lyrics
                .verse(usize::from(verse))
                .map(|_| MarkKind::Lyric),
            MarkRef::Chord(id) => notation(id)?.chord.map(|_| MarkKind::Chord),
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
        match mark {
            MarkRef::Text(id, text) => {
                return Some(format!("{} «{}»", kind.label(), self.text(id, text)?));
            }
            MarkRef::Lyric(id, verse) => {
                let syllable = self.event(id)?.notation.lyrics.verse(usize::from(verse))?;
                return Some(format!(
                    "Letra, estrofa {} «{}»",
                    verse + 1,
                    syllable.text()
                ));
            }
            MarkRef::Chord(id) => {
                return Some(format!(
                    "Cifrado {}",
                    self.event(id)?.notation.chord?.label()
                ));
            }
            _ => {}
        }
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
            (MarkFamily::Division, Some(MarkKind::Division(mark))) => {
                self.set_division(id, Some(mark))
            }
            (MarkFamily::Division, _) => self.set_division(id, None),
            (MarkFamily::Arpeggio, Some(MarkKind::Arpeggio(mark))) => {
                self.set_arpeggio(id, Some(mark))
            }
            (MarkFamily::Arpeggio, _) => self.set_arpeggio(id, None),
            (MarkFamily::Articulation, Some(MarkKind::Articulation(mark))) => {
                self.set_articulation(id, Some(mark))
            }
            (MarkFamily::Articulation, _) => self.set_articulation(id, None),
            (MarkFamily::Fermata, kind) => self.set_fermata(id, kind.is_some()),
            (MarkFamily::Tremolo, Some(MarkKind::Tremolo(mark))) => {
                self.set_tremolo(id, Some(mark))
            }
            (MarkFamily::Tremolo, _) => self.set_tremolo(id, None),
            (MarkFamily::Dynamic, Some(MarkKind::Dynamic(mark))) => {
                self.set_dynamic(id, Some(mark))
            }
            (MarkFamily::Dynamic, _) => self.set_dynamic(id, None),
            (MarkFamily::Technique, Some(MarkKind::Technique(mark))) => {
                self.set_technique(id, Some(mark))
            }
            (MarkFamily::Technique, _) => self.set_technique(id, None),
            (MarkFamily::Harmonic, Some(MarkKind::Harmonic(mark))) => {
                self.set_harmonic(id, Some(mark))
            }
            (MarkFamily::Harmonic, _) => self.set_harmonic(id, None),
            (MarkFamily::Grace, Some(MarkKind::Grace(kind))) => self.set_grace(id, Some(kind)),
            (MarkFamily::Grace, _) => self.set_grace(id, None),
            (
                MarkFamily::ExpressionText | MarkFamily::TechniqueText,
                Some(MarkKind::Text(kind)),
            ) => {
                if self.text(id, kind).is_some() {
                    Ok(())
                } else {
                    Err(ScoreError("Escribe el texto de la indicación"))
                }
            }
            (MarkFamily::Lyric | MarkFamily::Chord, Some(_)) => {
                Err(ScoreError("Escribe la letra o el cifrado en su editor"))
            }
            (MarkFamily::Lyric, None) => self.set_syllable(id, 0, None),
            (MarkFamily::Chord, None) => self.set_chord(id, None),
            (MarkFamily::ExpressionText, _) => self.set_text(id, TextKind::Expression, None),
            (MarkFamily::TechniqueText, _) => self.set_text(id, TextKind::Technique, None),
            (MarkFamily::Tie | MarkFamily::Slur | MarkFamily::Hairpin | MarkFamily::Line, _) => {
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
            MarkRef::Grace(id, index) => self.remove_grace_note(id, usize::from(index)),
            MarkRef::Lyric(id, verse) => self.set_syllable(id, usize::from(verse), None),
            _ => self.set_point_mark(mark.anchor(), kind.family(), None),
        }
    }
    /// Take a mark to another place on its staff, with `offset` as its manual
    /// adjustment there: a point mark to another note or rest, a line to other
    /// ends. Returns whether anything changed.
    pub fn move_mark(
        &mut self,
        from: MarkRef,
        to: MarkRef,
        offset: Offset,
    ) -> Result<bool, ScoreError> {
        let kind = self
            .mark_kind(from)
            .ok_or(ScoreError("La marca ya no está en la partitura"))?;
        if !from.can_move() {
            return Err(ScoreError("Esta marca no se puede mover"));
        }
        let (source, target) = (from.anchor(), to.anchor());
        if source.part != target.part || source.staff != target.staff {
            return Err(ScoreError("Las marcas se mueven dentro de su pentagrama"));
        }
        if from != to {
            match (from, to) {
                (MarkRef::Connection(old), MarkRef::Connection(new)) if old.kind == new.kind => {
                    self.remove_connection(old);
                    self.add_connection(new)?;
                }
                (MarkRef::Connection(_), _) | (_, MarkRef::Connection(_)) => {
                    return Err(ScoreError("La marca no pertenece a esa familia"));
                }
                (MarkRef::Chord(_), MarkRef::Chord(_)) => {
                    if self.event(target).and_then(|e| e.notation.chord).is_some() {
                        return Err(ScoreError("Ahí ya hay un cifrado"));
                    }
                    let chord = self.event(source).and_then(|e| e.notation.chord);
                    self.set_chord(source, None)?;
                    self.set_chord(target, chord)?;
                }
                (MarkRef::Text(_, kind), MarkRef::Text(_, other)) if kind == other => {
                    if self.text(target, kind).is_some() {
                        return Err(ScoreError("Ahí ya hay un texto de ese tipo"));
                    }
                    let words = self.text(source, kind).expect("written text");
                    self.set_text(source, kind, None)?;
                    self.set_text(target, kind, Some(words.as_str()))?;
                }
                _ => {
                    let family = kind.family();
                    if MarkRef::point(family, target) != Some(to) {
                        return Err(ScoreError("La marca no pertenece a esa familia"));
                    }
                    if self.has_mark(to) {
                        return Err(ScoreError(match family {
                            MarkFamily::Dynamic => "Ahí ya hay una dinámica",
                            MarkFamily::Technique => "Ahí ya hay una técnica",
                            MarkFamily::Fermata => "Ahí ya hay un calderón",
                            _ => "Ahí ya hay una marca de ese tipo",
                        }));
                    }
                    self.set_point_mark(source, family, None)?;
                    self.set_point_mark(target, family, Some(kind))?;
                }
            }
        }
        let adjusted = self.set_mark_offset(to, offset)?;
        Ok(from != to || adjusted)
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
    fn arpeggios_require_chords_replace_remove_and_clear_with_the_last_interval() {
        let mut score = Score::default();
        let id = score.insert(0, 0, note(Step::C)).unwrap();
        assert!(score.set_arpeggio(id, Some(Arpeggio::Normal)).is_err());
        score
            .add_pitch(id, Pitch::new(Step::E, 4).unwrap())
            .unwrap();
        score.set_dynamic(id, Some(Dynamic::F)).unwrap();
        for mark in Arpeggio::ALL {
            score
                .set_point_mark(id, MarkFamily::Arpeggio, Some(MarkKind::Arpeggio(mark)))
                .unwrap();
            assert_eq!(
                score.mark_kind(MarkRef::Arpeggio(id)),
                Some(MarkKind::Arpeggio(mark))
            );
            score.validate().unwrap();
        }
        score.remove_mark(MarkRef::Arpeggio(id)).unwrap();
        assert_eq!(score.event(id).unwrap().notation.dynamic, Some(Dynamic::F));
        score.set_arpeggio(id, Some(Arpeggio::Up)).unwrap();
        score.remove_highest_pitch(id).unwrap();
        assert!(!score.has_mark(MarkRef::Arpeggio(id)));
        let invalid = Event {
            notation: Notation {
                arpeggio: Some(Arpeggio::Down),
                ..Default::default()
            },
            ..note(Step::C)
        };
        assert!(score.insert(0, 1, invalid).is_err());
        assert!(
            score
                .insert(
                    0,
                    1,
                    Event {
                        pitches: Pitches::default(),
                        ..invalid
                    }
                )
                .is_err()
        );
        score.validate().unwrap();
    }

    #[test]
    fn the_catalog_lists_every_kind_once_with_its_family_and_glyph() {
        let mut seen = std::collections::HashSet::new();
        for kind in MarkKind::ALL {
            assert!(seen.insert(kind), "{kind:?}");
            assert!(!kind.label().is_empty());
            assert_eq!(
                kind.glyph().is_some(),
                kind.is_point()
                    && !matches!(
                        kind.family(),
                        MarkFamily::Technique
                            | MarkFamily::Division
                            | MarkFamily::Grace
                            | MarkFamily::ExpressionText
                            | MarkFamily::TechniqueText
                            | MarkFamily::Lyric
                            | MarkFamily::Chord
                    )
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

    #[test]
    fn a_moved_mark_leaves_its_note_and_keeps_only_the_offset_it_is_given() {
        let mut score = Score::default();
        let a = score.insert(0, 0, note(Step::C)).unwrap();
        let rest = Event {
            pitches: Pitches::default(),
            ..note(Step::C)
        };
        let b = score.insert(0, 1, rest).unwrap();
        let c = score.insert(0, 2, note(Step::E)).unwrap();
        score.set_dynamic(a, Some(Dynamic::F)).unwrap();
        score.set_dynamic(c, Some(Dynamic::P)).unwrap();
        score
            .set_mark_offset(MarkRef::Dynamic(a), Offset { dx: 30, dy: 12 })
            .unwrap();
        let low = Offset { dx: 0, dy: 12 };

        // A dynamic may land on a rest, but not where another one is written.
        assert!(
            score
                .move_mark(MarkRef::Dynamic(a), MarkRef::Dynamic(b), low)
                .unwrap()
        );
        assert_eq!(score.event(a).unwrap().notation.dynamic, None);
        assert_eq!(score.event(a).unwrap().notation.offsets.dynamic, None);
        assert_eq!(score.event(b).unwrap().notation.dynamic, Some(Dynamic::F));
        assert_eq!(score.mark_offset(MarkRef::Dynamic(b)), low);
        assert_eq!(
            score.move_mark(MarkRef::Dynamic(b), MarkRef::Dynamic(c), low),
            Err(ScoreError("Ahí ya hay una dinámica"))
        );
        assert!(
            !score
                .move_mark(MarkRef::Dynamic(b), MarkRef::Dynamic(b), low)
                .unwrap()
        );
        assert!(
            score
                .move_mark(MarkRef::Dynamic(b), MarkRef::Fermata(c), low)
                .is_err()
        );
        score.set_fermata(a, true).unwrap();
        score
            .move_mark(MarkRef::Fermata(a), MarkRef::Fermata(b), Offset::ZERO)
            .unwrap();
        assert!(score.event(b).unwrap().notation.fermata);
        score.validate().unwrap();

        let other = score.add_part(Instrument::Violin).unwrap();
        let elsewhere = score.insert_in(other, 0, 0, note(Step::G)).unwrap();
        assert_eq!(
            score.move_mark(
                MarkRef::Dynamic(b),
                MarkRef::Dynamic(elsewhere),
                Offset::ZERO
            ),
            Err(ScoreError("Las marcas se mueven dentro de su pentagrama"))
        );
    }

    #[test]
    fn a_shifted_hairpin_keeps_its_length_as_far_as_the_onsets_allow() {
        let mut score = Score::default();
        let quarter = |step| Event {
            duration: Duration::Quarter,
            ..note(step)
        };
        let ids: Vec<_> = (0..4)
            .map(|index| score.insert(0, index, quarter(Step::C)).unwrap())
            .collect();
        let hairpin = Connection {
            start: ids[0],
            end: ids[2],
            kind: ConnectionKind::Crescendo,
        };
        score.add_connection(hairpin).unwrap();
        let shifted = score.shifted_connection(hairpin, ids[1]).unwrap();
        assert_eq!((shifted.start, shifted.end), (ids[1], ids[3]));
        // Near the end of the music it ends on the last onset it can reach.
        let last = score.shifted_connection(hairpin, ids[2]).unwrap();
        assert_eq!(last.end, ids[3]);
        assert_eq!(score.shifted_connection(hairpin, ids[3]), None);

        assert!(
            score
                .move_mark(
                    MarkRef::Connection(hairpin),
                    MarkRef::Connection(shifted),
                    Offset { dx: 0, dy: 8 },
                )
                .unwrap()
        );
        assert_eq!(
            score.connections().iter().copied().collect::<Vec<_>>(),
            [shifted]
        );
        assert_eq!(
            score.mark_offset(MarkRef::Connection(shifted)),
            Offset { dx: 0, dy: 8 }
        );
        assert_eq!(score.previous_event(ids[1]), Some(ids[0]));
        assert_eq!(score.previous_event(ids[0]), None);
    }
}
