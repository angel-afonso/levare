use super::*;
use crate::TICKS_PER_WHOLE;

/// The tables below ran at 192 ticks per whole note; they are kept in that
/// unit, and so is the longest span any caller spells, a 16/1 bar.
const OLD_SCALE: u32 = TICKS_PER_WHOLE / 192;
const LONGEST: usize = 16 * 192;

fn old_ticks(d: &Duration) -> usize {
    (d.ticks() / OLD_SCALE) as usize
}

/// Whole to thirty-second, the values that existed with those tables.
fn old_values() -> Vec<Duration> {
    Duration::ALL[..6].to_vec()
}

fn sorted(mut durations: Vec<Duration>) -> Vec<Duration> {
    durations.sort_by_key(|d| (Reverse(d.ticks()), d.is_triplet(), d.is_dotted()));
    durations
}

/// The tick-indexed table that rests, divisi cuts and MusicXML splits used
/// before `Palette`: for every span up to `capacity`, its fewest values.
fn old_fewest(durations: Vec<Duration>, capacity: usize) -> Vec<Option<Vec<Duration>>> {
    let durations = sorted(durations);
    let mut result = vec![None; capacity + 1];
    result[0] = Some(Vec::new());
    for tick in 1..=capacity {
        let mut best: Option<Vec<Duration>> = None;
        for d in &durations {
            let length = old_ticks(d);
            if length <= tick
                && let Some(rest) = &result[tick - length]
                && best.as_ref().is_none_or(|b| b.len() > rest.len() + 1)
            {
                let mut parts = vec![*d];
                parts.extend(rest);
                best = Some(parts);
            }
        }
        result[tick] = best;
    }
    result
}

/// The paste splitter used before `Palette`: the first piece of each span.
fn old_longest_fitting(durations: Vec<Duration>, capacity: usize) -> Vec<Option<Duration>> {
    let durations = sorted(durations);
    let mut representable = vec![false; capacity + 1];
    representable[0] = true;
    for tick in 1..=capacity {
        representable[tick] = durations
            .iter()
            .any(|d| old_ticks(d) <= tick && representable[tick - old_ticks(d)]);
    }
    (0..=capacity)
        .map(|ticks| {
            durations
                .iter()
                .copied()
                .find(|d| old_ticks(d) <= ticks && representable[ticks - old_ticks(d)])
        })
        .collect()
}

fn palettes() -> [(&'static str, Vec<Duration>, Palette); 2] {
    let modified = old_values()
        .into_iter()
        .flat_map(|d| {
            [
                d,
                d.with_dotted(true),
                d.with_triplet(true),
                d.with_dotted(true).with_triplet(true),
            ]
        })
        .collect();
    [
        ("plain", old_values(), Palette::plain()),
        ("modified", modified, Palette::modified()),
    ]
}

#[test]
fn fewest_matches_the_old_tables_for_every_span() {
    for (name, durations, palette) in palettes() {
        let old = old_fewest(durations, LONGEST);
        for (ticks, expected) in old.into_iter().enumerate() {
            assert_eq!(
                palette.fewest(ticks as u32 * OLD_SCALE),
                expected,
                "{name} palette, {ticks} old ticks"
            );
        }
    }
}

#[test]
fn longest_fitting_matches_the_old_paste_splitter_for_every_span() {
    for (name, durations, palette) in palettes() {
        let old = old_longest_fitting(durations, LONGEST);
        for (ticks, expected) in old.into_iter().enumerate() {
            assert_eq!(
                palette.longest_fitting(ticks as u32 * OLD_SCALE),
                expected,
                "{name} palette, {ticks} old ticks"
            );
        }
    }
}

#[test]
fn spans_are_spelled_without_sixty_fourths() {
    for palette in [Palette::plain(), Palette::modified()] {
        assert!(
            palette
                .durations()
                .iter()
                .all(|d| d.value() != NoteValue::SixtyFourth)
        );
    }
}

#[test]
fn unreachable_spans_have_no_spelling() {
    let plain = Palette::plain();
    let thirty_second = Duration::ThirtySecond.ticks();
    assert_eq!(plain.fewest(0), Some(Vec::new()));
    assert_eq!(plain.longest_fitting(0), None);
    // Shorter than a thirty-second, or not a whole number of them.
    for ticks in [
        1,
        thirty_second / 2,
        thirty_second * 3 / 2,
        thirty_second * 8 + 1,
    ] {
        assert_eq!(plain.fewest(ticks), None, "{ticks} ticks");
        assert_eq!(plain.longest_fitting(ticks), None, "{ticks} ticks");
    }
    // A triplet thirty-second is the shortest value; half of it is not.
    let shortest = Duration::ThirtySecond.with_triplet(true).ticks();
    assert_eq!(Palette::modified().fewest(shortest / 2), None);
}

#[test]
fn tables_count_steps_not_ticks() {
    // A whole 16/1 bar of plain values: one step per thirty-second.
    let plain = Palette::plain();
    assert_eq!(plain.step, Duration::ThirtySecond.ticks());
    assert_eq!(
        plain.fewest(16 * TICKS_PER_WHOLE),
        Some(vec![Duration::Whole; 16])
    );
}
