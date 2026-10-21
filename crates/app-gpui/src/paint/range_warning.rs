//! Editor-only warning ink; export keeps the original engraving unchanged.
use score_core::{ElementId, Pitch, Score};
use score_layout::{Glyph, MeasureLayout};

pub(super) const OUT_OF_RANGE: u32 = 0xc73845;
const FOCUSED_OUT_OF_RANGE: u32 = 0x951f30;

pub(super) fn color(
    score: &Score,
    measure: &MeasureLayout,
    glyph: &Glyph,
    focus: Option<(ElementId, Pitch)>,
) -> Option<u32> {
    // Single notes may use a composite head/stem/flag glyph. Chords and beamed
    // notes use individual heads; do not color rests, accidentals or marks.
    if !glyph
        .text
        .chars()
        .next()
        .is_some_and(|c| matches!(c, '\u{e0a2}'..='\u{e0a4}' | '\u{e1d2}'..='\u{e1db}'))
    {
        return None;
    }
    let id = glyph.element?;
    let pitch = measure.events.iter().find_map(|head| {
        (head.id == id && (head.center.y - glyph.position.y).abs() < 0.01)
            .then_some(head.pitch)
            .flatten()
    })?;
    score.pitch_range_warning(id, pitch)?;
    Some(if focus == Some((id, pitch)) {
        FOCUSED_OUT_OF_RANGE
    } else {
        OUT_OF_RANGE
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use score_core::{Duration, Event, Instrument, Notation, Pitch, Pitches, StaffId, Step};
    use score_layout::LayoutEngine;

    #[test]
    fn marks_only_out_of_range_heads_for_every_note_value_and_chords() {
        for duration in [
            Duration::Whole,
            Duration::Half,
            Duration::Quarter,
            Duration::Eighth,
            Duration::Sixteenth,
        ] {
            for chord in [false, true] {
                let mut score = Score::new("Rangos".into(), &[Instrument::Flute], 1).unwrap();
                let low = Pitch::new(Step::C, 3).unwrap();
                let valid = Pitch::new(Step::C, 5).unwrap();
                let pitches = if chord {
                    Pitches::new(&[low, valid]).unwrap()
                } else {
                    Pitches::single(Some(low))
                };
                score
                    .insert_on(
                        score.parts()[0].id(),
                        StaffId::Upper,
                        0,
                        0,
                        Event {
                            pitches,
                            duration,
                            notation: Notation::default(),
                        },
                    )
                    .unwrap();
                let layout = LayoutEngine::default().layout(&score, 960.);
                let measure = &layout.measures[0].content;
                let warnings: Vec<_> = measure
                    .glyphs
                    .iter()
                    .filter(|g| color(&score, measure, g, None).is_some())
                    .collect();
                assert_eq!(warnings.len(), 1, "{duration:?}, chord={chord}");
                assert_eq!(
                    color(&score, measure, warnings[0], None),
                    Some(OUT_OF_RANGE)
                );
                assert_eq!(
                    color(
                        &score,
                        measure,
                        warnings[0],
                        Some((warnings[0].element.unwrap(), low))
                    ),
                    Some(FOCUSED_OUT_OF_RANGE)
                );
                assert_eq!(
                    color(
                        &score,
                        measure,
                        warnings[0],
                        Some((warnings[0].element.unwrap(), valid))
                    ),
                    Some(OUT_OF_RANGE)
                );
            }
        }
    }

    #[test]
    fn beamed_notes_and_drag_previews_use_the_current_pitch() {
        let mut score = Score::new("Rangos".into(), &[Instrument::Flute], 1).unwrap();
        let part = score.parts()[0].id();
        let mut ids = Vec::new();
        for index in 0..2 {
            ids.push(
                score
                    .insert_on(
                        part,
                        StaffId::Upper,
                        0,
                        index,
                        Event {
                            pitches: Pitches::single(Pitch::new(Step::C, 3)),
                            duration: Duration::Eighth,
                            notation: Notation::default(),
                        },
                    )
                    .unwrap(),
            );
        }
        let mut engine = LayoutEngine::default();
        let layout = engine.layout(&score, 960.);
        let content = &layout.measures[0].content;
        assert!(!content.beams.is_empty());
        assert_eq!(
            content
                .glyphs
                .iter()
                .filter(|g| color(&score, content, g, None).is_some())
                .count(),
            2
        );
        score
            .replace_pitch(ids[0], 0, Pitch::new(Step::C, 5).unwrap())
            .unwrap();
        let layout = engine.layout(&score, 960.);
        let content = &layout.measures[0].content;
        assert_eq!(
            content
                .glyphs
                .iter()
                .filter(|g| color(&score, content, g, None).is_some())
                .count(),
            1
        );
    }
}
