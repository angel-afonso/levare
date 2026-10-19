//! Expressive marks and stable, same-staff connections.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Articulation {
    Staccato,
    Tenuto,
    Accent,
    Marcato,
}
impl Articulation {
    pub const ALL: [Self; 4] = [Self::Staccato, Self::Tenuto, Self::Accent, Self::Marcato];
    pub const fn label(self) -> &'static str {
        match self {
            Self::Staccato => "Staccato",
            Self::Tenuto => "Tenuto",
            Self::Accent => "Acento",
            Self::Marcato => "Marcato",
        }
    }
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Staccato => "•",
            Self::Tenuto => "—",
            Self::Accent => ">",
            Self::Marcato => "∧",
        }
    }
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Staccato => "\u{e4a2}",
            Self::Tenuto => "\u{e4a4}",
            Self::Accent => "\u{e4a0}",
            Self::Marcato => "\u{e4ac}",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Dynamic {
    Ppp,
    Pp,
    P,
    Mp,
    Mf,
    F,
    Ff,
    Fff,
}
impl Dynamic {
    pub const ALL: [Self; 8] = [
        Self::Ppp,
        Self::Pp,
        Self::P,
        Self::Mp,
        Self::Mf,
        Self::F,
        Self::Ff,
        Self::Fff,
    ];
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ppp => "ppp",
            Self::Pp => "pp",
            Self::P => "p",
            Self::Mp => "mp",
            Self::Mf => "mf",
            Self::F => "f",
            Self::Ff => "ff",
            Self::Fff => "fff",
        }
    }
    pub fn velocity(self) -> u8 {
        match self {
            Self::Ppp => 32,
            Self::Pp => 44,
            Self::P => 56,
            Self::Mp => 72,
            Self::Mf => 88,
            Self::F => 100,
            Self::Ff => 112,
            Self::Fff => 124,
        }
    }
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Ppp => "\u{e520}\u{e520}\u{e520}",
            Self::Pp => "\u{e520}\u{e520}",
            Self::P => "\u{e520}",
            Self::Mp => "\u{e521}\u{e520}",
            Self::Mf => "\u{e521}\u{e522}",
            Self::F => "\u{e522}",
            Self::Ff => "\u{e522}\u{e522}",
            Self::Fff => "\u{e522}\u{e522}\u{e522}",
        }
    }
}
/// A persistent instruction for bowed strings, independent of articulation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlayingTechnique {
    #[default]
    Arco,
    Pizzicato,
}
impl PlayingTechnique {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Arco => "arco",
            Self::Pizzicato => "pizz.",
        }
    }
    pub fn program(self, instrument: Instrument) -> u8 {
        if self == Self::Pizzicato && instrument.family() == InstrumentFamily::Strings {
            45 // General MIDI Pizzicato Strings (zero based).
        } else {
            instrument.program()
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub technique: Option<PlayingTechnique>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub articulation: Option<Articulation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic: Option<Dynamic>,
    /// Hold the note or rest; written over it, independent of articulation.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fermata: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectionKind {
    Tie,
    Slur,
    Crescendo,
    Diminuendo,
}
impl ConnectionKind {
    pub fn is_hairpin(self) -> bool {
        matches!(self, Self::Crescendo | Self::Diminuendo)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Tie => "Ligadura de unión",
            Self::Slur => "Ligadura de expresión",
            Self::Crescendo => "Crescendo",
            Self::Diminuendo => "Diminuendo",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Connection {
    pub start: ElementId,
    pub end: ElementId,
    pub kind: ConnectionKind,
}

impl Score {
    pub fn set_duration(&mut self, id: ElementId, duration: Duration) -> Result<(), ScoreError> {
        let old = self
            .event(id)
            .ok_or(ScoreError("Selecciona una nota o silencio"))?
            .duration;
        let measure = &self
            .part(id.part)
            .unwrap()
            .staff(id.staff)
            .unwrap()
            .measures()[id.measure];
        if measure.used_ticks() - old.ticks() + duration.ticks() > self.time().capacity() {
            return Err(ScoreError("La duración nueva no cabe en el compás"));
        }
        Arc::make_mut(
            &mut self.part_mut(id.part)?.staff_mut(id.staff)?.measures[id.measure].events,
        )[id.event]
            .duration = duration;
        self.prune_connections();
        Ok(())
    }

    /// Effective instruction at this event, including its own mark.
    pub fn technique_at(&self, id: ElementId) -> Option<PlayingTechnique> {
        let staff = self.part(id.part)?.staff(id.staff)?;
        let mut technique = PlayingTechnique::Arco;
        for (measure, bar) in staff.measures().iter().enumerate().take(id.measure + 1) {
            for (event_id, event) in bar.iter() {
                if let Some(mark) = event.notation.technique {
                    technique = mark;
                }
                if measure == id.measure && event_id == id.event {
                    return Some(technique);
                }
            }
        }
        None
    }
    pub fn set_technique(
        &mut self,
        id: ElementId,
        value: Option<PlayingTechnique>,
    ) -> Result<(), ScoreError> {
        self.event(id)
            .ok_or(ScoreError("Selecciona una nota o silencio"))?;
        if value.is_some()
            && self.part(id.part).unwrap().instrument().family() != InstrumentFamily::Strings
        {
            return Err(ScoreError(
                "Pizzicato y arco son técnicas de violín, viola, violonchelo y contrabajo",
            ));
        }
        Arc::make_mut(
            &mut self.part_mut(id.part)?.staff_mut(id.staff)?.measures[id.measure].events,
        )[id.event]
            .notation
            .technique = value;
        Ok(())
    }

    pub fn connections(&self) -> &Vector<Connection> {
        &self.connections
    }
    pub fn event_tick(&self, id: ElementId) -> Option<u64> {
        let measure = self
            .part(id.part)?
            .staff(id.staff)?
            .measures()
            .get(id.measure)?;
        let mut tick = id.measure as u64 * u64::from(self.time.capacity());
        for (event, value) in measure.iter() {
            if event == id.event {
                return Some(tick);
            }
            tick += u64::from(value.duration.ticks());
        }
        None
    }
    pub fn next_event(&self, id: ElementId) -> Option<ElementId> {
        let staff = self.part(id.part)?.staff(id.staff)?;
        let index = staff.measures().get(id.measure)?.index_of(id.event)?;
        if let Some(event) = staff.measures()[id.measure].id_at(index + 1) {
            return Some(ElementId { event, ..id });
        }
        for measure in id.measure + 1..staff.measures().len() {
            if let Some(event) = staff.measures()[measure].id_at(0) {
                return Some(ElementId {
                    event,
                    measure,
                    ..id
                });
            }
        }
        None
    }
    fn valid_connection(&self, c: Connection) -> bool {
        let (Some(a), Some(b), Some(start), Some(end)) = (
            self.event(c.start),
            self.event(c.end),
            self.event_tick(c.start),
            self.event_tick(c.end),
        ) else {
            return false;
        };
        c.start.part == c.end.part
            && c.start.staff == c.end.staff
            && start < end
            && (c.kind.is_hairpin() || (!a.pitches.is_empty() && !b.pitches.is_empty()))
            && (c.kind.is_hairpin()
                || c.kind == ConnectionKind::Slur
                || (end == start + u64::from(a.duration.ticks())
                    && self.next_event(c.start) == Some(c.end)
                    && a.pitches
                        .in_key(self.key_signature)
                        .iter()
                        .map(|p| (p.diatonic(), p.alteration()))
                        .eq(b
                            .pitches
                            .in_key(self.key_signature)
                            .iter()
                            .map(|p| (p.diatonic(), p.alteration())))))
    }
    pub fn add_connection(&mut self, c: Connection) -> Result<(), ScoreError> {
        if !self.valid_connection(c) {
            return Err(ScoreError(
                "Elige dos posiciones distintas y en orden del mismo pentagrama. La unión requiere notas o acordes iguales y consecutivos",
            ));
        }
        if self.connections.contains(&c) {
            return Ok(());
        }
        if self.connections.len() >= 4096 {
            return Err(ScoreError("Máximo de 4096 ligaduras"));
        }
        if self
            .connections
            .iter()
            .any(|other| other.kind == c.kind && (other.start == c.start || other.end == c.end))
        {
            return Err(ScoreError(
                "Este extremo ya tiene una ligadura del mismo tipo",
            ));
        }
        if c.kind == ConnectionKind::Slur || c.kind.is_hairpin() {
            let start = self.event_tick(c.start).unwrap();
            let end = self.event_tick(c.end).unwrap();
            if self.connections.iter().any(|other| {
                (if c.kind.is_hairpin() {
                    other.kind.is_hairpin()
                } else {
                    other.kind == c.kind
                }) && other.start.part == c.start.part
                    && other.start.staff == c.start.staff
                    && self.event_tick(other.start).unwrap() < end
                    && self.event_tick(other.end).unwrap() > start
            }) {
                return Err(ScoreError(
                    "Las ligaduras de expresión o los reguladores del mismo pentagrama no pueden solaparse",
                ));
            }
        }
        self.connections.push_back(c);
        Ok(())
    }
    pub fn connection_contains(&self, c: Connection, id: ElementId) -> bool {
        if !c.kind.is_hairpin() {
            return c.start == id || c.end == id;
        }
        id.part == c.start.part
            && id.staff == c.start.staff
            && self.event_tick(id).is_some_and(|tick| {
                self.event_tick(c.start).unwrap() <= tick && tick <= self.event_tick(c.end).unwrap()
            })
    }
    pub fn remove_connection(&mut self, connection: Connection) {
        self.connections.retain(|c| *c != connection);
    }
    pub fn remove_connections(&mut self, id: ElementId, kind: ConnectionKind) {
        let remaining = self
            .connections
            .iter()
            .copied()
            .filter(|c| c.kind != kind || !self.connection_contains(*c, id))
            .collect();
        self.connections = remaining;
    }
    pub(super) fn prune_connections(&mut self) {
        let valid: Vector<_> = self
            .connections
            .iter()
            .copied()
            .filter(|c| self.valid_connection(*c))
            .collect();
        self.connections = valid;
    }
    pub(super) fn validate_connections(&self) -> Result<(), ScoreError> {
        if self.connections.len() > 4096 {
            return Err(ScoreError("Demasiadas ligaduras"));
        }
        let mut starts = std::collections::HashSet::new();
        let mut ends = std::collections::HashSet::new();
        let mut slurs = Vec::new();
        let mut hairpins = Vec::new();
        for c in &self.connections {
            if !self.valid_connection(*c)
                || !starts.insert((c.start, c.kind))
                || !ends.insert((c.end, c.kind))
            {
                return Err(ScoreError("Ligadura inválida o duplicada"));
            }
            if c.kind.is_hairpin() {
                hairpins.push((
                    c.start.part.0,
                    c.start.staff as u8,
                    self.event_tick(c.start).unwrap(),
                    self.event_tick(c.end).unwrap(),
                ));
            }
            if c.kind == ConnectionKind::Slur {
                slurs.push((
                    c.start.part.0,
                    c.start.staff as u8,
                    self.event_tick(c.start).unwrap(),
                    self.event_tick(c.end).unwrap(),
                ));
            }
        }
        hairpins.sort_unstable();
        if hairpins
            .windows(2)
            .any(|p| p[0].0 == p[1].0 && p[0].1 == p[1].1 && p[0].3 > p[1].2)
        {
            return Err(ScoreError("Reguladores solapados"));
        }
        slurs.sort_unstable();
        if slurs
            .windows(2)
            .any(|p| p[0].0 == p[1].0 && p[0].1 == p[1].1 && p[0].3 > p[1].2)
        {
            return Err(ScoreError("Ligaduras de expresión solapadas"));
        }
        Ok(())
    }
    pub fn set_articulation(
        &mut self,
        id: ElementId,
        value: Option<Articulation>,
    ) -> Result<(), ScoreError> {
        let event = self
            .event(id)
            .ok_or(ScoreError("Selecciona una nota o acorde"))?;
        if event.pitches.is_empty() {
            return Err(ScoreError("Un silencio no admite articulaciones"));
        }
        Arc::make_mut(
            &mut self.part_mut(id.part)?.staff_mut(id.staff)?.measures[id.measure].events,
        )[id.event]
            .notation
            .articulation = value;
        Ok(())
    }
    pub fn set_dynamic(&mut self, id: ElementId, value: Option<Dynamic>) -> Result<(), ScoreError> {
        self.event(id)
            .ok_or(ScoreError("Selecciona una nota o silencio"))?;
        Arc::make_mut(
            &mut self.part_mut(id.part)?.staff_mut(id.staff)?.measures[id.measure].events,
        )[id.event]
            .notation
            .dynamic = value;
        Ok(())
    }
    pub fn set_fermata(&mut self, id: ElementId, value: bool) -> Result<(), ScoreError> {
        self.event(id)
            .ok_or(ScoreError("Selecciona una nota o silencio"))?;
        Arc::make_mut(
            &mut self.part_mut(id.part)?.staff_mut(id.staff)?.measures[id.measure].events,
        )[id.event]
            .notation
            .fermata = value;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn note(step: Step, duration: Duration) -> Event {
        Event {
            pitches: Pitches::single(Pitch::new(step, 4)),
            duration,
            notation: Notation::default(),
        }
    }
    #[test]
    fn connections_require_order_identity_continuity_and_matching_chords() {
        let mut s = Score::default();
        let a = s.insert(0, 0, note(Step::C, Duration::Half)).unwrap();
        let b = s.insert(0, 1, note(Step::C, Duration::Half)).unwrap();
        let c = s.insert(1, 0, note(Step::D, Duration::Quarter)).unwrap();
        let tie = Connection {
            start: a,
            end: b,
            kind: ConnectionKind::Tie,
        };
        s.add_connection(tie).unwrap();
        s.add_connection(tie).unwrap();
        assert_eq!(s.connections.len(), 1);
        assert!(
            s.add_connection(Connection {
                start: b,
                end: c,
                ..tie
            })
            .is_err()
        );
        assert!(
            s.add_connection(Connection {
                start: b,
                end: a,
                kind: ConnectionKind::Slur
            })
            .is_err()
        );
        let left = s
            .insert_on(a.part, StaffId::Lower, 0, 0, note(Step::C, Duration::Half))
            .unwrap();
        assert!(s.add_connection(Connection { end: left, ..tie }).is_err());
        s.add_connection(Connection {
            start: a,
            end: c,
            kind: ConnectionKind::Slur,
        })
        .unwrap();
        assert!(
            s.add_connection(Connection {
                start: b,
                end: c,
                kind: ConnectionKind::Slur
            })
            .is_err()
        );
        let snapshot = s.clone();
        s.transpose(b, 1).unwrap();
        assert!(s.connections.iter().all(|c| c.kind != ConnectionKind::Tie));
        assert_eq!(snapshot.connections.len(), 2);
        s.remove(c).unwrap();
        assert!(s.connections.is_empty());
        s.validate().unwrap();
        // A bar's implicit remainder breaks adjacency in musical time.
        let mut gap = Score::default();
        let a = gap.insert(0, 0, note(Step::C, Duration::Half)).unwrap();
        let b = gap.insert(1, 0, note(Step::C, Duration::Half)).unwrap();
        assert!(
            gap.add_connection(Connection {
                start: a,
                end: b,
                kind: ConnectionKind::Tie
            })
            .is_err()
        );
    }
    #[test]
    fn invalidated_ties_are_removed_on_insertion_and_key_changes() {
        let mut s = Score::default();
        let a = s.insert(0, 0, note(Step::F, Duration::Quarter)).unwrap();
        let b = s.insert(0, 1, note(Step::F, Duration::Quarter)).unwrap();
        s.set_accidental(b, 0, Some(Accidental::Natural)).unwrap();
        let tie = Connection {
            start: a,
            end: b,
            kind: ConnectionKind::Tie,
        };
        s.add_connection(tie).unwrap();
        s.set_key_signature(KeySignature::new(1, KeyMode::Major).unwrap());
        assert!(s.connections.is_empty());
        s.set_accidental(b, 0, None).unwrap();
        s.add_connection(tie).unwrap();
        s.insert(0, 1, note(Step::G, Duration::Quarter)).unwrap();
        assert!(s.connections.is_empty());
        s.validate().unwrap();
    }
    #[test]
    fn articulation_rejects_rests_and_dynamic_can_start_at_a_rest() {
        let mut s = Score::default();
        let mut rest = note(Step::C, Duration::Quarter);
        rest.pitches = Pitches::default();
        let id = s.insert(0, 0, rest).unwrap();
        assert!(
            s.set_articulation(id, Some(Articulation::Staccato))
                .is_err()
        );
        s.set_dynamic(id, Some(Dynamic::Pp)).unwrap();
        s.validate().unwrap();
    }
}
