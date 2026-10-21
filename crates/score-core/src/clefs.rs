//! A clef changes the staff coordinates, never the written or sounding pitches.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ClefMap {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial: Option<Clef>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub changes: BTreeMap<usize, Clef>,
}

impl Part {
    /// Divisi branches share their instrumental clef; grand-staff hands are independent.
    fn clef_staff(&self, staff: StaffId) -> StaffId {
        if self.instrument.is_grand_staff() {
            staff
        } else {
            StaffId::Upper
        }
    }
    pub fn clef_at(&self, staff: StaffId, bar: usize) -> Clef {
        self.clefs
            .get(&self.clef_staff(staff))
            .and_then(|map| {
                map.changes
                    .range(..=bar)
                    .next_back()
                    .map(|(_, clef)| *clef)
                    .or(map.initial)
            })
            .unwrap_or_else(|| self.default_clef(staff))
    }
    /// Explicit override here; `None` means the previous or instrumental clef applies.
    pub fn clef_change_at(&self, staff: StaffId, bar: usize) -> Option<Clef> {
        let map = self.clefs.get(&self.clef_staff(staff))?;
        if bar == 0 {
            map.initial
        } else {
            map.changes.get(&bar).copied()
        }
    }
    pub fn can_use_clef(&self, clef: Clef) -> bool {
        (self.instrument.clef() == Clef::Percussion) == (clef == Clef::Percussion)
    }
    pub(super) fn validate_clefs(&self, bars: usize) -> Result<(), ScoreError> {
        for (&staff, map) in &self.clefs {
            if (staff == StaffId::Lower && !self.instrument.is_grand_staff())
                || map.changes.keys().any(|&bar| bar == 0 || bar >= bars)
                || map
                    .initial
                    .into_iter()
                    .chain(map.changes.values().copied())
                    .any(|clef| !self.can_use_clef(clef))
            {
                return Err(ScoreError("Cambio de clave inválido"));
            }
        }
        Ok(())
    }
}

impl Score {
    fn clef_target(&self, part: PartId, staff: StaffId, bar: usize) -> Result<StaffId, ScoreError> {
        let part = self
            .part(part)
            .ok_or(ScoreError("El instrumento no existe"))?;
        if bar >= self.measures().len() {
            return Err(ScoreError("El compás no existe"));
        }
        if part.staff(staff).is_none() && !(staff == StaffId::Lower && part.bar_has_divisi(bar)) {
            return Err(ScoreError("El pentagrama no existe"));
        }
        Ok(part.clef_staff(staff))
    }
    pub fn set_clef_at(
        &mut self,
        part: PartId,
        staff: StaffId,
        bar: usize,
        clef: Clef,
    ) -> Result<bool, ScoreError> {
        let staff = self.clef_target(part, staff, bar)?;
        let current = self.part(part).unwrap();
        if !current.can_use_clef(clef) {
            return Err(ScoreError("La clave no es compatible con el instrumento"));
        }
        if current.clef_at(staff, bar) == clef {
            return Ok(false);
        }
        let map = self.part_mut(part)?.clefs.entry(staff).or_default();
        if bar == 0 {
            map.initial = Some(clef);
        } else {
            map.changes.insert(bar, clef);
        }
        Ok(true)
    }
    pub fn remove_clef_change(
        &mut self,
        part: PartId,
        staff: StaffId,
        bar: usize,
    ) -> Result<bool, ScoreError> {
        let staff = self.clef_target(part, staff, bar)?;
        let part = self.part_mut(part)?;
        let Some(map) = part.clefs.get_mut(&staff) else {
            return Ok(false);
        };
        let removed = if bar == 0 {
            map.initial.take()
        } else {
            map.changes.remove(&bar)
        };
        if map.initial.is_none() && map.changes.is_empty() {
            part.clefs.remove(&staff);
        }
        Ok(removed.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn structural_edits_carry_clefs_and_keep_initial_overrides() {
        let mut score = Score::new("Claves".into(), &[Instrument::Viola], 6).unwrap();
        let p = PartId(0);
        let s = StaffId::Upper;
        score.set_clef_at(p, s, 0, Clef::Treble).unwrap();
        score.set_clef_at(p, s, 2, Clef::Bass).unwrap();
        score.set_clef_at(p, s, 4, Clef::Alto).unwrap();
        score.insert_measure(0).unwrap();
        assert_eq!(score.parts()[0].clef_at(s, 0), Clef::Treble);
        assert_eq!(score.parts()[0].clef_change_at(s, 3), Some(Clef::Bass));
        assert_eq!(score.parts()[0].clef_change_at(s, 5), Some(Clef::Alto));
        score.remove_measures(1, 3).unwrap();
        assert_eq!(score.parts()[0].clef_at(s, 1), Clef::Bass);
        assert_eq!(score.parts()[0].clef_at(s, 2), Clef::Alto);
        score.remove_measures(0, 2).unwrap();
        assert_eq!(score.parts()[0].clef_at(s, 0), Clef::Alto);
        score.remove_measures(0, 2).unwrap();
        assert_eq!(score.parts()[0].clef_at(s, 0), Clef::Alto);
        score.validate().unwrap();
    }
    #[test]
    fn temporary_divisi_branches_share_the_instrumental_clef() {
        let mut score = Score::new("Claves".into(), &[Instrument::Cello], 6).unwrap();
        score
            .set_passage_divisi(PartId(0), 1, Some(3), true)
            .unwrap();
        score
            .set_clef_at(PartId(0), StaffId::Lower, 2, Clef::Tenor)
            .unwrap();
        assert_eq!(score.parts()[0].clef_at(StaffId::Upper, 2), Clef::Tenor);
        assert_eq!(score.parts()[0].clef_at(StaffId::Lower, 2), Clef::Tenor);
        score.set_passage_divisi(PartId(0), 0, None, false).unwrap();
        score.validate().unwrap();
        assert_eq!(score.parts()[0].clef_at(StaffId::Upper, 5), Clef::Tenor);
    }
    #[test]
    fn clef_changes_preserve_music_and_are_local_to_the_staff_and_bar() {
        let mut score =
            Score::new("Claves".into(), &[Instrument::Viola, Instrument::Piano], 8).unwrap();
        let id = score
            .insert_on(
                PartId(0),
                StaffId::Upper,
                3,
                0,
                Event {
                    pitches: Pitches::single(Pitch::new(Step::A, 5)),
                    duration: Duration::Whole,
                    notation: Default::default(),
                },
            )
            .unwrap();
        let before = *score.event(id).unwrap();
        let sounding = score.sounding_pitches(id).unwrap();
        score
            .set_clef_at(PartId(0), StaffId::Upper, 3, Clef::Treble)
            .unwrap();
        score
            .set_clef_at(PartId(0), StaffId::Upper, 6, Clef::Alto)
            .unwrap();
        score
            .set_clef_at(PartId(1), StaffId::Lower, 0, Clef::Tenor)
            .unwrap();
        assert_eq!(score.parts()[0].clef_at(StaffId::Upper, 2), Clef::Alto);
        assert_eq!(score.parts()[0].clef_at(StaffId::Upper, 5), Clef::Treble);
        assert_eq!(score.parts()[0].clef_at(StaffId::Upper, 6), Clef::Alto);
        assert_eq!(score.parts()[1].clef_at(StaffId::Upper, 0), Clef::Treble);
        assert_eq!(score.parts()[1].clef_at(StaffId::Lower, 0), Clef::Tenor);
        assert_eq!(*score.event(id).unwrap(), before);
        assert_eq!(score.sounding_pitches(id).unwrap(), sounding);
        assert!(
            score
                .remove_clef_change(PartId(0), StaffId::Upper, 3)
                .unwrap()
        );
        assert_eq!(score.parts()[0].clef_at(StaffId::Upper, 5), Clef::Alto);
        score.validate().unwrap();
    }
    #[test]
    fn invalid_targets_and_percussion_changes_are_atomic() {
        let mut score = Score::new(
            "Claves".into(),
            &[Instrument::Viola, Instrument::SnareDrum],
            2,
        )
        .unwrap();
        let before = score.clone();
        for (part, staff, bar, clef) in [
            (PartId(0), StaffId::Lower, 0, Clef::Bass),
            (PartId(0), StaffId::Upper, 2, Clef::Bass),
            (PartId(0), StaffId::Upper, 0, Clef::Percussion),
            (PartId(1), StaffId::Upper, 0, Clef::Treble),
        ] {
            assert!(score.set_clef_at(part, staff, bar, clef).is_err());
        }
        assert_eq!(score, before);
    }
}
