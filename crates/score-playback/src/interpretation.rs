//! Performance rules shared by the complete plan and the brief audition. Sound
//! banks receive their results and never reinterpret accents or note lengths.
use std::collections::HashMap;

use score_core::{
    Articulation, ConnectionKind, PartId, Score, StaffId, TICKS_PER_WHOLE, TimeSignature,
};

/// Written length kept by a note without a slur or a length articulation.
pub(crate) const DETACHED_PERCENT: u64 = 92;
/// Upper bound of the detached release gap, so long notes do not leave holes.
pub(crate) const DETACHED_MAX_GAP_MS: u64 = 60;
/// A slurred staccato (portato) keeps more of its length than a plain staccato.
pub(crate) const PORTATO_PERCENT: u64 = 75;
pub(crate) const STACCATO_PERCENT: u64 = 50;
/// Connected notes under a slur overlap slightly, like a legato attack.
pub(crate) const LEGATO_OVERLAP_MS: u64 = 25;
// Small attack weights; expression CC11 remains on its written envelope.
const DOWNBEAT_BOOST: i8 = 4;
const BEAT_BOOST: i8 = 1;
const FOUR_FOUR_THIRD_BEAT_BOOST: i8 = 2;
const OFFBEAT_REDUCTION: i8 = -2;
// A phrase's pitch peak and unaccented final release.
const PHRASE_PEAK_BOOST: u16 = 4;
const PHRASE_END_REDUCTION: i8 = 3;
// Reproducible imprecision shared by a part's simultaneous attacks.
const HUMAN_MAX_DELAY_MS: u64 = 5;
const HUMAN_VELOCITY_SPREAD: u64 = 3;

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

/// Written downbeats, beats and subdivisions have a small velocity weight.
#[cfg(test)]
pub(crate) fn metric_offset(score: &Score, written_tick: u64) -> i8 {
    let (measure, within) = score.measure_at_tick(written_tick);
    metric_offset_in_bar(score.time_at(measure), within)
}

fn metric_offset_in_bar(time: TimeSignature, within: u16) -> i8 {
    if within == 0 {
        return DOWNBEAT_BOOST;
    }
    let mut pulse = TICKS_PER_WHOLE / u16::from(time.denominator);
    if time.denominator == 8 && [6, 9, 12].contains(&time.numerator) {
        pulse *= 3;
    }
    if !within.is_multiple_of(pulse) {
        OFFBEAT_REDUCTION
    } else if time.numerator == 4 && time.denominator == 4 && within == pulse * 2 {
        FOUR_FOUR_THIRD_BEAT_BOOST
    } else {
        BEAT_BOOST
    }
}

/// A phrase peaks at its highest pitch and releases its final unaccented note.
pub(crate) fn phrase_offsets(
    highest_keys: &[u8],
    final_articulation: Option<Articulation>,
) -> Vec<i8> {
    let mut offsets = vec![0; highest_keys.len()];
    if highest_keys.len() < 3 {
        return offsets;
    }
    let low = *highest_keys.iter().min().unwrap();
    let high = *highest_keys.iter().max().unwrap();
    if high > low {
        let range = u16::from(high - low);
        for (&key, offset) in highest_keys.iter().zip(&mut offsets) {
            *offset = ((PHRASE_PEAK_BOOST * u16::from(key - low) + range / 2) / range) as i8;
        }
    }
    if !matches!(
        final_articulation,
        Some(Articulation::Accent | Articulation::Marcato)
    ) {
        *offsets.last_mut().unwrap() -= PHRASE_END_REDUCTION;
    }
    offsets
}

fn shape_phrase(sounds: &[super::SoundSpan], indices: &[usize], offsets: &mut [i16]) {
    if indices.len() < 3 {
        return;
    }
    let highest: Vec<u8> = indices
        .iter()
        .map(|&index| {
            sounds[index]
                .pitches
                .iter()
                .map(super::midi_key)
                .max()
                .unwrap()
        })
        .collect();
    let final_articulation = sounds[*indices.last().unwrap()].articulation;
    for (&index, offset) in indices
        .iter()
        .zip(phrase_offsets(&highest, final_articulation))
    {
        offsets[index] += i16::from(offset);
    }
}

/// Fixed, portable mixing: neither IDs nor randomized map seeds affect a performance.
fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

pub(crate) fn human_variation(part_index: usize, written_tick: u64) -> (u64, i16) {
    let seed = splitmix64((part_index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ written_tick);
    let delay_ms = seed % (HUMAN_MAX_DELAY_MS + 1);
    let velocity =
        (splitmix64(seed) % (HUMAN_VELOCITY_SPREAD * 2 + 1)) as i16 - HUMAN_VELOCITY_SPREAD as i16;
    (delay_ms, velocity)
}

fn shorten_repeated_keys(sounds: &mut [super::SoundSpan]) {
    let mut by_key: HashMap<(u8, u8), Vec<usize>> = HashMap::new();
    for (index, sound) in sounds.iter().enumerate() {
        for pitch in sound.pitches.iter() {
            by_key
                .entry((sound.channel, super::midi_key(pitch)))
                .or_default()
                .push(index);
        }
    }
    for indices in by_key.values_mut() {
        indices.sort_by_key(|&index| sounds[index].start);
        for pair in indices.windows(2) {
            let (before, after) = (pair[0], pair[1]);
            if !sounds[before].slurred && sounds[before].start < sounds[after].start {
                sounds[before].end = sounds[before].end.min(sounds[after].start);
            }
        }
    }
}

/// Natural shaping runs after dynamic expression and before arpeggio grouping.
pub(crate) fn shape(
    sounds: &mut [super::SoundSpan],
    score: &Score,
    timeline: &super::Timeline,
    rate: u32,
    tracks: &[super::Track],
) {
    let mut starts = Vec::with_capacity(score.measures().len() + 1);
    starts.push(0u64);
    for measure in 0..score.measures().len() {
        starts.push(starts.last().copied().unwrap() + u64::from(score.time_at(measure).capacity()));
    }
    let mut offsets = vec![0i16; sounds.len()];
    for (index, sound) in sounds.iter().enumerate() {
        if sound.tremolo_repeat {
            continue;
        }
        let measure = starts.partition_point(|start| *start <= sound.written_range.0) - 1;
        let within = (sound.written_range.0 - starts[measure]) as u16;
        let metric = metric_offset_in_bar(score.time_at(measure), within);
        offsets[index] += i16::from(metric);
    }
    // A slur over an omitted ending does not bridge the performed jump.
    let forward_jumps: Vec<_> = timeline
        .instances()
        .windows(2)
        .enumerate()
        .filter(|(_, pair)| pair[1].bar > pair[0].bar + 1)
        .map(|(index, _)| timeline.performed_frame(index + 1, 0, rate))
        .collect();
    for track in tracks {
        let mut phrase: Vec<usize> = Vec::new();
        let slurs = Slurs::new(score, track.part, track.staff, track.voice);
        for index in track.sounds.clone() {
            if sounds[index].tremolo_repeat {
                continue;
            }
            if phrase.last().is_some_and(|&previous| {
                let previous_end = sounds[previous].written_range.1;
                let next_start = sounds[index].written_range.0;
                let connected = previous_end == next_start
                    || (previous_end < next_start
                        && !forward_jumps.iter().any(|&frame| {
                            sounds[previous].start < frame && frame <= sounds[index].start
                        })
                        && slurs
                            .find(sounds[previous].written_range.0)
                            .is_some_and(|range| {
                                range.0 <= sounds[previous].written_range.0 && next_start <= range.1
                            }));
                !connected
            }) {
                shape_phrase(sounds, &phrase, &mut offsets);
                phrase.clear();
            }
            phrase.push(index);
        }
        shape_phrase(sounds, &phrase, &mut offsets);
    }
    let part_indices: HashMap<PartId, usize> = score
        .parts()
        .iter()
        .enumerate()
        .map(|(index, part)| (part.id(), index))
        .collect();
    let duration = timeline.total_frames(rate);
    for (index, sound) in sounds.iter_mut().enumerate() {
        let part_index = part_indices[&sound.element.part];
        let (delay_ms, velocity) = human_variation(part_index, sound.written_range.0);
        offsets[index] += velocity;
        let delayed_start = sound
            .start
            .saturating_add(u64::from(rate) * delay_ms / 1000)
            .min(duration);
        let applied_delay = delayed_start - sound.start;
        sound.start = delayed_start;
        sound.end = sound.end.saturating_add(applied_delay);
        sound.written_end = sound.written_end.saturating_add(applied_delay);
    }
    shorten_repeated_keys(sounds);
    for (sound, offset) in sounds.iter_mut().zip(offsets) {
        sound.level = (i16::from(sound.level) + offset).clamp(1, 127) as u8;
    }
}

/// Resolve measured tremolo before expression calibration and key scheduling.
pub(crate) fn expand_tremolos(sounds: &mut Vec<super::SoundSpan>, from: usize, rate: u32) {
    if sounds[from..].iter().all(|sound| sound.tremolo.is_none()) {
        return;
    }
    let original = sounds.split_off(from);
    for sound in original {
        let repetitions = sound.tremolo.map_or(1, |mark| {
            sound.written_ticks / u64::from(mark.repetition_ticks())
        });
        if repetitions < 2 {
            sounds.push(sound);
            continue;
        }
        let duration = sound.written_end - sound.start;
        for repetition in 0..repetitions {
            let start = sound.start + duration * repetition / repetitions;
            let next = sound.start + duration * (repetition + 1) / repetitions;
            let unmeasured = sound.tremolo == Some(score_core::Tremolo::Three);
            let mut attack = super::SoundSpan {
                start,
                end: next - legato_overlap(rate).min((next - start) / 4),
                attack_duration: next - start,
                written_end: next,
                slurred: false,
                tremolo_repeat: repetition > 0,
                tremolo_attack: unmeasured.then_some(match repetition {
                    0 => super::TremoloAttack::First,
                    last if last + 1 == repetitions => super::TremoloAttack::Last,
                    _ => super::TremoloAttack::Repeat,
                }),
                ..sound
            };
            if repetition > 0 {
                attack.boost = 0;
                attack.level = attack.reference_level;
                attack.articulation = None;
            }
            sounds.push(attack);
        }
    }
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
