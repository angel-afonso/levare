//! The full score in concert pitch, as Sibelius' «Transposing Score» off or
//! MuseScore's «Concert Pitch» on: a view derived from the stored written
//! pitches, never stored itself.
//!
//! Each transposing part is redrawn at its sounding pitch within the octave,
//! with the key of the piece. Whole octaves stay, as conductors expect: the
//! piccolo, the double bass, the guitar, the glockenspiel and the tenor voice
//! look as in their parts, and the bass clarinet and the tenor and baritone
//! saxophones keep their clef and sound an octave below what the view shows.
use super::*;

impl Transposition {
    /// The whole octaves of the interval, which a concert-pitch score keeps.
    pub fn octaves(self) -> Self {
        let octaves = self.chromatic / 12;
        Self::new(7 * octaves, 12 * octaves)
    }
}

impl Score {
    /// This score with every transposing part written at concert pitch, its
    /// octave transposition kept. Every note sounds as before and keeps its
    /// identity, so selections, layouts and playback carry over.
    pub fn in_concert_pitch(&self) -> Score {
        let mut view = self.clone();
        for part in self.parts.iter() {
            let real = part.transposition;
            if real != real.octaves() && part.instrument.percussion_key().is_none() {
                view.retranspose(part.id, real.octaves(), None);
            }
        }
        view
    }

    /// The written score behind `self`, a concert-pitch view that an edit
    /// changed. `shown` is the view before the edit and `written` the score
    /// it was made from: notes the edit left alone get their written pitches
    /// back exactly; the rest are rewritten for each part's own transposition.
    pub fn from_concert_pitch(&self, shown: &Score, written: &Score) -> Score {
        let mut result = self.clone();
        for part in self.parts.iter() {
            let Some(real) = written.part(part.id).map(Part::transposition) else {
                // New parts were added with their own transposition.
                continue;
            };
            if real != real.octaves()
                && part.transposition == real.octaves()
                && part.instrument.percussion_key().is_none()
            {
                result.retranspose(part.id, real, Some((shown, written)));
            }
        }
        result
    }

    /// Rewrite part `id` for `to`, so that every note sounds as before.
    /// With `keep`, pitches equal to the shown ones take the written ones.
    fn retranspose(&mut self, id: PartId, to: Transposition, keep: Option<(&Score, &Score)>) {
        let measures = self.measures().len();
        let Ok(part) = self.part_mut(id) else {
            return;
        };
        let from = part.transposition;
        let delta = Transposition::new(
            from.diatonic.saturating_sub(to.diatonic),
            from.chromatic.saturating_sub(to.chromatic),
        );
        let interval = Interval::new(delta.diatonic, delta.chromatic);
        let keys: Vec<_> = (0..measures)
            .map(|measure| {
                let concert = self.key_signature_at(measure);
                (from.written_key(concert), to.written_key(concert))
            })
            .collect();
        let part = self.part_mut(id).expect("part checked above");
        part.transposition = to;
        for staff in part.staves.iter_mut() {
            let staff_id = staff.id;
            for (measure, bar) in staff.measures.iter_mut().enumerate() {
                let Some(&(from_key, to_key)) = keys.get(measure) else {
                    continue;
                };
                for (event_id, event) in Arc::make_mut(&mut bar.events).iter_mut() {
                    let element = ElementId {
                        part: id,
                        staff: staff_id,
                        measure,
                        event: event_id,
                    };
                    let kept = keep.and_then(|(shown, written)| {
                        Some((*shown.event(element)?, *written.event(element)?))
                    });
                    rewrite(event, kept, delta, interval, from_key, to_key);
                }
            }
        }
    }
}

/// The pitches of `event` for another transposition, `delta` away; those
/// equal to the `kept` shown event take its written ones instead.
fn rewrite(
    event: &mut Event,
    kept: Option<(Event, Event)>,
    delta: Transposition,
    interval: Interval,
    from_key: KeySignature,
    to_key: KeySignature,
) {
    let moved = |pitch: Pitch| -> Option<Pitch> {
        let moved = delta
            .pitches(Pitches::single(Some(pitch)), from_key)
            .ok()?
            .first()?;
        // A note that followed its key follows the other key too, unless
        // the other key says otherwise.
        let implied = to_key.accidental(moved.step).unwrap_or(Accidental::Natural);
        Some(
            if pitch.accidental().is_none() && moved.accidental() == Some(implied) {
                moved.with_accidental(None)
            } else {
                moved
            },
        )
    };
    let shown = kept.map(|(shown, _)| shown);
    let written = kept.map(|(_, written)| written);
    if let Some(written) = written.filter(|_| shown.map(|s| s.pitches) == Some(event.pitches)) {
        event.pitches = written.pitches;
    } else if let Some(pitches) = event
        .pitches
        .iter()
        .map(moved)
        .collect::<Option<Vec<_>>>()
        .and_then(|pitches| Pitches::new(&pitches).ok())
    {
        event.pitches = pitches;
    }
    let grace = event.notation.grace;
    if let Some(written) = written.filter(|_| shown.map(|s| s.notation.grace) == Some(grace)) {
        event.notation.grace = written.notation.grace;
    } else if let Some(moved) = grace.and_then(|grace| grace.map(moved)) {
        event.notation.grace = Some(moved);
    }
    let chord = event.notation.chord;
    if let Some(written) = written.filter(|_| shown.map(|s| s.notation.chord) == Some(chord)) {
        event.notation.chord = written.notation.chord;
    } else {
        event.notation.chord = chord.map(|chord| chord.transposed(interval));
    }
}

#[cfg(test)]
mod tests;
