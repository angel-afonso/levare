//! Research for plan 025: can the canvas show an extracted part while edits
//! still address the full score through the same `ElementId`s?
use super::*;
use score_core::{
    Connection, ConnectionKind, DivisiPoint, Event, Notation, Pitch, Pitches, Step, Voice,
};
use std::collections::HashSet;
use std::time::Instant;

fn note(step: Step, duration: Duration) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 4)),
        duration,
        notation: Notation::default(),
    }
}

/// Violin (with divisi), Bb clarinet (transposing) and piano (grand staff),
/// carrying a tie, a crescendo and a comment per part.
fn trio() -> Score {
    let mut score = Score::new(
        "Trío".into(),
        &[
            Instrument::Violin,
            Instrument::ClarinetBb,
            Instrument::Piano,
        ],
        4,
    )
    .unwrap();
    let ids: Vec<_> = score.parts().iter().map(|p| p.id()).collect();
    let (violin, clarinet, piano) = (ids[0], ids[1], ids[2]);
    // Violin: tied C-C, then a second voice with a divisi span.
    let c1 = score
        .insert_in(violin, 0, 0, note(Step::C, Duration::Quarter))
        .unwrap();
    let c2 = score
        .insert_in(violin, 0, 1, note(Step::C, Duration::Quarter))
        .unwrap();
    score
        .add_connection(Connection {
            start: c1,
            end: c2,
            kind: ConnectionKind::Tie,
        })
        .unwrap();
    score
        .insert_on(violin, StaffId::Upper, 1, 0, note(Step::G, Duration::Whole))
        .unwrap();
    score
        .insert_in_voice(
            violin,
            StaffId::Upper,
            1,
            Voice::Two,
            0,
            note(Step::C, Duration::Whole),
        )
        .unwrap();
    score
        .set_divisi_span(
            violin,
            DivisiPoint::new(1, 0),
            Some(DivisiPoint::new(2, 0)),
            true,
        )
        .unwrap();
    score.save_comment(None, "Arco", &[c1, c2]).unwrap();
    // Clarinet: crescendo over two notes.
    let a = score
        .insert_in(clarinet, 0, 0, note(Step::E, Duration::Half))
        .unwrap();
    let b = score
        .insert_in(clarinet, 0, 1, note(Step::G, Duration::Half))
        .unwrap();
    score
        .add_connection(Connection {
            start: a,
            end: b,
            kind: ConnectionKind::Crescendo,
        })
        .unwrap();
    score.save_comment(None, "Más aire", &[a]).unwrap();
    // Piano: both staves.
    let u = score
        .insert_on(piano, StaffId::Upper, 0, 0, note(Step::D, Duration::Whole))
        .unwrap();
    score
        .insert_on(piano, StaffId::Lower, 0, 0, note(Step::D, Duration::Whole))
        .unwrap();
    score.save_comment(None, "Pedal", &[u]).unwrap();
    score
}

/// Every `ElementId` the layout exposes: events, note-owned strokes and
/// glyphs, technique labels, curves and hairpin endpoints.
fn exposed_ids(layout: &Layout) -> HashSet<ElementId> {
    let mut ids = HashSet::new();
    for placed in &layout.measures {
        let c = &placed.content;
        ids.extend(c.events.iter().map(|e| e.id));
        ids.extend(c.lines.iter().filter_map(|l| l.element));
        ids.extend(c.glyphs.iter().filter_map(|g| g.element));
        ids.extend(c.techniques.iter().map(|t| t.element));
    }
    for curve in &layout.curves {
        ids.extend([curve.connection.start, curve.connection.end]);
    }
    for hairpin in &layout.hairpins {
        ids.extend([hairpin.connection.start, hairpin.connection.end]);
    }
    ids
}

#[test]
fn part_view_ids_exist_in_the_full_score_and_point_to_the_same_event() {
    let score = trio();
    for part in score.parts().iter().map(|p| p.id()).collect::<Vec<_>>() {
        let extracted = score.extract_part(part).unwrap();
        for mode in [ViewMode::Page, ViewMode::Continuous] {
            let layout = LayoutEngine::default().layout_part(&extracted, 1000., mode);
            let ids = exposed_ids(&layout);
            assert!(!ids.is_empty(), "the part view shows music");
            for id in &ids {
                assert_eq!(id.part, part, "a part view only exposes its own part");
                let original = score.event(*id).expect("id exists in the full score");
                assert_eq!(Some(original), extracted.event(*id));
            }
            // Nothing is lost: every sounding event of the part is exposed.
            let shown: HashSet<_> = layout
                .measures
                .iter()
                .flat_map(|m| m.content.events.iter().map(|e| e.id))
                .collect();
            let expected: HashSet<_> = layout_ids_of(&score, part, mode);
            assert_eq!(shown, expected);
        }
    }
}

/// Ids the full-score layout places for one part.
fn layout_ids_of(score: &Score, part: PartId, mode: ViewMode) -> HashSet<ElementId> {
    LayoutEngine::default()
        .layout_view(score, 1000., mode)
        .measures
        .iter()
        .filter(|m| m.part == part)
        .flat_map(|m| m.content.events.iter().map(|e| e.id))
        .collect()
}

#[test]
fn part_view_keeps_connections_and_comment_anchors_resolvable_in_the_full_score() {
    let score = trio();
    for part in score.parts().iter().map(|p| p.id()).collect::<Vec<_>>() {
        let extracted = score.extract_part(part).unwrap();
        for c in extracted.connections() {
            assert!(score.connections().contains(c));
            assert!(score.event(c.start).is_some() && score.event(c.end).is_some());
        }
        assert!(!extracted.comments().is_empty());
        for comment in extracted.comments() {
            assert_eq!(comment.part, part);
            for anchor in comment.anchors.iter() {
                assert!(score.event(*anchor).is_some());
            }
        }
    }
    // The violin tie and the clarinet crescendo appear as drawn strokes.
    let violin = score.parts()[0].id();
    let layout = LayoutEngine::default().layout_part(
        &score.extract_part(violin).unwrap(),
        1000.,
        ViewMode::Page,
    );
    assert!(!layout.curves.is_empty());
    let clarinet = score.parts()[1].id();
    let layout = LayoutEngine::default().layout_part(
        &score.extract_part(clarinet).unwrap(),
        1000.,
        ViewMode::Page,
    );
    assert!(!layout.hairpins.is_empty());
}

#[test]
fn part_view_hit_test_returns_ids_that_the_full_score_editor_accepts() {
    let score = trio();
    let part = score.parts()[1].id();
    let extracted = score.extract_part(part).unwrap();
    let layout = LayoutEngine::default().layout_part(&extracted, 1000., ViewMode::Page);
    let placed = layout
        .measures
        .iter()
        .find(|m| !m.content.events.is_empty())
        .unwrap();
    let event = &placed.content.events[0];
    let point = Point {
        x: (placed.origin.x + event.center.x) * layout.scale,
        y: (placed.origin.y + event.center.y) * layout.scale,
    };
    let (id, _) = layout.hit_pitch(point).unwrap();
    assert_eq!(id, event.id);
    assert!(score.event(id).is_some());
    assert_eq!(score.event(id), extracted.event(id));
}

fn orchestra() -> Score {
    let mut score = Score::new("Orquesta".into(), &Instrument::ORCHESTRA, 32).unwrap();
    let parts: Vec<_> = score.parts().iter().map(|p| p.id()).collect();
    for part in parts {
        for bar in 0..32 {
            for index in 0..4 {
                let step = [Step::C, Step::E, Step::G, Step::E][index];
                score
                    .insert_in(part, bar, index, note(step, Duration::Quarter))
                    .unwrap();
            }
        }
    }
    score
}

fn median_ms(mut runs: Vec<f64>) -> f64 {
    runs.sort_by(f64::total_cmp);
    runs[runs.len() / 2]
}

/// Run with `cargo test -p score-layout --release part_view_cost -- --ignored --nocapture`.
#[test]
#[ignore = "measurement, not a regression test"]
fn part_view_cost() {
    let score = orchestra();
    let parts: Vec<_> = score.parts().iter().map(|p| p.id()).collect();
    let staves = |p: PartId| score.part(p).unwrap().staves().len();
    let widest = parts.iter().copied().max_by_key(|&p| staves(p)).unwrap();
    let time = |f: &mut dyn FnMut()| {
        median_ms(
            (0..20)
                .map(|_| {
                    let start = Instant::now();
                    f();
                    start.elapsed().as_secs_f64() * 1000.
                })
                .collect(),
        )
    };
    for (label, part) in [("primera parte", parts[0]), ("más pentagramas", widest)] {
        let extract = time(&mut || {
            std::hint::black_box(score.extract_part(part).unwrap());
        });
        let extracted = score.extract_part(part).unwrap();
        let layout = time(&mut || {
            std::hint::black_box(LayoutEngine::default().layout_part(
                &extracted,
                1000.,
                ViewMode::Page,
            ));
        });
        let both = time(&mut || {
            let e = score.extract_part(part).unwrap();
            std::hint::black_box(LayoutEngine::default().layout_part(&e, 1000., ViewMode::Page));
        });
        println!(
            "{label}: extract_part {extract:.3} ms, layout_part {layout:.3} ms, juntos {both:.3} ms (mediana de 20)"
        );
    }
    let full = time(&mut || {
        std::hint::black_box(LayoutEngine::default().layout_view(&score, 1000., ViewMode::Page));
    });
    println!(
        "partitura completa ({} partes): layout_view en frío {full:.3} ms",
        parts.len()
    );
    // Warm cache: the realistic cost of redrawing after one edit.
    let mut engine = LayoutEngine::default();
    engine.layout_view(&score, 1000., ViewMode::Page);
    let warm = time(&mut || {
        std::hint::black_box(engine.layout_view(&score, 1000., ViewMode::Page));
    });
    println!("partitura completa: layout_view con caché {warm:.3} ms");
}
