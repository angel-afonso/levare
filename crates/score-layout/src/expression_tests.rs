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
        start: ids[2],
        end: ids[3],
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
    // Text remains in its own lane; the slur attaches below it, near the note.
    assert!(curve.from.y > m.origin.y + label.position.y + 4.);
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
        let lowest_head = m
            .content
            .events
            .iter()
            .map(|e| e.center.y)
            .fold(40., f32::max);
        assert!(h.upper.from.y > m.origin.y + lowest_head + 16.);
        for same_row in layout
            .measures
            .iter()
            .filter(|bar| bar.part == m.part && bar.origin.y == m.origin.y)
        {
            for mark in same_row
                .content
                .marks
                .iter()
                .filter(|mark| matches!(mark.mark, score_core::MarkRef::Dynamic(_)))
            {
                let left = same_row.origin.x + mark.center.x - mark.half.0;
                let right = same_row.origin.x + mark.center.x + mark.half.0;
                assert!(
                    h.upper.to.x <= left - 5.9 || h.upper.from.x >= right + 5.9,
                    "hairpin overlaps a dynamic"
                );
            }
        }
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

#[test]
fn clicks_pick_a_mark_around_its_note_but_the_notehead_still_picks_the_note() {
    use score_core::{Instrument, MarkRef, PlayingTechnique};
    let mut s = Score::default();
    let piano = s.parts()[0].id();
    let violin = s.add_part(Instrument::Violin).unwrap();
    s.remove_part(piano).unwrap();
    let quarter = Event {
        duration: Duration::Quarter,
        ..note()
    };
    let ids: Vec<_> = (0..4)
        .map(|i| s.insert_in(violin, 0, i, quarter).unwrap())
        .collect();
    s.set_articulation(ids[0], Some(Articulation::Accent))
        .unwrap();
    s.set_dynamic(ids[0], Some(Dynamic::Mf)).unwrap();
    s.set_technique(ids[1], Some(PlayingTechnique::Pizzicato))
        .unwrap();
    let slur = Connection {
        start: ids[0],
        end: ids[2],
        kind: ConnectionKind::Slur,
    };
    let crescendo = Connection {
        start: ids[1],
        end: ids[3],
        kind: ConnectionKind::Crescendo,
    };
    s.add_connection(slur).unwrap();
    s.add_connection(crescendo).unwrap();
    let layout = LayoutEngine::default().layout(&s, 1000.);
    let measure = &layout.measures[0];
    let at = |p: Point| Point {
        x: measure.origin.x + p.x,
        y: measure.origin.y + p.y,
    };
    let marks = &measure.content.marks;
    assert_eq!(marks.len(), 3);
    for mark in marks {
        assert_eq!(layout.hit(at(mark.center)), Some(Hit::Mark(mark.mark)));
        if let Some(index) = mark.glyph {
            assert_eq!(
                measure.content.glyphs[index].element,
                Some(mark.mark.anchor())
            );
        }
    }
    assert!(
        marks
            .iter()
            .any(|m| m.mark == MarkRef::Articulation(ids[0]))
    );
    assert!(marks.iter().any(|m| m.mark == MarkRef::Dynamic(ids[0])));
    assert!(marks.iter().any(|m| m.mark == MarkRef::Technique(ids[1])));
    let head = measure
        .content
        .events
        .iter()
        .find(|e| e.id == ids[0])
        .unwrap();
    assert!(matches!(layout.hit(at(head.center)), Some(Hit::Note(id, _)) if id == ids[0]));
    let curve = layout.curves.iter().find(|c| c.connection == slur).unwrap();
    let middle = Point {
        x: (curve.from.x + curve.to.x) / 2.,
        y: (curve.from.y + curve.to.y) / 2. + curve.bend * 0.75,
    };
    assert_eq!(
        layout.hit(middle),
        Some(Hit::Mark(MarkRef::Connection(slur)))
    );
    let hairpin = &layout.hairpins[0];
    let inside = Point {
        x: (hairpin.upper.from.x + hairpin.upper.to.x) / 2.,
        y: (hairpin.upper.to.y + hairpin.lower.to.y) / 2.,
    };
    assert_eq!(
        layout.hit(inside),
        Some(Hit::Mark(MarkRef::Connection(crescendo)))
    );
    // Empty paper far from every mark selects nothing.
    assert_eq!(
        layout.hit(Point {
            x: middle.x,
            y: layout.systems[0].top - 60.
        }),
        None
    );
}

#[test]
fn a_fermata_sits_above_the_articulation_and_is_clickable_on_notes_and_rests() {
    use score_core::MarkRef;
    let mut s = Score::default();
    let quarter = Event {
        duration: Duration::Quarter,
        ..note()
    };
    let a = s.insert(0, 0, quarter).unwrap();
    let rest = s
        .insert(
            0,
            1,
            Event {
                pitches: score_core::Pitches::default(),
                ..quarter
            },
        )
        .unwrap();
    s.set_articulation(a, Some(Articulation::Accent)).unwrap();
    s.set_fermata(a, true).unwrap();
    s.set_fermata(rest, true).unwrap();
    let layout = LayoutEngine::default().layout(&s, 1000.);
    let measure = &layout.measures[0];
    let mark = |m| {
        *measure
            .content
            .marks
            .iter()
            .find(|b| b.mark == m)
            .unwrap_or_else(|| panic!("{m:?}"))
    };
    let fermata = mark(MarkRef::Fermata(a));
    let accent = mark(MarkRef::Articulation(a));
    assert!(fermata.center.y < accent.center.y - 8., "above the accent");
    assert_eq!(
        measure.content.glyphs[fermata.glyph.unwrap()].text,
        "\u{e4c0}"
    );
    for target in [MarkRef::Fermata(a), MarkRef::Fermata(rest)] {
        let b = mark(target);
        let point = Point {
            x: measure.origin.x + b.center.x,
            y: measure.origin.y + b.center.y,
        };
        assert_eq!(layout.hit(point), Some(Hit::Mark(target)));
    }
    assert!(measure.content.top <= fermata.center.y - 9.);
}

#[test]
fn changed_signatures_print_in_each_staff_with_cancellation_and_refresh_cached_bars() {
    use score_core::{KeyMode, KeySignature, TimeSignature};
    let mut score = Score::default();
    score
        .set_time_signature_at(
            1,
            TimeSignature {
                numerator: 6,
                denominator: 8,
            },
        )
        .unwrap();
    score
        .set_key_signature_at(1, KeySignature::new(1, KeyMode::Major).unwrap())
        .unwrap();
    score
        .set_key_signature_at(2, KeySignature::default())
        .unwrap();
    let mut engine = LayoutEngine::default();
    for width in [500., 1420.] {
        let layout = engine.layout(&score, width);
        for staff in [StaffId::Upper, StaffId::Lower] {
            let bar = layout.measure_on(score.parts()[0].id(), staff, 1).unwrap();
            assert_eq!(bar.start_tick, 192);
            for symbol in ["\u{e086}", "\u{e088}", "\u{e262}"] {
                assert!(
                    bar.content.glyphs.iter().any(|g| g.text == symbol),
                    "missing {symbol:?}"
                );
            }
            let bar = layout.measure_on(score.parts()[0].id(), staff, 2).unwrap();
            assert_eq!(bar.start_tick, 336);
            assert!(bar.content.glyphs.iter().any(|g| g.text == "\u{e261}"));
        }
    }
    score.remove_time_change(1).unwrap();
    score.remove_key_change(1);
    let layout = engine.layout(&score, 1420.);
    let bar = layout
        .measure_on(score.parts()[0].id(), StaffId::Upper, 1)
        .unwrap();
    assert!(!bar.content.glyphs.iter().any(|g| g.text == "\u{e088}"));
    assert!(engine.recomputed_measures > 0);
}

#[test]
fn arpeggios_use_bravura_segments_fit_chords_and_remain_clickable_in_every_view() {
    use score_core::{Accidental, Arpeggio, MarkRef, Pitches};
    for mark in Arpeggio::ALL {
        for interval in [1, 4, 14, 35] {
            let mut score = Score::default();
            let mut chord = note();
            let bass = Pitch::new(Step::C, 3)
                .unwrap()
                .with_accidental(Some(Accidental::Sharp));
            chord.pitches = Pitches::new(&[
                bass,
                bass.shift(interval)
                    .unwrap()
                    .with_accidental(Some(Accidental::Flat)),
            ])
            .unwrap();
            chord.duration = Duration::Half;
            let first = score.insert(0, 0, chord).unwrap();
            let second = score.insert(0, 1, chord).unwrap();
            let mut engine = LayoutEngine::default();
            score.set_arpeggio(first, Some(mark)).unwrap();
            score.set_arpeggio(second, Some(mark)).unwrap();
            for mode in [ViewMode::Page, ViewMode::Continuous, ViewMode::SingleSystem] {
                let mut layout = engine.layout_view(&score, 1000., mode);
                for scale in [0.28, 0.7, 1.5] {
                    layout.scale = scale;
                    for id in [first, second] {
                        let measure = layout.measure_on(id.part, id.staff, id.measure).unwrap();
                        let mark_box = measure
                            .content
                            .marks
                            .iter()
                            .find(|m| m.mark == MarkRef::Arpeggio(id))
                            .unwrap();
                        let glyph = &measure.content.glyphs[mark_box.glyph.unwrap()];
                        assert!(glyph.rotated);
                        assert!(glyph.text.chars().all(|c| matches!(
                            c,
                            '\u{eaa9}' | '\u{eaaa}' | '\u{eaad}' | '\u{eaae}'
                        )));
                        let (_, top, _, bottom) = engine.metrics.bounds(&glyph.text, true);
                        let high = measure
                            .content
                            .events
                            .iter()
                            .filter(|e| e.id == id)
                            .map(|e| e.center.y)
                            .fold(f32::INFINITY, f32::min);
                        let low = measure
                            .content
                            .events
                            .iter()
                            .filter(|e| e.id == id)
                            .map(|e| e.center.y)
                            .fold(f32::NEG_INFINITY, f32::max);
                        assert!(glyph.position.y + top <= high - 4.);
                        assert!(glyph.position.y + bottom >= low + 4.);
                        assert!(measure.content.ink_top <= glyph.position.y + top);
                        assert!(measure.content.ink_bottom >= glyph.position.y + bottom);
                        assert_eq!(
                            layout.hit(Point {
                                x: (measure.origin.x + mark_box.center.x) * scale,
                                y: (measure.origin.y + mark_box.center.y) * scale
                            }),
                            Some(Hit::Mark(MarkRef::Arpeggio(id)))
                        );
                        let right = mark_box.center.x + mark_box.half.0 - 3.;
                        for other in measure
                            .content
                            .glyphs
                            .iter()
                            .filter(|g| g.element == Some(id) && !g.rotated)
                        {
                            let (left, _, _, _) = engine.metrics.bounds(&other.text, false);
                            assert!(right + 6.9 <= other.position.x + left);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn one_bravura_arpeggio_spans_both_hands_after_wrapping_pagination_and_zoom() {
    use score_core::{Accidental, Arpeggio, MarkRef, Pitches, StaffId};
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    while score.measures().len() < 20 {
        score.append_measure().unwrap();
    }
    let mut pairs = Vec::new();
    for bar in 0..20 {
        let upper = score
            .insert(
                bar,
                0,
                Event {
                    pitches: Pitches::new(&[
                        Pitch::new(Step::C, 5)
                            .unwrap()
                            .with_accidental(Some(Accidental::Sharp)),
                        Pitch::new(Step::G, 5).unwrap(),
                    ])
                    .unwrap(),
                    ..note()
                },
            )
            .unwrap();
        let lower = score
            .insert_on(
                piano,
                StaffId::Lower,
                bar,
                0,
                Event {
                    pitches: Pitches::single(Pitch::new(Step::C, 3)),
                    ..note()
                },
            )
            .unwrap();
        if [0, 8, 19].contains(&bar) {
            score.set_arpeggio_both_hands(lower, true).unwrap();
            score
                .set_arpeggio(upper, Some(Arpeggio::ALL[pairs.len()]))
                .unwrap();
            pairs.push((upper, lower));
        }
    }
    let mut engine = LayoutEngine::default();
    for mode in [ViewMode::Page, ViewMode::Continuous, ViewMode::SingleSystem] {
        let mut layout = engine.layout_view(&score, 700., mode);
        assert_eq!(layout.piano_arpeggios.len(), 3);
        for scale in [0.28, 0.7, 1.5] {
            layout.scale = scale;
            for (upper, lower) in &pairs {
                let mark = layout
                    .piano_arpeggios
                    .iter()
                    .find(|mark| mark.bounds.mark == MarkRef::Arpeggio(*upper))
                    .unwrap();
                assert!(mark.glyph.rotated);
                let (_, top, _, bottom) = engine.metrics.bounds(&mark.glyph.text, true);
                for id in [upper, lower] {
                    let bar = layout.measure_on(id.part, id.staff, id.measure).unwrap();
                    assert!(
                        !bar.content
                            .glyphs
                            .iter()
                            .any(|glyph| glyph.element == Some(*id) && glyph.rotated)
                    );
                    for head in bar.content.events.iter().filter(|head| head.id == *id) {
                        assert!(mark.glyph.position.y + top < bar.origin.y + head.center.y);
                        assert!(mark.glyph.position.y + bottom > bar.origin.y + head.center.y);
                    }
                    for glyph in bar.content.glyphs.iter().filter(|glyph| {
                        glyph.element == Some(*id)
                            && glyph.text.chars().next().is_some_and(
                                |c| matches!(c,'\u{e0a2}'..='\u{e0a4}'|'\u{e1d2}'..='\u{e1db}'|'\u{e260}'..='\u{e262}'),
                            )
                    }) {
                        assert!(
                            mark.bounds.center.x + mark.bounds.half.0 + 3.9
                                <= bar.origin.x
                                    + glyph.position.x
                                    + engine.metrics.bounds(&glyph.text, false).0
                        );
                    }
                }
                assert_eq!(
                    layout.hit(Point {
                        x: mark.bounds.center.x * scale,
                        y: mark.bounds.center.y * scale
                    }),
                    Some(Hit::Mark(MarkRef::Arpeggio(*upper)))
                );
            }
        }
    }
}
