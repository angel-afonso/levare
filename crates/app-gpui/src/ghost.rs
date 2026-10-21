//! The prospective note under the pointer while note entry is active.

use score_core::{Clef, Duration, PartId, Pitch, Score, StaffId, Step, Voice};
use score_layout::{Layout, Point};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Ghost {
    pub part: PartId,
    /// Canonical staff of the music, which can differ from the displayed row.
    pub staff: StaffId,
    pub measure: usize,
    pub voice: Voice,
    pub tick: u16,
    pub pitch: Pitch,
    pub duration: Duration,
    /// Notehead left edge and center in unscaled layout units.
    pub x: f32,
    pub y: f32,
    pub staff_top: f32,
}

pub(super) fn ghost_at(
    score: &Score,
    layout: &Layout,
    point: Point,
    preferred: Voice,
    duration: Duration,
) -> Option<Ghost> {
    let (part, display_staff, measure) = layout.measure_at(point)?;
    let placed = layout.measure_on(part, display_staff, measure)?;
    let local = Point {
        x: point.x / layout.scale - placed.origin.x,
        y: point.y / layout.scale - placed.origin.y,
    };
    let score_part = score.part(part)?;
    let clef = score_part.clef_at(placed.source_staff, measure);
    if clef == Clef::Percussion {
        return None;
    }
    let diatonic = clef.bottom_diatonic() + ((40. - local.y) / 5.).round() as i16;
    let pitch = Pitch::new(Step::C, 0)?.shift(diatonic)?;
    let (voice, _) = placed.insertion_near(preferred, local.x);
    let bar = &score_part.staff(placed.source_staff)?.measures()[measure];
    let capacity = score.time_at(measure).capacity();
    let step = usize::from(duration.ticks());
    let tick = placed.source_onsets[usize::from(voice == Voice::Two)]
        .iter()
        .copied()
        .chain((0..capacity).step_by(step))
        .filter(|&tick| {
            tick < capacity
                && placed
                    .regions
                    .iter()
                    .any(|region| region.contains(voice, tick))
                && !bar.timed_iter().any(|(v, start, _, event)| {
                    v == voice
                        && !event.pitches.is_empty()
                        && start < tick
                        && tick < start + event.duration.ticks()
                })
        })
        .min_by(|a, b| {
            (placed.content.x_at_tick(f64::from(*a)) - local.x)
                .abs()
                .total_cmp(&(placed.content.x_at_tick(f64::from(*b)) - local.x).abs())
        })?;
    Some(Ghost {
        part,
        staff: placed.source_staff,
        measure,
        voice,
        tick,
        pitch,
        duration,
        x: placed.origin.x + placed.content.x_at_tick(f64::from(tick)),
        y: placed.origin.y + 40. - f32::from(diatonic - clef.bottom_diatonic()) * 5.,
        staff_top: placed.origin.y,
    })
}

#[cfg(test)]
mod tests;
