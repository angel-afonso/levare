//! One voice of one measure as a line of text: `C4:4. R:8 [Eb4 G4]:2~ D5:8t{f}`.
//! Pitches are written pitches in scientific octaves (`C4` is middle C), as
//! stored in the score; the marks in braces are informational only.
use score_core::{
    Accidental, ConnectionKind, Duration, ElementId, Event, MarkKind, NoteValue, PartId, Pitch,
    Pitches, Score, StaffId, Step, Voice,
};
use std::fmt;

/// An event read from text, with the tie that joins it to the next event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WrittenEvent {
    pub event: Event,
    pub tie_to_next: bool,
}

/// A syntax error: the byte offset where the offending event starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextError {
    pub offset: usize,
    pub message: String,
}

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for TextError {}

/// Read the events of a voice. Mark blocks `{…}` are accepted and discarded,
/// so text read from a score can be sent back as it came.
pub fn parse_voice(text: &str) -> Result<Vec<WrittenEvent>, TextError> {
    let mut reader = Reader { text, pos: 0 };
    let mut events = Vec::new();
    loop {
        reader.skip_whitespace();
        if reader.peek().is_none() {
            return Ok(events);
        }
        let start = reader.pos;
        match reader.event() {
            Ok(event) => events.push(event),
            Err(reason) => {
                return Err(TextError {
                    offset: start,
                    message: format!("Evento «{}» no válido: {reason}", reader.token_at(start)),
                });
            }
        }
    }
}

/// Write one voice of one measure; empty when the measure or voice does not
/// exist or has no events. Pitches are the stored written pitches.
pub fn format_voice(
    score: &Score,
    part: PartId,
    staff: StaffId,
    measure: usize,
    voice: Voice,
) -> String {
    let Some(bar) = score
        .part(part)
        .and_then(|part| part.staff(staff))
        .and_then(|staff| staff.measures().get(measure))
    else {
        return String::new();
    };
    let here = |id: ElementId| id.part == part && id.staff == staff && id.measure == measure;
    let connections: Vec<_> = score
        .connections()
        .iter()
        .filter(|c| here(c.start) || here(c.end))
        .collect();
    let mut tokens = Vec::new();
    for (event_id, event) in bar.voice_iter(voice) {
        let id = ElementId {
            part,
            staff,
            measure,
            event: event_id,
        };
        let mut token = if event.pitches.is_empty() {
            String::from("R")
        } else if event.pitches.len() == 1 {
            pitch_text(event.pitches.first().expect("one pitch"))
        } else {
            let notes: Vec<_> = event.pitches.iter().map(pitch_text).collect();
            format!("[{}]", notes.join(" "))
        };
        token.push(':');
        token.push_str(&duration_text(event.duration));
        let mut marks: Vec<String> = MarkKind::ALL
            .into_iter()
            .filter(|kind| kind.is_point() && score.point_mark(id, kind.family()) == Some(*kind))
            .map(|kind| kind.label().to_string())
            .collect();
        for connection in &connections {
            if connection.kind == ConnectionKind::Tie {
                if connection.start == id {
                    token.push('~');
                }
                continue;
            }
            let label = connection.kind.label();
            if connection.start == id {
                marks.push(format!("{label} (inicio)"));
            }
            if connection.end == id {
                marks.push(format!("{label} (fin)"));
            }
        }
        if !marks.is_empty() {
            token.push('{');
            token.push_str(&marks.join(", "));
            token.push('}');
        }
        tokens.push(token);
    }
    tokens.join(" ")
}

fn pitch_text(pitch: Pitch) -> String {
    let accidental = match pitch.accidental() {
        Some(Accidental::Sharp) => "#",
        Some(Accidental::Flat) => "b",
        Some(Accidental::Natural) => "n",
        None => "",
    };
    format!("{:?}{accidental}{}", pitch.step, pitch.octave())
}

fn duration_text(duration: Duration) -> String {
    let value = match duration.value() {
        NoteValue::Whole => 1,
        NoteValue::Half => 2,
        NoteValue::Quarter => 4,
        NoteValue::Eighth => 8,
        NoteValue::Sixteenth => 16,
        NoteValue::ThirtySecond => 32,
    };
    format!(
        "{value}{}{}",
        if duration.is_dotted() { "." } else { "" },
        if duration.is_triplet() { "t" } else { "" }
    )
}

struct Reader<'a> {
    text: &'a str,
    pos: usize,
}

type Parsed<T> = Result<T, String>;

impl Reader<'_> {
    fn peek(&self) -> Option<char> {
        self.text[self.pos..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn eat(&mut self, expected: char) -> bool {
        let found = self.peek() == Some(expected);
        if found {
            self.pos += expected.len_utf8();
        }
        found
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }

    /// The event text from `start` to the next separator, for error messages.
    fn token_at(&self, start: usize) -> &str {
        let mut depth = 0u32;
        let mut end = self.text.len();
        for (offset, c) in self.text[start..].char_indices() {
            match c {
                '[' | '{' => depth += 1,
                ']' | '}' => depth = depth.saturating_sub(1),
                c if c.is_whitespace() && depth == 0 => {
                    end = start + offset;
                    break;
                }
                _ => {}
            }
        }
        &self.text[start..end]
    }

    fn event(&mut self) -> Parsed<WrittenEvent> {
        let (pitches, rest) = match self.peek() {
            Some('[') => (self.chord()?, false),
            Some('R') => {
                self.bump();
                (Pitches::single(None), true)
            }
            _ => (Pitches::single(Some(self.pitch()?)), false),
        };
        if !self.eat(':') {
            return Err("falta «:» y la duración (por ejemplo C4:4)".into());
        }
        let duration = self.duration()?;
        let tie_to_next = self.eat('~');
        if tie_to_next && rest {
            return Err("un silencio no se puede ligar".into());
        }
        if self.eat('{') {
            match self.text[self.pos..].find('}') {
                Some(close) => self.pos += close + 1,
                None => return Err("falta «}» al final de las marcas".into()),
            }
        }
        if self.peek().is_some_and(|c| !c.is_whitespace()) {
            return Err(format!(
                "no se esperaba «{}» tras el evento; sepáralos con un espacio",
                self.peek().expect("peeked")
            ));
        }
        Ok(WrittenEvent {
            event: Event {
                notation: Default::default(),
                pitches,
                duration,
            },
            tie_to_next,
        })
    }

    fn chord(&mut self) -> Parsed<Pitches> {
        self.bump();
        let mut pitches = Vec::new();
        loop {
            self.skip_whitespace();
            match self.peek() {
                None => return Err("falta «]» al final del acorde".into()),
                Some(']') => {
                    self.bump();
                    break;
                }
                Some(_) => pitches.push(self.pitch()?),
            }
        }
        if pitches.len() < 2 {
            return Err("un acorde necesita al menos dos alturas".into());
        }
        Pitches::new(&pitches).map_err(|error| error.0.to_string())
    }

    fn pitch(&mut self) -> Parsed<Pitch> {
        let letter = self
            .bump()
            .ok_or("falta la altura (letra de A a G o R para un silencio)")?;
        let step = Step::from_letter(&letter.to_ascii_lowercase().to_string())
            .filter(|_| letter.is_ascii_uppercase())
            .ok_or_else(|| {
                format!("«{letter}» no es una altura; usa una letra de A a G o R para un silencio")
            })?;
        let accidental = match self.peek() {
            Some('#') => Some(Accidental::Sharp),
            Some('b') => Some(Accidental::Flat),
            Some('n') => Some(Accidental::Natural),
            _ => None,
        };
        if accidental.is_some() {
            self.bump();
        }
        let digits = self.digits();
        if digits.is_empty() {
            return Err("falta la octava (de 0 a 8)".into());
        }
        let octave = digits
            .parse::<u8>()
            .ok()
            .filter(|octave| *octave <= 8)
            .ok_or_else(|| format!("octava {digits} fuera de rango (de 0 a 8)"))?;
        Pitch::new(step, octave)
            .map(|pitch| pitch.with_accidental(accidental))
            .ok_or_else(|| "octava fuera de rango (de 0 a 8)".into())
    }

    fn digits(&mut self) -> &str {
        let start = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.bump();
        }
        &self.text[start..self.pos]
    }

    fn duration(&mut self) -> Parsed<Duration> {
        let digits = self.digits();
        let value = match digits {
            "" => return Err("falta la duración tras «:» (1, 2, 4, 8, 16 o 32)".into()),
            "1" => NoteValue::Whole,
            "2" => NoteValue::Half,
            "4" => NoteValue::Quarter,
            "8" => NoteValue::Eighth,
            "16" => NoteValue::Sixteenth,
            "32" => NoteValue::ThirtySecond,
            other => {
                return Err(format!(
                    "duración «{other}» no válida; usa 1, 2, 4, 8, 16 o 32"
                ));
            }
        };
        let dotted = self.eat('.');
        let triplet = self.eat('t');
        Ok(Duration::default()
            .with_value(value)
            .with_dotted(dotted)
            .with_triplet(triplet))
    }
}
