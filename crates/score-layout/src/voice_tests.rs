use super::*;
use score_core::{Duration, Event, Pitch, Pitches, Step, Voice};
fn note(step: Step, duration: Duration) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 4)),
        duration,
        notation: Default::default(),
    }
}
#[test]
fn simultaneous_voices_share_time_opposite_stems_and_clear_seconds() {
    let mut s = Score::default();
    let p = s.parts()[0].id();
    let a = s.insert(0, 0, note(Step::E, Duration::Half)).unwrap();
    let b = s
        .insert_in_voice(
            p,
            StaffId::Upper,
            0,
            Voice::Two,
            0,
            note(Step::F, Duration::Quarter),
        )
        .unwrap();
    let rest = s
        .insert_in_voice(
            p,
            StaffId::Upper,
            0,
            Voice::Two,
            1,
            Event {
                pitches: Pitches::default(),
                ..note(Step::C, Duration::Quarter)
            },
        )
        .unwrap();
    let layout = LayoutEngine::default().layout_view(&s, 1000., ViewMode::Page);
    let bar = layout.measure_on(p, StaffId::Upper, 0).unwrap();
    let glyph_a = bar
        .content
        .glyphs
        .iter()
        .find(|g| g.element == Some(a))
        .unwrap();
    let glyph_b = bar
        .content
        .glyphs
        .iter()
        .find(|g| g.element == Some(b))
        .unwrap();
    assert_eq!(glyph_a.text, "\u{e1d3}");
    assert_eq!(glyph_b.text, "\u{e1d6}");
    assert!(glyph_b.position.x - glyph_a.position.x >= 12.);
    assert!(
        bar.content
            .events
            .iter()
            .find(|e| e.id == rest)
            .unwrap()
            .center
            .y
            > 20.
    );
    assert_eq!(bar.content.cursor_x.len(), 2);
    assert_eq!(bar.content.second_cursor_x.len(), 3);
    for mode in [ViewMode::Page, ViewMode::Continuous, ViewMode::SingleSystem] {
        let view = LayoutEngine::default().layout_view(&s, 1000., mode);
        assert!(
            view.voice_cursor(p, StaffId::Upper, 0, Voice::Two, 2)
                .is_some()
        );
        assert!(
            view.hit_pitch(
                view.measures
                    .iter()
                    .find(|m| m.part == p && m.index == 0)
                    .map(|m| Point {
                        x: m.origin.x + glyph_b.position.x + 6.,
                        y: m.origin.y + glyph_b.position.y
                    })
                    .unwrap()
            )
            .is_some()
        );
    }
}
#[test]
fn beams_never_join_voices_and_a_divided_violin_keeps_two_treble_clefs() {
    let mut s = Score::default();
    let p = s.add_part(Instrument::Violin).unwrap();
    for voice in Voice::ALL {
        for i in 0..4 {
            s.insert_in_voice(
                p,
                StaffId::Upper,
                0,
                voice,
                i,
                note(Step::E, Duration::Eighth),
            )
            .unwrap();
        }
    }
    let layout = LayoutEngine::default().layout_view(&s, 1000., ViewMode::Page);
    let bar = layout.measure_on(p, StaffId::Upper, 0).unwrap();
    assert!(!bar.content.beams.is_empty());
    for beam in &bar.content.beams {
        let voice = s.voice_of(beam.elements[0]).unwrap();
        assert!(
            beam.elements
                .iter()
                .all(|id| s.voice_of(*id) == Some(voice))
        );
        assert!(
            beam.stems
                .iter()
                .all(|stem| stem.down == (voice == Voice::Two))
        );
    }
    s.set_part_divisi(p, true).unwrap();
    let layout = LayoutEngine::default().layout_view(&s, 1000., ViewMode::Page);
    for staff in [StaffId::Upper, StaffId::Lower] {
        assert!(
            layout
                .measure_on(p, staff, 0)
                .unwrap()
                .content
                .glyphs
                .iter()
                .any(|g| g.text == "\u{e050}")
        );
    }
}

#[test]
fn accidentals_follow_staff_time_and_shared_unisons_select_the_active_voice() {
    use score_core::Accidental;
    let mut s = Score::default();
    let p = s.parts()[0].id();
    let mut sharp = note(Step::C, Duration::Quarter);
    sharp.pitches =
        Pitches::single(Pitch::new(Step::C, 4).map(|p| p.with_accidental(Some(Accidental::Sharp))));
    s.insert(0, 0, sharp).unwrap();
    s.insert_in_voice(
        p,
        StaffId::Upper,
        0,
        Voice::Two,
        0,
        Event {
            pitches: Pitches::default(),
            ..note(Step::C, Duration::Quarter)
        },
    )
    .unwrap();
    let natural = s
        .insert_in_voice(
            p,
            StaffId::Upper,
            0,
            Voice::Two,
            1,
            note(Step::C, Duration::Quarter),
        )
        .unwrap();
    let one = s.insert(1, 0, note(Step::G, Duration::Half)).unwrap();
    let two = s
        .insert_in_voice(
            p,
            StaffId::Upper,
            1,
            Voice::Two,
            0,
            note(Step::G, Duration::Half),
        )
        .unwrap();
    let view = LayoutEngine::default().layout_view(&s, 1000., ViewMode::Page);
    let bar = view.measure_on(p, StaffId::Upper, 0).unwrap();
    assert!(
        bar.content
            .glyphs
            .iter()
            .any(|g| g.element == Some(natural) && g.text == "\u{e261}")
    );
    let unison = view.measure_on(p, StaffId::Upper, 1).unwrap();
    let head = unison.content.events.iter().find(|e| e.id == one).unwrap();
    let point = Point {
        x: unison.origin.x + head.center.x,
        y: unison.origin.y + head.center.y,
    };
    assert_eq!(view.hit_pitch_in_voice(point, Voice::One).unwrap().0, one);
    assert_eq!(view.hit_pitch_in_voice(point, Voice::Two).unwrap().0, two);
}

#[test]
fn lower_voice_uses_bravura_below_marks_and_dynamics_clear_both_voices() {
    use score_core::{Articulation, Dynamic, MarkRef};
    let mut s = Score::default();
    let p = s.parts()[0].id();
    let one = s.insert(0, 0, note(Step::G, Duration::Whole)).unwrap();
    s.set_dynamic(one, Some(Dynamic::Mp)).unwrap();
    let two = s
        .insert_in_voice(
            p,
            StaffId::Upper,
            0,
            Voice::Two,
            0,
            note(Step::C, Duration::Half),
        )
        .unwrap();
    s.set_articulation(two, Some(Articulation::Staccato))
        .unwrap();
    s.set_fermata(two, true).unwrap();
    let view = LayoutEngine::default().layout_view(&s, 1000., ViewMode::Page);
    let bar = view.measure_on(p, StaffId::Upper, 0).unwrap();
    let staccato = bar
        .content
        .glyphs
        .iter()
        .find(|g| g.element == Some(two) && g.text == "\u{e4a3}")
        .unwrap();
    let fermata = bar
        .content
        .glyphs
        .iter()
        .find(|g| g.element == Some(two) && g.text == "\u{e4c1}")
        .unwrap();
    assert!(staccato.position.y > 40. && fermata.position.y > staccato.position.y);
    let dynamic = bar
        .content
        .marks
        .iter()
        .find(|m| m.mark == MarkRef::Dynamic(one))
        .unwrap();
    assert!(dynamic.center.y > 85.);
}

#[test]
fn passage_staff_counts_cursor_hits_and_spanning_connections_survive_every_view_and_cache() {
    use score_core::{Connection, ConnectionKind};
    let mut s = Score::default();
    let p = s.add_part(Instrument::Violin).unwrap();
    s.remove_part(s.parts()[0].id()).unwrap();
    while s.measures().len() < 6 {
        s.append_measure().unwrap();
    }
    let mut lower = Vec::new();
    for bar in 0..6 {
        s.insert_in(p, bar, 0, note(Step::G, Duration::Whole))
            .unwrap();
        lower.push(
            s.insert_in_voice(
                p,
                StaffId::Upper,
                bar,
                Voice::Two,
                0,
                note(Step::C, Duration::Whole),
            )
            .unwrap(),
        );
    }
    s.add_connection(Connection {
        start: lower[0],
        end: lower[5],
        kind: ConnectionKind::Slur,
    })
    .unwrap();
    s.add_connection(Connection {
        start: lower[0],
        end: lower[5],
        kind: ConnectionKind::Crescendo,
    })
    .unwrap();
    let mut engine = LayoutEngine::default();
    for mode in [ViewMode::Page, ViewMode::Continuous, ViewMode::SingleSystem] {
        s.set_passage_divisi(p, 1, Some(3), true).unwrap();
        let layout = engine.layout_view(&s, 1000., mode);
        for (bar, &lower_id) in lower.iter().enumerate() {
            assert_eq!(
                layout
                    .measures
                    .iter()
                    .filter(|m| m.part == p && m.index == bar)
                    .count(),
                if (1..=3).contains(&bar) { 2 } else { 1 }
            );
            let m = layout.event_measure(lower_id).unwrap();
            assert_eq!(
                m.staff,
                if (1..=3).contains(&bar) {
                    StaffId::Lower
                } else {
                    StaffId::Upper
                }
            );
            let e = m.content.events.iter().find(|e| e.id == lower_id).unwrap();
            assert_eq!(e.voice, Voice::Two);
            assert_eq!(
                layout
                    .hit_pitch_in_voice(
                        Point {
                            x: m.origin.x + e.center.x,
                            y: m.origin.y + e.center.y
                        },
                        Voice::Two
                    )
                    .unwrap()
                    .0,
                lower_id
            );
            let cursor = layout
                .voice_cursor(p, StaffId::Upper, bar, Voice::Two, 0)
                .unwrap();
            assert_eq!(cursor.y, m.origin.y);
        }
        let repeat = engine.layout_view(&s, 1000., mode);
        assert_eq!(engine.recomputed_measures, 0);
        assert_eq!(repeat.measures.len(), layout.measures.len());
        assert!(!layout.curves.is_empty());
        assert!(!layout.hairpins.is_empty());
        // A system may contain changing staff counts. Only the affected bars
        // get a lower staff; the other notes remain on their original row.
        for bar in 0..6 {
            assert_eq!(
                layout.measure_on(p, StaffId::Lower, bar).is_some(),
                (1..=3).contains(&bar)
            );
        }
        s.remove_divisi_change(p, 1).unwrap();
        s.remove_divisi_change(p, 4).unwrap();
        assert!(
            (0..6).all(|bar| !s.part(p).unwrap().divisi_at(bar)),
            "{:?}",
            s.part(p)
        );
        let reunited = engine.layout_view(&s, 1000., mode);
        assert_eq!(
            reunited.measures.len(),
            6,
            "{:?}",
            reunited
                .measures
                .iter()
                .map(|m| (m.index, m.staff))
                .collect::<Vec<_>>()
        );
        assert!(
            reunited
                .event_measure(lower[2])
                .unwrap()
                .content
                .events
                .iter()
                .any(|e| e.voice == Voice::Two)
        );
    }
}

#[test]
fn empty_and_second_voice_only_boundaries_always_have_a_visible_label() {
    let mut s = Score::default();
    let p = s.add_part(Instrument::Violin).unwrap();
    s.remove_part(s.parts()[0].id()).unwrap();
    s.set_passage_divisi(p, 0, Some(0), true).unwrap();
    for only_lower in [false, true] {
        if only_lower {
            s.insert_in_voice(
                p,
                StaffId::Upper,
                0,
                Voice::Two,
                0,
                note(Step::C, Duration::Whole),
            )
            .unwrap();
        }
        let layout = LayoutEngine::default().layout_view(&s, 1000., ViewMode::Page);
        assert!(
            layout.labels.iter().any(|label| label.text == "div.")
                || layout.measures.iter().any(|m| m
                    .content
                    .techniques
                    .iter()
                    .any(|t| t.text == "div."))
        );
        assert!(layout.labels.iter().any(|label| label.text == "unis."));
        for label in layout
            .labels
            .iter()
            .filter(|label| label.text == "div." || label.text == "unis.")
        {
            assert!(
                layout
                    .pages
                    .iter()
                    .any(|page| label.position.y > page.top + 80.
                        && label.position.y < page.top + page.height - 50.)
            );
        }
    }
}

#[test]
fn midbar_projection_routes_each_attack_cursor_and_staff_span_in_every_view() {
    use score_core::{DivisiPoint, Division};
    let mut s = Score::default();
    let p = s.add_part(Instrument::Violin).unwrap();
    s.remove_part(s.parts()[0].id()).unwrap();
    let mut lower = Vec::new();
    for i in 0..4 {
        s.insert_in(p, 0, i, note(Step::G, Duration::Quarter))
            .unwrap();
        lower.push(
            s.insert_in_voice(
                p,
                StaffId::Upper,
                0,
                Voice::Two,
                i,
                note(Step::C, Duration::Quarter),
            )
            .unwrap(),
        );
    }
    s.set_divisi_span(
        p,
        DivisiPoint::new(0, 48),
        Some(DivisiPoint::new(0, 144)),
        true,
    )
    .unwrap();
    let mut engine = LayoutEngine::default();
    for mode in [ViewMode::Page, ViewMode::Continuous, ViewMode::SingleSystem] {
        let layout = engine.layout_view(&s, 1000., mode);
        for (i, &id) in lower.iter().enumerate() {
            let staff = StaffId::Lower;
            let m = layout.event_measure(id).unwrap();
            assert_eq!(m.staff, staff);
            assert_eq!(
                layout
                    .voice_cursor(p, StaffId::Upper, 0, Voice::Two, i)
                    .unwrap()
                    .y,
                m.origin.y
            );
            let event = m.content.events.iter().find(|e| e.id == id).unwrap();
            assert_eq!(event.tick, i as u16 * 48);
            assert_eq!(
                layout
                    .hit_pitch_in_voice(
                        Point {
                            x: m.origin.x + event.center.x,
                            y: m.origin.y + event.center.y
                        },
                        Voice::Two
                    )
                    .unwrap()
                    .0,
                id
            );
        }
        let m = layout.measure_on(p, StaffId::Lower, 0).unwrap();
        assert_eq!(m.content.staff_spans, vec![(0., m.width)]);
        assert!(m.content.glyphs.iter().any(|g| g.text == "\u{e050}"));
        assert_eq!(
            layout.measure_at(Point {
                x: m.origin.x + 1.,
                y: m.origin.y + 20.
            }),
            Some((p, StaffId::Lower, 0))
        );
        assert!(
            layout
                .measures
                .iter()
                .flat_map(|m| &m.content.techniques)
                .any(|t| t.text == Division::Divisi.label())
        );
        let repeat = engine.layout_view(&s, 1000., mode);
        assert_eq!(engine.recomputed_measures, 0);
        assert_eq!(repeat.measures.len(), layout.measures.len());
    }
}

#[test]
fn sustained_notes_keep_their_duration_and_staff_through_midbar_division_labels() {
    use score_core::DivisiPoint;
    let mut s = Score::default();
    let p = s.add_part(Instrument::Violin).unwrap();
    s.remove_part(s.parts()[0].id()).unwrap();
    for i in 0..4 {
        s.insert_in(p, 0, i, note(Step::G, Duration::Quarter))
            .unwrap();
    }
    let held = s
        .insert_in_voice(
            p,
            StaffId::Upper,
            0,
            Voice::Two,
            0,
            note(Step::C, Duration::Whole),
        )
        .unwrap();
    s.set_divisi_span(
        p,
        DivisiPoint::new(0, 48),
        Some(DivisiPoint::new(0, 144)),
        true,
    )
    .unwrap();
    let before = s.clone();
    for mode in [ViewMode::Page, ViewMode::Continuous, ViewMode::SingleSystem] {
        let layout = LayoutEngine::default().layout_view(&s, 1000., mode);
        let mut fragments: Vec<_> = layout
            .measures
            .iter()
            .flat_map(|m| {
                m.content
                    .events
                    .iter()
                    .filter(move |e| e.id == held)
                    .map(move |e| (e.tick, e.end_tick, m.staff, e.voice))
            })
            .collect();
        fragments.sort_by_key(|f| f.0);
        assert_eq!(fragments, [(0, 192, StaffId::Lower, Voice::Two)]);
        assert!(
            !layout
                .curves
                .iter()
                .any(|c| c.connection.start == held && c.connection.end == held)
        );
        assert!(
            layout
                .measures
                .iter()
                .flat_map(|m| &m.content.events)
                .all(|e| s.event(e.id).is_some())
        );
        for m in &layout.measures {
            for e in m.content.events.iter().filter(|e| e.id == held) {
                let hit = layout
                    .hit_pitch_with_position(
                        Point {
                            x: m.origin.x + e.center.x,
                            y: m.origin.y + e.center.y,
                        },
                        Voice::Two,
                    )
                    .unwrap();
                assert_eq!((hit.0, hit.2), (held, e.tick));
            }
        }
        assert!(
            layout
                .voice_cursor(p, StaffId::Upper, 0, Voice::Two, 1)
                .is_some()
        );
    }
    assert_eq!(s, before);
    assert_eq!(s.event(held).unwrap().duration, Duration::Whole);
}

#[test]
fn redundant_boundary_labels_invalidate_cached_ink_and_reopening_resets_lower_accidentals() {
    use score_core::{Accidental, DivisiPoint, Division};
    let mut s = Score::default();
    let p = s.add_part(Instrument::Violin).unwrap();
    s.remove_part(s.parts()[0].id()).unwrap();
    let a = s
        .insert_in(p, 0, 0, note(Step::G, Duration::Whole))
        .unwrap();
    let mut ids = Vec::new();
    for i in 0..8 {
        let mut event = note(Step::C, Duration::Eighth);
        if i == 1 {
            event.pitches = Pitches::single(
                Pitch::new(Step::C, 4).map(|p| p.with_accidental(Some(Accidental::Sharp))),
            );
        }
        ids.push(
            s.insert_in_voice(p, StaffId::Upper, 0, Voice::Two, i, event)
                .unwrap(),
        );
    }
    let mut engine = LayoutEngine::default();
    engine.layout_view(&s, 1000., ViewMode::Page);
    s.set_division(a, Some(Division::Unison)).unwrap();
    let layout = engine.layout_view(&s, 1000., ViewMode::Page);
    assert!(engine.recomputed_measures > 0);
    assert!(
        layout
            .measures
            .iter()
            .any(|m| m.content.techniques.iter().any(|t| t.text == "unis."))
    );
    s.set_divisi_span(
        p,
        DivisiPoint::new(0, 24),
        Some(DivisiPoint::new(0, 72)),
        true,
    )
    .unwrap();
    s.set_divisi_span(
        p,
        DivisiPoint::new(0, 120),
        Some(DivisiPoint::new(0, 168)),
        true,
    )
    .unwrap();
    let layout = engine.layout_view(&s, 1000., ViewMode::Page);
    let lower = layout.event_measure(ids[5]).unwrap();
    assert_eq!(lower.content.staff_spans, vec![(0., lower.width)]);
    assert!(
        !lower
            .content
            .glyphs
            .iter()
            .any(|g| g.element == Some(ids[5]) && g.text == "\u{e261}")
    );
}

#[test]
fn projected_continuations_preserve_canonical_cursor_indices_and_attack_lookup() {
    use score_core::DivisiPoint;
    let mut s = Score::default();
    let p = s.add_part(Instrument::Violin).unwrap();
    s.remove_part(s.parts()[0].id()).unwrap();
    let a = s
        .insert_in_voice(
            p,
            StaffId::Upper,
            0,
            Voice::Two,
            0,
            note(Step::C, Duration::Half),
        )
        .unwrap();
    s.insert_in_voice(
        p,
        StaffId::Upper,
        0,
        Voice::Two,
        1,
        note(Step::D, Duration::Half),
    )
    .unwrap();
    s.set_divisi_span(p, DivisiPoint::new(0, 48), None, true)
        .unwrap();
    let layout = LayoutEngine::default().layout_view(&s, 1000., ViewMode::Page);
    for (index, tick) in [(1, 96), (2, 192)] {
        let m = layout.measure_on(p, StaffId::Lower, 0).unwrap();
        assert_eq!(
            layout
                .voice_cursor(p, StaffId::Upper, 0, Voice::Two, index)
                .unwrap()
                .x,
            m.origin.x + m.content.x_at_tick(tick as f64)
        );
    }
    s.set_divisi_span(
        p,
        DivisiPoint::new(0, 0),
        Some(DivisiPoint::new(0, 48)),
        true,
    )
    .unwrap();
    let layout = LayoutEngine::default().layout_view(&s, 1000., ViewMode::Page);
    assert_eq!(layout.event_measure(a).unwrap().staff, StaffId::Lower);
}
