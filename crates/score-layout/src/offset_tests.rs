use super::*;
use score_core::{
    BarForm, BarMark, BarMarkRef, Connection, ConnectionKind, Dynamic, Ending, Event, Jump,
    MarkRef, Notation, Offset, Pitch, Pitches, PlayingTechnique, Step,
};

fn note() -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(Step::C, 4)),
        duration: Duration::Whole,
        notation: Notation::default(),
    }
}

fn point_mark(layout: &Layout, mark: MarkRef) -> (&PlacedMeasure, &MarkBox) {
    layout
        .measures
        .iter()
        .find_map(|placed| {
            placed
                .content
                .marks
                .iter()
                .find(|box_| box_.mark == mark)
                .map(|box_| (placed, box_))
        })
        .expect("written mark in layout")
}

fn assert_mark_shift(before: &Layout, after: &Layout, mark: MarkRef, dx: f32, dy: f32) {
    let (_, old) = point_mark(before, mark);
    let (_, new) = point_mark(after, mark);
    assert_eq!(new.center.x - old.center.x, dx);
    assert_eq!(new.center.y - old.center.y, dy);
}

#[test]
fn dynamic_offset_moves_glyph_box_and_hit_target() {
    let mut score = Score::default();
    let id = score.insert(0, 0, note()).unwrap();
    score.set_dynamic(id, Some(Dynamic::F)).unwrap();
    let mark = MarkRef::Dynamic(id);
    let mut engine = LayoutEngine::default();
    let before = engine.layout_view(&score, 1000., ViewMode::Page);
    score
        .set_mark_offset(mark, Offset { dx: 20, dy: -15 })
        .unwrap();
    let after = engine.layout_view(&score, 1000., ViewMode::Page);
    assert_mark_shift(&before, &after, mark, 20., -15.);
    let (old_placed, old) = point_mark(&before, mark);
    let (new_placed, new) = point_mark(&after, mark);
    let old_glyph = &old_placed.content.glyphs[old.glyph.unwrap()];
    let new_glyph = &new_placed.content.glyphs[new.glyph.unwrap()];
    assert_eq!(new_glyph.position.x - old_glyph.position.x, 20.);
    assert_eq!(new_glyph.position.y - old_glyph.position.y, -15.);
    assert_eq!(
        after.hit(Point {
            x: (new_placed.origin.x + new.center.x) * after.scale,
            y: (new_placed.origin.y + new.center.y) * after.scale,
        }),
        Some(Hit::Mark(mark))
    );
}

#[test]
fn technique_offset_moves_its_label_and_box() {
    let mut score = Score::new("".into(), &[Instrument::Violin], 4).unwrap();
    let id = score.insert(0, 0, note()).unwrap();
    score
        .set_technique(id, Some(PlayingTechnique::Pizzicato))
        .unwrap();
    let mark = MarkRef::Technique(id);
    let mut engine = LayoutEngine::default();
    let before = engine.layout_view(&score, 1000., ViewMode::Page);
    score
        .set_mark_offset(mark, Offset { dx: -12, dy: 18 })
        .unwrap();
    let after = engine.layout_view(&score, 1000., ViewMode::Page);
    assert_mark_shift(&before, &after, mark, -12., 18.);
    let (old_placed, _) = point_mark(&before, mark);
    let (new_placed, _) = point_mark(&after, mark);
    let old = old_placed
        .content
        .techniques
        .iter()
        .find(|label| label.element == id && label.text == "pizz.")
        .unwrap();
    let new = new_placed
        .content
        .techniques
        .iter()
        .find(|label| label.element == id && label.text == "pizz.")
        .unwrap();
    assert_eq!(new.position.x - old.position.x, -12.);
    assert_eq!(new.position.y - old.position.y, 18.);
}

#[test]
fn every_segment_of_a_split_slur_moves() {
    let mut score = Score::default();
    for _ in 0..2 {
        score.append_measure().unwrap();
    }
    let ids: Vec<_> = (0..6)
        .map(|measure| score.insert(measure, 0, note()).unwrap())
        .collect();
    let slur = Connection {
        start: ids[0],
        end: ids[5],
        kind: ConnectionKind::Slur,
    };
    score.add_connection(slur).unwrap();
    let mut engine = LayoutEngine::default();
    let before = engine.layout_compact(&score, 680.);
    score
        .set_mark_offset(MarkRef::Connection(slur), Offset { dx: 11, dy: -7 })
        .unwrap();
    let after = engine.layout_compact(&score, 680.);
    let old: Vec<_> = before
        .curves
        .iter()
        .filter(|curve| curve.connection == slur)
        .collect();
    let new: Vec<_> = after
        .curves
        .iter()
        .filter(|curve| curve.connection == slur)
        .collect();
    assert!(old.len() > 1);
    assert_eq!(old.len(), new.len());
    for (old, new) in old.into_iter().zip(new) {
        for (a, b) in [(old.from, new.from), (old.to, new.to)] {
            assert_eq!((b.x - a.x, b.y - a.y), (11., -7.));
        }
    }
}

#[test]
fn crescendo_offset_moves_both_edges() {
    let mut score = Score::default();
    let a = score.insert(0, 0, note()).unwrap();
    let b = score.insert(1, 0, note()).unwrap();
    let crescendo = Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Crescendo,
    };
    score.add_connection(crescendo).unwrap();
    let mut engine = LayoutEngine::default();
    let before = engine.layout_compact(&score, 960.);
    score
        .set_mark_offset(MarkRef::Connection(crescendo), Offset { dx: 17, dy: 9 })
        .unwrap();
    let after = engine.layout_compact(&score, 960.);
    assert_eq!(before.hairpins.len(), after.hairpins.len());
    assert!(!before.hairpins.is_empty());
    for (old, new) in before.hairpins.iter().zip(&after.hairpins) {
        for (a, b) in [
            (old.upper.from, new.upper.from),
            (old.upper.to, new.upper.to),
            (old.lower.from, new.lower.from),
            (old.lower.to, new.lower.to),
        ] {
            assert_eq!((b.x - a.x, b.y - a.y), (17., 9.));
        }
    }
}

#[test]
fn all_layout_entry_points_apply_point_offsets_once() {
    let mut plain = Score::default();
    let id = plain.insert(0, 0, note()).unwrap();
    plain.set_dynamic(id, Some(Dynamic::P)).unwrap();
    let mut moved = plain.clone();
    let mark = MarkRef::Dynamic(id);
    moved
        .set_mark_offset(mark, Offset { dx: 14, dy: -9 })
        .unwrap();
    let mut engine = LayoutEngine::default();
    for (before, after) in [
        (engine.layout(&plain, 960.), engine.layout(&moved, 960.)),
        (
            engine.layout_compact(&plain, 960.),
            engine.layout_compact(&moved, 960.),
        ),
        (
            engine.layout_part(&plain, 960., ViewMode::Page),
            engine.layout_part(&moved, 960., ViewMode::Page),
        ),
        (
            engine.layout_view(&plain, 960., ViewMode::Continuous),
            engine.layout_view(&moved, 960., ViewMode::Continuous),
        ),
    ] {
        assert_mark_shift(&before, &after, mark, 14., -9.);
    }
    let base = engine.layout_view(&plain, 960., ViewMode::Page);
    let before = engine.layout_pitch_preview(&plain, &base);
    let after = engine.layout_pitch_preview(&moved, &base);
    assert_mark_shift(&before, &after, mark, 14., -9.);
}

#[test]
fn hot_layout_cache_does_not_accumulate_offsets() {
    let mut score = Score::default();
    let id = score.insert(0, 0, note()).unwrap();
    score.set_dynamic(id, Some(Dynamic::P)).unwrap();
    let mark = MarkRef::Dynamic(id);
    score
        .set_mark_offset(mark, Offset { dx: 21, dy: -13 })
        .unwrap();
    let mut engine = LayoutEngine::default();
    let first = engine.layout_view(&score, 960., ViewMode::Page);
    let second = engine.layout_view(&score, 960., ViewMode::Page);
    assert_eq!(engine.recomputed_measures, 0);
    assert_mark_shift(&first, &second, mark, 0., 0.);
}

#[test]
fn a_projected_mark_moves_once_on_each_display_staff() {
    let mut score = Score::new("".into(), &[Instrument::Violin], 4).unwrap();
    let id = score.insert(0, 0, note()).unwrap();
    score.set_dynamic(id, Some(Dynamic::P)).unwrap();
    let mark = MarkRef::Dynamic(id);
    score
        .set_mark_offset(mark, Offset { dx: 18, dy: -12 })
        .unwrap();
    let mut layout = LayoutEngine::default().layout_compact(&score, 960.);
    let original = point_mark(&layout, mark).0.clone();
    let mut projection = original.clone();
    projection.staff = StaffId::Lower;
    projection.origin.y += 200.;
    layout.measures = vec![original, projection];
    let before = layout.clone();
    offsets::apply(&score, &mut layout);
    for (old, new) in before.measures.iter().zip(&layout.measures) {
        let old_box = old
            .content
            .marks
            .iter()
            .find(|box_| box_.mark == mark)
            .unwrap();
        let new_box = new
            .content
            .marks
            .iter()
            .find(|box_| box_.mark == mark)
            .unwrap();
        assert!((new_box.center.x - old_box.center.x - 18.).abs() < 0.001);
        assert!((new_box.center.y - old_box.center.y + 12.).abs() < 0.001);
    }
}

fn bar_mark(measure: usize, mark: BarMark) -> BarMarkRef {
    BarMarkRef { measure, mark }
}

#[test]
fn tempo_bar_offset_moves_label_and_hit_at_every_zoom() {
    let mut score = Score::default();
    let mark = bar_mark(0, BarMark::Tempo);
    let plain = score.clone();
    score
        .set_bar_mark_offset(mark, Offset { dx: 30, dy: 20 })
        .unwrap();
    let mut engine = LayoutEngine::default();
    for (before, mut after) in [
        (engine.layout(&plain, 960.), engine.layout(&score, 960.)),
        (
            engine.layout_compact(&plain, 960.),
            engine.layout_compact(&score, 960.),
        ),
        (
            engine.layout_view(&plain, 960., ViewMode::Page),
            engine.layout_view(&score, 960., ViewMode::Page),
        ),
        (
            engine.layout_view(&plain, 960., ViewMode::Continuous),
            engine.layout_view(&score, 960., ViewMode::Continuous),
        ),
        (
            engine.layout_view(&plain, 960., ViewMode::SingleSystem),
            engine.layout_view(&score, 960., ViewMode::SingleSystem),
        ),
    ] {
        assert_eq!(
            after.tempos[0].position.x - before.tempos[0].position.x,
            30.
        );
        assert_eq!(
            after.tempos[0].position.y - before.tempos[0].position.y,
            20.
        );
        for scale in [0.5, 1., 2.] {
            after.scale = scale;
            assert_eq!(
                after.hit_bar_mark(Point {
                    x: (after.tempos[0].position.x + 3.) * scale,
                    y: (after.tempos[0].position.y - 10.) * scale,
                }),
                Some(mark)
            );
        }
    }
}

#[test]
fn rehearsal_bar_offset_moves_frame_and_hit() {
    let mut score = Score::default();
    score.toggle_rehearsal_mark(1).unwrap();
    let mark = bar_mark(1, BarMark::Rehearsal);
    let before = LayoutEngine::default().layout(&score, 960.);
    score
        .set_bar_mark_offset(mark, Offset { dx: -12, dy: 31 })
        .unwrap();
    let after = LayoutEngine::default().layout(&score, 960.);
    let old = &before.rehearsals[0];
    let moved = &after.rehearsals[0];
    assert_eq!(
        (
            moved.position.x - old.position.x,
            moved.position.y - old.position.y
        ),
        (-12., 31.)
    );
    for (a, b) in old.frame.iter().zip(&moved.frame) {
        assert_eq!((b.from.x - a.from.x, b.from.y - a.from.y), (-12., 31.));
        assert_eq!((b.to.x - a.to.x, b.to.y - a.to.y), (-12., 31.));
    }
    assert_eq!(
        after.hit_bar_mark(Point {
            x: moved.position.x,
            y: moved.position.y - 5.
        }),
        Some(mark)
    );
}

#[test]
fn segno_and_coda_in_one_bar_keep_distinct_ids_and_hits() {
    let mut score = Score::default();
    score
        .set_bar_form(
            1,
            BarForm {
                segno: true,
                coda: true,
                ..Default::default()
            },
        )
        .unwrap();
    let segno = bar_mark(1, BarMark::Segno);
    let coda = bar_mark(1, BarMark::Coda);
    let before = LayoutEngine::default().layout(&score, 960.);
    score
        .set_bar_mark_offset(segno, Offset { dx: -15, dy: -25 })
        .unwrap();
    score
        .set_bar_mark_offset(coda, Offset { dx: 20, dy: 35 })
        .unwrap();
    let after = LayoutEngine::default().layout(&score, 960.);
    for (mark, dx, dy) in [(segno, -15., -25.), (coda, 20., 35.)] {
        let old = before
            .navigation
            .iter()
            .find(|label| label.mark == mark.mark)
            .unwrap();
        let moved = after
            .navigation
            .iter()
            .find(|label| label.mark == mark.mark)
            .unwrap();
        assert_eq!(
            (
                moved.position.x - old.position.x,
                moved.position.y - old.position.y
            ),
            (dx, dy)
        );
        assert_eq!(
            after.hit_bar_mark(Point {
                x: moved.position.x + 3.,
                y: moved.position.y - 10.
            }),
            Some(mark)
        );
    }
}

#[test]
fn fine_and_jump_right_aligned_labels_have_separate_hits() {
    let mut score = Score::default();
    score
        .set_bar_form(
            2,
            BarForm {
                fine: true,
                jump: Some(Jump::DaCapo),
                ..Default::default()
            },
        )
        .unwrap();
    let fine = bar_mark(2, BarMark::Fine);
    let jump = bar_mark(2, BarMark::Jump);
    let before = LayoutEngine::default().layout(&score, 960.);
    score
        .set_bar_mark_offset(fine, Offset { dx: -25, dy: -12 })
        .unwrap();
    score
        .set_bar_mark_offset(jump, Offset { dx: 14, dy: 25 })
        .unwrap();
    let after = LayoutEngine::default().layout(&score, 960.);
    for (mark, dx, dy) in [(fine, -25., -12.), (jump, 14., 25.)] {
        let old = before
            .navigation
            .iter()
            .find(|label| label.mark == mark.mark)
            .unwrap();
        let moved = after
            .navigation
            .iter()
            .find(|label| label.mark == mark.mark)
            .unwrap();
        assert!(moved.right_aligned);
        assert_eq!(
            (
                moved.position.x - old.position.x,
                moved.position.y - old.position.y
            ),
            (dx, dy)
        );
        assert_eq!(
            after.hit_bar_mark(Point {
                x: moved.position.x - 5.,
                y: moved.position.y - 5.
            }),
            Some(mark)
        );
    }
}

#[test]
fn split_ending_moves_every_segment_and_both_hit_areas() {
    let mut score = Score::new("".into(), &[Instrument::Violin], 8).unwrap();
    score
        .set_ending(Ending {
            start: 1,
            end: 6,
            numbers: vec![1],
        })
        .unwrap();
    let mark = bar_mark(1, BarMark::Ending);
    let before = LayoutEngine::default().layout(&score, 2000.);
    assert!(before.endings.len() > 1);
    score
        .set_bar_mark_offset(mark, Offset { dx: 18, dy: 25 })
        .unwrap();
    let after = LayoutEngine::default().layout(&score, 2000.);
    for (old, moved) in before.endings.iter().zip(&after.endings) {
        assert_eq!(
            (
                moved.label_position.x - old.label_position.x,
                moved.label_position.y - old.label_position.y
            ),
            (18., 25.)
        );
        for (a, b) in old.lines.iter().zip(&moved.lines) {
            assert_eq!((b.from.x - a.from.x, b.from.y - a.from.y), (18., 25.));
            assert_eq!((b.to.x - a.to.x, b.to.y - a.to.y), (18., 25.));
        }
    }
    let first = &after.endings[0];
    assert_eq!(
        after.hit_bar_mark(Point {
            x: first.label_position.x + 2.,
            y: first.label_position.y - 5.
        }),
        Some(mark)
    );
    let continuation = &after.endings[1].lines[0];
    assert_eq!(
        after.hit_bar_mark(Point {
            x: (continuation.from.x + continuation.to.x) / 2.,
            y: continuation.from.y
        }),
        Some(mark)
    );
}

#[test]
fn pitch_preview_keeps_pretranslated_bar_labels_without_double_shift() {
    let mut score = Score::default();
    score.toggle_rehearsal_mark(1).unwrap();
    score
        .set_bar_form(
            1,
            BarForm {
                segno: true,
                ..Default::default()
            },
        )
        .unwrap();
    score
        .set_ending(Ending {
            start: 1,
            end: 2,
            numbers: vec![1],
        })
        .unwrap();
    for mark in [
        BarMark::Tempo,
        BarMark::Rehearsal,
        BarMark::Segno,
        BarMark::Ending,
    ] {
        let measure = if mark == BarMark::Tempo { 0 } else { 1 };
        score
            .set_bar_mark_offset(bar_mark(measure, mark), Offset { dx: 17, dy: -11 })
            .unwrap();
    }
    let mut engine = LayoutEngine::default();
    let base = engine.layout_view(&score, 960., ViewMode::Page);
    let preview = engine.layout_pitch_preview(&score, &base);
    assert_eq!(preview.tempos[0].position, base.tempos[0].position);
    assert_eq!(preview.rehearsals[0].position, base.rehearsals[0].position);
    assert_eq!(
        preview.rehearsals[0].frame[0].from,
        base.rehearsals[0].frame[0].from
    );
    assert_eq!(preview.navigation[0].position, base.navigation[0].position);
    assert_eq!(
        preview.endings[0].label_position,
        base.endings[0].label_position
    );
    assert_eq!(
        preview.endings[0].lines[0].from,
        base.endings[0].lines[0].from
    );
}

#[test]
fn moving_two_marks_does_not_add_their_vertical_offsets_to_ink_bounds() {
    let mut score = Score::new("".into(), &[Instrument::Violin], 4).unwrap();
    let id = score.insert(0, 0, note()).unwrap();
    score.set_dynamic(id, Some(Dynamic::P)).unwrap();
    score
        .set_technique(id, Some(PlayingTechnique::Pizzicato))
        .unwrap();
    let mut layout = LayoutEngine::default().layout_compact(&score, 960.);
    let original_top = point_mark(&layout, MarkRef::Dynamic(id)).0.content.ink_top;
    for mark in [MarkRef::Dynamic(id), MarkRef::Technique(id)] {
        layout.translate_mark(mark, 0., -400.);
    }
    let placed = point_mark(&layout, MarkRef::Dynamic(id)).0;
    let moved_top = placed
        .content
        .marks
        .iter()
        .filter(|box_| matches!(box_.mark, MarkRef::Dynamic(_) | MarkRef::Technique(_)))
        .map(|box_| box_.center.y - box_.half.1 - 4.)
        .fold(f32::INFINITY, f32::min);
    assert_eq!(placed.content.ink_top, original_top.min(moved_top));
    assert!(placed.content.ink_top > original_top - 600.);
}
