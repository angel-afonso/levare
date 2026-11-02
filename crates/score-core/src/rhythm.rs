//! Spelling a span of time as written values: generated rests, and notes cut
//! at a barline, a staff change or a divisi point.
use crate::{Duration, NoteValue, TimeSignature};
use std::cmp::Reverse;

/// The values a span may be spelled with, longest first. Tables are indexed
/// in steps of the values' greatest common divisor rather than in ticks, so
/// their size does not grow with the resolution of time.
#[derive(Clone, Debug)]
pub struct Palette {
    durations: Vec<Duration>,
    step: u32,
}

impl Palette {
    pub fn new(durations: impl IntoIterator<Item = Duration>) -> Self {
        let mut durations: Vec<_> = durations.into_iter().collect();
        durations.sort_by_key(|d| (Reverse(d.ticks()), d.is_triplet(), d.is_dotted()));
        // A dotted triplet lasts as long as its plain value, which sorts first.
        durations.dedup_by_key(|d| d.ticks());
        let step = durations.iter().map(|d| d.ticks()).fold(0, gcd);
        Self { durations, step }
    }
    /// Plain, dotted and triplet values.
    pub fn modified() -> Self {
        Self::new(spelled().flat_map(|d| [d, d.with_dotted(true), d.with_triplet(true)]))
    }
    pub fn plain() -> Self {
        Self::new(spelled())
    }
    /// Plain values inside one tuplet, down to sixty-fourths.
    pub fn in_tuplet(tuplet: crate::Tuplet) -> Self {
        Self::new(Duration::ALL.map(|d| d.with_tuplet(Some(tuplet))))
    }
    pub fn durations(&self) -> &[Duration] {
        &self.durations
    }

    /// The fewest values that add up to `ticks`; among equally short
    /// spellings, the one whose values come longest first.
    pub fn fewest(&self, ticks: u32) -> Option<Vec<Duration>> {
        let n = self.steps(ticks)?;
        // For each length in steps: how many values spell it, and the first.
        let mut best: Vec<Option<(usize, Duration)>> = vec![None; n + 1];
        best[0] = Some((0, Duration::Quarter));
        for i in 1..=n {
            for &d in &self.durations {
                let len = self.len(d);
                if len <= i
                    && let Some((count, _)) = best[i - len]
                    && best[i].is_none_or(|(old, _)| count + 1 < old)
                {
                    best[i] = Some((count + 1, d));
                }
            }
        }
        let mut remaining = n;
        let mut durations = Vec::new();
        while remaining > 0 {
            let (_, d) = best[remaining]?;
            durations.push(d);
            remaining -= self.len(d);
        }
        Some(durations)
    }

    /// The longest value that can open `ticks` and still leave a remainder
    /// this palette spells, so a split never runs into a dead end.
    pub fn longest_fitting(&self, ticks: u32) -> Option<Duration> {
        let n = self.steps(ticks)?;
        let mut reachable = vec![false; n + 1];
        reachable[0] = true;
        for i in 1..=n {
            reachable[i] = self.durations.iter().any(|&d| {
                let len = self.len(d);
                len <= i && reachable[i - len]
            });
        }
        self.durations.iter().copied().find(|&d| {
            let len = self.len(d);
            len <= n && reachable[n - len]
        })
    }

    fn steps(&self, ticks: u32) -> Option<usize> {
        (self.step != 0 && ticks.is_multiple_of(self.step))
            .then(|| usize::try_from(ticks / self.step).ok())
            .flatten()
    }
    fn len(&self, d: Duration) -> usize {
        (d.ticks() / self.step) as usize
    }
}

/// A stretch of a bar that belongs to one tuplet group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TupletSpan {
    pub start: u32,
    pub end: u32,
    pub tuplet: crate::Tuplet,
}

impl From<&crate::TupletGroup> for TupletSpan {
    fn from(group: &crate::TupletGroup) -> Self {
        Self {
            start: group.start,
            end: group.end,
            tuplet: group.tuplet,
        }
    }
}

/// Rests for a gap, with their onsets: in the tuplet of each group it falls
/// in, and plain values by the beats of `time` elsewhere.
pub(crate) fn rests_for_gap(
    time: TimeSignature,
    start: u32,
    ticks: u32,
    spans: &[TupletSpan],
) -> Result<Vec<(u32, Duration)>, crate::ScoreError> {
    let end = start + ticks;
    let mut at = start;
    let mut rests = Vec::new();
    while at < end {
        let span = spans.iter().find(|s| s.start <= at && at < s.end);
        let stop = match span {
            Some(span) => span.end.min(end),
            None => spans
                .iter()
                .map(|s| s.start)
                .filter(|&s| s > at)
                .min()
                .unwrap_or(end)
                .min(end),
        };
        let durations = match span {
            Some(span) => Palette::in_tuplet(span.tuplet).fewest(stop - at),
            None => rests_by_beat(time, at, stop - at),
        }
        .ok_or(crate::ScoreError(
            "Ese desplazamiento crea un silencio que todavía no se puede representar",
        ))?;
        for duration in durations {
            rests.push((at, duration));
            at += duration.ticks();
        }
    }
    Ok(rests)
}

/// Rests for `ticks` from `start` in a bar of `time`, written as Sibelius,
/// Dorico and MuseScore write them: no rest crosses a beat, except a half
/// rest on either half of a 4/4 bar (a dotted half in 12/8), and inside a
/// beat each rest starts on a multiple of its own value, so the beat's
/// subdivision shows. In compound time a rest may finish its beat from any
/// eighth: an eighth note and a quarter rest. Spans off that grid, such as
/// what a triplet leaves, take the fewest values.
pub fn rests_by_beat(time: TimeSignature, start: u32, ticks: u32) -> Option<Vec<Duration>> {
    let unit = crate::TICKS_PER_WHOLE / u32::from(time.denominator.max(1));
    let compound = time.denominator >= 8 && time.numerator > 3 && time.numerator.is_multiple_of(3);
    let beat = if compound { 3 * unit } else { unit };
    // Two beats under one rest: halves of 4/4 (or 4/2, 4/8) and of 12/8.
    let pairs = (!compound && time.numerator == 4) || (compound && time.numerator == 12);
    let value_of = |length: u32| {
        Duration::ALL
            .into_iter()
            .flat_map(|d| [d, d.with_dotted(true)])
            .find(|d| d.ticks() == length)
    };
    // A whole empty 4/4 bar is one whole rest.
    if start == 0 && ticks == time.capacity() && ticks == crate::TICKS_PER_WHOLE {
        return Some(vec![Duration::Whole]);
    }
    let end = start + ticks;
    let mut at = start;
    let mut rests = Vec::new();
    while at < end {
        let beat_start = at - at % beat;
        if at == beat_start {
            let pair = 2 * beat;
            if pairs
                && at.is_multiple_of(pair)
                && at + pair <= end
                && let Some(rest) = value_of(pair)
            {
                rests.push(rest);
                at += pair;
                continue;
            }
            if at + beat <= end
                && let Some(rest) = value_of(beat)
            {
                rests.push(rest);
                at += beat;
                continue;
            }
        }
        let stop = end.min(beat_start + beat);
        // Finishing a compound beat in one rest: an eighth and a quarter rest.
        if compound
            && stop == beat_start + beat
            && let Some(rest) = Duration::ALL.into_iter().find(|d| d.ticks() == stop - at)
        {
            rests.push(rest);
            at = stop;
            continue;
        }
        let within = at - beat_start;
        let grid = Duration::SixtyFourth.ticks();
        if !within.is_multiple_of(grid) || !(stop - at).is_multiple_of(grid) {
            // Off the plain grid: whatever spells the rest of the beat.
            rests.extend(Palette::modified().fewest(stop - at)?);
            at = stop;
            continue;
        }
        // On the grid of sixty-fourths a value aligned to itself always fits.
        let rest = Duration::ALL
            .into_iter()
            .find(|d| within.is_multiple_of(d.ticks()) && at + d.ticks() <= stop)
            .unwrap();
        rests.push(rest);
        at += rest.ticks();
    }
    Some(rests)
}

/// Spans are spelled without sixty-fourths: with them, a dotted thirty-second
/// and a thirty-second would become a sixteenth and a sixty-fourth.
fn spelled() -> impl Iterator<Item = Duration> {
    Duration::ALL
        .into_iter()
        .filter(|d| d.value() != NoteValue::SixtyFourth)
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

#[cfg(test)]
mod tests;
