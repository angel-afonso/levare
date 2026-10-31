use super::*;

/// Largest supported bar (16/1), the longest span any caller spells.
const LONGEST: usize = 16 * crate::TICKS_PER_WHOLE as usize;

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
        for &d in &durations {
            let length = usize::from(d.ticks());
            if length <= tick
                && let Some(rest) = &result[tick - length]
                && best.as_ref().is_none_or(|b| b.len() > rest.len() + 1)
            {
                let mut parts = vec![d];
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
        representable[tick] = durations.iter().any(|d| {
            usize::from(d.ticks()) <= tick && representable[tick - usize::from(d.ticks())]
        });
    }
    (0..=capacity)
        .map(|ticks| {
            durations.iter().copied().find(|d| {
                usize::from(d.ticks()) <= ticks && representable[ticks - usize::from(d.ticks())]
            })
        })
        .collect()
}

fn palettes() -> [(&'static str, Vec<Duration>, Palette); 3] {
    [
        ("plain", Duration::ALL.to_vec(), Palette::plain()),
        (
            "triplets",
            Duration::ALL.map(|d| d.with_triplet(true)).to_vec(),
            Palette::triplets(),
        ),
        ("modified", Duration::all_modified(), Palette::modified()),
    ]
}

#[test]
fn fewest_matches_the_old_tables_for_every_span() {
    for (name, durations, palette) in palettes() {
        let old = old_fewest(durations, LONGEST);
        for (ticks, expected) in old.into_iter().enumerate() {
            assert_eq!(
                palette.fewest(ticks as u32),
                expected,
                "{name} palette, {ticks} ticks"
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
                palette.longest_fitting(ticks as u32),
                expected,
                "{name} palette, {ticks} ticks"
            );
        }
    }
}

#[test]
fn generated_rests_spell_with_the_full_palette() {
    // Rests used plain, dotted and triplet values; the dotted triplets that
    // `Duration::all_modified` adds last as long as plain values.
    let rests = Palette::new(
        Duration::ALL
            .into_iter()
            .flat_map(|d| [d, d.with_dotted(true), d.with_triplet(true)]),
    );
    assert_eq!(rests.durations(), Palette::modified().durations());
}

#[test]
fn unreachable_spans_have_no_spelling() {
    let plain = Palette::plain();
    assert_eq!(plain.fewest(0), Some(Vec::new()));
    assert_eq!(plain.longest_fitting(0), None);
    // Shorter than a thirty-second, or not a whole number of them.
    for ticks in [1, 5, 7, 50] {
        assert_eq!(plain.fewest(ticks), None, "{ticks} ticks");
        assert_eq!(plain.longest_fitting(ticks), None, "{ticks} ticks");
    }
    // A triplet thirty-second is the shortest value; half of it is not.
    assert_eq!(Palette::modified().fewest(2), None);
}

#[test]
fn tables_count_steps_not_ticks() {
    // A whole 16/1 bar of plain values: one step per thirty-second.
    let plain = Palette::plain();
    assert_eq!(plain.step, u32::from(Duration::ThirtySecond.ticks()));
    assert_eq!(
        plain.fewest(LONGEST as u32),
        Some(vec![Duration::Whole; 16])
    );
}
