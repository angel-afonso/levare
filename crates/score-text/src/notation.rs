//! One voice of one measure as a line of text: `C4:4. R:8 [Eb4 G4]:2~ D5:8t{f}`.
//! A tuplet follows the dots: `t` for a triplet, `(5)` for the usual ratio
//! of five, `(7:8)` for any other, and `(3:2/8)` when it counts in another
//! value than the note's.
//! Pitches are written pitches in scientific octaves (`C4` is middle C), as
//! stored in the score; the marks in braces are informational only.
use score_core::{
    Accidental, ConnectionKind, Duration, ElementId, Event, MarkKind, NoteValue, PartId, Pitch,
    Pitches, Score, StaffId, Step, Tuplet, Voice,
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

/// A note value as its number: 4 for a quarter, 16 for a sixteenth.
fn value_number(value: NoteValue) -> u8 {
    match value {
        NoteValue::Whole => 1,
        NoteValue::Half => 2,
        NoteValue::Quarter => 4,
        NoteValue::Eighth => 8,
        NoteValue::Sixteenth => 16,
        NoteValue::ThirtySecond => 32,
        NoteValue::SixtyFourth => 64,
    }
}

fn value_from_number(number: &str) -> Option<NoteValue> {
    Duration::ALL
        .into_iter()
        .map(Duration::value)
        .find(|value| value_number(*value).to_string() == number)
}

fn tuplet_text(tuplet: Tuplet, value: NoteValue) -> String {
    let in_value = tuplet.unit() == value;
    if in_value && tuplet.is_triplet() {
        return "t".into();
    }
    let ratio = if in_value && Tuplet::usual(tuplet.actual(), value) == Some(tuplet) {
        tuplet.actual().to_string()
    } else {
        format!("{}:{}", tuplet.actual(), tuplet.normal())
    };
    if in_value {
        format!("({ratio})")
    } else {
        format!("({ratio}/{})", value_number(tuplet.unit()))
    }
}

fn duration_text(duration: Duration) -> String {
    format!(
        "{}{}{}",
        value_number(duration.value()),
        ".".repeat(usize::from(duration.dots())),
        duration
            .tuplet()
            .map_or_else(String::new, |t| tuplet_text(t, duration.value()))
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
        if digits.is_empty() {
            return Err("falta la duración tras «:» (1, 2, 4, 8, 16, 32 o 64)".into());
        }
        let value = value_from_number(digits)
            .ok_or_else(|| format!("duración «{digits}» no válida; usa 1, 2, 4, 8, 16, 32 o 64"))?;
        let mut dots = 0;
        while dots < Duration::MAX_DOTS && self.eat('.') {
            dots += 1;
        }
        let tuplet = if self.eat('t') {
            Some(Tuplet::triplet(value))
        } else if self.eat('(') {
            Some(self.tuplet(value)?)
        } else {
            None
        };
        Ok(Duration::default()
            .with_value(value)
            .with_dots(dots)
            .with_tuplet(tuplet))
    }

    /// `5)`, `7:8)` or `3:2/8)`, after the opening parenthesis.
    fn tuplet(&mut self, value: NoteValue) -> Parsed<Tuplet> {
        const FORM: &str = "escribe el grupillo como (5), (7:8) o (3:2/8)";
        let number = |digits: &str| digits.parse::<u8>().ok();
        let actual =
            number(self.digits()).ok_or_else(|| format!("falta el número de notas; {FORM}"))?;
        let normal = if self.eat(':') {
            Some(
                number(self.digits())
                    .ok_or_else(|| format!("falta el tiempo del grupillo; {FORM}"))?,
            )
        } else {
            None
        };
        let unit = if self.eat('/') {
            let digits = self.digits().to_owned();
            value_from_number(&digits).ok_or_else(|| {
                format!("unidad «{digits}» no válida; usa 1, 2, 4, 8, 16, 32 o 64")
            })?
        } else {
            value
        };
        if !self.eat(')') {
            return Err(format!("falta «)» tras el grupillo; {FORM}"));
        }
        let tuplet = match normal {
            Some(normal) => Tuplet::new(actual, normal, unit),
            None => Tuplet::usual(actual, unit),
        };
        tuplet.ok_or_else(|| {
            let ratio = normal.map_or(actual.to_string(), |normal| format!("{actual}:{normal}"));
            format!(
                "grupillo {ratio} no admitido: de 2 a 9 notas, en un tiempo de más de la mitad y menos del doble"
            )
        })
    }
}

#[cfg(test)]
mod duration_text_tests {
    use super::*;

    fn read(text: &str) -> Result<Duration, String> {
        parse_voice(&format!("C4:{text}"))
            .map(|events| events[0].event.duration)
            .map_err(|error| error.message)
    }

    #[test]
    fn every_supported_duration_reads_back_as_written() {
        let units = [NoteValue::Quarter, NoteValue::Eighth, NoteValue::Sixteenth];
        for base in Duration::ALL {
            let value = base.value();
            for dots in 0..=Duration::MAX_DOTS {
                let tuplets = (2..=Tuplet::MAX_ACTUAL).flat_map(|actual| {
                    (1..=2 * actual).flat_map(move |normal| {
                        std::iter::once(value)
                            .chain(units)
                            .filter_map(move |unit| Tuplet::new(actual, normal, unit))
                    })
                });
                for tuplet in std::iter::once(None).chain(tuplets.map(Some)) {
                    let duration = base.with_dots(dots).with_tuplet(tuplet);
                    let text = duration_text(duration);
                    assert_eq!(read(&text), Ok(duration), "{text}");
                }
            }
        }
    }

    #[test]
    fn tuplets_are_written_as_briefly_as_their_ratio_allows() {
        let in_value = |value: Duration, actual, normal| {
            value.with_tuplet(Tuplet::new(actual, normal, value.value()))
        };
        assert_eq!(duration_text(Duration::Eighth.with_triplet(true)), "8t");
        assert_eq!(duration_text(in_value(Duration::Sixteenth, 5, 4)), "16(5)");
        assert_eq!(
            duration_text(in_value(Duration::Sixteenth, 7, 8)),
            "16(7:8)"
        );
        let quarter_in_eighths =
            Duration::Quarter.with_tuplet(Some(Tuplet::triplet(NoteValue::Eighth)));
        assert_eq!(duration_text(quarter_in_eighths), "4(3:2/8)");
        assert_eq!(duration_text(Duration::SixtyFourth.with_dots(2)), "64..");
    }

    #[test]
    fn earlier_texts_keep_their_meaning() {
        assert_eq!(read("4."), Ok(Duration::Quarter.with_dotted(true)));
        assert_eq!(read("8t"), Ok(Duration::Eighth.with_triplet(true)));
        assert_eq!(
            read("8.t"),
            Ok(Duration::Eighth.with_dotted(true).with_triplet(true))
        );
    }

    #[test]
    fn malformed_tuplets_say_how_to_write_them() {
        for text in ["16(", "16(5", "16(5:)", "16(5/7)"] {
            let error = read(text).unwrap_err();
            assert!(
                error.contains("(5)") || error.contains("unidad"),
                "{text}: {error}"
            );
        }
        for text in ["16(10)", "16(3:3)", "16(5:1)"] {
            let error = read(text).unwrap_err();
            assert!(error.contains("no admitido"), "{text}: {error}");
        }
    }
}
