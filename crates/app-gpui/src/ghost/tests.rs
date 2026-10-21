use super::*;
use score_core::{Event, Instrument, Pitches};
use score_layout::{LayoutEngine, ViewMode};

fn geometry(score: &Score, scale: f32) -> Layout {
    let mut layout = LayoutEngine::default().layout_view(score, 1000., ViewMode::Continuous);
    layout.scale = scale;
    layout
}

fn single(instrument: Instrument) -> Score {
    Score::new("".into(), &[instrument], 2).unwrap()
}

fn pointer(layout: &Layout, part: PartId, staff: StaffId, tick: u16, y: f32) -> Point {
    let placed = layout.measure_on(part, staff, 0).unwrap();
    Point {
        x: (placed.origin.x + placed.content.x_at_tick(f64::from(tick))) * layout.scale,
        y: (placed.origin.y + y) * layout.scale,
    }
}

fn at(score: &Score, layout: &Layout, point: Point, voice: Voice, duration: Duration) -> Ghost {
    ghost_at(score, layout, point, voice, duration).unwrap()
}

#[test]
fn treble_lines_map_to_their_pitches() {
    let score = single(Instrument::Violin);
    for scale in [0.5, 1., 2.] {
        let layout = geometry(&score, scale);
        for (y, step, octave) in [
            (40., Step::E, 4),
            (20., Step::B, 4),
            (0., Step::F, 5),
            (-10., Step::A, 5),
        ] {
            let point = pointer(&layout, PartId(0), StaffId::Upper, 96, y);
            let ghost = at(&score, &layout, point, Voice::One, Duration::Quarter);
            assert_eq!(ghost.pitch, Pitch::new(step, octave).unwrap());
            assert_eq!(ghost.y - ghost.staff_top, y);
        }
    }
}

#[test]
fn alto_clef_middle_line_is_middle_c() {
    let score = single(Instrument::Viola);
    for scale in [0.5, 1., 2.] {
        let layout = geometry(&score, scale);
        let point = pointer(&layout, PartId(0), StaffId::Upper, 96, 20.);
        assert_eq!(
            at(&score, &layout, point, Voice::One, Duration::Quarter).pitch,
            Pitch::new(Step::C, 4).unwrap()
        );
    }
}

#[test]
fn bass_clef_and_ledger_lines() {
    let score = single(Instrument::Cello);
    for scale in [0.5, 1., 2.] {
        let layout = geometry(&score, scale);
        let point = pointer(&layout, PartId(0), StaffId::Upper, 96, 60.);
        assert_eq!(
            at(&score, &layout, point, Voice::One, Duration::Quarter).pitch,
            Pitch::new(Step::C, 2).unwrap()
        );
    }
}

#[test]
fn ticks_snap_to_the_entry_duration_grid_in_empty_bars() {
    let score = single(Instrument::Violin);
    for scale in [0.5, 1., 2.] {
        let layout = geometry(&score, scale);
        let point = pointer(&layout, PartId(0), StaffId::Upper, 115, 20.);
        let quarter = at(&score, &layout, point, Voice::One, Duration::Quarter);
        let eighth = at(&score, &layout, point, Voice::One, Duration::Eighth);
        assert_eq!(quarter.tick, 96);
        assert_eq!(eighth.tick % Duration::Eighth.ticks(), 0);
        assert_ne!(eighth.tick, quarter.tick);
    }
}

#[test]
fn ticks_never_fall_inside_a_written_note() {
    let mut score = single(Instrument::Violin);
    score
        .insert_in_voice(
            PartId(0),
            StaffId::Upper,
            0,
            Voice::One,
            0,
            Event {
                pitches: Pitches::single(Pitch::new(Step::C, 4)),
                duration: Duration::Half,
                notation: Default::default(),
            },
        )
        .unwrap();
    for scale in [0.5, 1., 2.] {
        let layout = geometry(&score, scale);
        let point = pointer(&layout, PartId(0), StaffId::Upper, 48, 20.);
        let tick = at(&score, &layout, point, Voice::One, Duration::Quarter).tick;
        assert!(tick == 0 || tick == 96, "tick {tick} entered a held note");
    }
}

#[test]
fn percussion_has_no_ghost() {
    let score = single(Instrument::SnareDrum);
    for scale in [0.5, 1., 2.] {
        let layout = geometry(&score, scale);
        let point = pointer(&layout, PartId(0), StaffId::Upper, 96, 20.);
        assert!(ghost_at(&score, &layout, point, Voice::One, Duration::Quarter).is_none());
    }
}

#[test]
fn outside_any_staff_is_none() {
    let score = single(Instrument::Violin);
    for scale in [0.5, 1., 2.] {
        let layout = geometry(&score, scale);
        let mut point = pointer(&layout, PartId(0), StaffId::Upper, 96, 20.);
        point.y = -1000.;
        assert!(ghost_at(&score, &layout, point, Voice::One, Duration::Quarter).is_none());
    }
}

#[test]
fn voice_two_uses_its_own_region_and_canonical_staff() {
    let mut score = single(Instrument::Violin);
    score
        .insert_in_voice(
            PartId(0),
            StaffId::Upper,
            0,
            Voice::Two,
            0,
            Event {
                pitches: Pitches::single(Pitch::new(Step::C, 4)),
                duration: Duration::Whole,
                notation: Default::default(),
            },
        )
        .unwrap();
    for scale in [0.5, 1., 2.] {
        let layout = geometry(&score, scale);
        let point = pointer(&layout, PartId(0), StaffId::Upper, 96, 20.);
        let ghost = at(&score, &layout, point, Voice::Two, Duration::Quarter);
        assert_eq!(ghost.voice, Voice::Two);
        assert_eq!(ghost.staff, StaffId::Upper);
        assert_eq!(ghost.tick, 0);
    }
}

#[test]
fn updated_duration_and_zoom_recompute_the_click_target() {
    let score = single(Instrument::Violin);
    for scale in [0.5, 1., 2.] {
        let layout = geometry(&score, scale);
        let point = pointer(&layout, PartId(0), StaffId::Upper, 115, 20.);
        let quarter = at(&score, &layout, point, Voice::One, Duration::Quarter);
        let eighth = at(&score, &layout, point, Voice::One, Duration::Eighth);
        assert_eq!(quarter.duration, Duration::Quarter);
        assert_eq!(eighth.duration, Duration::Eighth);
        assert_ne!(quarter.tick, eighth.tick);
    }
}

#[test]
fn divisi_lower_row_targets_the_upper_source_and_forced_voice_two() {
    let mut score = single(Instrument::Violin);
    score
        .insert_in_voice(
            PartId(0),
            StaffId::Upper,
            0,
            Voice::Two,
            0,
            Event {
                pitches: Pitches::single(Pitch::new(Step::C, 4)),
                duration: Duration::Whole,
                notation: Default::default(),
            },
        )
        .unwrap();
    score
        .set_passage_divisi(PartId(0), 0, Some(0), true)
        .unwrap();
    for scale in [0.5, 1., 2.] {
        let layout = geometry(&score, scale);
        let lower = layout.measure_on(PartId(0), StaffId::Lower, 0).unwrap();
        assert_eq!(lower.source_staff, StaffId::Upper);
        assert_eq!(lower.source_voice, Some(Voice::Two));
        let point = pointer(&layout, PartId(0), StaffId::Lower, 96, 20.);
        let ghost = at(&score, &layout, point, Voice::One, Duration::Quarter);
        assert_eq!(ghost.staff, StaffId::Upper);
        assert_eq!(ghost.voice, Voice::Two);
        assert_eq!(ghost.tick, 0);
    }
}
