use super::*;

fn rest(duration: Duration) -> Event {
    Event {
        pitches: Pitches::default(),
        duration,
        notation: Notation::default(),
    }
}

/// The sizes of the groups in the first bar, and whether each is complete.
fn groups(durations: &[Duration]) -> Vec<(usize, bool)> {
    let mut score = Score::default();
    for (index, &duration) in durations.iter().enumerate() {
        score.insert(0, index, rest(duration)).unwrap();
    }
    score.measures()[0]
        .tuplet_groups(Voice::One)
        .iter()
        .map(|group| (group.events.len(), group.is_complete()))
        .collect()
}

fn in_eighths(duration: Duration) -> Duration {
    duration.with_tuplet(Some(Tuplet::triplet(NoteValue::Eighth)))
}

#[test]
fn old_triplet_runs_group_in_threes() {
    let eighth = Duration::Eighth.with_triplet(true);
    assert_eq!(groups(&[eighth; 7]), [(3, true), (3, true), (1, false)]);
}

#[test]
fn mixed_values_fill_one_group() {
    let quarter = in_eighths(Duration::Quarter);
    let eighth = in_eighths(Duration::Eighth);
    assert_eq!(groups(&[quarter, eighth]), [(2, true)]);
    assert_eq!(
        groups(&[eighth, quarter, eighth, eighth]),
        [(2, true), (2, false)]
    );
}

#[test]
fn another_ratio_unit_or_a_plain_value_starts_a_new_group() {
    let triplet = Duration::Eighth.with_triplet(true);
    let quintuplet = Duration::Sixteenth.with_tuplet(Tuplet::usual(5, NoteValue::Sixteenth));
    let quarter_triplet = Duration::Quarter.with_triplet(true);
    assert_eq!(
        groups(&[triplet, triplet, quintuplet, quintuplet]),
        [(2, false), (2, false)]
    );
    assert_eq!(
        groups(&[triplet, quarter_triplet]),
        [(1, false), (1, false)]
    );
    assert_eq!(
        groups(&[triplet, Duration::Eighth, triplet]),
        [(1, false), (1, false)]
    );
}

#[test]
fn an_event_that_would_overfill_the_group_starts_the_next() {
    let dotted = Duration::Eighth.with_dotted(true).with_triplet(true);
    assert_eq!(groups(&[dotted; 3]), [(2, true), (1, false)]);
}

#[test]
fn spans_measure_what_a_complete_group_writes_and_lasts() {
    let score = {
        let mut score = Score::default();
        let quintuplet = Duration::Sixteenth.with_tuplet(Tuplet::usual(5, NoteValue::Sixteenth));
        score.insert(0, 0, rest(Duration::Eighth)).unwrap();
        for index in 1..=5 {
            score.insert(0, index, rest(quintuplet)).unwrap();
        }
        score
    };
    let group = &score.measures()[0].tuplet_groups(Voice::One)[0];
    let quarter = Duration::Quarter.ticks();
    assert_eq!(group.tuplet.written_span(), 5 * quarter / 4);
    assert_eq!(group.tuplet.span(), quarter);
    assert_eq!((group.start, group.end), (quarter / 2, 3 * quarter / 2));
    assert_eq!(group.span_end(), group.end);
}
