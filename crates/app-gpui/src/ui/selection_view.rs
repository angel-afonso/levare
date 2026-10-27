//! What the Selection tab shows: the score when nothing is selected, a note
//! or rest with its marks, a passage, or the editor that is open. Computed
//! from the editor's current state only, without GPUI, so it can be tested;
//! `selection_panel` draws it.
use score_core::{
    Accidental, Connection, ConnectionKind, ElementId, MarkFamily, MarkKind, MarkRef, Pitch, Score,
    TICKS_PER_WHOLE, TextKind,
};
use score_edit::{Command, Editor};

/// The editors that take over the Selection tab while they are open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorKind {
    Clef,
    Key,
    Time,
    Tempo,
    Text,
    Lyric,
    Chord,
    ScoreInfo,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SelectionView {
    Score(ScoreSummary),
    Note(NoteSummary),
    Rest(NoteSummary),
    Passage(PassageSummary),
    Editor(EditorKind),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScoreSummary {
    pub title: String,
    pub subtitle: String,
    pub composer: String,
    pub key: String,
    pub time: String,
    pub tempo: String,
    pub bars: usize,
    pub parts: usize,
    /// As written, without repeats or fermatas ("2:08").
    pub duration: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NoteSummary {
    pub part: String,
    /// From 1.
    pub bar: usize,
    pub beat: String,
    pub voice: u8,
    /// The selected tone as written; `None` for a rest.
    pub pitch: Option<String>,
    /// How the selected tone sounds, only when the part transposes.
    pub sounding: Option<String>,
    /// Every tone of a chord as written, lowest first; empty for one note.
    pub tones: Vec<String>,
    /// Which of `tones` is selected.
    pub tone: usize,
    pub duration: String,
    pub range_warning: Option<String>,
    pub marks: Vec<MarkRow>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PassageSummary {
    pub parts: Vec<String>,
    /// From 1.
    pub first_bar: usize,
    pub last_bar: usize,
    pub notes: usize,
    pub marks: Vec<MarkRow>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MarkRow {
    pub label: String,
    pub glyph: MarkGlyph,
    /// The bar of the note it is listed under, from 1.
    pub bar: usize,
    pub remove: MarkRemoval,
}

/// How a mark is pictured in its row.
#[derive(Clone, Debug, PartialEq)]
pub enum MarkGlyph {
    /// A music-font symbol.
    Music(&'static str),
    /// A tie or a slur, drawn as the ribbon draws them.
    Curve { slur: bool },
    /// Words in italics: techniques, texts, syllables, chord symbols.
    Words(String),
}

/// What the row's ✕ runs: one existing, undoable command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkRemoval {
    /// A mark written on the note itself.
    Mark(MarkRef),
    /// A slur, tie, hairpin or line that starts or ends on the note.
    Connection(Connection),
}

impl MarkRemoval {
    pub fn command(self) -> Command {
        match self {
            Self::Mark(mark) => Command::RemoveMark(mark),
            Self::Connection(connection) => Command::RemoveConnection(connection),
        }
    }
}

pub fn selection_view(
    score: &Score,
    editor: &Editor,
    open_editor: Option<EditorKind>,
) -> SelectionView {
    if let Some(kind) = open_editor {
        return SelectionView::Editor(kind);
    }
    let ids = editor.selected_ids();
    match ids.as_slice() {
        [] => SelectionView::Score(score_summary(score)),
        [id] => match note_summary(score, editor, *id) {
            Some(summary) if summary.pitch.is_some() => SelectionView::Note(summary),
            Some(summary) => SelectionView::Rest(summary),
            None => SelectionView::Score(score_summary(score)),
        },
        ids => SelectionView::Passage(passage_summary(score, ids)),
    }
}

fn score_summary(score: &Score) -> ScoreSummary {
    let credits = score.credits();
    let time = score.time_at(0);
    ScoreSummary {
        title: score.title().to_owned(),
        subtitle: credits.subtitle.clone(),
        composer: credits.composer.clone(),
        key: score.key_signature_at(0).label().to_owned(),
        time: format!("{}/{}", time.numerator, time.denominator),
        tempo: score.tempo_at(0).label(),
        bars: score.measures().len(),
        parts: score.parts().len(),
        duration: clock(written_seconds(score)),
    }
}

/// Seconds from the first bar to the last at the written tempos: one step
/// per tempo change, not per note.
fn written_seconds(score: &Score) -> f64 {
    let bars = score.measures().len();
    let marks: Vec<_> = score.tempo_marks().filter(|(bar, _)| *bar < bars).collect();
    marks
        .iter()
        .enumerate()
        .map(|(index, (bar, mark))| {
            let end = marks.get(index + 1).map_or(bars, |(next, _)| *next);
            let ticks = score.measure_start_tick(end) - score.measure_start_tick(*bar);
            ticks as f64 / f64::from(mark.beat.ticks()) * 60. / f64::from(mark.bpm)
        })
        .sum()
}

fn clock(seconds: f64) -> String {
    let seconds = seconds.round() as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// "Re 4", "Fa♯ 5": the step's Spanish name, its sign and its octave.
pub fn pitch_name(pitch: Pitch) -> String {
    let name = match pitch.step {
        score_core::Step::C => "Do",
        score_core::Step::D => "Re",
        score_core::Step::E => "Mi",
        score_core::Step::F => "Fa",
        score_core::Step::G => "Sol",
        score_core::Step::A => "La",
        score_core::Step::B => "Si",
    };
    format!(
        "{name}{} {}",
        pitch.accidental().map_or("", Accidental::symbol),
        pitch.octave()
    )
}

/// "1", "2,5": a beat from 1, with a decimal comma.
pub fn beat_label(beat: f32) -> String {
    let text = format!("{beat:.2}");
    text.trim_end_matches('0')
        .trim_end_matches('.')
        .replace('.', ",")
}

fn note_summary(score: &Score, editor: &Editor, id: ElementId) -> Option<NoteSummary> {
    let event = score.event(id)?;
    let part = score.part(id.part)?;
    let key = score.written_key_at(id.part, id.measure);
    let selected = editor.selected_pitch();
    let tone = event
        .pitches
        .iter()
        .position(|pitch| Some(pitch) == selected)
        .unwrap_or(0);
    let pitch = event.pitches.iter().nth(tone);
    // Sounding pitches come spelled in C: a natural there is no news.
    let sounding = (!part.transposition().is_concert())
        .then(|| score.sounding_pitches(id).ok())
        .flatten()
        .and_then(|pitches| pitches.iter().nth(tone))
        .map(|pitch| {
            pitch_name(
                pitch.with_accidental(pitch.accidental().filter(|a| *a != Accidental::Natural)),
            )
        });
    let onset = score
        .event_tick(id)
        .map_or(0, |tick| tick - score.measure_start_tick(id.measure));
    let beat_ticks = f32::from(TICKS_PER_WHOLE) / f32::from(score.time_at(id.measure).denominator);
    Some(NoteSummary {
        part: score.part_name(id.part),
        bar: id.measure + 1,
        beat: beat_label(1. + onset as f32 / beat_ticks),
        voice: score.voice_of(id).unwrap_or_default().number(),
        pitch: pitch.map(|pitch| pitch_name(pitch.in_key(key))),
        sounding: pitch.and(sounding),
        tones: if event.pitches.len() > 1 {
            event
                .pitches
                .iter()
                .map(|pitch| pitch_name(pitch.in_key(key)))
                .collect()
        } else {
            Vec::new()
        },
        tone,
        duration: event.duration.label(),
        range_warning: pitch
            .and_then(|pitch| score.pitch_range_warning(id, pitch))
            .map(|range| format!("Fuera del registro habitual. Rango en altura real: {range}.")),
        marks: marks_of(score, &[id]),
    })
}

fn passage_summary(score: &Score, ids: &[ElementId]) -> PassageSummary {
    let mut parts: Vec<String> = Vec::new();
    for id in ids {
        let name = score.part_name(id.part);
        if !parts.contains(&name) {
            parts.push(name);
        }
    }
    PassageSummary {
        parts,
        first_bar: ids.iter().map(|id| id.measure).min().unwrap_or(0) + 1,
        last_bar: ids.iter().map(|id| id.measure).max().unwrap_or(0) + 1,
        notes: ids
            .iter()
            .filter(|id| score.event(**id).is_some_and(|e| !e.pitches.is_empty()))
            .count(),
        marks: marks_of(score, ids),
    }
}

/// Point families in the order the rows list them: articulation first, then
/// dynamics and techniques. Graces, texts, syllables and chord symbols follow
/// the connections.
const POINT_FAMILIES: [MarkFamily; 8] = [
    MarkFamily::Articulation,
    MarkFamily::Fermata,
    MarkFamily::Tremolo,
    MarkFamily::Arpeggio,
    MarkFamily::Harmonic,
    MarkFamily::Dynamic,
    MarkFamily::Technique,
    MarkFamily::Division,
];

/// Every mark on `ids`, note by note; a connection is listed once, under
/// the first selected note it starts or ends on.
fn marks_of(score: &Score, ids: &[ElementId]) -> Vec<MarkRow> {
    let mut rows = Vec::new();
    let mut listed: Vec<Connection> = Vec::new();
    for &id in ids {
        let Some(event) = score.event(id) else {
            continue;
        };
        let bar = id.measure + 1;
        let point = |mark: MarkRef, rows: &mut Vec<MarkRow>| {
            if let (Some(kind), Some(label)) = (score.mark_kind(mark), score.mark_label(mark)) {
                rows.push(MarkRow {
                    label,
                    glyph: glyph(score, mark, kind),
                    bar,
                    remove: MarkRemoval::Mark(mark),
                });
            }
        };
        for family in POINT_FAMILIES {
            if let Some(mark) = MarkRef::point(family, id) {
                point(mark, &mut rows);
            }
        }
        for connection in score.connections().iter() {
            if (connection.start == id || connection.end == id) && !listed.contains(connection) {
                listed.push(*connection);
                rows.push(connection_row(*connection, id, bar));
            }
        }
        let graces = event.notation.grace.map_or(0, |grace| grace.len());
        for index in 0..graces {
            point(MarkRef::Grace(id, index as u8), &mut rows);
        }
        for kind in TextKind::ALL {
            point(MarkRef::Text(id, kind), &mut rows);
        }
        for (verse, _) in event.notation.lyrics.verses() {
            point(MarkRef::Lyric(id, verse as u8), &mut rows);
        }
        point(MarkRef::Chord(id), &mut rows);
    }
    rows
}

fn connection_row(connection: Connection, at: ElementId, bar: usize) -> MarkRow {
    let kind = connection.kind;
    let label = if connection.start == at && connection.end.measure != at.measure {
        format!(
            "{} hasta el c. {}",
            kind.label(),
            connection.end.measure + 1
        )
    } else if connection.end == at && connection.start.measure != at.measure {
        format!(
            "{} desde el c. {}",
            kind.label(),
            connection.start.measure + 1
        )
    } else {
        kind.label().to_owned()
    };
    MarkRow {
        label,
        glyph: match kind {
            ConnectionKind::Tie | ConnectionKind::Slur => MarkGlyph::Curve {
                slur: kind == ConnectionKind::Slur,
            },
            _ => MarkGlyph::Music(MarkKind::Connection(kind).symbol().unwrap_or_default()),
        },
        bar,
        remove: MarkRemoval::Connection(connection),
    }
}

fn glyph(score: &Score, mark: MarkRef, kind: MarkKind) -> MarkGlyph {
    if let Some(symbol) = kind.symbol() {
        return MarkGlyph::Music(symbol);
    }
    let notation = score.event(mark.anchor()).map(|event| event.notation);
    MarkGlyph::Words(match mark {
        MarkRef::Text(id, text) => score
            .text(id, text)
            .map_or_else(|| kind.label().to_owned(), |words| words.to_string()),
        MarkRef::Lyric(_, verse) => notation
            .and_then(|n| n.lyrics.verse(usize::from(verse)))
            .map_or_else(String::new, |syllable| syllable.text().to_owned()),
        MarkRef::Chord(_) => notation
            .and_then(|n| n.chord)
            .map_or_else(String::new, |chord| chord.label()),
        _ => kind.label().to_owned(),
    })
}
