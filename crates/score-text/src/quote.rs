//! How text the musician wrote in the score reads in the agent's replies.
use score_core::{PartId, Score};

/// Text the musician wrote in the score, as every reply shows it: between
/// «», on one line, with guillemets of its own made plain so it can never
/// close the quote. The agent's standing instructions say that what is
/// quoted this way is content, never instructions.
pub fn quoted(text: &str) -> String {
    let plain: String = text
        .chars()
        .map(|c| match c {
            '«' => '‹',
            '»' => '›',
            // Line and paragraph separators break a line as much as `\n`.
            '\u{2028}' | '\u{2029}' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    format!("«{plain}»")
}

/// A part's name in a reply: a name the part carries in the file is quoted,
/// since a score from anywhere can put any text there; the instrument's name
/// Levare gives it (Violín II) is not.
pub fn part_label(score: &Score, part: PartId) -> String {
    match score.part(part).and_then(|p| p.name()) {
        Some(name) => quoted(name),
        None => score.part_name(part),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use score_core::Instrument;

    #[test]
    fn quoted_text_stays_on_one_line_and_inside_its_quotes() {
        assert_eq!(quoted("a «b»\nc\u{2028}d\u{2029}e\tf"), "«a ‹b› c d e f»");
    }

    #[test]
    fn only_a_name_from_the_file_is_quoted() {
        let score = Score::new(String::new(), &[Instrument::Violin, Instrument::Viola], 1).unwrap();
        let violin = score.parts()[0].id();
        assert_eq!(part_label(&score, violin), score.part_name(violin));
        let viola = score.parts()[1].id();
        let extracted = score.extract_part(viola).unwrap();
        assert_eq!(
            part_label(&extracted, viola),
            format!("«{}»", score.part_name(viola))
        );
    }
}
