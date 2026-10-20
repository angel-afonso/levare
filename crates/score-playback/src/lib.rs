//! Deterministic musical timing, independent of audio devices and UI clocks.
use score_core::{
    Articulation, ElementId, Instrument, MAX_SOUNDING_NOTES, MAX_STAVES, PartId, Pitch, Pitches,
    PlayingTechnique, Score, ScoreError, StaffId, Step,
};

mod arpeggio;
mod audition;
mod dynamics;
mod interpretation;
mod timeline;
pub use audition::{Audition, MAX_AUDITION_NOTES};
pub use timeline::{FERMATA_HOLD, Timeline};

pub const PLAYBACK_CHANNELS: usize = 128;
/// Channel for a validated score's part index; percussion and audition are reserved.
pub fn part_channel(index: usize) -> u8 {
    debug_assert!(index < score_core::MAX_PARTS);
    let slot = index % 14;
    ((index / 14) * 32 + slot + usize::from(slot >= 9)) as u8
}
pub const TICKS_PER_QUARTER: u64 = score_core::TICKS_PER_QUARTER as u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tempo(u16);
impl Default for Tempo {
    fn default() -> Self {
        Self(120)
    }
}
impl Tempo {
    pub fn new(bpm: u16) -> Option<Self> {
        (30..=240).contains(&bpm).then_some(Self(bpm))
    }
    pub fn bpm(self) -> u16 {
        self.0
    }
    pub fn frame_at(self, tick: u64, sample_rate: u32) -> u64 {
        tick * u64::from(sample_rate) * 60 / (u64::from(self.0) * TICKS_PER_QUARTER)
    }
}

/// Convert a resolved pitch. Score callers first resolve the key with `in_key`.
pub fn midi_key(pitch: Pitch) -> u8 {
    let semitone = match pitch.step {
        Step::C => 0,
        Step::D => 2,
        Step::E => 4,
        Step::F => 5,
        Step::G => 7,
        Step::A => 9,
        Step::B => 11,
    };
    (i16::from(pitch.octave() + 1) * 12 + semitone + i16::from(pitch.alteration())) as u8
}

/// One interpreted attack: musical intent, not General MIDI state. Providers
/// choose recordings from it; they must not reapply the accent already in
/// `level` nor reinterpret the sounding length, which the plan schedules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoteEvent {
    pub key: u8,
    /// Attack intensity on the MIDI velocity scale (1..=127): the written or
    /// hairpin dynamic plus any accent or marcato.
    pub level: u8,
    pub instrument: Instrument,
    /// Inherited bowed-string technique; `Arco` for every other instrument.
    pub technique: PlayingTechnique,
    /// Per-attack harmonic timbre, independent of bowed/plucked technique.
    pub harmonic: Option<score_core::Harmonic>,
    /// Written articulation of this attack.
    pub articulation: Option<Articulation>,
    /// Under a slur. The plan already overlaps connected notes; a slurred
    /// staccato is portato and should keep a sustained recording.
    pub legato: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Message {
    Expression(u8),
    NoteOff(u8),
    /// Release only the harmonic timbre when it shares a key with normal strings.
    HarmonicOff(u8),
    NoteOn(NoteEvent),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScheduledMessage {
    pub frame: u64,
    pub channel: u8,
    pub message: Message,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub element: ElementId,
    pub start: u64,
    pub end: u64,
    pub channel: u8,
    pub pitches: Pitches,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeldNote {
    pub channel: u8,
    pub note: NoteEvent,
}

/// Sounding intervals differ from notation for articulation, slurs and tied chains.
#[derive(Clone, Copy, Debug)]
struct SoundSpan {
    element: ElementId,
    arpeggio: Option<score_core::Arpeggio>,
    /// Original first fragment, so tied continuations never widen the roll.
    attack_duration: u64,
    articulation: Option<Articulation>,
    boost: u8,
    reference_level: u8,
    technique: PlayingTechnique,
    harmonic: Option<score_core::Harmonic>,
    instrument: Instrument,
    legato: bool,
    /// The slur continues from this sound into the next written note.
    slurred: bool,
    start: u64,
    end: u64,
    written_end: u64,
    channel: u8,
    pitches: Pitches,
    level: u8,
}
#[derive(Debug)]
struct KeySound {
    note: NoteEvent,
    start: u64,
    end: u64,
}

/// Prepared on the control thread. One immutable plan can be shared with the
/// audio callback and with the view's playback cursor.
#[derive(Debug)]
pub struct Track {
    pub voice: score_core::Voice,
    pub staff: StaffId,
    pub part: PartId,
    pub channel: u8,
    pub instrument: Instrument,
    spans: std::ops::Range<usize>,
    sounds: std::ops::Range<usize>,
    expression: std::ops::Range<usize>,
}
#[derive(Debug)]
struct ExpressionPoint {
    frame: u64,
    value: u8,
}
#[derive(Debug)]
pub struct PlaybackPlan {
    pub messages: Vec<ScheduledMessage>,
    pub tracks: Vec<Track>,
    pub spans: Vec<Span>,
    sounds: Vec<SoundSpan>,
    expression: Vec<ExpressionPoint>,
    key_sounds: std::collections::HashMap<(u8, u8, bool), Vec<KeySound>>,
    pub duration_frames: u64,
    pub sample_rate: u32,
    /// Musical to audio time for this score, fermatas included.
    pub timeline: Timeline,
}

/// Overlap each slurred sound into the next written note of the same staff.
/// A repeated key is re-attacked instead, so a unison never merges into a tie.
fn connect_slurred(sounds: &mut [SoundSpan], rate: u32) {
    let overlap = interpretation::legato_overlap(rate);
    for index in 1..sounds.len() {
        let (before, after) = sounds.split_at_mut(index);
        let (previous, next) = (&mut before[index - 1], &after[0]);
        let repeated = previous
            .pitches
            .iter()
            .any(|a| next.pitches.iter().any(|b| midi_key(a) == midi_key(b)));
        if previous.slurred
            && previous.end == previous.written_end
            && previous.written_end == next.start
            && !repeated
        {
            previous.end = next.start + overlap.min(next.end - next.start);
        }
    }
}

impl PlaybackPlan {
    pub fn new(score: &Score, tempo: Tempo, sample_rate: u32) -> Result<Self, ScoreError> {
        score.validate()?;
        if !(16_000..=192_000).contains(&sample_rate) {
            return Err(ScoreError("Frecuencia de audio no compatible"));
        }
        let timeline = Timeline::new(score, tempo);
        let mut messages = Vec::new();
        let mut spans = Vec::new();
        let mut sounds: Vec<SoundSpan> = Vec::new();
        let ties: std::collections::HashSet<_> = score
            .connections()
            .iter()
            .filter(|c| c.kind == score_core::ConnectionKind::Tie)
            .copied()
            .collect();
        let tied_starts: std::collections::HashSet<_> = ties.iter().map(|c| c.start).collect();
        let tied_ends: std::collections::HashSet<_> = ties.iter().map(|c| c.end).collect();
        let mut duration_frames = 0;
        let mut tracks = Vec::new();
        let mut expression: Vec<ExpressionPoint> = Vec::new();
        for (part_index, part) in score.parts().iter().enumerate() {
            let channel = part_channel(part_index);
            let first_part_sound = sounds.len();
            let first_part_track = tracks.len();
            let split_hands = part.instrument().is_grand_staff()
                && score
                    .connections()
                    .iter()
                    .any(|c| c.kind.is_hairpin() && c.start.part == part.id());
            for staff in part.staves() {
                // A second internal synth bank isolates piano hands without exhausting MIDI channels.
                let channel = channel
                    + if split_hands && staff.id() == StaffId::Lower {
                        16
                    } else {
                        0
                    };
                let first_expression = expression.len();
                let expressive = score.connections().iter().any(|c| {
                    c.kind.is_hairpin()
                        && c.start.part == part.id()
                        && (c.start.staff == staff.id() || part.is_divisi())
                });
                let first_sound = sounds.len();
                let mut dynamics = std::collections::BTreeMap::new();
                let mut techniques = std::collections::BTreeMap::new();
                for source in part.expression_staves(staff.id()) {
                    for (bar, measure) in source.measures().iter().enumerate() {
                        for (_, onset, _, event) in measure.timed_iter() {
                            let tick = score.measure_start_tick(bar) + u64::from(onset);
                            if let Some(mark) = event.notation.dynamic {
                                dynamics.insert(tick, mark);
                            }
                            if let Some(mark) = event.notation.technique {
                                techniques.insert(tick, mark);
                            }
                        }
                    }
                }
                let mut voice_ranges = Vec::new();
                for voice in score_core::Voice::ALL {
                    if voice == score_core::Voice::Two
                        && staff.measures().iter().all(|m| m.voice_len(voice) == 0)
                    {
                        continue;
                    }
                    let first_voice_span = spans.len();
                    let first_voice_sound = sounds.len();
                    let slurs = interpretation::Slurs::new(score, part.id(), staff.id(), voice);
                    for (index, measure) in staff.measures().iter().enumerate() {
                        let mut tick = score.measure_start_tick(index);
                        for (id, event) in measure.voice_iter(voice) {
                            let written_tick = tick;
                            let start = timeline.frame_at(tick, sample_rate);
                            tick += u64::from(event.duration.ticks());
                            let end = timeline.frame_at(tick, sample_rate);
                            let element = ElementId {
                                staff: staff.id(),
                                part: part.id(),
                                measure: index,
                                event: id,
                            };
                            let pitches = score.sounding_pitches(element)?;
                            spans.push(Span {
                                element,
                                start,
                                end,
                                channel,
                                pitches,
                            });
                            let technique = techniques
                                .range(..=written_tick)
                                .next_back()
                                .map_or(PlayingTechnique::Arco, |(_, mark)| *mark);
                            let dynamic = dynamics
                                .range(..=written_tick)
                                .next_back()
                                .map_or(score_core::Dynamic::Mf, |(_, mark)| *mark);
                            let articulation = event.notation.articulation;
                            let slurred = slurs.continues(written_tick);
                            if !pitches.is_empty() {
                                if tied_ends.contains(&element) {
                                    // Whole chords tie as a unit; the original attack and level survive.
                                    let sound = sounds
                                        .last_mut()
                                        .expect("validated consecutive tied chord");
                                    // The final articulation shapes the combined tied duration.
                                    // This also preserves staccato when paste splits a long note.
                                    sound.written_end = end;
                                    sound.slurred = slurred;
                                    sound.end = if tied_starts.contains(&element) {
                                        end
                                    } else {
                                        interpretation::sounding_end(
                                            sound.start,
                                            end,
                                            articulation,
                                            slurred,
                                            sample_rate,
                                        )
                                    };
                                    // Paste may put the release articulation on the
                                    // final tied fragment. Preserve its short sample
                                    // choice as well as the combined duration.
                                    if !tied_starts.contains(&element)
                                        && articulation == Some(Articulation::Staccato)
                                    {
                                        sound.articulation = articulation;
                                    }
                                } else {
                                    let tied = tied_starts.contains(&element);
                                    sounds.push(SoundSpan {
                                        element,
                                        arpeggio: event.notation.arpeggio,
                                        attack_duration: end - start,
                                        articulation: if tied
                                            && articulation == Some(Articulation::Staccato)
                                        {
                                            None
                                        } else {
                                            articulation
                                        },
                                        boost: interpretation::accent_boost(articulation),
                                        reference_level: dynamic.velocity(),
                                        technique,
                                        harmonic: event.notation.harmonic,
                                        instrument: part.instrument(),
                                        legato: slurs.contains(written_tick),
                                        slurred,
                                        start,
                                        end: if tied {
                                            end
                                        } else {
                                            interpretation::sounding_end(
                                                start,
                                                end,
                                                articulation,
                                                slurred,
                                                sample_rate,
                                            )
                                        },
                                        written_end: end,
                                        channel,
                                        pitches,
                                        level: interpretation::level(
                                            dynamic.velocity(),
                                            articulation,
                                        ),
                                    });
                                }
                            }
                            duration_frames = duration_frames.max(end);
                        }
                    }
                    connect_slurred(&mut sounds[first_voice_sound..], sample_rate);
                    voice_ranges.push((
                        voice,
                        first_voice_span..spans.len(),
                        first_voice_sound..sounds.len(),
                    ));
                }
                if expressive && !part.is_divisi() {
                    let envelope =
                        score_core::DynamicEnvelope::new(score, part.id(), staff.id()).unwrap();
                    let points = dynamics::prepare(
                        envelope,
                        &mut sounds[first_sound..],
                        &timeline,
                        sample_rate,
                    );
                    for point in &points {
                        messages.push(ScheduledMessage {
                            frame: point.frame,
                            channel,
                            message: Message::Expression(point.value),
                        });
                    }
                    expression.extend(points);
                }
                for (voice, span_range, sound_range) in voice_ranges {
                    tracks.push(Track {
                        voice,
                        staff: staff.id(),
                        part: part.id(),
                        channel,
                        instrument: part.instrument(),
                        spans: span_range,
                        sounds: sound_range,
                        expression: first_expression..expression.len(),
                    });
                }
            }
            if part.is_divisi()
                && score
                    .connections()
                    .iter()
                    .any(|c| c.kind.is_hairpin() && c.start.part == part.id())
            {
                let first = expression.len();
                let envelope =
                    score_core::DynamicEnvelope::new(score, part.id(), StaffId::Upper).unwrap();
                let points = dynamics::prepare(
                    envelope,
                    &mut sounds[first_part_sound..],
                    &timeline,
                    sample_rate,
                );
                for point in &points {
                    messages.push(ScheduledMessage {
                        frame: point.frame,
                        channel,
                        message: Message::Expression(point.value),
                    });
                }
                expression.extend(points);
                for track in &mut tracks[first_part_track..] {
                    track.expression = first..expression.len();
                }
            }
        }
        let sound_indices: std::collections::HashMap<_, _> = sounds
            .iter()
            .enumerate()
            .map(|(index, span)| (span.element, index))
            .collect();
        let piano_arpeggios: std::collections::HashMap<_, _> = score
            .piano_arpeggios()
            .iter()
            .flat_map(|mark| [(mark.upper, mark), (mark.lower, mark)])
            .collect();
        for span in &sounds {
            let group = piano_arpeggios.get(&span.element);
            let mut keys = [None; MAX_AUDITION_NOTES];
            let mut count = 0;
            let mut gate = span.attack_duration.min(span.end - span.start);
            let members = group.map_or([Some(span.element), None], |mark| {
                [Some(mark.upper), Some(mark.lower)]
            });
            for id in members.into_iter().flatten() {
                // A tied continuation has no new sound; preserve that sustain.
                if let Some(index) = sound_indices.get(&id) {
                    let member = sounds[*index];
                    gate = gate.min(member.attack_duration.min(member.end - member.start));
                    for pitch in member.pitches.iter() {
                        keys[count] = Some(midi_key(pitch));
                        count += 1;
                    }
                }
            }
            let (keys, delays) = arpeggio::attacks(
                keys,
                group.map(|mark| mark.kind).or(span.arpeggio),
                gate,
                sample_rate,
            );
            for (key, delay) in keys
                .into_iter()
                .zip(delays)
                .filter_map(|(key, delay)| key.map(|key| (key, delay)))
            {
                if !span.pitches.iter().any(|pitch| midi_key(pitch) == key) {
                    continue;
                }
                messages.push(ScheduledMessage {
                    frame: span.start + delay,
                    channel: span.channel,
                    message: Message::NoteOn(NoteEvent {
                        key,
                        level: span.level,
                        instrument: span.instrument,
                        technique: span.technique,
                        harmonic: span.harmonic,
                        articulation: span.articulation,
                        legato: span.legato,
                    }),
                });
                messages.push(ScheduledMessage {
                    frame: span.end,
                    channel: span.channel,
                    message: if span.harmonic.is_some() {
                        Message::HarmonicOff(key)
                    } else {
                        Message::NoteOff(key)
                    },
                });
            }
        }
        // At a shared boundary a repeated key is released before it is retriggered.
        messages.sort_by_key(|message| {
            (
                message.frame,
                match message.message {
                    Message::NoteOff(_) | Message::HarmonicOff(_) => 0,
                    Message::Expression(_) => 1,
                    Message::NoteOn(_) => 2,
                },
            )
        });
        // Overlapping unisons of the same timbre form one held key. Harmonics
        // remain separate from normal notes in another voice on this channel.
        let mut held = [[[0u8; 128]; 2]; PLAYBACK_CHANNELS];
        messages.retain(|scheduled| {
            let (key, harmonic) = match scheduled.message {
                Message::Expression(_) => return true,
                Message::NoteOn(note) => (note.key, note.harmonic.is_some()),
                Message::NoteOff(key) => (key, false),
                Message::HarmonicOff(key) => (key, true),
            };
            let count =
                &mut held[usize::from(scheduled.channel)][usize::from(harmonic)][usize::from(key)];
            match scheduled.message {
                Message::Expression(_) => unreachable!(),
                Message::NoteOn(_) => {
                    *count += 1;
                    *count == 1
                }
                Message::NoteOff(_) | Message::HarmonicOff(_) => {
                    *count -= 1;
                    *count == 0
                }
            }
        });
        // Chase the actual merged attack, including overlap chains between hands.
        let mut key_sounds: std::collections::HashMap<(u8, u8, bool), Vec<KeySound>> =
            std::collections::HashMap::new();
        for message in &messages {
            match message.message {
                Message::Expression(_) => {}
                Message::NoteOn(note) => key_sounds
                    .entry((message.channel, note.key, note.harmonic.is_some()))
                    .or_default()
                    .push(KeySound {
                        note,
                        start: message.frame,
                        end: 0,
                    }),
                Message::NoteOff(key) | Message::HarmonicOff(key) => {
                    let harmonic = matches!(message.message, Message::HarmonicOff(_));
                    key_sounds
                        .get_mut(&(message.channel, key, harmonic))
                        .unwrap()
                        .last_mut()
                        .unwrap()
                        .end = message.frame
                }
            }
        }
        Ok(Self {
            key_sounds,
            expression,
            messages,
            tracks,
            spans,
            sounds,
            duration_frames,
            sample_rate,
            timeline,
        })
    }

    /// Resolved on the control thread before starting, including inside held notes.
    pub fn expressions_at(&self, frame: u64) -> [u8; PLAYBACK_CHANNELS] {
        let mut values = [127; PLAYBACK_CHANNELS];
        for track in &self.tracks {
            let points = &self.expression[track.expression.clone()];
            if let Some(index) = points.partition_point(|p| p.frame <= frame).checked_sub(1) {
                values[usize::from(track.channel)] = points[index].value;
            }
        }
        values
    }
    pub fn elements_at(&self, frame: u64) -> [Option<ElementId>; MAX_STAVES * 2] {
        let mut elements = [None; MAX_STAVES * 2];
        for (index, track) in self.tracks.iter().enumerate() {
            let spans = &self.spans[track.spans.clone()];
            if let Some(span) = spans
                .partition_point(|span| span.start <= frame)
                .checked_sub(1)
                .map(|index| &spans[index])
            {
                elements[index] = (frame < span.end).then_some(span.element);
            }
        }
        elements
    }
    pub fn element_at(&self, frame: u64) -> Option<ElementId> {
        self.elements_at(frame).into_iter().flatten().next()
    }

    /// Prepared before Play is queued: the callback inspects at most 448 notes,
    /// regardless of the score's length.
    pub fn held_notes_at(&self, frame: u64) -> [Option<HeldNote>; MAX_SOUNDING_NOTES] {
        let mut notes = [None; MAX_SOUNDING_NOTES];
        let mut count = 0;
        let mut seen = [[[false; 128]; 2]; PLAYBACK_CHANNELS];
        for track in &self.tracks {
            let spans = &self.sounds[track.sounds.clone()];
            if let Some(span) = spans
                .partition_point(|span| span.start < frame)
                .checked_sub(1)
                .map(|index| spans[index])
                && frame < span.end
            {
                for pitch in span.pitches.iter() {
                    let key = midi_key(pitch);
                    let harmonic = span.harmonic.is_some();
                    let seen = &mut seen[usize::from(span.channel)][usize::from(harmonic)]
                        [usize::from(key)];
                    if !*seen {
                        let intervals = &self.key_sounds[&(span.channel, key, harmonic)];
                        let Some(index) = intervals
                            .partition_point(|sound| sound.start < frame)
                            .checked_sub(1)
                        else {
                            continue;
                        };
                        if frame >= intervals[index].end {
                            continue;
                        }
                        notes[count] = Some(HeldNote {
                            channel: span.channel,
                            note: intervals[index].note,
                        });
                        count += 1;
                        *seen = true;
                    }
                }
            }
        }
        notes
    }

    pub fn frame_for_element(&self, element: ElementId) -> Option<u64> {
        self.spans
            .iter()
            .find(|span| span.element == element)
            .map(|span| span.start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use score_core::{Duration, Event};
    pub(crate) fn piano_note(key: u8, level: u8) -> NoteEvent {
        NoteEvent {
            key,
            level,
            instrument: Instrument::Piano,
            technique: PlayingTechnique::Arco,
            harmonic: None,
            articulation: None,
            legato: false,
        }
    }
    fn note(step: Step, duration: Duration) -> Event {
        Event {
            notation: Default::default(),
            pitches: score_core::Pitches::single(Pitch::new(step, 4)),
            duration,
        }
    }

    #[test]
    fn key_applies_to_chords_both_hands_and_held_notes_but_respects_overrides() {
        use score_core::{Accidental, KeyMode, KeySignature};
        let mut score = Score::default();
        score.set_key_signature(KeySignature::new(2, KeyMode::Major).unwrap());
        let chord = Pitches::new(&[
            Pitch::new(Step::F, 4).unwrap(),
            Pitch::new(Step::A, 4).unwrap(),
            Pitch::new(Step::C, 5).unwrap(),
        ])
        .unwrap();
        score
            .insert(
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: chord,
                    duration: Duration::Whole,
                },
            )
            .unwrap();
        score
            .insert_on(
                score.parts()[0].id(),
                StaffId::Lower,
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: Pitches::new(&[
                        Pitch::new(Step::F, 3).unwrap(),
                        Pitch::new(Step::C, 4)
                            .unwrap()
                            .with_accidental(Some(Accidental::Natural)),
                    ])
                    .unwrap(),
                    duration: Duration::Whole,
                },
            )
            .unwrap();
        let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
        assert_eq!(
            plan.held_notes_at(24_000)
                .into_iter()
                .flatten()
                .map(|note| note.note.key)
                .collect::<Vec<_>>(),
            [66, 69, 73, 54, 60]
        );
        let keys: Vec<_> = plan
            .messages
            .iter()
            .filter_map(|message| {
                if let Message::NoteOn(NoteEvent { key, .. }) = message.message {
                    Some(key)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(keys, [66, 69, 73, 54, 60]);
        score.set_key_signature(KeySignature::new(-2, KeyMode::Major).unwrap());
        let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
        assert_eq!(
            plan.held_notes_at(24_000)
                .into_iter()
                .flatten()
                .map(|note| note.note.key)
                .collect::<Vec<_>>(),
            [65, 69, 72, 53, 60]
        );
    }

    #[test]
    fn altered_pitches_sound_by_semitone_and_enharmonic_chords_share_a_key() {
        use score_core::Accidental::{Flat, Natural, Sharp};
        let c = Pitch::new(Step::C, 4).unwrap();
        let sharp = c.with_accidental(Some(Sharp));
        let flat = Pitch::new(Step::D, 4).unwrap().with_accidental(Some(Flat));
        assert_eq!(midi_key(sharp), 61);
        assert_eq!(midi_key(flat), 61);
        assert_eq!(midi_key(c.with_accidental(Some(Natural))), 60);
        assert_eq!(
            midi_key(Pitch::new(Step::C, 0).unwrap().with_accidental(Some(Flat))),
            11
        );
        assert_eq!(
            midi_key(Pitch::new(Step::B, 8).unwrap().with_accidental(Some(Sharp))),
            120
        );
        let mut score = Score::default();
        score
            .insert(
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: Pitches::new(&[sharp, flat]).unwrap(),
                    duration: Duration::Whole,
                },
            )
            .unwrap();
        let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
        assert_eq!(plan.messages.len(), 2);
        assert_eq!(plan.held_notes_at(100).into_iter().flatten().count(), 1);
        assert_eq!(plan.held_notes_at(100)[0].unwrap().note.key, 61);
    }

    #[test]
    fn chord_tones_start_together_and_restore_all_pitches_from_a_selection() {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        let chord = Pitches::new(&[
            Pitch::new(Step::C, 4).unwrap(),
            Pitch::new(Step::E, 4).unwrap(),
            Pitch::new(Step::G, 4).unwrap(),
        ])
        .unwrap();
        let right = score
            .insert(
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: chord,
                    duration: Duration::Whole,
                },
            )
            .unwrap();
        score
            .insert_on(
                piano,
                StaffId::Lower,
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: Pitches::default(),
                    duration: Duration::Quarter,
                },
            )
            .unwrap();
        let left = score
            .insert_on(
                piano,
                StaffId::Lower,
                0,
                1,
                Event {
                    notation: Default::default(),
                    pitches: Pitches::single(Pitch::new(Step::C, 3)),
                    duration: Duration::Quarter,
                },
            )
            .unwrap();
        let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
        let starts: Vec<_> = plan
            .messages
            .iter()
            .filter(|message| message.frame == 0)
            .map(|message| message.message)
            .collect();
        assert_eq!(
            starts,
            vec![
                Message::NoteOn(piano_note(60, 88)),
                Message::NoteOn(piano_note(64, 88)),
                Message::NoteOn(piano_note(67, 88))
            ]
        );
        assert_eq!(plan.frame_for_element(right), Some(0));
        assert_eq!(plan.frame_for_element(left), Some(24_000));
        assert_eq!(
            plan.held_notes_at(24_000)
                .into_iter()
                .flatten()
                .map(|note| note.note.key)
                .collect::<Vec<_>>(),
            vec![60, 64, 67]
        );
        assert!(
            plan.messages
                .iter()
                .filter(|message| matches!(message.message, Message::NoteOff(60 | 64 | 67)))
                // A detached whole note releases at most 60 ms early.
                .all(|message| message.frame == 93_120)
        );
        assert_eq!(plan.spans.len(), 3);
    }

    #[test]
    fn shared_chord_tones_between_hands_release_only_after_both_are_done() {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        let c = Pitch::new(Step::C, 4).unwrap();
        score
            .insert(
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: Pitches::new(&[c, Pitch::new(Step::E, 4).unwrap()]).unwrap(),
                    duration: Duration::Whole,
                },
            )
            .unwrap();
        score
            .insert_on(
                piano,
                StaffId::Lower,
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: Pitches::new(&[c, Pitch::new(Step::G, 3).unwrap()]).unwrap(),
                    duration: Duration::Half,
                },
            )
            .unwrap();
        let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
        assert_eq!(
            plan.messages
                .iter()
                .filter(|message| message.message == (Message::NoteOn(piano_note(60, 88))))
                .count(),
            1
        );
        assert_eq!(
            plan.messages
                .iter()
                .filter(|message| message.message == Message::NoteOff(60))
                .map(|message| message.frame)
                .collect::<Vec<_>>(),
            vec![93_120]
        );
        assert_eq!(
            plan.held_notes_at(24_000)
                .into_iter()
                .flatten()
                .map(|note| note.note.key)
                .collect::<Vec<_>>(),
            vec![60, 64, 55]
        );
    }

    #[test]
    fn both_hands_share_time_and_timbre_but_keep_distinct_positions() {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        let right = score
            .insert_on(
                piano,
                StaffId::Upper,
                0,
                0,
                note(Step::E, Duration::Quarter),
            )
            .unwrap();
        let left = score
            .insert_on(
                piano,
                StaffId::Lower,
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(Pitch::new(Step::C, 3)),
                    duration: Duration::Whole,
                },
            )
            .unwrap();
        let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
        assert_eq!(plan.tracks.len(), 2);
        assert_eq!(plan.tracks[0].channel, plan.tracks[1].channel);
        assert_eq!(plan.tracks[0].instrument, plan.tracks[1].instrument);
        assert_eq!(plan.tracks[1].staff, StaffId::Lower);
        assert_eq!(plan.frame_for_element(left), Some(0));
        assert_eq!(
            plan.elements_at(0)
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
            vec![right, left]
        );
        assert_eq!(plan.duration_frames, 96_000);
        assert!(plan.elements_at(24_000).contains(&Some(left)));
        assert!(!plan.elements_at(24_000).contains(&Some(right)));
        assert_eq!(
            plan.held_notes_at(24_000)
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
            vec![HeldNote {
                channel: 0,
                note: piano_note(48, 88)
            }]
        );
    }

    #[test]
    fn overlapping_unisons_do_not_cut_off_the_other_hand_or_double_the_attack() {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        score
            .insert_on(piano, StaffId::Upper, 0, 0, note(Step::C, Duration::Whole))
            .unwrap();
        score
            .insert_on(
                piano,
                StaffId::Lower,
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(None),
                    duration: Duration::Quarter,
                },
            )
            .unwrap();
        let left = score
            .insert_on(piano, StaffId::Lower, 0, 1, note(Step::C, Duration::Half))
            .unwrap();
        let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
        assert_eq!(
            plan.messages,
            vec![
                ScheduledMessage {
                    frame: 0,
                    channel: 0,
                    message: Message::NoteOn(piano_note(60, 88))
                },
                ScheduledMessage {
                    frame: 93_120,
                    channel: 0,
                    message: Message::NoteOff(60)
                }
            ]
        );
        assert_eq!(plan.frame_for_element(left), Some(24_000));
        assert_eq!(
            plan.held_notes_at(48_000)
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
            vec![HeldNote {
                channel: 0,
                note: piano_note(60, 88)
            }]
        );
    }

    #[test]
    fn fourteen_pianos_can_track_and_restore_all_twenty_eight_hands() {
        let mut score = Score::default();
        for _ in 1..score_core::MAX_PARTS {
            score.add_part(score_core::Instrument::Piano).unwrap();
        }
        let ids: Vec<_> = score.parts().iter().map(|part| part.id()).collect();
        for part in ids {
            for (staff, step) in [(StaffId::Upper, Step::C), (StaffId::Lower, Step::E)] {
                score
                    .insert_on(part, staff, 0, 0, note(step, Duration::Whole))
                    .unwrap();
            }
        }
        let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
        assert_eq!(plan.tracks.len(), MAX_STAVES);
        assert_eq!(
            plan.elements_at(24_000).into_iter().flatten().count(),
            MAX_STAVES
        );
        assert_eq!(
            plan.held_notes_at(24_000).into_iter().flatten().count(),
            MAX_STAVES
        );
        assert_eq!(plan.messages.len(), MAX_STAVES * 2);
    }

    #[test]
    fn ensemble_uses_independent_channels_and_a_shared_sample_clock() {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        let long = score
            .insert_in(piano, 1, 0, note(Step::C, Duration::Whole))
            .unwrap();
        let flute = score.add_part(score_core::Instrument::Flute).unwrap();
        let short = score
            .insert_in(flute, 0, 0, note(Step::C, Duration::Quarter))
            .unwrap();
        let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
        assert_eq!(plan.duration_frames, 192_000); // The last part ends earlier.
        assert_eq!(plan.frame_for_element(short), Some(0));
        assert_eq!(plan.frame_for_element(long), Some(96_000));
        assert_eq!(plan.tracks[0].channel, 0);
        assert_eq!(
            plan.tracks
                .iter()
                .find(|track| track.part == flute)
                .unwrap()
                .channel,
            1
        );
        assert_eq!(
            plan.tracks
                .iter()
                .find(|track| track.part == flute)
                .unwrap()
                .instrument,
            score_core::Instrument::Flute
        );
        assert!(plan.elements_at(0).contains(&Some(short)));
        assert_eq!(plan.elements_at(100_000)[0], Some(long));
        for _ in score.parts().len()..score_core::MAX_PARTS {
            score.add_part(score_core::Instrument::Violin).unwrap();
        }
        let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
        let channels: std::collections::HashSet<_> =
            plan.tracks.iter().map(|track| track.channel).collect();
        assert_eq!(channels.len(), score_core::MAX_PARTS);
        assert!(!channels.contains(&9));
        assert!(!channels.contains(&15));
    }

    #[test]
    fn simultaneous_notes_and_releases_keep_their_part_channels() {
        let mut score = Score::default();
        let first = score
            .insert(0, 0, note(Step::C, Duration::Quarter))
            .unwrap();
        let violin = score.add_part(score_core::Instrument::Violin).unwrap();
        let second = score
            .insert_in(violin, 0, 0, note(Step::C, Duration::Half))
            .unwrap();
        let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
        assert_eq!(
            plan.elements_at(0)
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
            vec![first, second]
        );
        assert_eq!(
            plan.elements_at(24_000)
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
            vec![second]
        );
        assert_eq!(
            plan.messages
                .iter()
                .filter(|message| message.frame == 0)
                .count(),
            2
        );
        let releases: Vec<_> = plan
            .messages
            .iter()
            .filter(|message| matches!(message.message, Message::NoteOff(_)))
            .map(|message| (message.channel, message.frame))
            .collect();
        assert_eq!(releases, vec![(0, 22_080), (1, 45_120)]);
    }

    #[test]
    fn pitches_and_real_time_follow_standard_octaves_and_tempo() {
        assert_eq!(midi_key(Pitch::new(Step::C, 4).unwrap()), 60);
        assert_eq!(midi_key(Pitch::new(Step::A, 4).unwrap()), 69);
        assert_eq!(midi_key(Pitch::new(Step::B, 8).unwrap()), 119);
        assert_eq!(Tempo::default().frame_at(48, 48_000), 24_000);
        assert_eq!(Tempo::new(60).unwrap().frame_at(48, 44_100), 44_100);
        assert!(Tempo::new(0).is_none());
    }

    #[test]
    fn rests_and_incomplete_measures_keep_their_musical_time() {
        let mut score = Score::default();
        let first = score
            .insert(0, 0, note(Step::C, Duration::Quarter))
            .unwrap();
        let rest = score
            .insert(
                0,
                1,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(None),
                    duration: Duration::Quarter,
                },
            )
            .unwrap();
        let next = score.insert(1, 0, note(Step::D, Duration::Eighth)).unwrap();
        let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
        assert_eq!(plan.messages.len(), 4);
        assert_eq!(plan.element_at(0), Some(first));
        assert_eq!(plan.element_at(24_000), Some(rest));
        assert_eq!(plan.element_at(60_000), None); // Implicit remainder of bar 1.
        assert_eq!(plan.element_at(96_000), Some(next));
        assert_eq!(plan.frame_for_element(first), Some(0));
        assert_eq!(plan.frame_for_element(rest), Some(24_000));
        assert_eq!(plan.frame_for_element(next), Some(96_000));
        assert_eq!(plan.duration_frames, 108_000); // No three trailing empty measures.
        assert_eq!(plan.element_at(108_000), None);
    }

    #[test]
    fn repeated_notes_release_before_retrigger_and_empty_score_has_no_duration() {
        let mut score = Score::default();
        assert_eq!(
            PlaybackPlan::new(&score, Tempo::default(), 48_000)
                .unwrap()
                .duration_frames,
            0
        );
        score
            .insert(0, 0, note(Step::C, Duration::Quarter))
            .unwrap();
        score
            .insert(0, 1, note(Step::C, Duration::Quarter))
            .unwrap();
        let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
        assert_eq!(
            plan.messages[1],
            ScheduledMessage {
                frame: 22_080,
                channel: 0,
                message: Message::NoteOff(60)
            }
        );
        assert_eq!(
            plan.messages[2],
            ScheduledMessage {
                frame: 24_000,
                channel: 0,
                message: Message::NoteOn(piano_note(60, 88))
            }
        );
    }
}

#[cfg(test)]
mod expression_tests;

#[cfg(test)]
mod voice_tests;

#[cfg(test)]
mod harmonic_tests;

#[cfg(test)]
mod orchestra_tests;
