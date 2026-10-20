//! Two independent rhythmic streams share one staff and one event store.
use super::*;
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub enum Voice {
    #[default]
    One,
    Two,
}
impl Voice {
    pub const ALL: [Self; 2] = [Self::One, Self::Two];
    pub const fn number(self) -> u8 {
        match self {
            Self::One => 1,
            Self::Two => 2,
        }
    }
}
impl Measure {
    pub(crate) fn voice_order(&self, voice: Voice) -> &Vector<EventId> {
        match voice {
            Voice::One => &self.order,
            Voice::Two => &self.second_order,
        }
    }
    pub(crate) fn voice_order_mut(&mut self, voice: Voice) -> &mut Vector<EventId> {
        match voice {
            Voice::One => &mut self.order,
            Voice::Two => &mut self.second_order,
        }
    }
    pub fn voice_iter(&self, voice: Voice) -> impl Iterator<Item = (EventId, &Event)> {
        self.voice_order(voice)
            .iter()
            .filter_map(|id| self.events.get(*id).map(|event| (*id, event)))
    }
    pub fn timed_iter(&self) -> impl Iterator<Item = (Voice, u16, EventId, &Event)> {
        Voice::ALL.into_iter().flat_map(move |voice| {
            self.voice_iter(voice).scan(0, move |tick, (id, event)| {
                let start = *tick;
                *tick += event.duration.ticks();
                Some((voice, start, id, event))
            })
        })
    }
    pub fn voice_of(&self, id: EventId) -> Option<Voice> {
        Voice::ALL
            .into_iter()
            .find(|voice| self.voice_order(*voice).contains(&id))
    }
    pub fn voice_len(&self, voice: Voice) -> usize {
        self.voice_order(voice).len()
    }
    pub fn voice_id_at(&self, voice: Voice, index: usize) -> Option<EventId> {
        self.voice_order(voice).get(index).copied()
    }
    pub fn voice_index_of(&self, voice: Voice, id: EventId) -> Option<usize> {
        self.voice_order(voice).iter().position(|key| *key == id)
    }
    pub fn voice_ticks(&self, voice: Voice) -> u16 {
        self.voice_iter(voice)
            .map(|(_, e)| e.duration.ticks())
            .sum()
    }
    pub fn has_two_voices(&self) -> bool {
        !self.second_order.is_empty()
    }
}
impl Score {
    pub fn voice_of(&self, id: ElementId) -> Option<Voice> {
        self.part(id.part)?
            .staff(id.staff)?
            .measures
            .get(id.measure)?
            .voice_of(id.event)
    }
}
