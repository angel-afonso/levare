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
        let Some(other) = placed
            .iter()
            .find(|m| m.part == part && m.index == index && m.staff == target)
        else {
            continue;
        };
        let dy = other.origin.y - placed[source].origin.y;
        // Read with the notes of the staff they are drawn on.
        let signs = other.content.incoming_signs.clone();
        let Some(score_part) = score.part(part) else {
            continue;
        };
        let clef = score_part.clef_at(target, index);
        let key = score.written_key_at(part, index);
        let content = Arc::make_mut(&mut placed[source].content);
        let notes = std::mem::take(&mut content.crossing);
        let events: HashMap<_, _> = notes.iter().map(|n| (n.element, n.event)).collect();
        // Unbeamed, a stem points back to the note's own staff.
        let mut directions = HashMap::new();
        for mut group in std::mem::take(&mut content.crossing_beams) {
            let mut drawn = empty_layout();
            if group.spans_staves() {
                for note in group.notes.iter_mut().filter(|n| n.cross) {
                    let (high, low) = beaming::heads(&events[&note.element], clef);
                    (note.high, note.low) = (high + dy, low + dy);
                    directions.insert(note.element, target == StaffId::Upper);
                }
                let upper = |note: &beaming::Note| {
                    (if note.cross { target } else { staff }) == StaffId::Upper
                };
                beaming::engrave_between(group, upper, &mut drawn, metrics);
            } else {
                // Every note crosses: an ordinary beam on the other staff.
                for note in &mut group.notes {
                    (note.high, note.low) = beaming::heads(&events[&note.element], clef);
                }
                group.settle();
                for note in &group.notes {
                    directions.insert(note.element, group.down);
                }
                beaming::engrave_group(group, &mut drawn, metrics);
                shift(&mut drawn, dy);
            }
            append(content, drawn);
        }
        for note in notes {
            let beam = directions.get(&note.element).copied();
            let mut drawn = empty_layout();
            engrave_chord(
                &mut drawn,
                note.event,
                note.element,
                note.onset,
                clef,
                metrics,
                Some(beam.unwrap_or(target == StaffId::Upper)),
                beam.is_some(),
                note.voice,
                key,
            );
            let head = if note.event.duration.value() == score_core::NoteValue::Whole {
                metrics.whole
            } else {
                metrics.head
            };
            let signs = signs
                .get(&note.element.event)
                .map_or(&[][..], Vec::as_slice);
            engrave_accidentals(&mut drawn, note.element, signs, head);
            rhythm::dots(&mut drawn, note.event, note.element, metrics);
            shift(&mut drawn, dy);
            append(content, drawn);
        }
    }
}

/// Move what was engraved on one staff to another, `dy` lower.
fn shift(drawn: &mut MeasureLayout, dy: f32) {
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
    for group in &mut drawn.beams {
        for stem in &mut group.stems {
            stem.from.y += dy;
            stem.to.y += dy;
        }
        for corner in group.segments.iter_mut().flat_map(|s| &mut s.corners) {
            corner.y += dy;
        }
    }
}

fn append(content: &mut MeasureLayout, mut drawn: MeasureLayout) {
    content.glyphs.append(&mut drawn.glyphs);
    content.lines.append(&mut drawn.lines);
    content.events.append(&mut drawn.events);
    content.marks.append(&mut drawn.marks);
    content.beams.append(&mut drawn.beams);
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
        crossing_beams: Vec::new(),
        incoming_signs: HashMap::new(),
    }
}
