use super::*;
use gpui::point;
use score_core::{
    Arpeggio, BarMark, BarMarkRef, Connection, ConnectionKind, Duration, Dynamic, Ending, Event,
    Instrument, Pitches, Step, Tremolo,
};
use score_edit::Command;
use score_layout::{LayoutEngine, ViewMode};

fn fixture() -> (Score, MarkRef) {
    let mut score = Score::default();
    let id = score
        .insert(
            0,
            0,
            Event {
                duration: Duration::Quarter,
                pitches: Pitches::single(score_core::Pitch::new(Step::C, 4)),
                notation: Default::default(),
            },
        )
        .unwrap();
    score.set_dynamic(id, Some(Dynamic::P)).unwrap();
    (score, MarkRef::Dynamic(id))
}

fn geometry(score: &Score, scale: f32) -> Rc<Layout> {
    let mut layout = LayoutEngine::default().layout_view(score, 1000., ViewMode::Continuous);
    layout.scale = scale;
    Rc::new(layout)
}

fn mark_x(layout: &Layout, mark: MarkRef) -> f32 {
    layout
        .measures
        .iter()
        .flat_map(|placed| &placed.content.marks)
        .find(|box_| box_.mark == mark)
        .unwrap()
        .center
        .x
}

#[test]
fn previews_follow_pointer_in_tenths_at_all_zooms_and_commit_as_one_undo() {
    for scale in [0.5, 0.7, 1., 1.5, 2.] {
        let (score, mark) = fixture();
        let shown = geometry(&score, scale);
        let origin = point(px(-120.), px(230.));
        let start = origin + point(px(400.), px(180.));
        let mut drag = MarkDrag::new(&score, mark, start, origin, scale, shown.clone()).unwrap();
        let position = start + point(px(30.), px(20.));
        assert!(drag.update(position, origin, false));
        let expected = Offset {
            dx: (30. / scale).round() as i16,
            dy: (20. / scale).round() as i16,
        };
        assert_eq!(drag.target(), Some(expected));
        let preview = drag.preview.as_ref().unwrap();
        assert!(
            (mark_x(preview, mark) - mark_x(&shown, mark) - f32::from(expected.dx)).abs() < 0.001
        );
        let mut document = score_document::Document::new(score.clone()).unwrap();
        assert!(!document.is_dirty());
        document
            .execute(Command::SetMarkOffset(mark, drag.target().unwrap()))
            .unwrap();
        assert_eq!(document.editor.score().mark_offset(mark), expected);
        assert!(document.is_dirty());
        assert!(document.editor.undo());
        assert_eq!(document.editor.score(), &score);
        assert!(!document.editor.can_undo());
    }
}

#[test]
fn jitter_under_five_pixels_is_a_click() {
    let (score, mark) = fixture();
    let start = point(px(20.), px(30.));
    let mut drag = MarkDrag::new(
        &score,
        mark,
        start,
        Point::default(),
        1.,
        geometry(&score, 1.),
    )
    .unwrap();
    let jitter = start + point(px(2.), px(4.));
    assert!(drag.pending(jitter));
    assert!(!drag.update(jitter, Point::default(), false));
    assert!(!drag.active);
    assert_eq!(drag.target(), None);
}

#[test]
fn shift_locks_the_dominant_axis() {
    let (score, mark) = fixture();
    let start = point(px(20.), px(30.));
    let mut drag = MarkDrag::new(
        &score,
        mark,
        start,
        Point::default(),
        1.,
        geometry(&score, 1.),
    )
    .unwrap();
    assert!(drag.update(start + point(px(30.), px(20.)), Point::default(), true));
    assert_eq!(drag.target(), Some(Offset { dx: 30, dy: 0 }));
    assert!(drag.update(start + point(px(10.), px(40.)), Point::default(), true));
    assert_eq!(drag.target(), Some(Offset { dx: 0, dy: 40 }));
}

#[test]
fn existing_offset_is_the_base() {
    let (mut score, mark) = fixture();
    score
        .set_mark_offset(mark, Offset { dx: 10, dy: 0 })
        .unwrap();
    let shown = geometry(&score, 1.);
    let start = point(px(20.), px(30.));
    let mut drag = MarkDrag::new(&score, mark, start, Point::default(), 1., shown.clone()).unwrap();
    assert!(drag.update(start + point(px(5.), px(0.)), Point::default(), false));
    assert_eq!(drag.target(), Some(Offset { dx: 15, dy: 0 }));
    assert!(
        (mark_x(drag.preview.as_ref().unwrap(), mark) - mark_x(&shown, mark) - 5.).abs() < 0.001
    );
}

#[test]
fn ties_tremolos_and_arpeggios_do_not_start_a_drag() {
    let (mut score, dynamic) = fixture();
    let a = dynamic.anchor();
    let b = score
        .insert(
            0,
            1,
            Event {
                duration: Duration::Quarter,
                pitches: Pitches::single(score_core::Pitch::new(Step::C, 4)),
                notation: Default::default(),
            },
        )
        .unwrap();
    let tie = MarkRef::Connection(Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Tie,
    });
    if let MarkRef::Connection(connection) = tie {
        score.add_connection(connection).unwrap();
    }
    let unsupported = |score: &Score, mark| {
        assert!(score.has_mark(mark));
        assert!(!mark.can_move());
        assert!(
            MarkDrag::new(
                score,
                mark,
                Point::default(),
                Point::default(),
                1.,
                geometry(score, 1.)
            )
            .is_none()
        );
    };
    unsupported(&score, tie);
    score.set_tremolo(a, Some(Tremolo::One)).unwrap();
    unsupported(&score, MarkRef::Tremolo(a));
    score
        .add_pitch(a, score_core::Pitch::new(Step::E, 4).unwrap())
        .unwrap();
    score.set_arpeggio(a, Some(Arpeggio::Up)).unwrap();
    unsupported(&score, MarkRef::Arpeggio(a));
}

#[test]
fn scrolling_during_the_drag_does_not_move_the_mark() {
    let (score, mark) = fixture();
    let origin = point(px(100.), px(200.));
    let start = origin + point(px(20.), px(30.));
    let mut drag = MarkDrag::new(&score, mark, start, origin, 1., geometry(&score, 1.)).unwrap();
    assert!(drag.update(start + point(px(30.), px(20.)), origin, false));
    let first = drag.target();
    let new_origin = origin + point(px(80.), px(-40.));
    assert!(!drag.update(
        start + point(px(30.), px(20.)) + (new_origin - origin),
        new_origin,
        false
    ));
    assert_eq!(drag.target(), first);
}

#[test]
fn arrows_nudge_two_tenths_and_cmd_one_space() {
    for (key, expected) in [
        ("up", (0, -2)),
        ("down", (0, 2)),
        ("left", (-2, 0)),
        ("right", (2, 0)),
    ] {
        assert_eq!(nudge(key, Modifiers::default()), Some(expected));
        assert_eq!(
            nudge(
                key,
                Modifiers {
                    platform: true,
                    ..Default::default()
                }
            ),
            Some((expected.0 * 5, expected.1 * 5))
        );
    }
    assert_eq!(nudge("tab", Modifiers::default()), None);
    assert_eq!(
        nudge(
            "left",
            Modifiers {
                shift: true,
                ..Default::default()
            }
        ),
        None
    );
    assert_eq!(
        nudge(
            "up",
            Modifiers {
                alt: true,
                ..Default::default()
            }
        ),
        None
    );
    assert_eq!(
        nudge(
            "down",
            Modifiers {
                control: true,
                ..Default::default()
            }
        ),
        None
    );
}

#[test]
fn rehearsal_bar_drag_previews_at_every_zoom_and_commits_one_undo_step() {
    for scale in [0.5, 1., 2.] {
        let mut score = Score::default();
        score.toggle_rehearsal_mark(1).unwrap();
        let mark = BarMarkRef {
            measure: 1,
            mark: BarMark::Rehearsal,
        };
        let shown = geometry(&score, scale);
        let origin = point(px(-40.), px(80.));
        let start = origin + point(px(300.), px(160.));
        let mut drag =
            MarkDrag::new_bar(&score, mark, start, origin, scale, shown.clone()).unwrap();
        assert!(drag.update(start + point(px(20.), px(-10.)), origin, false));
        let expected = Offset {
            dx: (20. / scale).round() as i16,
            dy: (-10. / scale).round() as i16,
        };
        assert_eq!(drag.target(), Some(expected));
        assert_eq!(score.bar_mark_offset(mark), Offset::ZERO);
        let preview = drag.preview.as_ref().unwrap();
        assert_eq!(
            preview.rehearsals[0].position.x - shown.rehearsals[0].position.x,
            f32::from(expected.dx)
        );
        assert_eq!(
            preview.rehearsals[0].position.y - shown.rehearsals[0].position.y,
            f32::from(expected.dy)
        );
        let mut document = score_document::Document::new(score.clone()).unwrap();
        document
            .execute(Command::SetBarMarkOffset(mark, drag.target().unwrap()))
            .unwrap();
        assert_eq!(document.editor.score().bar_mark_offset(mark), expected);
        assert!(document.editor.undo());
        assert_eq!(document.editor.score(), &score);
        assert!(!document.editor.can_undo());
    }
}

#[test]
fn tempo_click_below_drag_threshold_has_no_target() {
    let score = Score::default();
    let mark = BarMarkRef {
        measure: 0,
        mark: BarMark::Tempo,
    };
    let start = point(px(40.), px(90.));
    let mut drag = MarkDrag::new_bar(
        &score,
        mark,
        start,
        Point::default(),
        1.,
        geometry(&score, 1.),
    )
    .unwrap();
    assert!(drag.pending(start));
    assert!(!drag.update(start + point(px(2.), px(3.)), Point::default(), false));
    assert!(!drag.active);
    assert_eq!(drag.target(), None);
}

#[test]
fn dragging_an_ending_continuation_moves_every_segment_in_one_undo_step() {
    let mut score = Score::new("".into(), &[Instrument::Violin], 8).unwrap();
    score
        .set_ending(Ending {
            start: 1,
            end: 6,
            numbers: vec![1],
        })
        .unwrap();
    let mark = BarMarkRef {
        measure: 1,
        mark: BarMark::Ending,
    };
    let shown = Rc::new(LayoutEngine::default().layout(&score, 2000.));
    assert!(shown.endings.len() > 1);
    let continuation = &shown.endings[1].lines[0];
    assert_eq!(
        shown.hit_bar_mark(score_layout::Point {
            x: (continuation.from.x + continuation.to.x) / 2.,
            y: continuation.from.y,
        }),
        Some(mark)
    );
    let start = point(px(100.), px(100.));
    let mut drag =
        MarkDrag::new_bar(&score, mark, start, Point::default(), 1., shown.clone()).unwrap();
    assert!(drag.update(start + point(px(18.), px(25.)), Point::default(), false));
    let offset = Offset { dx: 18, dy: 25 };
    assert_eq!(drag.target(), Some(offset));
    let preview = drag.preview.as_ref().unwrap();
    for (old, moved) in shown.endings.iter().zip(&preview.endings) {
        assert_eq!(
            (
                moved.label_position.x - old.label_position.x,
                moved.label_position.y - old.label_position.y,
            ),
            (18., 25.)
        );
        for (a, b) in old.lines.iter().zip(&moved.lines) {
            assert_eq!((b.from.x - a.from.x, b.from.y - a.from.y), (18., 25.));
        }
    }
    let mut document = score_document::Document::new(score.clone()).unwrap();
    document
        .execute(Command::SetBarMarkOffset(mark, offset))
        .unwrap();
    assert_eq!(document.editor.score().bar_mark_offset(mark), offset);
    assert!(document.editor.undo());
    assert_eq!(document.editor.score(), &score);
    assert!(!document.editor.can_undo());
}
