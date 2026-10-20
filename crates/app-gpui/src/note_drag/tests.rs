use super::*;
use gpui::point;
use score_core::{Duration, Event, Pitches, StaffId, Step};

fn fixture() -> (Score, ElementId, Pitch) {
    let mut score = Score::default();
    let pitch = Pitch::new(Step::C, 4).unwrap();
    let id = score
        .insert(
            0,
            0,
            Event {
                duration: Duration::default(),
                pitches: Pitches::single(Some(pitch)),
                notation: Default::default(),
            },
        )
        .unwrap();
    (score, id, pitch)
}

#[test]
fn previews_follow_staff_steps_at_all_zooms_and_commit_as_one_undo() {
    for scale in [0.5, 0.7, 1., 1.5, 2.] {
        let (score, id, pitch) = fixture();
        let origin = point(px(-120.), px(230.));
        let start = origin + point(px(400.), px(180.));
        let mut drag =
            NoteDrag::new(&score, id, pitch, start, origin, &geometry(&score, scale)).unwrap();
        for steps in [1, 3, -2, 7] {
            let position = start - point(px(0.), px(5. * scale * f32::from(steps)));
            assert!(drag.update(position, origin));
            assert_eq!(drag.target, pitch.shift(steps));
        }
        // The tentative gesture never touches the document or its saved state.
        let mut document = score_document::Document::new(score.clone()).unwrap();
        assert!(!document.is_dirty());
        document.editor.select_pitch(id, Some(pitch));
        document
            .execute(score_edit::Command::ReplacePitch(drag.target.unwrap()))
            .unwrap();
        assert_eq!(document.editor.score(), drag.preview.as_ref().unwrap());
        assert!(document.is_dirty());
        assert!(document.editor.undo());
        assert_eq!(document.editor.score(), &score);
        assert!(!document.is_dirty());
        assert!(!document.editor.can_undo());
        assert!(document.editor.redo());
        assert_eq!(document.editor.selected_pitch(), pitch.shift(7));
    }
}

#[test]
fn gesture_intent_keeps_jitter_pitch_and_time_separate() {
    let (score, id, pitch) = fixture();
    let start = point(px(20.), px(30.));
    let mut drag = NoteDrag::new(
        &score,
        id,
        pitch,
        start,
        Point::default(),
        &geometry(&score, 1.),
    )
    .unwrap();
    assert_eq!(drag.motion(start + point(px(2.), px(4.))), Motion::Pending);
    assert_eq!(drag.motion(start + point(px(10.), px(5.))), Motion::Time);
    assert_eq!(drag.motion(start + point(px(3.), px(-10.))), Motion::Pitch);
    drag.update(start + point(px(0.), px(-10.)), Point::default());
    assert_eq!(drag.motion(start + point(px(100.), px(0.))), Motion::Pitch);
    drag.update(start, Point::default());
    assert!(drag.target.is_none());
    assert!(drag.preview.is_none());
    assert!(drag.error.is_none());
}

#[test]
fn chord_tones_move_independently_and_reject_collisions_without_losing_notes() {
    let (mut score, id, pitch) = fixture();
    let high = Pitch::new(Step::G, 4).unwrap();
    score.add_pitch(id, high).unwrap();
    let mut drag = NoteDrag::new(
        &score,
        id,
        pitch,
        Point::default(),
        Point::default(),
        &geometry(&score, 1.),
    )
    .unwrap();
    drag.update(point(px(0.), px(-10.)), Point::default());
    let preview = drag.preview.as_ref().unwrap();
    assert_eq!(
        preview
            .event(id)
            .unwrap()
            .pitches
            .iter()
            .collect::<Vec<_>>(),
        [Pitch::new(Step::E, 4).unwrap(), high]
    );
    drag.update(point(px(0.), px(-20.)), Point::default());
    assert!(drag.error.is_some());
    assert!(drag.preview.is_none());
    assert!(drag.target.is_none());
    assert_eq!(score.event(id).unwrap().pitches.first(), Some(pitch));
    // Cancellation drops preview only; existing document/history remain intact.
    drop(drag);
    assert_eq!(score.event(id).unwrap().pitches.len(), 2);
}

#[test]
fn scrolling_accidentals_limits_and_harmonics_are_checked() {
    let (mut score, id, pitch) = fixture();
    let sharp = pitch.with_accidental(Some(score_core::Accidental::Sharp));
    score.replace_pitch(id, 0, sharp).unwrap();
    let mut drag = NoteDrag::new(
        &score,
        id,
        sharp,
        Point::default(),
        Point::default(),
        &geometry(&score, 0.5),
    )
    .unwrap();
    drag.update(point(px(0.), px(-55.)), point(px(0.), px(-50.)));
    assert_eq!(drag.target, sharp.shift(2));
    drag.update(point(px(0.), px(10000.)), Point::default());
    assert_eq!(drag.target.unwrap().diatonic(), 0);
    drag.update(point(px(0.), px(-10000.)), Point::default());
    assert_eq!(drag.target.unwrap().diatonic(), 62);

    let mut strings = Score::new("Harmonics".into(), &[score_core::Instrument::Violin], 1).unwrap();
    let part = strings.parts()[0].id();
    let id = strings
        .insert_on(
            part,
            StaffId::Upper,
            0,
            0,
            Event {
                duration: Duration::default(),
                pitches: Pitches::single(Some(pitch)),
                notation: Default::default(),
            },
        )
        .unwrap();
    strings
        .set_harmonic(id, Some(score_core::Harmonic::Artificial))
        .unwrap();
    let mut drag = NoteDrag::new(
        &strings,
        id,
        pitch,
        Point::default(),
        Point::default(),
        &geometry(&strings, 1.),
    )
    .unwrap();
    drag.update(point(px(0.), px(-150.)), Point::default());
    assert!(drag.error.is_some());
    assert_eq!(strings.event(id).unwrap().pitches.first(), Some(pitch));
    drag.update(point(px(0.), px(-5.)), Point::default());
    assert!(drag.error.is_none());
    assert_eq!(
        drag.preview.unwrap().event(id).unwrap().notation.harmonic,
        Some(score_core::Harmonic::Artificial)
    );
}

fn geometry(score: &Score, scale: f32) -> score_layout::Layout {
    let mut layout =
        LayoutEngine::default().layout_view(score, 1000., score_layout::ViewMode::Continuous);
    layout.scale = scale;
    layout
}

#[test]
fn preview_keeps_high_note_staff_and_rhythmic_grid_fixed_in_every_view() {
    let (mut score, id, _) = fixture();
    let pitch = Pitch::new(Step::C, 6).unwrap();
    score.replace_pitch(id, 0, pitch).unwrap();
    for mode in [
        score_layout::ViewMode::Page,
        score_layout::ViewMode::Continuous,
        score_layout::ViewMode::SingleSystem,
    ] {
        let base = LayoutEngine::default().layout_view(&score, 1000., mode);
        let bar = base.event_measure(id).unwrap();
        let old = bar.content.events.iter().find(|e| e.id == id).unwrap();
        let mut drag =
            NoteDrag::new(&score, id, pitch, Point::default(), Point::default(), &base).unwrap();
        for steps in [1i16, 7, -14] {
            drag.update(point(px(0.), px(-5. * f32::from(steps))), Point::default());
            let preview = drag
                .layout
                .layout_pitch_preview(drag.preview.as_ref().unwrap(), &drag.geometry);
            for (before, after) in base.measures.iter().zip(&preview.measures) {
                assert_eq!(before.index, after.index);
                assert_eq!(before.origin, after.origin);
                assert_eq!(before.width, after.width);
                assert_eq!(before.content.time_positions, after.content.time_positions);
            }
            let new = preview
                .event_measure(id)
                .unwrap()
                .content
                .events
                .iter()
                .find(|e| e.id == id)
                .unwrap();
            assert!((new.center.y - old.center.y + 5. * f32::from(steps)).abs() < 0.001);
            assert!((new.center.x - old.center.x).abs() < 0.001);
        }
    }
}

fn head_position(layout: &score_layout::Layout, id: ElementId) -> Point<Pixels> {
    let m = layout.event_measure(id).unwrap();
    let e = m.content.events.iter().find(|e| e.id == id).unwrap();
    point(
        px((m.origin.x + e.center.x) * layout.scale),
        px((m.origin.y + e.center.y) * layout.scale),
    )
}

fn time_pointer(drag: &NoteDrag, bar: usize, tick: u16, origin: Point<Pixels>) -> Point<Pixels> {
    let source = drag.geometry.event_measure(drag.id).unwrap();
    let head = source
        .content
        .events
        .iter()
        .find(|e| e.id == drag.id)
        .unwrap();
    let voice = drag.base.voice_of(drag.id).unwrap();
    let target = drag
        .geometry
        .voice_measure_at_tick(drag.id.part, drag.id.staff, bar, voice, f64::from(tick))
        .unwrap();
    let offset = head.center.x - source.content.x_at_tick(f64::from(head.tick));
    origin
        + point(
            px(
                (target.origin.x + target.content.x_at_tick(f64::from(tick)) + offset)
                    * drag.geometry.scale,
            ),
            px((target.origin.y + head.center.y) * drag.geometry.scale),
        )
}

#[test]
fn time_drag_snaps_across_bars_views_zoom_and_scroll_without_changing_the_source() {
    let (score, id, pitch) = fixture();
    for mode in [
        score_layout::ViewMode::Page,
        score_layout::ViewMode::Continuous,
        score_layout::ViewMode::SingleSystem,
    ] {
        for scale in [0.5, 0.7, 1., 1.5] {
            let mut layout = LayoutEngine::default().layout_view(&score, 1000., mode);
            layout.scale = scale;
            let origin = point(px(-120.), px(230.));
            let start = head_position(&layout, id) + origin;
            let mut drag = NoteDrag::new(&score, id, pitch, start, origin, &layout).unwrap();
            for (bar, tick) in [(1, 0), (2, 24), (3, 72)] {
                let point = time_pointer(&drag, bar, tick, origin);
                drag.update_time(point, origin);
                let target = drag.time_target.expect("valid time destination");
                assert_eq!((target.measure, target.tick), (bar, tick));
                assert!(!target.swap);
                assert!(drag.preview.is_none());
                assert!(drag.target.is_none());
                assert_eq!(drag.base, score);
            }
            let moved_origin = origin + point(px(40.), px(-100.));
            drag.update_time(time_pointer(&drag, 2, 24, moved_origin), moved_origin);
            assert_eq!(
                (
                    drag.time_target.unwrap().measure,
                    drag.time_target.unwrap().tick
                ),
                (2, 24)
            );
            drag.update_time(start, origin);
            assert!(drag.time_target.is_none());
            assert!(drag.error.is_none());
        }
    }
}

#[test]
fn time_drag_previews_swaps_and_rejects_overlaps_and_outside_drops() {
    let (mut score, id, pitch) = fixture();
    score
        .insert(
            1,
            0,
            Event {
                duration: Duration::Quarter,
                pitches: Pitches::single(Pitch::new(Step::G, 4)),
                notation: Default::default(),
            },
        )
        .unwrap();
    score
        .insert(
            2,
            0,
            Event {
                duration: Duration::Half,
                pitches: Pitches::single(Pitch::new(Step::D, 4)),
                notation: Default::default(),
            },
        )
        .unwrap();
    let layout = geometry(&score, 1.);
    let start = head_position(&layout, id);
    let mut drag = NoteDrag::new(&score, id, pitch, start, Point::default(), &layout).unwrap();
    drag.update_time(
        time_pointer(&drag, 1, 0, Point::default()),
        Point::default(),
    );
    assert!(drag.time_target.unwrap().swap);
    drag.update_time(
        time_pointer(&drag, 2, 0, Point::default()),
        Point::default(),
    );
    assert!(drag.time_target.is_none());
    assert!(drag.error.is_some());
    drag.update_time(point(px(-100.), px(-100.)), Point::default());
    assert!(drag.time_target.is_none());
    assert!(drag.error.is_some());
    assert_eq!(score, drag.base);
}

#[test]
fn projected_divisi_voice_keeps_its_canonical_staff_and_voice_when_moved() {
    let score = score_io::decode(include_bytes!(
        "../../../../examples/divisi-dentro-del-compas.partitura"
    ))
    .unwrap();
    let part = score.parts()[0].id();
    let (bar, event, value) = score
        .part(part)
        .unwrap()
        .staff(StaffId::Upper)
        .unwrap()
        .measures()
        .iter()
        .enumerate()
        .flat_map(|(bar, m)| {
            m.voice_iter(score_core::Voice::Two)
                .filter(|(_, e)| !e.pitches.is_empty())
                .map(move |(id, e)| (bar, id, *e))
        })
        .next()
        .unwrap();
    let id = ElementId {
        part,
        staff: StaffId::Upper,
        measure: bar,
        event,
    };
    let layout = geometry(&score, 0.7);
    let start = head_position(&layout, id);
    let mut drag = NoteDrag::new(
        &score,
        id,
        value.pitches.first().unwrap(),
        start,
        Point::default(),
        &layout,
    )
    .unwrap();
    // Existing onsets remain reachable even on the projected second staff.
    let target = score
        .part(part)
        .unwrap()
        .staff(StaffId::Upper)
        .unwrap()
        .measures()[bar]
        .timed_iter()
        .filter(|(v, _, _, e)| *v == score_core::Voice::Two && !e.pitches.is_empty())
        .nth(1)
        .unwrap()
        .1;
    drag.update_time(
        time_pointer(&drag, bar, target, Point::default()),
        Point::default(),
    );
    assert_eq!(drag.time_target.unwrap().tick, target);
    let mut moved = score.clone();
    let id = moved.move_event(id, bar, target).unwrap();
    assert_eq!(id.staff, StaffId::Upper);
    assert_eq!(moved.voice_of(id), Some(score_core::Voice::Two));
}
