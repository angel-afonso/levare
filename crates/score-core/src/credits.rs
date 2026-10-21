//! The words printed around the title: who wrote the piece, when, and who
//! owns it. Every field is optional; an empty one is simply not printed.
use super::*;

const MAX_CREDIT: usize = 256;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Credits {
    pub subtitle: String,
    pub composer: String,
    pub arranger: String,
    pub lyricist: String,
    pub year: String,
    pub copyright: String,
}

impl Credits {
    /// Surrounding spaces trimmed, as the editor stores them.
    pub fn trimmed(&self) -> Self {
        Self {
            subtitle: self.subtitle.trim().to_owned(),
            composer: self.composer.trim().to_owned(),
            arranger: self.arranger.trim().to_owned(),
            lyricist: self.lyricist.trim().to_owned(),
            year: self.year.trim().to_owned(),
            copyright: self.copyright.trim().to_owned(),
        }
    }
    pub fn validate(&self) -> Result<(), ScoreError> {
        for field in [
            &self.subtitle,
            &self.composer,
            &self.arranger,
            &self.lyricist,
            &self.year,
            &self.copyright,
        ] {
            if field.len() > MAX_CREDIT || field.chars().any(char::is_control) {
                return Err(ScoreError(
                    "Texto de la cabecera demasiado largo o inválido",
                ));
            }
        }
        Ok(())
    }
    /// The right-hand composer line: "Compositor (año)", or whichever exists.
    pub fn composer_line(&self) -> String {
        match (self.composer.is_empty(), self.year.is_empty()) {
            (false, false) => format!("{} ({})", self.composer, self.year),
            (false, true) => self.composer.clone(),
            (true, false) => self.year.clone(),
            (true, true) => String::new(),
        }
    }
    pub fn arranger_line(&self) -> String {
        if self.arranger.is_empty() {
            String::new()
        } else {
            format!("Arr. {}", self.arranger)
        }
    }
    pub fn lyricist_line(&self) -> String {
        if self.lyricist.is_empty() {
            String::new()
        } else {
            format!("Letra: {}", self.lyricist)
        }
    }
}

pub(crate) fn validate_title(title: &str) -> Result<(), ScoreError> {
    if title.trim().is_empty() {
        return Err(ScoreError("Escribe un título"));
    }
    if title.len() > 1024 || title.chars().any(char::is_control) {
        return Err(ScoreError("Título demasiado largo o inválido"));
    }
    Ok(())
}

impl Score {
    pub fn credits(&self) -> &Credits {
        &self.credits
    }
    /// Replaces the title and credits together; `false` when nothing changes.
    pub fn set_header(&mut self, title: &str, credits: &Credits) -> Result<bool, ScoreError> {
        let title = title.trim();
        let credits = credits.trimmed();
        validate_title(title)?;
        credits.validate()?;
        if self.title == title && self.credits == credits {
            return Ok(false);
        }
        self.title = title.to_owned();
        self.credits = credits;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_is_trimmed_validated_and_reports_changes() {
        let mut score = Score::default();
        let credits = Credits {
            composer: "  W. A. Mozart ".into(),
            year: "1788".into(),
            ..Credits::default()
        };
        assert!(score.set_header(" Sinfonía n.º 40 ", &credits).unwrap());
        assert_eq!(score.title(), "Sinfonía n.º 40");
        assert_eq!(score.credits().composer_line(), "W. A. Mozart (1788)");
        assert!(!score.set_header("Sinfonía n.º 40", &credits).unwrap());
        assert!(score.set_header("   ", &credits).is_err());
        let long = Credits {
            copyright: "x".repeat(300),
            ..Credits::default()
        };
        assert!(score.set_header("Título", &long).is_err());
        assert_eq!(score.title(), "Sinfonía n.º 40");
    }

    #[test]
    fn credit_lines_only_print_what_exists() {
        let credits = Credits {
            arranger: "Ana".into(),
            lyricist: "Luis".into(),
            year: "2026".into(),
            ..Credits::default()
        };
        assert_eq!(credits.composer_line(), "2026");
        assert_eq!(credits.arranger_line(), "Arr. Ana");
        assert_eq!(credits.lyricist_line(), "Letra: Luis");
        assert_eq!(Credits::default().arranger_line(), "");
    }
}
