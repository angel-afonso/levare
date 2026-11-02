//! Notes drawn on the other staff of a piano, once both staves of a system
//! are placed. They are engraved here in the target staff's coordinates and
//! written into their own bar, so selecting, hit testing and painting treat
//! them as any other note of that bar.
use super::*;

/// A note or chord left as a gap by its own staff, to draw on the other.
#[derive(Clone, Debug)]
pub(crate) struct CrossingNote {
    pub element: ElementId,
    pub onset: f32,
    pub event: score_core::Event,
    pub voice: Voice,
    pub signs: Vec<PlacedAccidental>,
}

/// Draw the crossing notes of one part's placed bars in one system.
pub(super) fn draw(score: &Score, placed: &mut [PlacedMeasure], metrics: &Metrics) {
    let sources: Vec<usize> = (0..placed.len())
        .filter(|&i| !placed[i].content.crossing.is_empty())
        .collect();
    for source in sources {
        let (part, index, staff) = (
            placed[source].part,
            placed[source].index,
            placed[source].staff,
        );
        let target = match staff {
            StaffId::Upper => StaffId::Lower,
            StaffId::Lower => StaffId::Upper,
        };
        let Some(dy) = placed
            .iter()
            .find(|m| m.part == part && m.index == index && m.staff == target)
            .map(|m| m.origin.y - placed[source].origin.y)
        else {
            continue;
        };
        let Some(score_part) = score.part(part) else {
            continue;
        };
        let clef = score_part.clef_at(target, index);
        let key = score.written_key_at(part, index);
        let content = Arc::make_mut(&mut placed[source].content);
        for note in std::mem::take(&mut content.crossing) {
            // The stem points back to the note's own staff.
            let down = target == StaffId::Upper;
            let mut drawn = empty_layout();
            engrave_chord(
                &mut drawn,
                note.event,
                note.element,
                note.onset,
                clef,
                metrics,
                Some(down),
                false,
                note.voice,
                key,
            );
            let head = if note.event.duration.value() == score_core::NoteValue::Whole {
                metrics.whole
            } else {
                metrics.head
            };
            engrave_accidentals(&mut drawn, note.element, &note.signs, head);
            rhythm::dots(&mut drawn, note.event, note.element, metrics);
            for glyph in &mut drawn.glyphs {
                glyph.position.y += dy;
            }
            for line in &mut drawn.lines {
                line.from.y += dy;
                line.to.y += dy;
            }
            for event in &mut drawn.events {
                event.center.y += dy;
            }
            for mark in &mut drawn.marks {
                mark.center.y += dy;
            }
            content.glyphs.append(&mut drawn.glyphs);
            content.lines.append(&mut drawn.lines);
            content.events.append(&mut drawn.events);
            content.marks.append(&mut drawn.marks);
        }
    }
}

/// A bar's layout with nothing in it, to engrave one note into.
fn empty_layout() -> MeasureLayout {
    MeasureLayout {
        lines: Vec::new(),
        glyphs: Vec::new(),
        techniques: Vec::new(),
        expressions: Vec::new(),
        lyrics: Vec::new(),
        marks: Vec::new(),
        tuplets: Vec::new(),
        beams: Vec::new(),
        events: Vec::new(),
        staff_spans: Vec::new(),
        cursor_x: Default::default(),
        time_positions: Vec::new(),
        top: 0.,
        bottom: 40.,
        ink_top: 0.,
        ink_bottom: 40.,
        music_ink_bottom: 40.,
        cue: Vec::new(),
        crossing: Vec::new(),
    }
}
