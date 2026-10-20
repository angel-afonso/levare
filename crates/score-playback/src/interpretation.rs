//! Performance rules shared by the complete plan and the brief audition. Sound
//! banks receive their results and never reinterpret accents or note lengths.
use score_core::{Articulation, ConnectionKind, PartId, Score, StaffId};

/// Written length kept by a note without a slur or a length articulation.
pub(crate) const DETACHED_PERCENT: u64 = 92;
/// Upper bound of the detached release gap, so long notes do not leave holes.
pub(crate) const DETACHED_MAX_GAP_MS: u64 = 60;
/// A slurred staccato (portato) keeps more of its length than a plain staccato.
pub(crate) const PORTATO_PERCENT: u64 = 75;
pub(crate) const STACCATO_PERCENT: u64 = 50;
/// Connected notes under a slur overlap slightly, like a legato attack.
pub(crate) const LEGATO_OVERLAP_MS: u64 = 25;

pub(crate) fn accent_boost(articulation: Option<Articulation>) -> u8 {
    match articulation {
        Some(Articulation::Accent) => 16,
        Some(Articulation::Marcato) => 24,
        _ => 0,
    }
}

/// Attack intensity on the MIDI velocity scale: dynamic level plus accent.
pub(crate) fn level(dynamic: u8, articulation: Option<Articulation>) -> u8 {
    dynamic
        .saturating_add(accent_boost(articulation))
        .clamp(1, 127)
}

/// Sounding end of a written interval. `slurred` means the slur continues to
/// the following note; the plan then extends a full-length note into it.
pub(crate) fn sounding_end(
    start: u64,
    end: u64,
    articulation: Option<Articulation>,
    slurred: bool,
    rate: u32,
) -> u64 {
    let length = end - start;
    match (articulation, slurred) {
        (Some(Articulation::Staccato), true) => start + length * PORTATO_PERCENT / 100,
        (Some(Articulation::Staccato), false) => start + length * STACCATO_PERCENT / 100,
        (Some(Articulation::Tenuto), _) | (_, true) => end,
        _ => {
            let gap = (length * (100 - DETACHED_PERCENT) / 100)
                .min(u64::from(rate) * DETACHED_MAX_GAP_MS / 1000);
            end - gap
        }
    }
}

pub(crate) fn legato_overlap(rate: u32) -> u64 {
    u64::from(rate) * LEGATO_OVERLAP_MS / 1000
}

/// Tick ranges of the slurs on one staff, from the first to the last note.
#[derive(Default)]
pub(crate) struct Slurs(Vec<(u64, u64)>);
impl Slurs {
    pub(crate) fn new(
        score: &Score,
        part: PartId,
        staff: StaffId,
        voice: score_core::Voice,
    ) -> Self {
        let mut ranges: Vec<_> = score
            .connections()
            .iter()
            .filter(|c| {
                c.kind == ConnectionKind::Slur
                    && c.start.part == part
                    && c.start.staff == staff
                    && score.voice_of(c.start) == Some(voice)
            })
            .filter_map(|c| Some((score.event_tick(c.start)?, score.event_tick(c.end)?)))
            .collect();
        ranges.sort_unstable();
        Self(ranges)
    }
    fn find(&self, tick: u64) -> Option<(u64, u64)> {
        let index = self
            .0
            .partition_point(|range| range.0 <= tick)
            .checked_sub(1)?;
        let range = self.0[index];
        (tick <= range.1).then_some(range)
    }
    /// Any note from the first to the last one under the slur.
    pub(crate) fn contains(&self, tick: u64) -> bool {
        self.find(tick).is_some()
    }
    /// The slur continues past this note to a later one.
    pub(crate) fn continues(&self, tick: u64) -> bool {
        self.find(tick).is_some_and(|range| tick < range.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_lengths_follow_articulation_and_slur_with_a_bounded_detached_gap() {
        let rate = 48_000;
        let quarter = 24_000;
        assert_eq!(sounding_end(0, quarter, None, false, rate), 22_080);
        assert_eq!(
            sounding_end(0, quarter, Some(Articulation::Accent), false, rate),
            22_080
        );
        assert_eq!(
            sounding_end(0, quarter, Some(Articulation::Tenuto), false, rate),
            quarter
        );
        assert_eq!(sounding_end(0, quarter, None, true, rate), quarter);
        assert_eq!(
            sounding_end(0, quarter, Some(Articulation::Staccato), false, rate),
            12_000
        );
        assert_eq!(
            sounding_end(0, quarter, Some(Articulation::Staccato), true, rate),
            18_000
        );
        // Long notes lose at most 60 ms, not a proportional hole.
        assert_eq!(sounding_end(0, 192_000, None, false, rate), 192_000 - 2_880);
        assert_eq!(legato_overlap(rate), 1_200);
    }

    #[test]
    fn accents_raise_the_level_within_the_midi_range() {
        assert_eq!(level(56, None), 56);
        assert_eq!(level(56, Some(Articulation::Accent)), 72);
        assert_eq!(level(124, Some(Articulation::Marcato)), 127);
        assert_eq!(level(0, None), 1);
    }
}
