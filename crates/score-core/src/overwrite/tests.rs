use super::*;

/// A quarter note.
const Q: u32 = crate::TICKS_PER_QUARTER;

fn note(step: Step, duration: Duration) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 4)),
        duration,
        notation: Notation::default(),
    }
}

fn rest(duration: Duration) -> Event {
    Event {
        pitches: Pitches::default(),
        duration,
        notation: Notation::default(),
    }
}

fn write(
    score: &mut Score,
    measure: usize,
    tick: u32,
    event: Event,
) -> Result<Vec<ElementId>, ScoreError> {
    score.overwrite(PartId(0), StaffId::Upper, measure, Voice::One, tick, event)
}

#[test]
fn writes_into_an_empty_bar_without_filling_the_remainder() {
    let mut score = Score::default();
    let ids = write(&mut score, 0, 0, note(Step::C, Duration::Quarter)).unwrap();
    assert_eq!(ids.len(), 1);
    assert_eq!(score.measures()[0].voice_ticks(Voice::One), Q);
    score.validate().unwrap();
}

#[test]
fn an_unsplit_duration_keeps_its_exact_dot_and_triplet_flags() {
    let duration = Duration::Quarter.with_dotted(true).with_triplet(true);
    for event in [note(Step::C, duration), rest(duration)] {
        let mut score = Score::default();
        let ids = write(&mut score, 0, 0, event).unwrap();
        assert_eq!(score.event(ids[0]).unwrap().duration, duration);
        score.validate().unwrap();
    }
}

fn score_with_bars(bars: usize) -> Score {
    Score::new("".into(), &[Instrument::Violin], bars).unwrap()
}

fn voice(score: &Score, bar: usize) -> Vec<(ElementId, Event)> {
    score.measures()[bar]
        .voice_iter(Voice::One)
        .map(|(key, event)| {
            (
                ElementId {
                    part: PartId(0),
                    staff: StaffId::Upper,
                    measure: bar,
                    event: key,
                },
                *event,
            )
        })
        .collect()
}

fn quarters(score: &mut Score, bar: usize, steps: &[Step]) -> Vec<ElementId> {
    steps
        .iter()
        .copied()
        .enumerate()
        .map(|(index, step)| {
            score
                .insert(bar, index, note(step, Duration::Quarter))
                .unwrap()
        })
        .collect()
}

#[test]
fn replacing_a_whole_rest_leaves_rests_by_beat() {
    let mut score = Score::default();
    score.insert(0, 0, rest(Duration::Whole)).unwrap();
    write(&mut score, 0, 0, note(Step::C, Duration::Quarter)).unwrap();
    let events = voice(&score, 0);
    assert_eq!(
        events[0].1.pitches,
        note(Step::C, Duration::Quarter).pitches
    );
    // A quarter rest finishes beat 2 and a half rest the second half, never
    // a dotted half from beat 2.
    let rests: Vec<_> = events[1..].iter().map(|e| e.1.duration).collect();
    assert_eq!(rests, [Duration::Quarter, Duration::Half]);
    assert!(events[1..].iter().all(|e| e.1.pitches.is_empty()));
    score.validate().unwrap();
}

#[test]
fn a_dotted_quarter_over_a_whole_rest_leaves_an_eighth_then_a_half() {
    let mut score = Score::default();
    score.insert(0, 0, rest(Duration::Whole)).unwrap();
    write(
        &mut score,
        0,
        0,
        note(Step::C, Duration::Quarter.with_dotted(true)),
    )
    .unwrap();
    let rests: Vec<_> = voice(&score, 0)[1..].iter().map(|e| e.1.duration).collect();
    assert_eq!(rests, [Duration::Eighth, Duration::Half]);
}

#[test]
fn overwriting_two_notes_keeps_neighbors_and_their_keys() {
    let mut score = Score::default();
    let ids = quarters(&mut score, 0, &[Step::C, Step::D, Step::E, Step::F]);
    write(&mut score, 0, Q, note(Step::G, Duration::Half)).unwrap();
    let events = voice(&score, 0);
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].0, ids[0]);
    assert_eq!(events[2].0, ids[3]);
    assert_eq!(events[1].1.pitches, note(Step::G, Duration::Half).pitches);
    assert_eq!(score.event_tick(ids[3]), Some(u64::from(3 * Q)));
    score.validate().unwrap();
}

#[test]
fn a_short_write_leaves_silence_before_the_next_note() {
    let mut score = Score::default();
    let ids = quarters(&mut score, 0, &[Step::C, Step::D, Step::E, Step::F]);
    write(&mut score, 0, Q, note(Step::G, Duration::Eighth)).unwrap();
    let events = voice(&score, 0);
    assert_eq!(events.len(), 5);
    assert_eq!(events[0].0, ids[0]);
    assert_eq!(events[1].1.pitches, note(Step::G, Duration::Eighth).pitches);
    assert!(events[2].1.pitches.is_empty());
    assert_eq!(events[2].1.duration, Duration::Eighth);
    assert_eq!(events[3].0, ids[2]);
    assert_eq!(events[4].0, ids[3]);
    score.validate().unwrap();
}

#[test]
fn a_replaced_rests_dynamic_and_offset_follow_the_new_note() {
    let mut score = Score::default();
    let old = score.insert(0, 0, rest(Duration::Quarter)).unwrap();
    score.set_dynamic(old, Some(Dynamic::P)).unwrap();
    let offset = Offset { dx: 16, dy: -9 };
    score
        .set_mark_offset(MarkRef::Dynamic(old), offset)
        .unwrap();
    let new = write(&mut score, 0, 0, note(Step::C, Duration::Quarter)).unwrap()[0];
    assert_eq!(score.event(new).unwrap().notation.dynamic, Some(Dynamic::P));
    assert_eq!(score.mark_offset(MarkRef::Dynamic(new)), offset);
    score.validate().unwrap();
}

#[test]
fn a_crescendo_and_its_offset_follow_a_replaced_start() {
    let mut score = Score::default();
    let ids = quarters(&mut score, 0, &[Step::C, Step::D, Step::E, Step::F]);
    let old = Connection {
        start: ids[1],
        end: ids[3],
        kind: ConnectionKind::Crescendo,
    };
    score.add_connection(old).unwrap();
    let offset = Offset { dx: 19, dy: 7 };
    score
        .set_mark_offset(MarkRef::Connection(old), offset)
        .unwrap();
    let new_start = write(&mut score, 0, Q, note(Step::G, Duration::Quarter)).unwrap()[0];
    let new = Connection {
        start: new_start,
        ..old
    };
    assert!(score.connections().contains(&new));
    assert_eq!(score.mark_offset(MarkRef::Connection(new)), offset);
    score.validate().unwrap();
}

#[test]
fn changing_a_tied_notes_pitch_prunes_the_old_tie() {
    let mut score = Score::default();
    let ids = quarters(&mut score, 0, &[Step::C, Step::C]);
    score
        .add_connection(Connection {
            start: ids[0],
            end: ids[1],
            kind: ConnectionKind::Tie,
        })
        .unwrap();
    write(&mut score, 0, Q, note(Step::D, Duration::Quarter)).unwrap();
    assert!(score.connections().is_empty());
    score.validate().unwrap();
}

#[test]
fn a_write_inside_an_existing_note_fails_atomically() {
    let mut score = Score::default();
    score
        .insert(0, 0, note(Step::C, Duration::Quarter))
        .unwrap();
    let before = score.clone();
    let error = write(&mut score, 0, Q / 2, note(Step::D, Duration::Eighth)).unwrap_err();
    assert_eq!(
        error.0,
        "La posición cae dentro de una nota; elige el inicio de una figura o un silencio"
    );
    assert_eq!(score, before);
    score.validate().unwrap();
}

#[test]
fn writing_into_a_rest_preserves_later_onsets() {
    let mut score = Score::default();
    score.insert(0, 0, rest(Duration::Half)).unwrap();
    let later = score
        .insert(0, 1, note(Step::D, Duration::Quarter))
        .unwrap();
    write(&mut score, 0, Q, note(Step::C, Duration::Quarter)).unwrap();
    let events = voice(&score, 0);
    assert!(events[0].1.pitches.is_empty());
    assert_eq!(events[0].1.duration, Duration::Quarter);
    assert_eq!(
        events[1].1.pitches,
        note(Step::C, Duration::Quarter).pitches
    );
    assert_eq!(events[2].0, later);
    assert_eq!(score.event_tick(later), Some(u64::from(2 * Q)));
    score.validate().unwrap();
}

#[test]
fn overwriting_voice_one_leaves_voice_two_untouched() {
    let mut score = Score::default();
    let second = score
        .insert_in_voice(
            PartId(0),
            StaffId::Upper,
            0,
            Voice::Two,
            0,
            note(Step::E, Duration::Quarter),
        )
        .unwrap();
    let before = score.event(second).copied();
    write(&mut score, 0, 0, note(Step::C, Duration::Quarter)).unwrap();
    assert_eq!(score.event(second).copied(), before);
    assert_eq!(score.event_tick(second), Some(0));
    score.validate().unwrap();
}

fn assert_tied(score: &Score, ids: &[ElementId]) {
    for pair in ids.windows(2) {
        assert!(score.connections().contains(&Connection {
            start: pair[0],
            end: pair[1],
            kind: ConnectionKind::Tie,
        }));
    }
    score.validate().unwrap();
}

#[test]
fn a_half_note_on_the_fourth_beat_extends_to_a_new_bar() {
    let mut score = score_with_bars(1);
    quarters(&mut score, 0, &[Step::C, Step::D, Step::E]);
    let ids = write(&mut score, 0, 3 * Q, note(Step::G, Duration::Half)).unwrap();
    assert_eq!(score.measures().len(), 2);
    assert_eq!(ids.len(), 2);
    assert_eq!((ids[0].measure, ids[1].measure), (0, 1));
    assert_eq!(score.event(ids[0]).unwrap().duration, Duration::Quarter);
    assert_eq!(score.event(ids[1]).unwrap().duration, Duration::Quarter);
    assert_tied(&score, &ids);
}

#[test]
fn dotted_half_from_beat_three_splits_into_half_and_quarter() {
    let mut score = score_with_bars(2);
    let ids = write(
        &mut score,
        0,
        2 * Q,
        note(Step::G, Duration::Half.with_dotted(true)),
    )
    .unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(score.event(ids[0]).unwrap().duration, Duration::Half);
    assert_eq!(score.event(ids[1]).unwrap().duration, Duration::Quarter);
    assert_eq!(score.measures()[0].voice_ticks(Voice::One), 4 * Q);
    assert_tied(&score, &ids);
}

#[test]
fn whole_note_from_beat_two_splits_into_dotted_half_and_quarter() {
    let mut score = score_with_bars(2);
    let ids = write(&mut score, 0, Q, note(Step::G, Duration::Whole)).unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(
        score.event(ids[0]).unwrap().duration,
        Duration::Half.with_dotted(true)
    );
    assert_eq!(score.event(ids[1]).unwrap().duration, Duration::Quarter);
    assert_tied(&score, &ids);
}

#[test]
fn a_key_change_rewrites_the_second_pieces_written_accidental() {
    let mut score = score_with_bars(2);
    score
        .set_key_signature_at(1, KeySignature::new(1, KeyMode::Major).unwrap())
        .unwrap();
    let ids = write(&mut score, 0, 3 * Q, note(Step::F, Duration::Half)).unwrap();
    let first = score
        .event(ids[0])
        .unwrap()
        .pitches
        .in_key(score.written_key_at(PartId(0), 0));
    let second = score
        .event(ids[1])
        .unwrap()
        .pitches
        .in_key(score.written_key_at(PartId(0), 1));
    assert_eq!(
        first
            .iter()
            .map(|pitch| (pitch.diatonic(), pitch.alteration()))
            .collect::<Vec<_>>(),
        second
            .iter()
            .map(|pitch| (pitch.diatonic(), pitch.alteration()))
            .collect::<Vec<_>>()
    );
    assert_tied(&score, &ids);
}

#[test]
fn a_crossbar_chord_keeps_both_pitches_in_each_tied_piece() {
    let mut score = score_with_bars(1);
    let mut event = note(Step::C, Duration::Half);
    event.pitches = Pitches::new(&[
        Pitch::new(Step::C, 4).unwrap(),
        Pitch::new(Step::E, 4).unwrap(),
    ])
    .unwrap();
    let ids = write(&mut score, 0, 3 * Q, event).unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(score.event(ids[0]).unwrap().pitches.len(), 2);
    assert_eq!(score.event(ids[1]).unwrap().pitches, event.pitches);
    assert_tied(&score, &ids);
}

#[test]
fn a_triplet_cannot_cross_the_bar_and_changes_nothing() {
    let mut score = score_with_bars(1);
    let before = score.clone();
    let event = note(Step::C, Duration::Quarter.with_triplet(true));
    let error = write(&mut score, 0, 11 * Q / 3, event).unwrap_err();
    assert_eq!(error.0, "Un grupillo no puede cruzar la barra de compás");
    assert_eq!(score, before);
    score.validate().unwrap();
}

#[test]
fn crossing_rests_split_without_creating_ties() {
    let mut score = score_with_bars(1);
    let ids = write(&mut score, 0, 3 * Q, rest(Duration::Half)).unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(score.event(ids[0]).unwrap().duration, Duration::Quarter);
    assert_eq!(score.event(ids[1]).unwrap().duration, Duration::Quarter);
    assert!(score.connections().is_empty());
    score.validate().unwrap();
}

#[test]
fn a_whole_in_three_four_becomes_dotted_half_then_quarter() {
    let mut score = score_with_bars(1);
    score
        .set_time_signature_at(
            0,
            TimeSignature {
                numerator: 3,
                denominator: 4,
            },
        )
        .unwrap();
    let ids = write(&mut score, 0, 0, note(Step::C, Duration::Whole)).unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(
        score.event(ids[0]).unwrap().duration,
        Duration::Half.with_dotted(true)
    );
    assert_eq!(score.event(ids[1]).unwrap().duration, Duration::Quarter);
    assert_tied(&score, &ids);
}

#[test]
fn a_whole_in_two_four_becomes_two_tied_halves() {
    let mut score = score_with_bars(1);
    score
        .set_time_signature_at(
            0,
            TimeSignature {
                numerator: 2,
                denominator: 4,
            },
        )
        .unwrap();
    let ids = write(&mut score, 0, 0, note(Step::C, Duration::Whole)).unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(score.event(ids[0]).unwrap().duration, Duration::Half);
    assert_eq!(score.event(ids[1]).unwrap().duration, Duration::Half);
    assert_tied(&score, &ids);
}

#[test]
fn articulation_stays_first_and_fermata_with_its_offset_goes_last() {
    let mut score = score_with_bars(1);
    let mut event = note(Step::C, Duration::Half);
    event.notation.articulation = Some(Articulation::Staccato);
    event.notation.fermata = true;
    event.notation.offsets.fermata = Some(Offset { dx: 12, dy: -5 });
    let ids = write(&mut score, 0, 3 * Q, event).unwrap();
    assert_eq!(
        score.event(ids[0]).unwrap().notation.articulation,
        Some(Articulation::Staccato)
    );
    assert!(!score.event(ids[0]).unwrap().notation.fermata);
    assert_eq!(score.event(ids[0]).unwrap().notation.offsets.fermata, None);
    assert_eq!(score.event(ids[1]).unwrap().notation.articulation, None);
    assert!(score.event(ids[1]).unwrap().notation.fermata);
    assert_eq!(
        score.mark_offset(MarkRef::Fermata(ids[1])),
        Offset { dx: 12, dy: -5 }
    );
    assert_tied(&score, &ids);
}

#[test]
fn writing_past_the_maximum_measure_count_is_atomic() {
    let mut score = score_with_bars(MAX_MEASURES);
    let before = score.clone();
    let error = write(
        &mut score,
        MAX_MEASURES - 1,
        3 * Q,
        note(Step::C, Duration::Half),
    )
    .unwrap_err();
    assert_eq!(error.0, "Se alcanzó el límite de compases de esta versión");
    assert_eq!(score, before);
    score.validate().unwrap();
}

#[test]
fn the_second_tied_piece_replaces_only_the_first_next_bar_note() {
    let mut score = score_with_bars(2);
    let old = quarters(&mut score, 1, &[Step::E, Step::F, Step::G, Step::A]);
    let ids = write(&mut score, 0, 3 * Q, note(Step::C, Duration::Half)).unwrap();
    let events = voice(&score, 1);
    assert_eq!(events.len(), 4);
    assert_eq!(events[0].0, ids[1]);
    assert_eq!(
        [events[1].0, events[2].0, events[3].0],
        [old[1], old[2], old[3]]
    );
    assert_tied(&score, &ids);
}

#[test]
fn artificial_harmonic_and_offset_are_kept_on_each_tied_piece() {
    let mut score = score_with_bars(1);
    let mut event = note(Step::C, Duration::Half);
    event.notation.harmonic = Some(Harmonic::Artificial);
    event.notation.offsets.harmonic = Some(Offset { dx: 8, dy: -14 });
    event.notation.fermata = true;
    event.notation.offsets.fermata = Some(Offset { dx: 11, dy: 9 });
    let ids = write(&mut score, 0, 3 * Q, event).unwrap();
    for &id in &ids {
        assert_eq!(
            score.event(id).unwrap().notation.harmonic,
            Some(Harmonic::Artificial)
        );
        assert_eq!(
            score.mark_offset(MarkRef::Harmonic(id)),
            Offset { dx: 8, dy: -14 }
        );
    }
    assert!(!score.event(ids[0]).unwrap().notation.fermata);
    assert_eq!(
        score.mark_offset(MarkRef::Fermata(ids[1])),
        Offset { dx: 11, dy: 9 }
    );
    assert_tied(&score, &ids);
}

#[test]
fn an_existing_crossbar_tie_is_reused_after_both_ends_are_replaced() {
    let mut score = score_with_bars(2);
    quarters(&mut score, 0, &[Step::D, Step::E, Step::F]);
    let old_first = score
        .insert(0, 3, note(Step::C, Duration::Quarter))
        .unwrap();
    let old_second = score
        .insert(1, 0, note(Step::C, Duration::Quarter))
        .unwrap();
    let tie = Connection {
        start: old_first,
        end: old_second,
        kind: ConnectionKind::Tie,
    };
    let slur = Connection {
        kind: ConnectionKind::Slur,
        ..tie
    };
    score.add_connection(tie).unwrap();
    score.add_connection(slur).unwrap();
    let offset = Offset { dx: 13, dy: -7 };
    score
        .set_mark_offset(MarkRef::Connection(slur), offset)
        .unwrap();
    let ids = write(&mut score, 0, 3 * Q, note(Step::C, Duration::Half)).unwrap();
    let new_tie = Connection {
        start: ids[0],
        end: ids[1],
        kind: ConnectionKind::Tie,
    };
    let new_slur = Connection {
        kind: ConnectionKind::Slur,
        ..new_tie
    };
    assert_eq!(
        score
            .connections()
            .iter()
            .filter(|&&c| c == new_tie)
            .count(),
        1
    );
    assert!(score.connections().contains(&new_slur));
    assert_eq!(score.mark_offset(MarkRef::Connection(new_slur)), offset);
    assert_tied(&score, &ids);
}

fn quintuplet(value: Duration) -> Duration {
    value.with_tuplet(Tuplet::usual(5, NoteValue::Sixteenth))
}

/// Each event of the first bar: its onset, duration and whether it sounds.
fn bar(score: &Score) -> Vec<(u32, Duration, bool)> {
    score.measures()[0]
        .timed_iter()
        .filter(|(voice, ..)| *voice == Voice::One)
        .map(|(_, tick, _, event)| (tick, event.duration, !event.pitches.is_empty()))
        .collect()
}

fn group_sizes(score: &Score) -> Vec<usize> {
    score.measures()[0]
        .tuplet_groups(Voice::One)
        .iter()
        .map(|group| group.events.len())
        .collect()
}

#[test]
fn writing_a_tuplet_value_fills_the_rest_of_its_group_with_its_rests() {
    let mut score = Score::default();
    let sixteenth = quintuplet(Duration::Sixteenth);
    write(&mut score, 0, 0, note(Step::C, sixteenth)).unwrap();
    // The fewest rests, as with other gaps: a quarter in the quintuplet.
    assert_eq!(
        bar(&score),
        [
            (0, sixteenth, true),
            (Q / 5, quintuplet(Duration::Quarter), false)
        ]
    );
    assert_eq!(group_sizes(&score), [2]);
    assert_eq!(score.measures()[0].voice_ticks(Voice::One), Q);
    score.validate().unwrap();
}

#[test]
fn writing_inside_a_group_with_the_same_tuplet_overwrites_within_it() {
    let mut score = Score::default();
    let sixteenth = quintuplet(Duration::Sixteenth);
    write(&mut score, 0, 0, note(Step::C, sixteenth)).unwrap();
    write(&mut score, 0, Q / 5, note(Step::D, sixteenth)).unwrap();
    let events = bar(&score);
    assert_eq!(events.iter().filter(|e| e.2).count(), 2);
    assert_eq!((events[1].0, events[1].2), (Q / 5, true));
    assert_eq!(group_sizes(&score), [events.len()]);
    assert_eq!(score.measures()[0].voice_ticks(Voice::One), Q);
}

#[test]
fn gaps_inside_a_group_are_filled_with_its_rests() {
    let mut score = Score::default();
    write(
        &mut score,
        0,
        0,
        note(Step::C, quintuplet(Duration::Eighth)),
    )
    .unwrap();
    // An eighth of the five, then three sixteenths' worth of rests.
    assert_eq!(group_sizes(&score), [3]);
    write(
        &mut score,
        0,
        0,
        note(Step::D, quintuplet(Duration::Sixteenth)),
    )
    .unwrap();
    let events = bar(&score);
    assert_eq!(events[1], (Q / 5, quintuplet(Duration::Sixteenth), false));
    assert_eq!(group_sizes(&score).iter().sum::<usize>(), events.len());
    assert_eq!(score.measures()[0].voice_ticks(Voice::One), Q);
}

#[test]
fn a_value_that_would_pass_the_end_of_its_group_is_refused() {
    let mut score = Score::default();
    write(
        &mut score,
        0,
        0,
        note(Step::C, quintuplet(Duration::Sixteenth)),
    )
    .unwrap();
    let before = score.clone();
    let error = write(
        &mut score,
        0,
        4 * Q / 5,
        note(Step::D, quintuplet(Duration::Eighth)),
    )
    .unwrap_err();
    assert_eq!(error.0, "La figura no cabe en el grupillo");
    assert_eq!(score, before);
}

#[test]
fn a_plain_value_on_a_groups_first_event_replaces_the_whole_group() {
    let mut score = Score::default();
    write(
        &mut score,
        0,
        0,
        note(Step::C, quintuplet(Duration::Sixteenth)),
    )
    .unwrap();
    write(&mut score, 0, 0, note(Step::D, Duration::Eighth)).unwrap();
    assert_eq!(
        bar(&score),
        [
            (0, Duration::Eighth, true),
            (Q / 2, Duration::Eighth, false)
        ]
    );
    assert!(group_sizes(&score).is_empty());
}

#[test]
fn writing_a_plain_value_strictly_inside_a_group_is_refused() {
    let mut score = Score::default();
    write(
        &mut score,
        0,
        0,
        note(Step::C, quintuplet(Duration::Sixteenth)),
    )
    .unwrap();
    let before = score.clone();
    let error = write(&mut score, 0, Q / 5, note(Step::D, Duration::Sixteenth)).unwrap_err();
    assert!(error.0.starts_with("Dentro de un grupillo"), "{}", error.0);
    assert_eq!(score, before);
}

#[test]
fn a_new_group_cannot_reach_past_the_barline() {
    let mut score = Score::default();
    let error = write(
        &mut score,
        0,
        7 * Q / 2,
        note(Step::C, Duration::Eighth.with_triplet(true)),
    )
    .unwrap_err();
    assert_eq!(error.0, "Un grupillo no puede cruzar la barra de compás");
}

#[test]
fn a_value_that_cuts_a_group_is_refused_and_one_that_covers_it_replaces_it() {
    let mut score = Score::default();
    write(
        &mut score,
        0,
        Q,
        note(Step::C, quintuplet(Duration::Sixteenth)),
    )
    .unwrap();
    let before = score.clone();
    let dotted = note(Step::D, Duration::Quarter.with_dotted(true));
    let error = write(&mut score, 0, 0, dotted).unwrap_err();
    assert!(
        error.0.starts_with("La figura corta un grupillo"),
        "{}",
        error.0
    );
    assert_eq!(score, before);
    write(&mut score, 0, 0, note(Step::D, Duration::Half)).unwrap();
    assert!(group_sizes(&score).is_empty());
    assert_eq!(score.measures()[0].voice_ticks(Voice::One), 2 * Q);
}
