//! What the parts panel lists, without GPUI: families in score order, each
//! part's name and a short note of its clefs and transposition.
use score_core::{Clef, InstrumentFamily, Part, PartId, Score, StaffId, Transposition};

pub struct PartRow {
    pub id: PartId,
    pub name: String,
    /// Clefs and transposition, as «Si♭ · Sol» or «Fa · 8vb».
    pub meta: String,
}

pub struct FamilyRows {
    pub family: InstrumentFamily,
    pub label: &'static str,
    pub folded: bool,
    /// The family's parts that match, also when folded.
    pub count: usize,
    /// Empty when folded.
    pub parts: Vec<PartRow>,
}

/// One pass over the parts. A query keeps the parts whose name contains it,
/// ignoring case and accents, and shows every family unfolded.
pub fn parts_view(
    score: &Score,
    measure: usize,
    query: &str,
    folded: &[String],
) -> Vec<FamilyRows> {
    let query = plain(query.trim());
    let mut families: Vec<FamilyRows> = InstrumentFamily::ALL
        .into_iter()
        .map(|family| FamilyRows {
            family,
            label: family.label(),
            folded: query.is_empty() && folded.iter().any(|name| name == family.label()),
            count: 0,
            parts: Vec::new(),
        })
        .collect();
    for (part, name) in score.parts().iter().zip(score.part_names()) {
        if !query.is_empty() && !plain(&name).contains(&query) {
            continue;
        }
        let Some(family) = families
            .iter_mut()
            .find(|rows| rows.family == part.instrument().family())
        else {
            continue;
        };
        family.count += 1;
        if !family.folded {
            family.parts.push(PartRow {
                id: part.id(),
                name,
                meta: meta(part, measure),
            });
        }
    }
    families.retain(|family| family.count > 0);
    families
}

/// Lower case without accents, so «violin» finds «Violín».
fn plain(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

fn meta(part: &Part, measure: usize) -> String {
    let upper = clef(part.clef_at(StaffId::Upper, measure));
    let clefs = if part.instrument().is_grand_staff() {
        format!("{upper} / {}", clef(part.clef_at(StaffId::Lower, measure)))
    } else if part.bar_has_divisi(measure) {
        format!("Divisi · {upper}")
    } else {
        upper.to_owned()
    };
    match transposition(part.transposition()) {
        Some(Written::Key(key)) => format!("{key} · {clefs}"),
        Some(Written::Octave(octave)) => format!("{clefs} · {octave}"),
        None => clefs,
    }
}

fn clef(clef: Clef) -> &'static str {
    match clef {
        Clef::Treble => "Sol",
        Clef::Alto => "Do 3.ª",
        Clef::Tenor => "Do 4.ª",
        Clef::Bass => "Fa",
        Clef::Percussion => "Percusión",
    }
}

enum Written {
    /// The instrument's key, as in «Clarinete en Si♭».
    Key(String),
    /// Sounds whole octaves away from the written notes.
    Octave(&'static str),
}

/// A written C sounds `transposition` away: its letter and accidental name
/// the instrument's key, and a pure octave is told as 8va or 8vb.
fn transposition(transposition: Transposition) -> Option<Written> {
    if transposition.is_concert() {
        return None;
    }
    const STEPS: [(&str, i16); 7] = [
        ("Do", 0),
        ("Re", 2),
        ("Mi", 4),
        ("Fa", 5),
        ("Sol", 7),
        ("La", 9),
        ("Si", 11),
    ];
    let diatonic = i16::from(transposition.diatonic);
    let (letter, natural) = STEPS[diatonic.rem_euclid(7) as usize];
    let octaves = diatonic.div_euclid(7);
    let alteration = i16::from(transposition.chromatic) - natural - 12 * octaves;
    if letter == "Do" && alteration == 0 {
        return Some(Written::Octave(match octaves {
            1 => "8va",
            -1 => "8vb",
            2 => "15ma",
            _ => "15mb",
        }));
    }
    let accidental = match alteration {
        -2 => "𝄫",
        -1 => "♭",
        1 => "♯",
        2 => "𝄪",
        _ => "",
    };
    Some(Written::Key(format!("{letter}{accidental}")))
}
