//! Up to four independent rhythmic streams share one staff and one event store.
//! Voices 1 and 3 point their stems up, 2 and 4 down, as in Sibelius and MuseScore.
use super::*;
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub enum Voice {
    #[default]
    One,
    Two,
    Three,
    Four,
}
impl Voice {
    pub const ALL: [Self; 4] = [Self::One, Self::Two, Self::Three, Self::Four];
    pub const fn number(self) -> u8 {
        match self {
            Self::One => 1,
            Self::Two => 2,
            Self::Three => 3,
            Self::Four => 4,
        }
    }
    /// Position in `Voice::ALL`, for per-voice arrays.
    pub const fn index(self) -> usize {
        self.number() as usize - 1
    }
    /// The lower voices of a shared staff: their stems and rests go down.
    pub const fn is_lower(self) -> bool {
        matches!(self, Self::Two | Self::Four)
    }
}
impl Measure {
    pub(crate) fn voice_order(&self, voice: Voice) -> &Vector<EventId> {
        match voice {
            Voice::One => &self.order,
            Voice::Two => &self.second_order,
            Voice::Three => &self.third_order,
            Voice::Four => &self.fourth_order,
        }
    }
    /// Writing in a bar replaces its %.
    pub(crate) fn voice_order_mut(&mut self, voice: Voice) -> &mut Vector<EventId> {
        self.repeat = false;
        match voice {
            Voice::One => &mut self.order,
            Voice::Two => &mut self.second_order,
            Voice::Three => &mut self.third_order,
            Voice::Four => &mut self.fourth_order,
        }
    }
    pub fn voice_iter(&self, voice: Voice) -> impl Iterator<Item = (EventId, &Event)> {
        self.voice_order(voice)
            .iter()
            .filter_map(|id| self.events.get(*id).map(|event| (*id, event)))
    }
    pub fn timed_iter(&self) -> impl Iterator<Item = (Voice, u32, EventId, &Event)> {
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
    pub fn voice_ticks(&self, voice: Voice) -> u32 {
        self.voice_iter(voice)
            .map(|(_, e)| e.duration.ticks())
            .sum()
    }
    /// Whether any voice besides the first holds events, so stems take sides.
    pub fn has_several_voices(&self) -> bool {
        Voice::ALL[1..]
            .iter()
            .any(|voice| !self.voice_order(*voice).is_empty())
    }
    /// Whether voice 3 or 4 holds events, which divisi cannot place on two staves.
    pub fn has_extra_voices(&self) -> bool {
        !self.third_order.is_empty() || !self.fourth_order.is_empty()
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
