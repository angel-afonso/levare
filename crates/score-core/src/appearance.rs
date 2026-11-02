//! How a note is drawn without changing how it sounds: its notehead and the
//! way its beam joins its neighbours.
use super::*;

/// A notehead other than the usual oval. The note keeps its pitch and sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Notehead {
    /// An x: spoken or unpitched notes, ghost notes.
    Cross,
    /// A diamond, open or filled as the note value asks.
    Diamond,
    /// A slash, or diagonal: rhythm to improvise on.
    Slash,
}
impl Notehead {
    pub const ALL: [Self; 3] = [Self::Cross, Self::Diamond, Self::Slash];
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cross => "Cabeza en x",
            Self::Diamond => "Cabeza de rombo",
            Self::Slash => "Cabeza diagonal",
        }
    }
    /// The Bravura head for a note value: open for halves and wholes.
    pub const fn glyph(self, value: NoteValue) -> char {
        match (self, value) {
            (Self::Cross, NoteValue::Whole) => '\u{e0a7}',
            (Self::Cross, NoteValue::Half) => '\u{e0a8}',
            (Self::Cross, _) => '\u{e0a9}',
            (Self::Diamond, NoteValue::Whole) => '\u{e0d8}',
            (Self::Diamond, NoteValue::Half) => '\u{e0d9}',
            (Self::Diamond, _) => '\u{e0db}',
            (Self::Slash, NoteValue::Whole) => '\u{e102}',
            (Self::Slash, NoteValue::Half) => '\u{e103}',
            (Self::Slash, _) => '\u{e101}',
        }
    }
}

/// A change to the automatic beaming at one note, as Sibelius' keypad and
/// MuseScore's beam properties offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Beam {
    /// A new beam starts at this note, even within a beat.
    Start,
    /// The note joins the previous note's beam, even across a beat.
    Join,
    /// The note keeps its own flag.
    Off,
}
impl Beam {
    pub const ALL: [Self; 3] = [Self::Start, Self::Join, Self::Off];
    pub const fn label(self) -> &'static str {
        match self {
            Self::Start => "Iniciar barra de corchea",
            Self::Join => "Unir a la barra anterior",
            Self::Off => "Sin barra de corchea",
        }
    }
}

impl Score {
    pub fn set_notehead(
        &mut self,
        id: ElementId,
        value: Option<Notehead>,
    ) -> Result<(), ScoreError> {
        self.appearance(id, "Un silencio no tiene cabeza")?.notehead = value;
        Ok(())
    }
    pub fn set_beam(&mut self, id: ElementId, value: Option<Beam>) -> Result<(), ScoreError> {
        self.appearance(id, "Un silencio no lleva barra de corchea")?
            .beam = value;
        Ok(())
    }
    fn appearance(
        &mut self,
        id: ElementId,
        rest: &'static str,
    ) -> Result<&mut Notation, ScoreError> {
        let event = self
            .event(id)
            .ok_or(ScoreError("Selecciona una nota o acorde"))?;
        if event.pitches.is_empty() {
            return Err(ScoreError(rest));
        }
        Ok(&mut Arc::make_mut(
            &mut self.part_mut(id.part)?.staff_mut(id.staff)?.measures[id.measure].events,
        )[id.event]
            .notation)
    }
}
