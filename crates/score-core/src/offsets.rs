//! User adjustments to the engraved positions of written marks.
use crate::{Connection, MarkFamily, MarkRef, Score, ScoreError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Position in tenths of a staff space; positive y points down.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Offset {
    pub dx: i16,
    pub dy: i16,
}

impl Offset {
    pub const ZERO: Self = Self { dx: 0, dy: 0 };
    pub const MAX: i16 = 400;

    pub fn is_zero(self) -> bool {
        self == Self::ZERO
    }

    pub fn is_valid(self) -> bool {
        (-Self::MAX..=Self::MAX).contains(&self.dx) && (-Self::MAX..=Self::MAX).contains(&self.dy)
    }
}

/// A written mark anchored to a bar rather than to an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BarMark {
    Tempo,
    Rehearsal,
    Segno,
    Coda,
    ToCoda,
    Fine,
    Jump,
    Ending,
}

impl BarMark {
    pub const ALL: [Self; 8] = [
        Self::Tempo,
        Self::Rehearsal,
        Self::Segno,
        Self::Coda,
        Self::ToCoda,
        Self::Fine,
        Self::Jump,
        Self::Ending,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BarMarkRef {
    pub measure: usize,
    pub mark: BarMark,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BarOffsets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tempo: Option<Offset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rehearsal: Option<Offset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segno: Option<Offset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coda: Option<Offset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_coda: Option<Offset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fine: Option<Offset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jump: Option<Offset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ending: Option<Offset>,
}

impl BarOffsets {
    pub fn is_empty(&self) -> bool {
        BarMark::ALL
            .into_iter()
            .all(|mark| self.get(mark).is_none())
    }

    pub fn get(&self, mark: BarMark) -> Option<Offset> {
        *self.slot(mark)
    }

    fn slot(&self, mark: BarMark) -> &Option<Offset> {
        match mark {
            BarMark::Tempo => &self.tempo,
            BarMark::Rehearsal => &self.rehearsal,
            BarMark::Segno => &self.segno,
            BarMark::Coda => &self.coda,
            BarMark::ToCoda => &self.to_coda,
            BarMark::Fine => &self.fine,
            BarMark::Jump => &self.jump,
            BarMark::Ending => &self.ending,
        }
    }

    pub fn slot_mut(&mut self, mark: BarMark) -> &mut Option<Offset> {
        match mark {
            BarMark::Tempo => &mut self.tempo,
            BarMark::Rehearsal => &mut self.rehearsal,
            BarMark::Segno => &mut self.segno,
            BarMark::Coda => &mut self.coda,
            BarMark::ToCoda => &mut self.to_coda,
            BarMark::Fine => &mut self.fine,
            BarMark::Jump => &mut self.jump,
            BarMark::Ending => &mut self.ending,
        }
    }
}

impl MarkRef {
    /// Whether this written mark supports a manual position adjustment.
    pub fn can_move(self) -> bool {
        matches!(
            self,
            Self::Dynamic(_)
                | Self::Articulation(_)
                | Self::Fermata(_)
                | Self::Technique(_)
                | Self::Harmonic(_)
        ) || matches!(self, Self::Connection(connection) if connection.kind != crate::ConnectionKind::Tie)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkOffsets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic: Option<Offset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub articulation: Option<Offset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fermata: Option<Offset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub technique: Option<Offset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harmonic: Option<Offset>,
}

impl MarkOffsets {
    pub fn is_empty(&self) -> bool {
        self.dynamic.is_none()
            && self.articulation.is_none()
            && self.fermata.is_none()
            && self.technique.is_none()
            && self.harmonic.is_none()
    }

    pub fn get(&self, family: MarkFamily) -> Option<Offset> {
        match family {
            MarkFamily::Dynamic => self.dynamic,
            MarkFamily::Articulation => self.articulation,
            MarkFamily::Fermata => self.fermata,
            MarkFamily::Technique => self.technique,
            MarkFamily::Harmonic => self.harmonic,
            _ => None,
        }
    }

    fn slot_mut(&mut self, family: MarkFamily) -> Option<&mut Option<Offset>> {
        match family {
            MarkFamily::Dynamic => Some(&mut self.dynamic),
            MarkFamily::Articulation => Some(&mut self.articulation),
            MarkFamily::Fermata => Some(&mut self.fermata),
            MarkFamily::Technique => Some(&mut self.technique),
            MarkFamily::Harmonic => Some(&mut self.harmonic),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionOffset {
    pub connection: Connection,
    pub offset: Offset,
}

impl Score {
    pub fn has_bar_mark(&self, reference: BarMarkRef) -> bool {
        let bar = reference.measure;
        if bar >= self.measures().len() {
            return false;
        }
        match reference.mark {
            BarMark::Tempo => bar == 0 || self.tempo_changes.contains_key(&bar),
            BarMark::Rehearsal => self.has_rehearsal_mark(bar),
            BarMark::Segno => self.bar_form(bar).segno,
            BarMark::Coda => self.bar_form(bar).coda,
            BarMark::ToCoda => self.bar_form(bar).to_coda,
            BarMark::Fine => self.bar_form(bar).fine,
            BarMark::Jump => self.bar_form(bar).jump.is_some(),
            BarMark::Ending => self.endings().any(|ending| ending.start == bar),
        }
    }

    pub fn bar_offsets(&self) -> &BTreeMap<usize, BarOffsets> {
        &self.bar_offsets
    }

    pub fn bar_mark_offset(&self, reference: BarMarkRef) -> Offset {
        self.bar_offsets
            .get(&reference.measure)
            .and_then(|offsets| offsets.get(reference.mark))
            .unwrap_or(Offset::ZERO)
    }

    pub fn set_bar_mark_offset(
        &mut self,
        reference: BarMarkRef,
        offset: Offset,
    ) -> Result<bool, ScoreError> {
        if !self.has_bar_mark(reference) {
            return Err(ScoreError("La marca ya no está en la partitura"));
        }
        if !offset.is_valid() {
            return Err(ScoreError("Desplazamiento fuera de la página"));
        }
        let current = self.bar_mark_offset(reference);
        if current == offset {
            return Ok(false);
        }
        let entry = self.bar_offsets.entry(reference.measure).or_default();
        *entry.slot_mut(reference.mark) = (!offset.is_zero()).then_some(offset);
        if entry.is_empty() {
            self.bar_offsets.remove(&reference.measure);
        }
        Ok(true)
    }

    pub fn prune_bar_offsets(&mut self) {
        let old = std::mem::take(&mut self.bar_offsets);
        for (measure, mut offsets) in old {
            for mark in BarMark::ALL {
                if !self.has_bar_mark(BarMarkRef { measure, mark }) {
                    *offsets.slot_mut(mark) = None;
                }
            }
            if !offsets.is_empty() {
                self.bar_offsets.insert(measure, offsets);
            }
        }
    }

    pub(crate) fn validate_bar_offsets(&self) -> Result<(), ScoreError> {
        let invalid = ScoreError("Desplazamiento de marca de compás inválido");
        for (&measure, offsets) in &self.bar_offsets {
            if offsets.is_empty() {
                return Err(invalid);
            }
            for mark in BarMark::ALL {
                if let Some(offset) = offsets.get(mark)
                    && (offset.is_zero()
                        || !offset.is_valid()
                        || !self.has_bar_mark(BarMarkRef { measure, mark }))
                {
                    return Err(invalid);
                }
            }
        }
        Ok(())
    }

    pub fn mark_offset(&self, mark: MarkRef) -> Offset {
        match mark {
            MarkRef::Connection(connection) => self
                .connection_offsets
                .iter()
                .find(|entry| entry.connection == connection)
                .map_or(Offset::ZERO, |entry| entry.offset),
            _ => self
                .event(mark.anchor())
                .and_then(|event| {
                    let family = self.mark_kind(mark)?.family();
                    event.notation.offsets.get(family)
                })
                .unwrap_or(Offset::ZERO),
        }
    }

    pub fn set_mark_offset(&mut self, mark: MarkRef, offset: Offset) -> Result<bool, ScoreError> {
        let family = self
            .mark_kind(mark)
            .ok_or(ScoreError("La marca ya no está en la partitura"))?
            .family();
        if !mark.can_move() {
            return Err(ScoreError("Esta marca no se puede desplazar"));
        }
        if !offset.is_valid() {
            return Err(ScoreError("Desplazamiento fuera de la página"));
        }
        if let MarkRef::Connection(connection) = mark {
            let current = self
                .connection_offsets
                .iter()
                .position(|entry| entry.connection == connection);
            if current
                .map(|index| self.connection_offsets[index].offset)
                .unwrap_or(Offset::ZERO)
                == offset
            {
                return Ok(false);
            }
            if let Some(index) = current {
                if offset.is_zero() {
                    self.connection_offsets.remove(index);
                } else {
                    self.connection_offsets[index].offset = offset;
                }
            } else if !offset.is_zero() {
                self.connection_offsets
                    .push_back(ConnectionOffset { connection, offset });
            }
        } else {
            let id = mark.anchor();
            let notation = &mut Arc::make_mut(
                &mut self.part_mut(id.part)?.staff_mut(id.staff)?.measures[id.measure].events,
            )[id.event]
                .notation;
            let slot = notation
                .offsets
                .slot_mut(family)
                .expect("supported point mark");
            let next = (!offset.is_zero()).then_some(offset);
            if *slot == next {
                return Ok(false);
            }
            *slot = next;
        }
        Ok(true)
    }

    pub fn connection_offsets(&self) -> &imbl::Vector<ConnectionOffset> {
        &self.connection_offsets
    }

    pub(crate) fn validate_offsets(&self) -> Result<(), ScoreError> {
        let invalid = ScoreError("Desplazamiento de marca inválido");
        let mut seen = std::collections::HashSet::new();
        for entry in &self.connection_offsets {
            if entry.offset.is_zero()
                || !entry.offset.is_valid()
                || entry.connection.kind == crate::ConnectionKind::Tie
                || !self.connections.contains(&entry.connection)
                || !seen.insert(entry.connection)
            {
                return Err(invalid);
            }
        }
        for part in &self.parts {
            for staff in &part.staves {
                for measure in &staff.measures {
                    for (_, event) in measure.iter() {
                        let notation = event.notation;
                        let offsets = notation.offsets;
                        for (offset, written) in [
                            (offsets.dynamic, notation.dynamic.is_some()),
                            (
                                offsets.articulation,
                                notation.articulation.is_some() && !event.pitches.is_empty(),
                            ),
                            (offsets.fermata, notation.fermata),
                            (offsets.technique, notation.technique.is_some()),
                            (offsets.harmonic, notation.harmonic.is_some()),
                        ] {
                            if offset.is_some_and(|value| {
                                value.is_zero() || !value.is_valid() || !written
                            }) {
                                return Err(invalid);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
