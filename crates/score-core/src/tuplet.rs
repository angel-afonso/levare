//! Tuplet groups are derived from the music, never stored: consecutive events
//! of one voice with the same ratio and unit form a group until their written
//! values fill it.
use super::*;

impl Tuplet {
    /// What a complete group writes: three eighths for 3:2 in eighths.
    pub fn written_span(self) -> u32 {
        self.unit().ticks() * u32::from(self.actual())
    }
    /// How long a complete group lasts: the time of two eighths for 3:2.
    pub fn span(self) -> u32 {
        self.unit().ticks() * u32::from(self.normal())
    }
}

impl Duration {
    /// How long this value would last outside a tuplet.
    pub fn written_ticks(self) -> u32 {
        self.with_tuplet(None).ticks()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TupletGroup {
    pub tuplet: Tuplet,
    /// Onset of the first event, in the bar.
    pub start: u32,
    /// Where the last event ends.
    pub end: u32,
    pub events: Vec<EventId>,
    /// Written ticks of its events, up to `tuplet.written_span()`.
    filled: u32,
}

impl TupletGroup {
    pub fn is_complete(&self) -> bool {
        self.filled >= self.tuplet.written_span()
    }
    /// Where the group ends once complete.
    pub fn span_end(&self) -> u32 {
        self.start + self.tuplet.span()
    }
}

impl Measure {
    /// The tuplet groups of a voice, in order. An event that would overfill
    /// the open group starts the next one, so every sequence has groups; a
    /// group still being written is incomplete.
    pub fn tuplet_groups(&self, voice: Voice) -> Vec<TupletGroup> {
        let mut groups: Vec<TupletGroup> = Vec::new();
        let mut open = false;
        let mut tick = 0;
        for (id, event) in self.voice_iter(voice) {
            let start = tick;
            tick += event.duration.ticks();
            let Some(tuplet) = event.duration.tuplet() else {
                open = false;
                continue;
            };
            let written = event.duration.written_ticks();
            match groups.last_mut() {
                Some(group)
                    if open
                        && group.tuplet == tuplet
                        && group.filled + written <= tuplet.written_span() =>
                {
                    group.events.push(id);
                    group.filled += written;
                    group.end = tick;
                }
                _ => groups.push(TupletGroup {
                    tuplet,
                    start,
                    end: tick,
                    events: vec![id],
                    filled: written,
                }),
            }
            open = !groups.last().is_some_and(TupletGroup::is_complete);
        }
        groups
    }
}

#[cfg(test)]
mod tests;
