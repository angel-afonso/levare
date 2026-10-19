use super::*;
use score_core::{DynamicEnvelope, MAX_CHORD_NOTES};

/// Control-side, provider-independent preview of one written note/chord.
/// It shares the plan's level, technique, slur and length rules.
#[derive(Clone, Copy, Debug)]
pub struct Audition {
    pub keys: [Option<u8>; MAX_CHORD_NOTES],
    pub level: u8,
    pub instrument: Instrument,
    pub technique: PlayingTechnique,
    pub articulation: Option<Articulation>,
    pub legato: bool,
    pub duration_frames: u64,
    pub sample_rate: u32,
}
impl Audition {
    /// A plain note or chord at mezzo-forte, for instrument and chord previews.
    pub fn plain(keys: [Option<u8>; MAX_CHORD_NOTES], instrument: Instrument, rate: u32) -> Self {
        Self {
            keys,
            level: score_core::Dynamic::Mf.velocity(),
            instrument,
            technique: PlayingTechnique::Arco,
            articulation: None,
            legato: false,
            duration_frames: u64::from(rate) / 4,
            sample_rate: rate,
        }
    }
    pub fn note(&self, key: u8) -> NoteEvent {
        NoteEvent {
            key,
            level: self.level,
            instrument: self.instrument,
            technique: self.technique,
            articulation: self.articulation,
            legato: self.legato,
        }
    }
    pub fn for_element(
        score: &Score,
        id: ElementId,
        tempo: Tempo,
        rate: u32,
    ) -> Result<Self, ScoreError> {
        if !(16_000..=192_000).contains(&rate) {
            return Err(ScoreError("Frecuencia de audio no compatible"));
        }
        let event = score
            .event(id)
            .ok_or(ScoreError("La nota de audición no existe"))?;
        let part = score
            .part(id.part)
            .ok_or(ScoreError("El instrumento no existe"))?;
        let tick = score
            .event_tick(id)
            .ok_or(ScoreError("La nota de audición no existe"))?;
        let level = DynamicEnvelope::new(score, id.part, id.staff)
            .ok_or(ScoreError("El pentagrama no existe"))?
            .level_at(tick);
        let mut keys = [None; MAX_CHORD_NOTES];
        let mut count = 0;
        for pitch in event.pitches.in_key(score.key_signature()).iter() {
            let key = Some(midi_key(pitch));
            if !keys.contains(&key) {
                keys[count] = key;
                count += 1;
            }
        }
        if count == 0 {
            return Err(ScoreError("Selecciona una nota o acorde para escuchar"));
        }
        let articulation = event.notation.articulation;
        let slurs = interpretation::Slurs::new(score, id.part, id.staff);
        let brief = tempo
            .frame_at(u64::from(event.duration.ticks()), rate)
            .min(u64::from(rate) / 4);
        Ok(Self {
            keys,
            level: interpretation::level(level, articulation),
            instrument: part.instrument(),
            technique: score.technique_at(id).unwrap_or_default(),
            articulation,
            legato: slurs.contains(tick),
            duration_frames: interpretation::sounding_end(
                0,
                brief,
                articulation,
                slurs.continues(tick),
                rate,
            )
            .max(1),
            sample_rate: rate,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use score_core::{
        Articulation, Connection, ConnectionKind, Duration, Dynamic, Event, Instrument, Notation,
        PlayingTechnique, Step,
    };

    #[test]
    fn brief_audition_and_playback_share_inherited_dynamics_articulation_and_gate() {
        for articulation in [
            None,
            Some(Articulation::Accent),
            Some(Articulation::Marcato),
            Some(Articulation::Staccato),
            Some(Articulation::Tenuto),
        ] {
            let mut score = Score::default();
            let first = score
                .insert(
                    0,
                    0,
                    Event {
                        pitches: Pitches::single(Pitch::new(Step::C, 4)),
                        duration: Duration::Quarter,
                        notation: Notation {
                            dynamic: Some(Dynamic::P),
                            ..Default::default()
                        },
                    },
                )
                .unwrap();
            let second = score
                .insert(
                    0,
                    1,
                    Event {
                        pitches: Pitches::single(Pitch::new(Step::D, 4)),
                        duration: Duration::Quarter,
                        notation: Notation {
                            articulation,
                            ..Default::default()
                        },
                    },
                )
                .unwrap();
            for rate in [16_000, 48_000, 192_000] {
                let plan = PlaybackPlan::new(&score, Tempo::default(), rate).unwrap();
                let preview =
                    Audition::for_element(&score, second, Tempo::default(), rate).unwrap();
                let start = plan.frame_for_element(second).unwrap();
                let note = plan
                    .messages
                    .iter()
                    .find(|m| m.frame == start && matches!(m.message, Message::NoteOn(_)))
                    .unwrap();
                assert_eq!(note.message, Message::NoteOn(preview.note(62)));
                let release = plan
                    .messages
                    .iter()
                    .find(|m| matches!(m.message, Message::NoteOff(62)))
                    .unwrap();
                assert_eq!(preview.duration_frames * 2, release.frame - start);
                assert!(preview.level < Dynamic::Mf.velocity());
                assert_eq!(preview.keys[0], Some(62));
                assert!(Audition::for_element(&score, first, Tempo::default(), rate).is_ok());
            }
        }
    }

    #[test]
    fn audition_resolves_hairpin_level_key_technique_and_enharmonic_chords() {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        let violin = score.add_part(Instrument::Violin).unwrap();
        score.remove_part(piano).unwrap();
        let mut ids = Vec::new();
        for i in 0..3 {
            ids.push(
                score
                    .insert_on(
                        violin,
                        StaffId::Upper,
                        0,
                        i,
                        Event {
                            pitches: Pitches::new(&[
                                Pitch::new(Step::C, 4)
                                    .unwrap()
                                    .with_accidental(Some(score_core::Accidental::Sharp)),
                                Pitch::new(Step::D, 4)
                                    .unwrap()
                                    .with_accidental(Some(score_core::Accidental::Flat)),
                                Pitch::new(Step::E, 4).unwrap(),
                            ])
                            .unwrap(),
                            duration: Duration::Quarter,
                            notation: Notation::default(),
                        },
                    )
                    .unwrap(),
            );
        }
        score.set_dynamic(ids[0], Some(Dynamic::P)).unwrap();
        score.set_dynamic(ids[2], Some(Dynamic::F)).unwrap();
        score
            .set_technique(ids[0], Some(PlayingTechnique::Pizzicato))
            .unwrap();
        score
            .add_connection(Connection {
                start: ids[0],
                end: ids[2],
                kind: ConnectionKind::Crescendo,
            })
            .unwrap();
        let preview = Audition::for_element(&score, ids[1], Tempo::default(), 48_000).unwrap();
        assert_eq!(preview.level, 78);
        assert_eq!(preview.technique, PlayingTechnique::Pizzicato);
        assert_eq!(preview.instrument, Instrument::Violin);
        assert_eq!(
            preview.keys,
            [Some(61), Some(64), None, None, None, None, None, None]
        );
        score.set_dynamic(ids[1], Some(Dynamic::Fff)).unwrap();
        score
            .set_articulation(ids[1], Some(Articulation::Marcato))
            .unwrap();
        assert_eq!(
            Audition::for_element(&score, ids[1], Tempo::default(), 48_000)
                .unwrap()
                .level,
            127
        );
    }
}
