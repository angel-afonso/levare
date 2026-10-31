//! Spelling a span of time as written values: generated rests, and notes cut
//! at a barline, a staff change or a divisi point.
use crate::Duration;
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
    /// Every value Levare writes: plain, dotted and triplet.
    pub fn modified() -> Self {
        Self::new(Duration::all_modified())
    }
    pub fn plain() -> Self {
        Self::new(Duration::ALL)
    }
    pub fn triplets() -> Self {
        Self::new(Duration::ALL.map(|d| d.with_triplet(true)))
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

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

#[cfg(test)]
mod tests;
