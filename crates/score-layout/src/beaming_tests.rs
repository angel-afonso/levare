use super::*;
use score_core::{
    Articulation, Connection, ConnectionKind, Duration, Dynamic, Event, NoteValue, Pitch, Pitches,
    Step,
};
fn note(duration: Duration) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(Step::C, 4)),
        duration,
        notation: Default::default(),
    }
}
fn ids(
    s: &mut Score,
    part: PartId,
    staff: StaffId,
    bar: usize,
    durations: &[Duration],
) -> Vec<ElementId> {
    durations
        .iter()
        .enumerate()
        .map(|(i, d)| s.insert_on(part, staff, bar, i, note(*d)).unwrap())
        .collect()
}
fn no_flags(content: &MeasureLayout, ids: &[ElementId]) -> bool {
    content
        .glyphs
        .iter()
        .filter(|g| g.element.is_some_and(|id| ids.contains(&id)))
        .all(|g| {
            !g.text
                .chars()
                .any(|c| matches!(c as u32,0xe1d7..=0xe1da|0xe240..=0xe243))
        })
}
fn meter(n: u8, d: u8) -> Score {
    let mut value = serde_json::to_value(Score::default()).unwrap();
    value["time"] = serde_json::json!({"numerator":n,"denominator":d});
    let score: Score = serde_json::from_value(value).unwrap();
    score.validate().unwrap();
    score
}
#[test]
fn eighths_and_sixteenths_group_by_quarter_beats_with_independent_hands() {
    let mut s = Score::default();
    let part = s.parts()[0].id();
    let upper = ids(&mut s, part, StaffId::Upper, 0, &[Duration::Eighth; 8]);
    let lower = ids(&mut s, part, StaffId::Lower, 0, &[Duration::Sixteenth; 16]);
    let before = s.clone();
    let layout = LayoutEngine::default().layout_compact(&s, 960.);
    assert_eq!(s, before);
    for (staff, ids, size, levels) in [
        (StaffId::Upper, &upper, 2, 1),
        (StaffId::Lower, &lower, 4, 2),
    ] {
        let m = layout.measure_on(part, staff, 0).unwrap();
        assert_eq!(m.content.beams.len(), 4);
        assert!(no_flags(&m.content, ids));
        for group in &m.content.beams {
            assert_eq!(group.elements.len(), size);
            assert_eq!(group.segments.len(), levels);
            assert!(
                group
                    .elements
                    .iter()
                    .map(|id| s.event_tick(*id).unwrap() / 48)
                    .all(|beat| beat == s.event_tick(group.elements[0]).unwrap() / 48)
            );
            for stem in &group.stems {
                assert_eq!(stem.down, group.stems[0].down);
                assert!(stem.to.y.is_finite());
                assert!((stem.to.y - stem.from.y).abs() >= 34.9);
            }
            for segment in &group.segments {
                assert!(segment.corners[0].x < segment.corners[1].x);
                assert!((segment.corners[3].y - segment.corners[0].y).abs() >= 4.9);
            }
        }
    }
}
#[test]
fn rests_and_notes_crossing_beats_break_groups_but_ties_do_not() {
    let mut s = Score::default();
    let part = s.parts()[0].id();
    let mut durations = vec![Duration::Eighth; 4];
    durations.extend([Duration::Quarter, Duration::Eighth, Duration::Eighth]);
    let all = ids(&mut s, part, StaffId::Upper, 0, &durations);
    s.remove(all[1]).unwrap();
    let mut rest = note(Duration::Eighth);
    rest.pitches = Pitches::default();
    s.insert(0, 1, rest).unwrap();
    s.add_connection(Connection {
        start: all[2],
        end: all[3],
        kind: ConnectionKind::Tie,
    })
    .unwrap();
    let layout = LayoutEngine::default().layout_compact(&s, 960.);
    let m = layout.measure(part, 0).unwrap();
    assert_eq!(
        m.content
            .beams
            .iter()
            .map(|g| g.elements.len())
            .collect::<Vec<_>>(),
        [2, 2]
    );
    assert!(m.content.beam_stem(all[0]).is_none());
    assert!(m.content.beam_stem(all[2]).is_some());
    assert!(m.content.glyphs.iter().any(|g| g.element == Some(all[0])
        && g.text.chars().any(|c| matches!(c as u32, 0xe1d7..=0xe1d8))));
    let all = ids(
        &mut s,
        part,
        StaffId::Upper,
        1,
        &[
            Duration::Sixteenth,
            Duration::Sixteenth,
            Duration::Sixteenth,
            Duration::Eighth,
            Duration::Eighth,
            Duration::Sixteenth,
        ],
    );
    let layout = LayoutEngine::default().layout_compact(&s, 960.);
    let m = layout.measure(part, 1).unwrap();
    assert_eq!(
        m.content
            .beams
            .iter()
            .map(|g| g.elements.len())
            .collect::<Vec<_>>(),
        [3, 2]
    );
    assert!(m.content.beam_stem(all[3]).is_none());
}
#[test]
fn mixed_dotted_rhythms_get_secondary_beams_and_metric_hooks() {
    let mut s = Score::default();
    let part = s.parts()[0].id();
    let durations = [
        Duration::Eighth.with_dotted(true),
        Duration::Sixteenth,
        Duration::Sixteenth,
        Duration::Eighth.with_dotted(true),
        Duration::Eighth,
        Duration::Sixteenth,
        Duration::Sixteenth,
        Duration::Sixteenth,
        Duration::Sixteenth,
        Duration::Eighth,
    ];
    let all = ids(&mut s, part, StaffId::Upper, 0, &durations);
    let layout = LayoutEngine::default().layout_compact(&s, 1100.);
    let groups = &layout.measure(part, 0).unwrap().content.beams;
    assert_eq!(
        groups.iter().map(|g| g.elements.len()).collect::<Vec<_>>(),
        [2, 2, 3, 3]
    );
    for group in groups {
        assert_eq!(group.segments.iter().filter(|s| s.level == 2).count(), 1);
    }
    let left_hook = groups[0].segments.iter().find(|s| s.level == 2).unwrap();
    let stem = groups[0].stems[1].to;
    assert!(left_hook.corners[0].x < stem.x && left_hook.corners[1].x >= stem.x);
    assert!(left_hook.corners[1].x - left_hook.corners[0].x < 14.);
    let right_hook = groups[1].segments.iter().find(|s| s.level == 2).unwrap();
    let stem = groups[1].stems[0].to;
    assert!(right_hook.corners[0].x <= stem.x && right_hook.corners[1].x > stem.x);
    assert!(no_flags(&layout.measure(part, 0).unwrap().content, &all));
}
#[test]
fn simple_compound_and_odd_meters_have_repeatable_groups() {
    for (n, d, count, expected) in [
        (4, 4, 8, vec![2, 2, 2, 2]),
        (3, 4, 6, vec![2, 2, 2]),
        (2, 2, 8, vec![4, 4]),
        (3, 8, 3, vec![3]),
        (6, 8, 6, vec![3, 3]),
        (9, 8, 9, vec![3, 3, 3]),
        (12, 8, 12, vec![3, 3, 3, 3]),
        (5, 8, 5, vec![3, 2]),
        (7, 8, 7, vec![2, 2, 3]),
    ] {
        let mut s = meter(n, d);
        let part = s.parts()[0].id();
        ids(
            &mut s,
            part,
            StaffId::Upper,
            0,
            &vec![Duration::Eighth; count],
        );
        let layout = LayoutEngine::default().layout_compact(&s, 960.);
        let actual: Vec<_> = layout
            .measure(part, 0)
            .unwrap()
            .content
            .beams
            .iter()
            .map(|g| g.elements.len())
            .collect();
        assert_eq!(actual, expected, "{n}/{d}");
    }
}
#[test]
fn triplet_beams_follow_tuplet_limits_and_keep_numbers_and_partial_brackets() {
    for duration in [
        Duration::Eighth.with_triplet(true),
        Duration::Sixteenth.with_triplet(true),
    ] {
        let mut s = Score::default();
        let part = s.parts()[0].id();
        let count = usize::from(s.time().capacity() / duration.ticks());
        ids(&mut s, part, StaffId::Upper, 0, &vec![duration; count]);
        let layout = LayoutEngine::default().layout_compact(&s, 960.);
        let m = layout.measure(part, 0).unwrap();
        assert_eq!(m.content.beams.len(), count / 3);
        assert!(m.content.beams.iter().all(|g| g.elements.len() == 3));
        assert!(m.content.tuplets.iter().all(|g| !g.bracketed));
        assert_eq!(
            m.content
                .glyphs
                .iter()
                .filter(|g| g.text == "\u{e883}")
                .count(),
            count / 3
        );
        assert!(m.content.beams.iter().all(|g| g.segments.len()
            == if duration.value() == NoteValue::Sixteenth {
                2
            } else {
                1
            }));
    }
    let mut s = Score::default();
    let part = s.parts()[0].id();
    ids(
        &mut s,
        part,
        StaffId::Upper,
        0,
        &[Duration::Eighth.with_triplet(true); 2],
    );
    let layout = LayoutEngine::default().layout_compact(&s, 960.);
    assert!(layout.measure(part, 0).unwrap().content.tuplets[0].bracketed);
    let mut rest = note(Duration::Eighth.with_triplet(true));
    rest.pitches = Pitches::default();
    s.insert(0, 0, rest).unwrap();
    let layout = LayoutEngine::default().layout_compact(&s, 960.);
    let m = layout.measure(part, 0).unwrap();
    assert!(m.content.tuplets[0].bracketed);
    assert_eq!(m.content.beams[0].elements.len(), 2);
}
#[test]
fn forced_chord_stems_attach_seconds_and_expression_clears_beams_in_all_clefs() {
    for instrument in [Instrument::Violin, Instrument::Viola, Instrument::Cello] {
        let mut s = Score::default();
        let part = s.add_part(instrument).unwrap();
        let mut chord = note(Duration::Eighth);
        chord.pitches = Pitches::new(&[
            Pitch::new(Step::C, 4).unwrap(),
            Pitch::new(Step::D, 4).unwrap(),
        ])
        .unwrap();
        let a = s.insert_in(part, 0, 0, chord).unwrap();
        let mut high = note(Duration::Eighth);
        high.pitches = Pitches::single(Pitch::new(Step::C, 6));
        let b = s.insert_in(part, 0, 1, high).unwrap();
        s.set_dynamic(a, Some(Dynamic::F)).unwrap();
        s.set_articulation(b, Some(Articulation::Accent)).unwrap();
        let layout = LayoutEngine::default().layout_compact(&s, 960.);
        let m = layout.measure(part, 0).unwrap();
        let group = &m.content.beams[0];
        assert!(group.stems.iter().all(|s| s.down));
        let stem = m.content.beam_stem(a).unwrap();
        let half = Metrics::bravura().head / 2.;
        for head in m.content.events.iter().filter(|h| h.id == a) {
            assert!(
                (head.center.x - half - stem.from.x).abs() < 0.1
                    || (head.center.x + half - stem.from.x).abs() < 0.1
            );
            assert_eq!(
                layout.hit_test(Point {
                    x: m.origin.x + head.center.x,
                    y: m.origin.y + head.center.y
                }),
                Some(a)
            );
        }
        let dynamic = m
            .content
            .glyphs
            .iter()
            .find(|g| g.text == Dynamic::F.glyph())
            .unwrap();
        assert!(
            dynamic.position.y
                > group
                    .stems
                    .iter()
                    .map(|s| s.to.y)
                    .fold(f32::NEG_INFINITY, f32::max)
                    + 30.
        );
        assert!(
            m.content.ink_bottom
                >= group
                    .segments
                    .iter()
                    .flat_map(|s| s.corners)
                    .map(|p| p.y)
                    .fold(f32::NEG_INFINITY, f32::max)
        );
    }
}
#[test]
fn ties_use_the_shared_stem_direction_and_reengraving_removes_obsolete_beams() {
    let mut s = Score::default();
    let part = s.parts()[0].id();
    let all = ids(
        &mut s,
        part,
        StaffId::Upper,
        0,
        &[Duration::Sixteenth, Duration::Sixteenth, Duration::Eighth],
    );
    s.transpose(all[2], 14).unwrap();
    s.add_connection(Connection {
        start: all[0],
        end: all[1],
        kind: ConnectionKind::Tie,
    })
    .unwrap();
    let mut engine = LayoutEngine::default();
    let layout = engine.layout_compact(&s, 960.);
    let m = layout.measure(part, 0).unwrap();
    assert!(m.content.beam_stem(all[0]).unwrap().down);
    assert!(layout.curves[0].bend < 0.);
    engine.layout_compact(&s, 960.);
    assert_eq!(engine.recomputed_measures, 0);
    s.remove(all[1]).unwrap();
    s.set_duration(all[0], Duration::Quarter).unwrap();
    let layout = engine.layout_compact(&s, 960.);
    let m = layout.measure(part, 0).unwrap();
    assert!(m.content.beams.is_empty());
    assert!(engine.recomputed_measures > 0);
    assert!(m.content.glyphs.iter().any(|g| g.element == Some(all[2])
        && g.text.chars().any(|c| matches!(c as u32, 0xe1d7..=0xe1d8))));
}
