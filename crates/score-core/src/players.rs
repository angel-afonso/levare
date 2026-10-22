//! Whether a part is played by one musician or by a section. Sound packs pick
//! solo or section recordings from it; the notation does not change.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Players {
    Solo,
    Section,
}

impl Players {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Solo => "Solista",
            Self::Section => "Sección",
        }
    }
}

impl Part {
    /// The choice written in the score; `None` follows `Score::players`.
    pub fn players_choice(&self) -> Option<Players> {
        self.players
    }
}

impl Score {
    /// The part's choice, or `default_players` when it has none.
    pub fn players(&self, part: PartId) -> Players {
        self.part(part)
            .and_then(|found| found.players)
            .unwrap_or_else(|| self.default_players(part))
    }

    /// Bowed strings play as sections when there are more than four string
    /// parts, and as soloists in chamber music. A part named "solista" or
    /// "solo" is a soloist, and every other instrument plays alone.
    pub fn default_players(&self, part: PartId) -> Players {
        let Some(found) = self.part(part) else {
            return Players::Solo;
        };
        let soloist = found.name.as_deref().is_some_and(|name| {
            let name = name.to_lowercase();
            name.contains("solista") || name.contains("solo")
        });
        let strings = self
            .parts()
            .iter()
            .filter(|part| part.instrument().family() == InstrumentFamily::Strings)
            .count();
        if found.instrument().family() == InstrumentFamily::Strings && !soloist && strings > 4 {
            Players::Section
        } else {
            Players::Solo
        }
    }

    pub fn set_players(
        &mut self,
        part: PartId,
        players: Option<Players>,
    ) -> Result<bool, ScoreError> {
        let found = self
            .part(part)
            .ok_or(ScoreError("El instrumento no existe"))?;
        if found.players == players {
            return Ok(false);
        }
        self.part_mut(part)?.players = players;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_are_sections_in_an_orchestra_and_soloists_in_chamber_music() {
        let quartet = Score::new(
            "".into(),
            &[
                Instrument::Flute,
                Instrument::Violin,
                Instrument::Violin,
                Instrument::Viola,
                Instrument::Cello,
            ],
            1,
        )
        .unwrap();
        for part in quartet.parts() {
            assert_eq!(quartet.players(part.id()), Players::Solo);
        }
        let mut orchestra = Score::new(
            "".into(),
            &[
                Instrument::Flute,
                Instrument::Viola,
                Instrument::Violin,
                Instrument::Violin,
                Instrument::Viola,
                Instrument::Cello,
                Instrument::DoubleBass,
            ],
            1,
        )
        .unwrap();
        let ids: Vec<_> = orchestra.parts().iter().map(|part| part.id()).collect();
        assert_eq!(orchestra.players(ids[0]), Players::Solo);
        assert_eq!(orchestra.players(ids[2]), Players::Section);
        orchestra.part_mut(ids[1]).unwrap().name = Some("Viola solista".into());
        assert_eq!(orchestra.players(ids[1]), Players::Solo);

        assert!(orchestra.set_players(ids[2], Some(Players::Solo)).unwrap());
        assert!(!orchestra.set_players(ids[2], Some(Players::Solo)).unwrap());
        assert_eq!(orchestra.players(ids[2]), Players::Solo);
        assert_eq!(
            orchestra.part(ids[2]).unwrap().players_choice(),
            Some(Players::Solo)
        );
        assert!(
            orchestra
                .set_players(ids[0], Some(Players::Section))
                .unwrap()
        );
        assert_eq!(orchestra.players(ids[0]), Players::Section);
        assert!(orchestra.set_players(ids[2], None).unwrap());
        assert_eq!(orchestra.players(ids[2]), Players::Section);
    }
}
