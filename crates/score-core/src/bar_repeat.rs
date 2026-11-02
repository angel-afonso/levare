//! The bar repeat: an empty bar marked with % plays the bar before it again.
use super::*;

impl Measure {
    /// Marked with %: empty, it plays the bar before it.
    pub fn repeats_previous(&self) -> bool {
        self.repeat
    }
}

impl Part {
    /// The bar whose music bar `bar` of `staff` plays: itself, or the
    /// nearest bar before it that is not a %.
    pub fn bar_repeat_source(&self, staff: StaffId, bar: usize) -> usize {
        let Some(staff) = self.staff(staff) else {
            return bar;
        };
        let mut source = bar;
        while source > 0 && staff.measures()[source].repeat {
            source -= 1;
        }
        source
    }
}

impl Score {
    /// Mark bar `bar` of a staff with %, emptying it, or remove the mark.
    /// Whether anything changed.
    pub fn set_bar_repeat(
        &mut self,
        part: PartId,
        staff: StaffId,
        bar: usize,
        on: bool,
    ) -> Result<bool, ScoreError> {
        if on && bar == 0 {
            return Err(ScoreError(
                "El primer compás no tiene un compás anterior que repetir",
            ));
        }
        if on && self.time_at(bar) != self.time_at(bar - 1) {
            return Err(ScoreError(
                "El compás de repetición necesita la misma métrica que el compás anterior",
            ));
        }
        let current = self
            .part_mut(part)?
            .staff_mut(staff)?
            .measures
            .get_mut(bar)
            .ok_or(ScoreError("El compás no existe"))?;
        if current.repeat == on {
            return Ok(false);
        }
        *current = Measure {
            repeat: on,
            ..Measure::default()
        };
        self.prune_connections();
        Ok(true)
    }

    pub(crate) fn validate_bar_repeats(&self) -> Result<(), ScoreError> {
        for part in &self.parts {
            for staff in part.staves() {
                for (bar, measure) in staff.measures().iter().enumerate() {
                    if measure.repeat
                        && (bar == 0
                            || !measure.is_empty()
                            || self.time_at(bar) != self.time_at(bar - 1))
                    {
                        return Err(ScoreError("Compás de repetición inválido"));
                    }
                }
            }
        }
        Ok(())
    }
}
