//! Conductor signatures persist from their bar until the next explicit change.
use super::*;

impl Score {
    pub fn time_at(&self, measure: usize) -> TimeSignature {
        self.time_changes
            .range(..=measure)
            .next_back()
            .map_or(self.time, |(_, time)| *time)
    }
    pub fn key_signature_at(&self, measure: usize) -> KeySignature {
        self.key_changes
            .range(..=measure)
            .next_back()
            .map_or(self.key_signature, |(_, key)| *key)
    }
    pub fn time_change_at(&self, measure: usize) -> Option<TimeSignature> {
        if measure == 0 {
            Some(self.time)
        } else {
            self.time_changes.get(&measure).copied()
        }
    }
    pub fn key_change_at(&self, measure: usize) -> Option<KeySignature> {
        if measure == 0 {
            Some(self.key_signature)
        } else {
            self.key_changes.get(&measure).copied()
        }
    }
    /// Also extrapolates beyond the last bar for bounded paste/append operations.
    pub fn measure_start_tick(&self, measure: usize) -> u64 {
        let (mut start, mut tick, mut time) = (0, 0, self.time);
        for (&bar, &next) in self.time_changes.range(..measure) {
            tick += (bar - start) as u64 * u64::from(time.capacity());
            start = bar;
            time = next;
        }
        tick + (measure - start) as u64 * u64::from(time.capacity())
    }
    pub fn total_ticks(&self) -> u64 {
        self.measure_start_tick(self.measures().len())
    }
    /// At a boundary, the new bar owns the tick. It may be just past the score.
    pub fn measure_at_tick(&self, tick: u64) -> (usize, u16) {
        let (mut start, mut begin, mut time) = (0, 0, self.time);
        for (&bar, &next) in &self.time_changes {
            let end = begin + (bar - start) as u64 * u64::from(time.capacity());
            if tick < end {
                break;
            }
            start = bar;
            begin = end;
            time = next;
        }
        let capacity = u64::from(time.capacity());
        (
            start + ((tick - begin) / capacity) as usize,
            ((tick - begin) % capacity) as u16,
        )
    }
    pub fn set_time_signature_at(
        &mut self,
        measure: usize,
        time: TimeSignature,
    ) -> Result<bool, ScoreError> {
        if !time.is_valid() {
            return Err(ScoreError("Indicación de compás inválida"));
        }
        if measure >= self.measures().len() {
            return Err(ScoreError("El compás no existe"));
        }
        if self.time_at(measure) == time {
            return Ok(false);
        }
        let mut next = self.clone();
        if measure == 0 {
            next.time = time;
        } else {
            next.time_changes.insert(measure, time);
        }
        self.commit_time_change(next)
    }
    pub fn remove_time_change(&mut self, measure: usize) -> Result<bool, ScoreError> {
        let mut next = self.clone();
        if next.time_changes.remove(&measure).is_none() {
            return Ok(false);
        }
        self.commit_time_change(next)
    }
    fn commit_time_change(&mut self, mut next: Self) -> Result<bool, ScoreError> {
        for part in &next.parts {
            for staff in &part.staves {
                for (index, bar) in staff.measures.iter().enumerate() {
                    if bar.used_ticks() > next.time_at(index).capacity() {
                        return Err(ScoreError(
                            "La nueva métrica no permite conservar todas las notas de un compás. Reduce su contenido antes de aplicarla",
                        ));
                    }
                }
            }
        }
        next.prune_connections();
        next.validate()?;
        *self = next;
        Ok(true)
    }
    pub fn set_key_signature_at(
        &mut self,
        measure: usize,
        key: KeySignature,
    ) -> Result<bool, ScoreError> {
        if measure >= self.measures().len() {
            return Err(ScoreError("El compás no existe"));
        }
        if self.key_signature_at(measure) == key {
            return Ok(false);
        }
        if measure == 0 {
            self.key_signature = key;
        } else {
            self.key_changes.insert(measure, key);
        }
        self.prune_connections();
        Ok(true)
    }
    pub fn remove_key_change(&mut self, measure: usize) -> bool {
        let changed = self.key_changes.remove(&measure).is_some();
        if changed {
            self.prune_connections();
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn meter(numerator: u8, denominator: u8) -> TimeSignature {
        TimeSignature {
            numerator,
            denominator,
        }
    }
    fn note(duration: Duration) -> Event {
        Event {
            pitches: Pitches::single(Pitch::new(Step::F, 4)),
            duration,
            notation: Default::default(),
        }
    }
    #[test]
    fn signatures_persist_until_the_next_change_and_ticks_follow_each_meter() {
        let mut score = Score::default();
        score.set_time_signature_at(1, meter(3, 4)).unwrap();
        score.set_time_signature_at(3, meter(6, 8)).unwrap();
        let key = KeySignature::new(1, KeyMode::Major).unwrap();
        score.set_key_signature_at(1, key).unwrap();
        score
            .set_key_signature_at(3, KeySignature::default())
            .unwrap();
        assert_eq!(
            (0..=4)
                .map(|i| score.measure_start_tick(i))
                .collect::<Vec<_>>(),
            [0, 192, 336, 480, 624]
        );
        assert_eq!(score.measure_at_tick(192), (1, 0));
        assert_eq!(score.measure_at_tick(479), (2, 143));
        assert_eq!(score.measure_at_tick(624), (4, 0));
        assert_eq!(score.time_at(2), meter(3, 4));
        assert_eq!(score.key_signature_at(2), key);
        assert_eq!(score.key_signature_at(3), KeySignature::default());
        score.set_time_signature_at(0, meter(2, 4)).unwrap();
        assert_eq!(score.measure_start_tick(1), 96);
        assert_eq!(score.time_at(1), meter(3, 4));
        score.validate().unwrap();
    }
    #[test]
    fn shrinking_a_bar_is_atomic_and_checks_both_hands_and_other_instruments() {
        let mut score = Score::default();
        let part = score.add_part(Instrument::Flute).unwrap();
        score.insert_in(part, 2, 0, note(Duration::Whole)).unwrap();
        let before = score.clone();
        assert!(score.set_time_signature_at(1, meter(3, 4)).is_err());
        assert_eq!(score, before);
        score.set_time_signature_at(3, meter(3, 4)).unwrap();
        let before = score.clone();
        assert!(score.set_time_signature_at(0, meter(3, 4)).is_err());
        assert_eq!(score, before);
        assert!(score.set_time_signature_at(0, meter(4, 0)).is_err());
    }
    #[test]
    fn removing_changes_restores_inheritance_and_rejects_overfull_bars() {
        let mut score = Score::default();
        score.set_time_signature_at(0, meter(3, 4)).unwrap();
        score.set_time_signature_at(1, meter(4, 4)).unwrap();
        score.insert(1, 0, note(Duration::Whole)).unwrap();
        let before = score.clone();
        assert!(score.remove_time_change(1).is_err());
        assert_eq!(score, before);
        score
            .set_key_signature_at(1, KeySignature::new(-3, KeyMode::Minor).unwrap())
            .unwrap();
        assert!(score.remove_key_change(1));
        assert_eq!(score.key_signature_at(2), KeySignature::default());
    }
    #[test]
    fn modulation_rechecks_ties_with_each_endpoints_key() {
        let mut score = Score::default();
        let a = score.insert(0, 0, note(Duration::Whole)).unwrap();
        let b = score.insert(1, 0, note(Duration::Quarter)).unwrap();
        score
            .add_connection(Connection {
                start: a,
                end: b,
                kind: ConnectionKind::Tie,
            })
            .unwrap();
        score
            .set_key_signature_at(1, KeySignature::new(1, KeyMode::Major).unwrap())
            .unwrap();
        assert!(score.connections().is_empty());
        score
            .set_accidental(b, 0, Some(Accidental::Natural))
            .unwrap();
        score
            .add_connection(Connection {
                start: a,
                end: b,
                kind: ConnectionKind::Tie,
            })
            .unwrap();
        score.validate().unwrap();
    }
}
