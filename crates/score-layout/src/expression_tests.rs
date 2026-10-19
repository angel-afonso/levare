use super::*;
use score_core::{Articulation, Connection, ConnectionKind, Dynamic, Event, Notation, Pitch, Step};
fn note() -> Event {
    Event {
        pitches: score_core::Pitches::single(Pitch::new(Step::C, 4)),
        duration: Duration::Whole,
        notation: Notation::default(),
    }
}
#[test]
fn marks_reserve_space_and_connections_split_at_system_boundaries() {
    let mut s = Score::default();
    s.append_measure().unwrap();
    s.append_measure().unwrap();
    let mut ids = Vec::new();
    for i in 0..6 {
        ids.push(s.insert(i, 0, note()).unwrap());
    }
    s.set_dynamic(ids[0], Some(Dynamic::Ppp)).unwrap();
    s.set_dynamic(ids[1], Some(Dynamic::Fff)).unwrap();
    s.set_articulation(ids[0], Some(Articulation::Staccato))
        .unwrap();
    s.add_connection(Connection {
        start: ids[0],
        end: ids[5],
        kind: ConnectionKind::Slur,
    })
    .unwrap();
    s.add_connection(Connection {
        start: ids[1],
        end: ids[2],
        kind: ConnectionKind::Tie,
    })
    .unwrap();
    let mut engine = LayoutEngine::default();
    let layout = engine.layout_compact(&s, 680.);
    assert!(
        layout
            .curves
            .iter()
            .filter(|c| c.connection.kind == ConnectionKind::Slur)
            .count()
            > 1
    );
    assert_eq!(
        layout
            .curves
            .iter()
            .filter(|c| c.connection.kind == ConnectionKind::Tie)
            .count(),
        2
    );
    for c in &layout.curves {
        assert!(c.from.x < c.to.x);
        assert!(c.from.y + c.bend > 0.);
        assert!(c.to.y + c.bend < layout.height);
    }
    let m = layout.measure_on(ids[0].part, ids[0].staff, 0).unwrap();
    assert!(
        m.content
            .glyphs
            .iter()
            .any(|g| g.text == Dynamic::Ppp.glyph())
    );
    assert!(
        m.content
            .glyphs
            .iter()
            .any(|g| g.text == Articulation::Staccato.glyph())
    );
    let next = layout.measure_on(ids[0].part, StaffId::Lower, 0).unwrap();
    assert!(next.origin.y + next.content.ink_top > m.origin.y + m.content.ink_bottom);
    engine.layout_compact(&s, 680.);
    assert_eq!(engine.recomputed_measures, 0);
    s.set_articulation(ids[0], Some(Articulation::Accent))
        .unwrap();
    engine.layout_compact(&s, 680.);
    assert_eq!(engine.recomputed_measures, 1);
}
#[test]
fn chord_ties_have_one_curve_per_pitch_and_stay_outside_the_chord() {
    let mut s = Score::default();
    let mut event = note();
    event.duration = Duration::Half;
    event.pitches = score_core::Pitches::new(&[
        Pitch::new(Step::C, 4).unwrap(),
        Pitch::new(Step::E, 4).unwrap(),
        Pitch::new(Step::G, 4).unwrap(),
    ])
    .unwrap();
    let a = s.insert(0, 0, event).unwrap();
    let b = s.insert(0, 1, event).unwrap();
    s.add_connection(Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Tie,
    })
    .unwrap();
    let layout = LayoutEngine::default().layout_compact(&s, 960.);
    assert_eq!(layout.curves.len(), 3);
    assert!(layout.curves.iter().any(|c| c.bend > 0.));
    assert!(layout.curves.iter().any(|c| c.bend < 0.));
}

#[test]
fn forte_ascenders_clear_ties_below_low_whole_notes() {
    let mut s = Score::default();
    let a = s.insert(0, 0, note()).unwrap();
    let b = s.insert(1, 0, note()).unwrap();
    s.set_dynamic(a, Some(Dynamic::Fff)).unwrap();
    s.add_connection(Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Tie,
    })
    .unwrap();
    let layout = LayoutEngine::default().layout_compact(&s, 960.);
    let m = layout.measure_on(a.part, a.staff, 0).unwrap();
    let mark = m
        .content
        .glyphs
        .iter()
        .find(|g| g.text == Dynamic::Fff.glyph())
        .unwrap();
    let top = m.origin.y + mark.position.y + Metrics::bravura().glyph_bounds[&'\u{e522}'].0;
    let tie = layout
        .curves
        .iter()
        .find(|c| c.connection.kind == ConnectionKind::Tie)
        .unwrap();
    assert!(top > tie.from.y.max(tie.to.y) + tie.bend + 2.);
}

#[test]
fn technique_text_clears_articulation_slurs_and_adjacent_staves_and_invalidates_cache() {
    use score_core::{Instrument, PlayingTechnique as T};
    let mut s = Score::default();
    let part = s.add_part(Instrument::Violin).unwrap();
    let a = s.insert_in(part, 0, 0, note()).unwrap();
    let b = s.insert_in(part, 1, 0, note()).unwrap();
    s.set_technique(a, Some(T::Pizzicato)).unwrap();
    s.set_articulation(a, Some(Articulation::Accent)).unwrap();
    s.add_connection(Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Slur,
    })
    .unwrap();
    let mut engine = LayoutEngine::default();
    let layout = engine.layout_compact(&s, 960.);
    let m = layout.measure_on(part, a.staff, 0).unwrap();
    let label = &m.content.techniques[0];
    assert_eq!(label.text, "pizz.");
    assert!(
        label.position.y + 4.
            < m.content
                .glyphs
                .iter()
                .find(|g| g.text == Articulation::Accent.glyph())
                .unwrap()
                .position
                .y
                - 8.
    );
    assert!(m.content.ink_top <= label.position.y - 16.);
    let curve = layout
        .curves
        .iter()
        .find(|c| c.connection.start == a)
        .unwrap();
    assert!(curve.from.y < m.origin.y + label.position.y - 16.);
    assert!(m.origin.y + m.content.ink_top > 0.);
    engine.layout_compact(&s, 960.);
    assert_eq!(engine.recomputed_measures, 0);
    s.set_technique(a, Some(T::Arco)).unwrap();
    let layout = engine.layout_compact(&s, 960.);
    assert_eq!(engine.recomputed_measures, 1);
    assert_eq!(
        layout
            .measure_on(part, a.staff, 0)
            .unwrap()
            .content
            .techniques[0]
            .text,
        "arco"
    );
}

#[test]
fn hairpins_cross_systems_clear_dynamics_and_keep_openings_and_cache_correct() {
    let mut s = Score::default();
    s.append_measure().unwrap();
    s.append_measure().unwrap();
    let mut ids = Vec::new();
    for i in 0..6 {
        ids.push(s.insert(i, 0, note()).unwrap());
    }
    s.set_dynamic(ids[0], Some(Dynamic::P)).unwrap();
    s.set_dynamic(ids[5], Some(Dynamic::F)).unwrap();
    s.add_connection(Connection {
        start: ids[0],
        end: ids[5],
        kind: ConnectionKind::Crescendo,
    })
    .unwrap();
    let mut engine = LayoutEngine::default();
    let layout = engine.layout_compact(&s, 680.);
    assert!(layout.curves.is_empty());
    assert!(layout.hairpins.len() > 1);
    let first = &layout.hairpins[0];
    assert_eq!(first.upper.from.y, first.lower.from.y);
    let last = layout.hairpins.last().unwrap();
    assert!((last.lower.to.y - last.upper.to.y - 12.).abs() < 0.01);
    for h in &layout.hairpins {
        assert!(h.upper.from.x < h.upper.to.x);
        assert!(h.upper.from.y <= h.lower.from.y);
        let m = layout
            .measures
            .iter()
            .find(|m| {
                m.part == ids[0].part
                    && m.staff == StaffId::Upper
                    && h.upper.from.x >= m.origin.x
                    && h.upper.from.x <= m.origin.x + m.width
                    && h.upper.from.y > m.origin.y
                    && h.upper.from.y < m.origin.y + 160.
            })
            .unwrap();
        assert!(h.upper.from.y > m.origin.y + m.content.ink_bottom);
        let lower = layout.measure_on(m.part, StaffId::Lower, m.index).unwrap();
        assert!(h.lower.from.y < lower.origin.y + lower.content.ink_top);
    }
    engine.layout_compact(&s, 680.);
    assert_eq!(engine.recomputed_measures, 0);
    let crescendo = *s
        .connections()
        .iter()
        .find(|c| c.kind.is_hairpin())
        .unwrap();
    s.remove_connection(crescendo);
    s.add_connection(Connection {
        kind: ConnectionKind::Diminuendo,
        ..crescendo
    })
    .unwrap();
    let layout = engine.layout_compact(&s, 680.);
    assert_eq!(engine.recomputed_measures, 0);
    assert_eq!(
        layout.hairpins.last().unwrap().upper.to.y,
        layout.hairpins.last().unwrap().lower.to.y
    );
}
