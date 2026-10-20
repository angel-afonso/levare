use super::*;

fn orchestra() -> Score {
    let native: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../examples/tres-luces-orquesta.partitura"
    ))
    .unwrap();
    serde_json::from_value(native["score"].clone()).unwrap()
}

#[test]
fn orchestra_fits_four_bar_phrases_and_several_systems_per_page() {
    let score = orchestra();
    let mut engine = LayoutEngine::default();
    let layout = engine.layout_view(&score, 1400., ViewMode::Page);
    assert!(
        layout.pages.len() <= 4,
        "16 short orchestral bars should fit at most four A4 pages, got {}",
        layout.pages.len()
    );
    assert_eq!(
        layout.systems[0].last_measure, 3,
        "divisi should not force a break halfway through the first phrase"
    );
    let phrase = layout
        .systems
        .iter()
        .find(|s| s.first_measure == 4)
        .unwrap();
    assert_eq!(
        phrase.last_measure, 7,
        "a short four-bar pizzicato phrase should fit in one system"
    );
    let mut origins: Vec<_> = layout
        .measures
        .iter()
        .filter(|m| m.system_start == 0)
        .map(|m| m.origin.y)
        .collect();
    origins.sort_by(f32::total_cmp);
    origins.dedup();
    for pair in origins.windows(2) {
        assert!(
            // Ordered text lanes may add one text height; still keep the
            // compact orchestral layout well below doubled staff spacing.
            pair[1] - pair[0] <= 230.,
            "excessive gap between adjacent staves: {}",
            pair[1] - pair[0]
        );
    }
}

#[test]
fn midbar_divisi_keeps_a_held_voice_on_one_continuous_staff() {
    use score_core::{DivisiPoint, Event, Notation, Pitch, Pitches, Step};
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    let part = score.add_part(Instrument::Violin).unwrap();
    score.remove_part(piano).unwrap();
    let event = |d| Event {
        pitches: Pitches::single(Pitch::new(Step::C, 4)),
        duration: d,
        notation: Notation::default(),
    };
    score.insert_in(part, 0, 0, event(Duration::Whole)).unwrap();
    let held = score
        .insert_in_voice(
            part,
            StaffId::Upper,
            0,
            Voice::Two,
            0,
            event(Duration::Whole),
        )
        .unwrap();
    score
        .set_divisi_span(
            part,
            DivisiPoint::new(0, 48),
            Some(DivisiPoint::new(0, 144)),
            true,
        )
        .unwrap();
    for mode in [ViewMode::Page, ViewMode::Continuous, ViewMode::SingleSystem] {
        let layout = LayoutEngine::default().layout_view(&score, 1400., mode);
        let heads: Vec<_> = layout
            .measures
            .iter()
            .flat_map(|m| {
                m.content
                    .events
                    .iter()
                    .filter(move |e| e.id == held)
                    .map(move |e| Point {
                        x: m.origin.x + e.center.x,
                        y: m.origin.y + e.center.y,
                    })
            })
            .collect();
        let ties: Vec<_> = layout
            .curves
            .iter()
            .filter(|c| c.connection.start == held && c.connection.end == held)
            .collect();
        assert_eq!(
            heads.len(),
            1,
            "held notes must not become artificial fragments"
        );
        assert!(
            ties.is_empty(),
            "no synthetic ties are needed for a held note"
        );
        let lower = layout.event_measure(held).unwrap();
        assert_eq!(lower.staff, StaffId::Lower);
        assert_eq!(lower.content.staff_spans, vec![(0., lower.width)]);
        assert!(!score.part(part).unwrap().divisi_at_tick(0, 0));
        assert!(score.part(part).unwrap().divisi_at_tick(0, 48));
        assert!(!score.part(part).unwrap().divisi_at_tick(0, 144));
        let upper = layout.measure_on(part, StaffId::Upper, 0).unwrap();
        for (tick, text) in [(48, "div."), (144, "unis.")] {
            assert!(layout.labels.iter().any(|label| label.part == part
                && label.text == text
                && (label.position.x - upper.origin.x - upper.content.x_at_tick(tick as f64)).abs()
                    < 0.01));
        }
    }
}

#[test]
fn expressions_do_not_double_the_width_of_a_plain_four_bar_phrase() {
    use score_core::{Dynamic, Event, Notation, Pitch, Pitches, PlayingTechnique, Step};
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    let part = score.add_part(Instrument::Violin).unwrap();
    score.remove_part(piano).unwrap();
    for b in 0..4 {
        for n in 0..4 {
            let id = score
                .insert_in(
                    part,
                    b,
                    n,
                    Event {
                        pitches: Pitches::single(Pitch::new(Step::C, 4)),
                        duration: Duration::Quarter,
                        notation: Notation::default(),
                    },
                )
                .unwrap();
            if n == 0 {
                score.set_dynamic(id, Some(Dynamic::P)).unwrap();
                score
                    .set_technique(id, Some(PlayingTechnique::Arco))
                    .unwrap();
            }
        }
    }
    let layout = LayoutEngine::default().layout_compact(&score, 1000.);
    assert_eq!(
        layout.systems.len(),
        1,
        "a plain four-bar phrase with p and arco should fit on one line"
    );
}

#[test]
fn shared_hairpin_lane_clears_low_notes_after_a_high_dynamic() {
    use score_core::{Connection, ConnectionKind, Dynamic, Event, Notation, Pitch, Pitches, Step};
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    let part = score.add_part(Instrument::Violin).unwrap();
    score.remove_part(piano).unwrap();
    let mut ids = vec![];
    for (i, octave) in [5, 3, 3, 5].into_iter().enumerate() {
        ids.push(
            score
                .insert_in(
                    part,
                    0,
                    i,
                    Event {
                        pitches: Pitches::single(Pitch::new(Step::C, octave)),
                        duration: Duration::Quarter,
                        notation: Notation::default(),
                    },
                )
                .unwrap(),
        );
    }
    score.set_dynamic(ids[0], Some(Dynamic::P)).unwrap();
    score
        .add_connection(Connection {
            start: ids[0],
            end: ids[3],
            kind: ConnectionKind::Crescendo,
        })
        .unwrap();
    let layout = LayoutEngine::default().layout_view(&score, 1000., ViewMode::Page);
    let bar = layout.measure(part, 0).unwrap();
    for h in &layout.hairpins {
        for head in bar
            .content
            .events
            .iter()
            .filter(|e| ids[1..3].contains(&e.id))
        {
            let x = bar.origin.x + head.center.x;
            if (h.upper.from.x..=h.upper.to.x).contains(&x) {
                let y = h.upper.from.y
                    + (h.upper.to.y - h.upper.from.y) * (x - h.upper.from.x)
                        / (h.upper.to.x - h.upper.from.x);
                assert!(
                    y >= bar.origin.y + head.center.y + 20.,
                    "hairpin crosses a low note: {y}"
                );
            }
        }
    }
}

#[test]
fn repeated_long_dynamics_keep_horizontal_clearance_in_dense_music() {
    use score_core::{Dynamic, Event, MarkRef, Notation, Pitch, Pitches, Step};
    let mut score = Score::default();
    let part = score.parts()[0].id();
    for i in 0..8 {
        let id = score
            .insert_in(
                part,
                0,
                i,
                Event {
                    pitches: Pitches::single(Pitch::new(Step::C, 5)),
                    duration: Duration::Eighth,
                    notation: Notation::default(),
                },
            )
            .unwrap();
        score.set_dynamic(id, Some(Dynamic::Ppp)).unwrap();
    }
    let layout = LayoutEngine::default().layout_view(&score, 1000., ViewMode::SingleSystem);
    let m = layout.measure(part, 0).unwrap();
    let dynamics: Vec<_> = m
        .content
        .marks
        .iter()
        .filter(|mark| matches!(mark.mark, MarkRef::Dynamic(_)))
        .collect();
    for pair in dynamics.windows(2) {
        assert!(
            pair[0].center.x + pair[0].half.0 + 6. <= pair[1].center.x - pair[1].half.0,
            "dense dynamics overlap"
        );
    }
}

#[test]
fn written_ties_cross_divisi_staff_changes_without_forcing_a_system_break() {
    use score_core::{
        Connection, ConnectionKind, DivisiPoint, Event, Notation, Pitch, Pitches, Step,
    };
    let mut s = Score::default();
    let piano = s.parts()[0].id();
    let p = s.add_part(Instrument::Violin).unwrap();
    s.remove_part(piano).unwrap();
    let event = |step| Event {
        pitches: Pitches::single(Pitch::new(step, 4)),
        duration: Duration::Whole,
        notation: Notation::default(),
    };
    let mut lower = vec![];
    for b in 0..4 {
        s.insert_in(p, b, 0, event(Step::G)).unwrap();
        lower.push(
            s.insert_in_voice(p, StaffId::Upper, b, Voice::Two, 0, event(Step::C))
                .unwrap(),
        );
    }
    s.set_divisi_span(
        p,
        DivisiPoint::new(1, 0),
        Some(DivisiPoint::new(2, 0)),
        true,
    )
    .unwrap();
    let c = Connection {
        start: lower[0],
        end: lower[1],
        kind: ConnectionKind::Tie,
    };
    s.add_connection(c).unwrap();
    let mut engine = LayoutEngine::default();
    let layout = engine.layout_view(&s, 1600., ViewMode::Continuous);
    assert_eq!(layout.systems.len(), 1);
    assert!(layout.measure_on(p, StaffId::Lower, 0).is_none());
    let first = layout.event_measure(lower[0]).unwrap();
    let last = layout.event_measure(lower[1]).unwrap();
    let head = |m: &PlacedMeasure, id| {
        let h = m.content.events.iter().find(|e| e.id == id).unwrap();
        Point {
            x: m.origin.x + h.center.x,
            y: m.origin.y + h.center.y,
        }
    };
    let curve = layout
        .curves
        .iter()
        .find(|curve| curve.connection == c)
        .unwrap();
    for (end, head) in [
        (curve.from, head(first, lower[0])),
        (curve.to, head(last, lower[1])),
    ] {
        assert!((end.x - head.x).abs() < 14. && (end.y - head.y).abs() < 12.);
    }
    engine.layout_view(&s, 1600., ViewMode::Continuous);
    assert_eq!(engine.recomputed_measures, 0);
}

#[test]
fn original_slurs_and_hairpins_reach_upper_voice_after_divisi_within_one_system() {
    use score_core::{
        Connection, ConnectionKind, DivisiPoint, Event, Notation, Pitch, Pitches, Step,
    };
    for kind in [ConnectionKind::Slur, ConnectionKind::Crescendo] {
        let mut s = Score::default();
        let piano = s.parts()[0].id();
        let p = s.add_part(Instrument::Violin).unwrap();
        s.remove_part(piano).unwrap();
        let mut ids = vec![];
        for b in 0..4 {
            for (voice, step) in [(Voice::One, Step::G), (Voice::Two, Step::C)] {
                let id = s
                    .insert_in_voice(
                        p,
                        StaffId::Upper,
                        b,
                        voice,
                        0,
                        Event {
                            pitches: Pitches::single(Pitch::new(step, 4)),
                            duration: Duration::Whole,
                            notation: Notation::default(),
                        },
                    )
                    .unwrap();
                if voice == Voice::Two {
                    ids.push(id);
                }
            }
        }
        s.set_divisi_span(
            p,
            DivisiPoint::new(1, 0),
            Some(DivisiPoint::new(2, 0)),
            true,
        )
        .unwrap();
        let c = Connection {
            start: ids[0],
            end: ids[2],
            kind,
        };
        s.add_connection(c).unwrap();
        let l = LayoutEngine::default().layout_view(&s, 1600., ViewMode::Continuous);
        let end = l.event_measure(ids[2]).unwrap();
        let head = end.content.events.iter().find(|e| e.id == ids[2]).unwrap();
        let end_x = end.origin.x + head.center.x;
        let segments: Vec<_> = if kind == ConnectionKind::Slur {
            l.curves
                .iter()
                .filter(|x| x.connection == c)
                .map(|x| x.to.x)
                .collect()
        } else {
            l.hairpins
                .iter()
                .filter(|x| x.connection == c)
                .map(|x| x.upper.to.x)
                .collect()
        };
        assert_eq!(
            segments.len(),
            3,
            "every staff run should retain its continuation for {kind:?}"
        );
        assert!(
            (segments.last().unwrap() - end_x).abs() < 16.,
            "connection does not reach its final note"
        );
    }
}

#[test]
fn menuetto_bar_fifteen_slur_stays_below_unison_text() {
    let native: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../examples/menuetto-y-trio-cuerdas.partitura"
    ))
    .unwrap();
    let score: Score = serde_json::from_value(native["score"].clone()).unwrap();
    for mode in [ViewMode::Page, ViewMode::Continuous, ViewMode::SingleSystem] {
        let layout = LayoutEngine::default().layout_view(&score, 1400., mode);
        let curve = layout
            .curves
            .iter()
            .find(|c| {
                c.connection.kind == score_core::ConnectionKind::Slur
                    && c.connection.start.measure == 14
            })
            .unwrap();
        let bar = layout.event_measure(curve.connection.start).unwrap();
        let unison = bar
            .content
            .techniques
            .iter()
            .find(|t| t.text == "unis.")
            .unwrap();
        let (top, _) = expression::notation_bounds(&bar.content, &Metrics::bravura());
        assert!((curve.from.y - bar.origin.y - (top - 8.)).abs() < 0.01);
        assert!((curve.from.y - curve.to.y).abs() < 0.01);
        assert_text_clears_curve(bar, unison, curve);
    }
}

#[test]
fn midbar_unison_keeps_text_outside_the_original_slur_lane() {
    use score_core::{
        Connection, ConnectionKind, DivisiPoint, Event, Notation, Pitch, Pitches, Step,
    };
    let mut s = Score::default();
    let piano = s.parts()[0].id();
    let part = s.add_part(Instrument::Violin).unwrap();
    s.remove_part(piano).unwrap();
    s.set_time_signature_at(
        0,
        score_core::TimeSignature {
            numerator: 3,
            denominator: 4,
        },
    )
    .unwrap();
    let mut ids = Vec::new();
    for (n, (step, octave)) in [(Step::E, 5), (Step::C, 5), (Step::A, 4)]
        .into_iter()
        .enumerate()
    {
        ids.push(
            s.insert_in(
                part,
                0,
                n,
                Event {
                    pitches: Pitches::single(Pitch::new(step, octave)),
                    duration: Duration::Quarter,
                    notation: Notation::default(),
                },
            )
            .unwrap(),
        );
    }
    s.insert_in_voice(
        part,
        StaffId::Upper,
        0,
        Voice::Two,
        0,
        Event {
            pitches: Pitches::single(Pitch::new(Step::G, 4)),
            duration: Duration::Half,
            notation: Notation::default(),
        },
    )
    .unwrap();
    s.set_divisi_span(
        part,
        DivisiPoint::new(0, 0),
        Some(DivisiPoint::new(0, 96)),
        true,
    )
    .unwrap();
    s.add_connection(Connection {
        start: ids[0],
        end: ids[2],
        kind: ConnectionKind::Slur,
    })
    .unwrap();
    let layout = LayoutEngine::default().layout_view(&s, 1400., ViewMode::Continuous);
    let curve = &layout.curves[0];
    let bar = layout.event_measure(ids[0]).unwrap();
    let (top, _) = expression::notation_bounds(&bar.content, &Metrics::bravura());
    assert!((curve.from.y - bar.origin.y - (top - 8.)).abs() < 0.01);
    assert_eq!(curve.from.y, curve.to.y);
    assert_eq!(curve.bend, -14.);
    for label in &bar.content.techniques {
        assert_text_clears_curve(bar, label, curve);
    }
}

#[test]
fn original_slur_envelope_excludes_text_and_dynamics_in_both_voices() {
    use score_core::{
        Connection, ConnectionKind, Dynamic, Event, Notation, Pitch, Pitches, PlayingTechnique,
        Step,
    };
    for voice in Voice::ALL {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        let part = score.add_part(Instrument::Violin).unwrap();
        score.remove_part(piano).unwrap();
        let mut ids = Vec::new();
        for (index, (step, octave)) in [(Step::C, 6), (Step::C, 4), (Step::E, 4), (Step::G, 4)]
            .into_iter()
            .enumerate()
        {
            ids.push(
                score
                    .insert_in_voice(
                        part,
                        StaffId::Upper,
                        0,
                        voice,
                        index,
                        Event {
                            pitches: Pitches::single(Pitch::new(step, octave)),
                            duration: Duration::Quarter,
                            notation: Notation::default(),
                        },
                    )
                    .unwrap(),
            );
        }
        let connection = Connection {
            start: ids[1],
            end: ids[3],
            kind: ConnectionKind::Slur,
        };
        score.add_connection(connection).unwrap();
        let mut engine = LayoutEngine::default();
        score
            .set_technique(ids[0], Some(PlayingTechnique::Pizzicato))
            .unwrap();
        score.set_dynamic(ids[2], Some(Dynamic::Fff)).unwrap();
        for mode in [ViewMode::Page, ViewMode::Continuous, ViewMode::SingleSystem] {
            let layout = engine.layout_view(&score, 1400., mode);
            let curve = layout
                .curves
                .iter()
                .find(|c| c.connection == connection)
                .unwrap();
            let bar = layout.event_measure(ids[1]).unwrap();
            let (top, bottom) = expression::notation_bounds(&bar.content, &Metrics::bravura());
            let expected = if voice == Voice::Two {
                bottom + 8.
            } else {
                top - 8.
            };
            assert!((curve.from.y - bar.origin.y - expected).abs() < 0.01);
            assert_eq!(curve.from.y, curve.to.y);
            assert_eq!(curve.bend.abs(), 14.);
        }
    }
}

#[test]
fn menuetto_slurs_keep_the_original_even_shallow_arches() {
    let native: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../examples/menuetto-y-trio-cuerdas.partitura"
    ))
    .unwrap();
    let score: Score = serde_json::from_value(native["score"].clone()).unwrap();
    for mode in [ViewMode::Page, ViewMode::Continuous, ViewMode::SingleSystem] {
        let layout = LayoutEngine::default().layout_view(&score, 1400., mode);
        for curve in layout
            .curves
            .iter()
            .filter(|c| c.connection.kind == score_core::ConnectionKind::Slur)
        {
            assert!(
                (curve.from.y - curve.to.y).abs() < 0.01,
                "uneven slur: {curve:?}"
            );
            assert!(curve.bend.abs() <= 14., "excessive arch: {curve:?}");
        }
    }
}

#[test]
fn indication_lanes_keep_margins_and_reflow_when_a_slur_is_toggled() {
    use score_core::{
        Articulation, Connection, ConnectionKind, Division, Event, MarkRef, Notation, Pitch,
        Pitches, PlayingTechnique, Step,
    };
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    let part = score.add_part(Instrument::Violin).unwrap();
    score.remove_part(piano).unwrap();
    let a = score
        .insert_in(
            part,
            0,
            0,
            Event {
                pitches: Pitches::single(Pitch::new(Step::C, 5)),
                duration: Duration::Quarter,
                notation: Notation {
                    technique: Some(PlayingTechnique::Arco),
                    division: Some(Division::Divisi),
                    articulation: Some(Articulation::Accent),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    let b = score
        .insert_in(
            part,
            0,
            1,
            Event {
                pitches: Pitches::single(Pitch::new(Step::E, 5)),
                duration: Duration::Quarter,
                notation: Notation::default(),
            },
        )
        .unwrap();
    let connection = Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Slur,
    };
    let mut engine = LayoutEngine::default();
    for mode in [ViewMode::Page, ViewMode::Continuous, ViewMode::SingleSystem] {
        let plain = engine.layout_view(&score, 1400., mode);
        let before = plain
            .event_measure(a)
            .unwrap()
            .content
            .techniques
            .iter()
            .find(|t| t.text == "arco")
            .unwrap()
            .position
            .y;
        score.add_connection(connection).unwrap();
        let layout = engine.layout_view(&score, 1400., mode);
        assert_eq!(
            engine.recomputed_measures, 0,
            "connections must not invalidate cached note engraving"
        );
        let bar = layout.event_measure(a).unwrap();
        let technique = bar
            .content
            .techniques
            .iter()
            .find(|t| t.text == "arco")
            .unwrap();
        let division = bar
            .content
            .techniques
            .iter()
            .find(|t| t.text == "div.")
            .unwrap();
        let curve = layout
            .curves
            .iter()
            .find(|c| c.connection == connection)
            .unwrap();
        assert!(technique.position.y <= before + 0.01);
        assert_text_clears_curve(bar, technique, curve);
        assert!(division.position.y + 4. + 6. <= technique.position.y - 16. + 0.01);
        for (label, mark) in [
            (technique, MarkRef::Technique(a)),
            (division, MarkRef::Division(a)),
        ] {
            let hitbox = bar.content.marks.iter().find(|m| m.mark == mark).unwrap();
            assert!((hitbox.center.y - label.position.y + 5.).abs() < 0.01);
            assert_eq!(
                layout.hit(Point {
                    x: bar.origin.x + hitbox.center.x,
                    y: bar.origin.y + hitbox.center.y
                }),
                Some(Hit::Mark(mark))
            );
        }
        score.remove_connection(connection);
        let restored = engine.layout_view(&score, 1400., mode);
        let after = restored
            .event_measure(a)
            .unwrap()
            .content
            .techniques
            .iter()
            .find(|t| t.text == "arco")
            .unwrap()
            .position
            .y;
        assert!((before - after).abs() < 0.01);
    }
}

#[test]
fn triplet_numbers_stay_inside_the_slur_and_technique_lanes() {
    use score_core::{
        Connection, ConnectionKind, Event, Notation, Pitch, Pitches, PlayingTechnique, Step,
    };
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    let part = score.add_part(Instrument::Violin).unwrap();
    score.remove_part(piano).unwrap();
    let mut ids = Vec::new();
    for (i, step) in [Step::C, Step::D, Step::E].into_iter().enumerate() {
        ids.push(
            score
                .insert_in(
                    part,
                    0,
                    i,
                    Event {
                        pitches: Pitches::single(Pitch::new(step, 5)),
                        duration: Duration::Eighth.with_triplet(true),
                        notation: Notation::default(),
                    },
                )
                .unwrap(),
        );
    }
    score
        .add_connection(Connection {
            start: ids[0],
            end: ids[2],
            kind: ConnectionKind::Slur,
        })
        .unwrap();
    let mut engine = LayoutEngine::default();
    let plain = engine.layout_view(&score, 1400., ViewMode::Continuous);
    let initial = plain.event_measure(ids[0]).unwrap().content.tuplets[0].top;
    score
        .set_technique(ids[0], Some(PlayingTechnique::Pizzicato))
        .unwrap();
    let layout = engine.layout_view(&score, 1400., ViewMode::Continuous);
    let bar = layout.event_measure(ids[0]).unwrap();
    assert_eq!(
        bar.content.tuplets[0].top, initial,
        "text must not push the tuplet upward"
    );
    let curve = &layout.curves[0];
    let label = &bar.content.techniques[0];
    assert_text_clears_curve(bar, label, curve);
    assert!(curve.from.y <= bar.origin.y + initial - 8.);
}

// Check clearance at the text's actual horizontal footprint, rather than at
// the apex of a slur elsewhere in the bar.
fn assert_text_clears_curve(bar: &PlacedMeasure, label: &TechniqueLabel, curve: &Curve) {
    let left = (bar.origin.x + label.position.x).max(curve.from.x);
    let right =
        (bar.origin.x + label.position.x + label.text.chars().count() as f32 * 7.).min(curve.to.x);
    if left > right {
        return;
    }
    for i in 0..=16 {
        let x = left + (right - left) * i as f32 / 16.;
        let t = (x - curve.from.x) / (curve.to.x - curve.from.x);
        let u = 1. - t;
        let y = u * u * u * curve.from.y
            + 3. * u * u * t * (curve.from.y + curve.bend)
            + 3. * u * t * t * (curve.to.y + curve.bend)
            + t * t * t * curve.to.y;
        assert!(bar.origin.y + label.position.y + 4. + 6. <= y + 0.01);
    }
}
