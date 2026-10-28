//! Bounded MusicXML reader. Unsupported musical data is rejected or reported,
//! never silently used to overwrite the source document.
use crate::{FileError, error};
use roxmltree::{Document, Node, ParsingOptions};
use score_core::*;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs::File,
    io::{Cursor, Read},
    path::Path,
};

mod build;

/// MusicXML is far more verbose than the native format, so it keeps its own,
/// older limit.
const MAX_MUSICXML_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug)]
pub struct ImportResult {
    pub score: Score,
    /// Deduplicated descriptions of notation that could not be retained exactly.
    pub warnings: Vec<String>,
}
#[derive(Default)]
struct Warnings(BTreeSet<String>);
impl Warnings {
    fn add(&mut self, message: impl Into<String>) {
        if self.0.len() < 32 {
            self.0.insert(message.into().chars().take(280).collect());
        } else {
            self.0.insert("Hay más elementos no admitidos; revisa la partitura original antes de utilizar la copia importada".into());
        }
    }
}

fn bad_number(failure: impl std::fmt::Display) -> FileError {
    FileError::damaged_with("MusicXML: hay un valor numérico no válido", failure)
}
fn damaged_mxl(failure: impl std::fmt::Display) -> FileError {
    FileError::damaged_with("MusicXML comprimido dañado o incompleto", failure)
}
fn child<'a, 'b>(n: Node<'a, 'b>, name: &str) -> Option<Node<'a, 'b>> {
    n.children().find(|c| c.has_tag_name(name))
}
fn text<'a>(n: Node<'a, '_>, name: &str) -> Option<&'a str> {
    child(n, name).and_then(|c| c.text()).map(str::trim)
}
fn number(n: Node<'_, '_>, name: &str) -> Result<f64, FileError> {
    let x: f64 = text(n, name)
        .ok_or_else(|| error(format!("MusicXML: falta {name}")))?
        .parse()
        .map_err(bad_number)?;
    if x.is_finite() && x.abs() <= 1e9 {
        Ok(x)
    } else {
        Err(error(format!("MusicXML: {name} fuera de rango")))
    }
}
fn integer(n: Node<'_, '_>, name: &str) -> Result<i32, FileError> {
    let x = number(n, name)?;
    if x.fract() != 0.0 {
        return Err(error(format!("MusicXML: {name} requiere un entero")));
    }
    Ok(x as i32)
}
fn ticks(value: f64, divisions: f64) -> Result<u16, FileError> {
    let exact = value * f64::from(TICKS_PER_QUARTER) / divisions;
    if !exact.is_finite()
        || exact < 0.0
        || exact > f64::from(16 * TICKS_PER_WHOLE)
        || (exact - exact.round()).abs() > 1e-6
    {
        return Err(error(
            "MusicXML: una duración no cabe en la resolución rítmica admitida",
        ));
    }
    Ok(exact.round() as u16)
}
fn parse_xml(data: &str) -> Result<Document<'_>, FileError> {
    Document::parse_with_options(
        data,
        ParsingOptions {
            allow_dtd: true,
            nodes_limit: 1_000_000,
        },
    )
    .map_err(|failure| FileError::damaged_with("MusicXML: el archivo no es XML válido", failure))
}

pub fn load(path: &Path) -> Result<ImportResult, FileError> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take((MAX_MUSICXML_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    decode(&bytes)
}
/// Reads uncompressed XML or a compressed MXL container without extracting files.
pub fn decode(bytes: &[u8]) -> Result<ImportResult, FileError> {
    if bytes.len() > MAX_MUSICXML_BYTES {
        return Err(FileError::too_large("MusicXML supera el límite de 16 MiB"));
    }
    let expanded;
    let bytes = if bytes.starts_with(b"PK") {
        expanded = unpack(bytes)?;
        expanded.as_slice()
    } else {
        bytes
    };
    let data = std::str::from_utf8(bytes)
        .map_err(|_| error("MusicXML: guarda el archivo con codificación UTF-8"))?;
    let doc = parse_xml(data)?;
    let root = doc.root_element();
    if !matches!(root.tag_name().name(), "score-partwise" | "score-timewise") {
        return Err(error(
            "El archivo no contiene una partitura MusicXML partwise o timewise",
        ));
    }
    let title = child(root, "work")
        .and_then(|w| text(w, "work-title"))
        .or_else(|| text(root, "movement-title"))
        .unwrap_or("Partitura importada")
        .to_owned();
    let list = child(root, "part-list")
        .ok_or_else(|| error("MusicXML: falta la lista de instrumentos"))?;
    let mut warnings = Warnings::default();
    let mut ids = BTreeSet::new();
    let mut parts = Vec::new();
    for p in list.children().filter(|p| p.has_tag_name("score-part")) {
        let id = p
            .attribute("id")
            .ok_or_else(|| error("MusicXML: instrumento sin identificador"))?;
        if !ids.insert(id) {
            return Err(error("MusicXML: identificador de instrumento repetido"));
        }
        if parts.len() >= MAX_PARTS {
            return Err(error("MusicXML: máximo de 48 instrumentos"));
        }
        let measures = if root.has_tag_name("score-partwise") {
            let matches: Vec<_> = root
                .children()
                .filter(|n| n.has_tag_name("part") && n.attribute("id") == Some(id))
                .collect();
            if matches.len() != 1 {
                return Err(error("MusicXML: parte ausente o repetida"));
            }
            matches[0]
                .children()
                .filter(|m| m.has_tag_name("measure"))
                .collect::<Vec<_>>()
        } else {
            root.children()
                .filter(|m| m.has_tag_name("measure"))
                .map(|m| {
                    let matches: Vec<_> = m
                        .children()
                        .filter(|p| p.has_tag_name("part") && p.attribute("id") == Some(id))
                        .collect();
                    if matches.len() == 1 {
                        Ok(matches[0])
                    } else {
                        Err(error("MusicXML timewise: parte ausente o repetida"))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        if measures.is_empty() || measures.len() > MAX_MEASURES {
            return Err(error("MusicXML: cantidad de compases no admitida"));
        }
        let instrument = instrument(p, &measures, &mut warnings);
        parts.push(read_part(instrument, &measures, &mut warnings)?);
    }
    if parts.is_empty() {
        return Err(error("MusicXML: no hay instrumentos"));
    }
    let declared: BTreeSet<_> = list
        .children()
        .filter(|n| n.has_tag_name("score-part"))
        .filter_map(|n| n.attribute("id"))
        .collect();
    let actual: Vec<_> = if root.has_tag_name("score-partwise") {
        root.children().filter(|n| n.has_tag_name("part")).collect()
    } else {
        root.children()
            .filter(|n| n.has_tag_name("measure"))
            .flat_map(|m| m.children().filter(|n| n.has_tag_name("part")))
            .collect()
    };
    if actual
        .iter()
        .any(|p| p.attribute("id").is_none_or(|id| !declared.contains(id)))
    {
        return Err(error(
            "MusicXML: parte no declarada en la lista de instrumentos",
        ));
    }
    let mut score = build::score(title, parts, &mut warnings)?;
    let header = credits(root);
    let title = score.title().to_owned();
    if score.set_header(&title, &header).is_err() {
        warnings.add("MusicXML: se omitieron créditos de la cabecera no válidos");
    }
    Ok(ImportResult {
        score,
        warnings: warnings.0.into_iter().collect(),
    })
}
/// Composer, arranger, lyricist and rights from `<identification>`, the year
/// from a miscellaneous field, and the subtitle (or missing creators) from
/// typed `<credit>` texts, as MuseScore and Sibelius write them.
fn credits(root: Node<'_, '_>) -> score_core::Credits {
    let mut credits = score_core::Credits::default();
    if let Some(identification) = child(root, "identification") {
        for creator in identification
            .children()
            .filter(|n| n.has_tag_name("creator"))
        {
            let slot = match creator.attribute("type") {
                Some("composer") => &mut credits.composer,
                Some("arranger") => &mut credits.arranger,
                Some("lyricist" | "poet") => &mut credits.lyricist,
                _ => continue,
            };
            if slot.is_empty() {
                *slot = clean(creator.text().unwrap_or_default());
            }
        }
        if let Some(rights) = text(identification, "rights") {
            credits.copyright = clean(rights);
        }
        if let Some(field) = child(identification, "miscellaneous").and_then(|m| {
            m.children().find(|n| {
                n.has_tag_name("miscellaneous-field") && n.attribute("name") == Some("year")
            })
        }) {
            credits.year = clean(field.text().unwrap_or_default());
        }
    }
    for credit in root.children().filter(|n| n.has_tag_name("credit")) {
        let words: Vec<_> = credit
            .children()
            .filter(|n| n.has_tag_name("credit-words"))
            .filter_map(|n| n.text())
            .collect();
        let words = clean(&words.join(" "));
        for kind in credit.children().filter(|n| n.has_tag_name("credit-type")) {
            let slot = match kind.text().map(str::trim) {
                Some("subtitle") => &mut credits.subtitle,
                Some("composer") => &mut credits.composer,
                Some("arranger") => &mut credits.arranger,
                Some("lyricist" | "poet") => &mut credits.lyricist,
                Some("rights") => &mut credits.copyright,
                _ => continue,
            };
            if slot.is_empty() {
                *slot = words.clone();
            }
        }
    }
    credits
}
/// One printable line of at most 256 bytes.
fn clean(text: &str) -> String {
    let mut line: String = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    let mut end = line.len().min(256);
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    line.truncate(end);
    line
}
fn unpack(bytes: &[u8]) -> Result<Vec<u8>, FileError> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(damaged_mxl)?;
    if zip.len() > 4096 {
        return Err(error("MusicXML comprimido contiene demasiados archivos"));
    }
    let container = {
        let mut f = zip.by_name("META-INF/container.xml").map_err(damaged_mxl)?;
        let mut data = String::new();
        f.by_ref()
            .take(65_537)
            .read_to_string(&mut data)
            .map_err(damaged_mxl)?;
        if data.len() > 65_536 {
            return Err(error("Contenedor MusicXML demasiado grande"));
        }
        data
    };
    let doc = parse_xml(&container)?;
    let name = doc
        .descendants()
        .find(|n| n.has_tag_name("rootfile"))
        .and_then(|n| n.attribute("full-path"))
        .ok_or_else(|| error("Contenedor MusicXML sin partitura principal"))?;
    if Path::new(name).is_absolute() || name.split(['/', '\\']).any(|p| p == "..") {
        return Err(error("Ruta inválida en el contenedor MusicXML"));
    }
    let mut f = zip.by_name(name).map_err(damaged_mxl)?;
    if f.size() > MAX_MUSICXML_BYTES as u64 {
        return Err(FileError::too_large("MusicXML descomprimido supera 16 MiB"));
    }
    let mut out = Vec::new();
    f.by_ref()
        .take((MAX_MUSICXML_BYTES + 1) as u64)
        .read_to_end(&mut out)
        .map_err(damaged_mxl)?;
    if out.len() > MAX_MUSICXML_BYTES {
        return Err(FileError::too_large("MusicXML descomprimido supera 16 MiB"));
    }
    Ok(out)
}
fn instrument(part: Node<'_, '_>, bars: &[Node<'_, '_>], warnings: &mut Warnings) -> Instrument {
    let label = child(part, "score-instrument").and_then(|n| text(n, "instrument-name"));
    let name = label
        .unwrap_or_else(|| text(part, "part-name").unwrap_or(""))
        .to_lowercase();
    if let Some(instrument) = Instrument::ALL
        .into_iter()
        .find(|i| i.name().to_lowercase() == name)
    {
        return instrument;
    }
    let chromatic = bars
        .iter()
        .flat_map(|b| b.descendants())
        .find(|n| n.has_tag_name("transpose"))
        .and_then(|n| text(n, "chromatic"))
        .and_then(|s| s.parse::<i8>().ok())
        .unwrap_or(0);
    if let Some(key) = child(part, "midi-instrument")
        .and_then(|n| text(n, "midi-unpitched"))
        .and_then(|s| s.parse::<u8>().ok())
        && let Some(instrument) = Instrument::ALL
            .into_iter()
            .find(|i| i.percussion_key().map(|k| k + 1) == Some(key))
    {
        return instrument;
    }
    for (names, instrument) in [
        (&["piccolo", "flautín"][..], Instrument::Piccolo),
        (&["alto flute", "flauta alto"][..], Instrument::AltoFlute),
        (&["bass flute", "flauta baja"][..], Instrument::BassFlute),
        (
            &["english horn", "cor anglais", "corno inglés"][..],
            Instrument::EnglishHorn,
        ),
        (
            &["oboe d'amore", "oboe de amor"][..],
            Instrument::OboeDAmore,
        ),
        (
            &["bass clarinet", "clarinete bajo"][..],
            Instrument::BassClarinet,
        ),
        (
            &["contrabassoon", "contrafagot"][..],
            Instrument::Contrabassoon,
        ),
        (
            &["soprano sax", "saxofón soprano"][..],
            Instrument::SopranoSax,
        ),
        (&["alto sax", "saxofón alto"][..], Instrument::AltoSax),
        (&["tenor sax", "saxofón tenor"][..], Instrument::TenorSax),
        (
            &["baritone sax", "saxofón barítono"][..],
            Instrument::BaritoneSax,
        ),
        (
            &["bass trombone", "trombón bajo"][..],
            Instrument::BassTrombone,
        ),
        (&["tuba"][..], Instrument::Tuba),
        (&["timpani", "timbales"][..], Instrument::Timpani),
        (&["snare", "caja"][..], Instrument::SnareDrum),
        (&["bass drum", "bombo"][..], Instrument::BassDrum),
        (&["cymbal", "platillos"][..], Instrument::Cymbals),
        (&["triangle", "triángulo"][..], Instrument::Triangle),
        (&["tambourine", "pandereta"][..], Instrument::Tambourine),
        (&["tam-tam", "tam tam", "gong"][..], Instrument::TamTam),
        (&["glockenspiel"][..], Instrument::Glockenspiel),
        (&["xylophone", "xilófono"][..], Instrument::Xylophone),
        (&["marimba"][..], Instrument::Marimba),
        (&["vibraphone", "vibráfono"][..], Instrument::Vibraphone),
        (&["tubular bells", "campanas"][..], Instrument::TubularBells),
        (&["celesta"][..], Instrument::Celesta),
        (&["organ", "órgano"][..], Instrument::Organ),
        (&["harp", "arpa"][..], Instrument::Harp),
        (&["violin", "violín", "vln"][..], Instrument::Violin),
        (&["viola"][..], Instrument::Viola),
        (&["cello", "violonchelo"][..], Instrument::Cello),
        (&["contrab", "double bass"][..], Instrument::DoubleBass),
        (&["piano", "pianoforte"][..], Instrument::Piano),
        (&["guitar", "guitarra"][..], Instrument::Guitar),
        (&["flute", "flauta"][..], Instrument::Flute),
        (&["oboe"][..], Instrument::Oboe),
        (&["clarinet"][..], Instrument::Clarinet),
        (&["bassoon", "fagot"][..], Instrument::Bassoon),
        (&["trumpet", "trompeta"][..], Instrument::Trumpet),
        (&["trombone", "trombón"][..], Instrument::Trombone),
        (&["horn", "trompa", "corno"][..], Instrument::Horn),
        // Voices last: «tenor» or «alto» also name instruments above.
        (&["soprano", "tiple"][..], Instrument::Soprano),
        (&["contralto", "mezzo", "alto"][..], Instrument::Contralto),
        (&["tenor"][..], Instrument::Tenor),
        (
            &["baritone", "barítono", "basso", "bajo"][..],
            Instrument::BassVoice,
        ),
        (
            &["voice", "voz", "vocal", "canto", "cantante"][..],
            Instrument::Voice,
        ),
    ] {
        if names.iter().any(|s| name.contains(s)) {
            return match (instrument, chromatic) {
                (Instrument::Clarinet, -2) => Instrument::ClarinetBb,
                (Instrument::Clarinet, -3) => Instrument::ClarinetA,
                (Instrument::Clarinet, 3) => Instrument::ClarinetEb,
                (Instrument::Horn, -7) => Instrument::HornF,
                (Instrument::Trumpet, -2) => Instrument::TrumpetBb,
                (Instrument::Trumpet, 2) => Instrument::TrumpetD,
                _ => instrument,
            };
        }
    }
    if let Some(program) = child(part, "midi-instrument")
        .and_then(|n| text(n, "midi-program"))
        .and_then(|s| s.parse::<u8>().ok())
        && let Some(instrument) = Instrument::ALL
            .into_iter()
            .find(|i| i.program() + 1 == program)
    {
        return instrument;
    }
    let two = bars.iter().any(|b| {
        b.descendants()
            .any(|n| n.has_tag_name("staves") && n.text().is_some_and(|s| s.trim() == "2"))
    });
    let fallback = if two {
        Instrument::Piano
    } else {
        Instrument::Violin
    };
    warnings.add(format!(
        "Instrumento «{}» interpretado como {}",
        text(part, "part-name").unwrap_or("sin nombre"),
        fallback.name()
    ));
    fallback
}

#[derive(Clone, Default)]
struct Bar {
    clefs: BTreeMap<u8, Clef>,
    time: Option<TimeSignature>,
    key: Option<KeySignature>,
    tempo: Option<TempoMark>,
    rehearsal: Option<String>,
    events: Vec<RawEvent>,
    directions: Vec<Mark>,
    implicit: bool,
    staves: u8,
    /// Free words, and whether their direction gave a placement.
    texts: Vec<(Mark, bool)>,
    repeat_start: bool,
    repeat_end: Option<u8>,
    barline: Option<Barline>,
    ending_start: Option<Vec<u8>>,
    ending_stop: bool,
    segno: bool,
    coda: bool,
    to_coda: bool,
    fine: bool,
    jump: Option<Jump>,
    jump_from_sound: bool,
}
struct PartData {
    instrument: Instrument,
    transposition: Option<Transposition>,
    bars: Vec<Bar>,
    passage: bool,
}
#[derive(Clone)]
struct RawEvent {
    onset: u16,
    length: u16,
    written: Option<Duration>,
    staff: u8,
    voice: String,
    pitches: Vec<Tone>,
    notation: Notation,
    endpoints: Vec<(ConnectionKind, bool, String)>,
    arpeggio: Option<(String, Arpeggio)>,
}
#[derive(Clone)]
struct Tone {
    pitch: Pitch,
    harmonic: Option<(Harmonic, String)>,
    diamond: bool,
    ties: [bool; 2],
}
#[derive(Clone)]
struct Mark {
    tick: u16,
    staff: u8,
    voice: Option<String>,
    kind: MarkType,
}
#[derive(Clone)]
enum MarkType {
    Dynamic(Dynamic),
    Technique(PlayingTechnique),
    Division(Division),
    Visibility(bool),
    Wedge(bool, ConnectionKind, String),
    /// Words that are neither tempo nor a known technique.
    Text(TextKind, String),
    Chord(ChordSymbol),
    /// An octave line or pedal: its stop names the family, not the kind.
    Span(bool, ConnectionKind, String),
}
struct State {
    divisions: f64,
    time: TimeSignature,
    key: KeySignature,
    transpose: [i32; 2],
    intervals: [Option<Transposition>; 2],
    staves: u8,
}

fn read_part(
    instrument: Instrument,
    measures: &[Node<'_, '_>],
    warnings: &mut Warnings,
) -> Result<PartData, FileError> {
    let mut state = State {
        divisions: 1.0,
        time: TimeSignature::default(),
        key: KeySignature::default(),
        transpose: [0; 2],
        intervals: [None; 2],
        staves: 1,
    };
    let mut bars = Vec::new();
    let mut passage = false;
    for (index, measure) in measures.iter().enumerate() {
        let parent = if measure.has_tag_name("part") {
            measure.parent().unwrap()
        } else {
            *measure
        };
        let mut bar = Bar {
            implicit: parent.attribute("implicit") == Some("yes"),
            ..Bar::default()
        };
        let mut cursor = 0u16;
        let mut last_note = None;
        // Grace notes wait for the next note of their voice and staff.
        let mut graces: Vec<(String, usize, Pitch, bool)> = Vec::new();
        for n in measure.children().filter(Node::is_element) {
            match n.tag_name().name() {
                "attributes" => attributes(n, &mut state, &mut bar, cursor, warnings)?,
                "backup" | "forward" => {
                    let d = ticks(number(n, "duration")?, state.divisions)?;
                    if d == 0 {
                        return Err(error("MusicXML: desplazamiento de duración cero"));
                    }
                    cursor = if n.has_tag_name("backup") {
                        cursor.checked_sub(d)
                    } else {
                        cursor.checked_add(d)
                    }
                    .ok_or_else(|| error("MusicXML: desplazamiento fuera del compás"))?;
                    last_note = None;
                }
                "direction" => read_direction(n, cursor, &state, &mut bar, warnings)?,
                "harmony" => read_harmony(n, cursor, &state, &mut bar, warnings)?,
                "sound" => {
                    navigation_sound(n, &mut bar);
                    if let Some(technique) = sound_technique(n)? {
                        bar.directions.push(Mark {
                            tick: cursor,
                            staff: 1,
                            voice: None,
                            kind: MarkType::Technique(technique),
                        });
                    }
                    if let Some(mark) = sound_tempo(n, warnings)? {
                        if cursor == 0 {
                            bar.tempo = Some(mark);
                        } else {
                            warnings.add("Los cambios de tempo dentro del compás no se importan");
                        }
                    }
                }
                "note" => {
                    if let Some(grace) = child(n, "grace") {
                        let pitch = child(n, "pitch");
                        if child(n, "chord").is_some() || pitch.is_none() {
                            warnings.add("Los acordes y las notas de adorno sin altura se importan solo con su primera nota");
                            continue;
                        }
                        let staff = text(n, "staff").map_or(Ok(0), staff_index)?;
                        let pitch = pitch_value(
                            pitch.expect("checked pitch"),
                            state.transpose[staff],
                            state.intervals[staff],
                            warnings,
                        )?;
                        // An accidental the key already implies is shown only if written.
                        let implied = state.key.accidental(pitch.step);
                        let redundant = pitch.accidental() == implied
                            || (implied.is_none()
                                && pitch.accidental() == Some(Accidental::Natural));
                        let pitch = if redundant && child(n, "accidental").is_none() {
                            pitch.with_accidental(None)
                        } else {
                            pitch
                        };
                        graces.push((
                            text(n, "voice").unwrap_or("1").to_owned(),
                            staff,
                            pitch,
                            grace.attribute("slash") == Some("yes"),
                        ));
                        continue;
                    }
                    let chord_parent = if child(n, "chord").is_some() {
                        last_note.and_then(|i| bar.events.get(i))
                    } else {
                        None
                    };
                    let mut event = read_note(n, cursor, &state, warnings, chord_parent)?;
                    if child(n, "chord").is_none() {
                        let staff = usize::from(event.staff - 1);
                        let (mine, others): (Vec<_>, Vec<_>) = std::mem::take(&mut graces)
                            .into_iter()
                            .partition(|(voice, at, _, _)| *voice == event.voice && *at == staff);
                        graces = others;
                        if !mine.is_empty() && !event.pitches.is_empty() {
                            if mine.len() > MAX_GRACE_NOTES {
                                warnings.add("Solo se importan las cuatro primeras notas de adorno de cada nota");
                            }
                            let notes: Vec<Pitch> = mine
                                .iter()
                                .take(MAX_GRACE_NOTES)
                                .map(|(_, _, pitch, _)| *pitch)
                                .collect();
                            let kind = if mine.len() == 1 && mine[0].3 {
                                GraceKind::Acciaccatura
                            } else {
                                GraceKind::Appoggiatura
                            };
                            event.notation.grace = Grace::from_notes(kind, &notes);
                        }
                    }
                    if child(n, "chord").is_some() {
                        let last: &mut RawEvent = last_note
                            .and_then(|i| bar.events.get_mut(i))
                            .ok_or_else(|| error("MusicXML: acorde sin nota inicial"))?;
                        if last.length != event.length
                            || last.staff != event.staff
                            || last.voice != event.voice
                            || last.pitches.is_empty()
                            || event.pitches.is_empty()
                        {
                            return Err(error(
                                "MusicXML: acorde con duraciones o pentagramas distintos aún no admitido",
                            ));
                        }
                        last.pitches.append(&mut event.pitches);
                        last.endpoints.extend(event.endpoints);
                        if event.notation.technique.is_some()
                            && event.notation.technique != last.notation.technique
                        {
                            warnings.add("Las técnicas diferentes entre notas de un acorde se unifican para el acorde completo");
                            if last.notation.technique.is_none() {
                                last.notation.technique = event.notation.technique;
                            }
                        }
                        if event.notation.articulation.is_some()
                            && event.notation.articulation != last.notation.articulation
                        {
                            warnings.add("Las articulaciones de notas individuales se unifican para el acorde completo");
                            if last.notation.articulation.is_none() {
                                last.notation.articulation = event.notation.articulation;
                            }
                        }
                        if event.arpeggio.is_some() {
                            last.arpeggio = event.arpeggio;
                        }
                        last.notation.fermata |= event.notation.fermata;
                        if event.notation.tremolo.is_some() {
                            last.notation.tremolo = event.notation.tremolo;
                        }
                        if last.pitches.len() > MAX_CHORD_NOTES + 1 {
                            return Err(error("MusicXML: acorde demasiado grande"));
                        }
                    } else {
                        cursor = cursor
                            .checked_add(event.length)
                            .ok_or_else(|| error("MusicXML: duración fuera de rango"))?;
                        bar.events.push(event);
                        last_note = Some(bar.events.len() - 1);
                    }
                    if bar.events.len() > MAX_EVENTS_PER_MEASURE * 4 {
                        return Err(error("MusicXML: demasiadas notas en un compás"));
                    }
                }
                "barline" => read_barline(n, &mut bar, index + 1 == measures.len(), warnings),
                "print" => {}
                _ => warnings.add(format!(
                    "Elemento MusicXML «{}» no importado",
                    n.tag_name().name()
                )),
            }
        }
        if bar.jump_from_sound {
            bar.jump = bar.jump.map(|jump| {
                if bar.fine {
                    if jump.from_segno() {
                        Jump::DalSegnoAlFine
                    } else {
                        Jump::DaCapoAlFine
                    }
                } else if bar.to_coda {
                    if jump.from_segno() {
                        Jump::DalSegnoAlCoda
                    } else {
                        Jump::DaCapoAlCoda
                    }
                } else {
                    jump
                }
            });
        }
        bar.staves = state.staves;
        if index == 0 {
            bar.time.get_or_insert(state.time);
            bar.key.get_or_insert(transpose_key(
                state.key,
                state.transpose[0],
                state.intervals[0],
            ));
        }
        // Unplaced words at the start of a bar with a tempo complete its
        // indication, as Levare wrote them before staff texts.
        for (mark, placed) in std::mem::take(&mut bar.texts) {
            if let MarkType::Text(_, text) = &mark.kind
                && mark.tick == 0
                && !placed
                && let Some(tempo) = &mut bar.tempo
            {
                if !tempo.text.is_empty() {
                    tempo.text.push(' ');
                }
                tempo.text.push_str(text);
            } else {
                bar.directions.push(mark);
            }
        }
        if let Some(t) = &mut bar.tempo
            && t.text.chars().count() > 80
        {
            t.text = t.text.chars().take(80).collect();
            warnings.add("Los textos de tempo se limitan a 80 caracteres");
        }
        passage |= !instrument.is_grand_staff()
            && bar.directions.iter().any(|m| match m.kind {
                MarkType::Division(Division::Unison) | MarkType::Visibility(false) => true,
                MarkType::Division(Division::Divisi) => index > 0 || m.tick > 0,
                _ => false,
            });
        bars.push(bar);
    }
    let two = bars
        .iter()
        .any(|b| b.staves == 2 || b.events.iter().any(|e| e.staff == 2));
    if !two
        && !instrument.is_grand_staff()
        && bars.iter().any(|b| {
            b.directions
                .iter()
                .any(|m| matches!(m.kind, MarkType::Division(_)))
        })
    {
        passage = true;
    }
    let transposition = measures
        .iter()
        .flat_map(|b| b.descendants())
        .find(|n| n.has_tag_name("transpose"))
        .map(|n| xml_interval(n, instrument.standard_transposition()))
        .transpose()?;
    Ok(PartData {
        instrument,
        transposition,
        bars,
        passage,
    })
}

fn xml_interval(n: Node<'_, '_>, fallback: Transposition) -> Result<Transposition, FileError> {
    let octave = text(n, "octave-change").map_or(Ok(0i32), |s| s.parse().map_err(bad_number))?;
    let chromatic = integer(n, "chromatic")?
        .checked_add(
            octave
                .checked_mul(12)
                .ok_or_else(|| error("Transposición fuera de rango"))?,
        )
        .ok_or_else(|| error("Transposición fuera de rango"))?;
    let diatonic = if let Some(s) = text(n, "diatonic") {
        s.parse::<i32>()
            .map_err(bad_number)?
            .checked_add(
                octave
                    .checked_mul(7)
                    .ok_or_else(|| error("Transposición fuera de rango"))?,
            )
            .ok_or_else(|| error("Transposición fuera de rango"))?
    } else if chromatic == i32::from(fallback.chromatic) {
        i32::from(fallback.diatonic)
    } else {
        (f64::from(chromatic) * 7. / 12.).round() as i32
    };
    let value = Transposition::new(
        i8::try_from(diatonic).map_err(bad_number)?,
        i8::try_from(chromatic).map_err(bad_number)?,
    );
    value.validate()?;
    Ok(value)
}

fn attributes(
    n: Node<'_, '_>,
    state: &mut State,
    bar: &mut Bar,
    cursor: u16,
    warnings: &mut Warnings,
) -> Result<(), FileError> {
    if let Some(d) = child(n, "divisions") {
        state.divisions = d.text().unwrap_or("").trim().parse().map_err(bad_number)?;
        if !state.divisions.is_finite() || state.divisions <= 0.0 || state.divisions > 1e9 {
            return Err(error("MusicXML: divisiones inválidas"));
        }
    }
    if let Some(s) = text(n, "staves") {
        state.staves = s.parse().map_err(bad_number)?;
        if !(1..=2).contains(&state.staves) {
            return Err(error("MusicXML: máximo de dos pentagramas por instrumento"));
        }
    }
    for t in n.children().filter(|c| c.has_tag_name("transpose")) {
        let chromatic = integer(t, "chromatic")?;
        let octave =
            text(t, "octave-change").map_or(Ok(0), |s| s.parse::<i32>().map_err(bad_number))?;
        let amount = chromatic
            .checked_add(
                octave
                    .checked_mul(12)
                    .ok_or_else(|| error("Transposición fuera de rango"))?,
            )
            .ok_or_else(|| error("Transposición fuera de rango"))?;
        if !(-48..=48).contains(&amount) {
            return Err(error("MusicXML: transposición fuera de rango"));
        }
        if let Some(staff) = t.attribute("number") {
            state.transpose[staff_index(staff)?] = amount;
            state.intervals[staff_index(staff)?] = Some(xml_interval(t, Transposition::default())?);
        } else {
            state.transpose = [amount; 2];
            state.intervals = [Some(xml_interval(t, Transposition::default())?); 2];
        }
        if child(t, "double").is_some() {
            warnings.add("La duplicación de octava de un instrumento no se importa");
        }
    }
    if let Some(time) = child(n, "time") {
        if cursor != 0 {
            return Err(error(
                "MusicXML: cambio de compás dentro de un compás no admitido",
            ));
        }
        if time.children().filter(|c| c.has_tag_name("beats")).count() != 1 {
            return Err(error("MusicXML: métrica compuesta aditiva no admitida"));
        }
        let numerator = u8::try_from(integer(time, "beats")?).map_err(bad_number)?;
        let denominator = u8::try_from(integer(time, "beat-type")?).map_err(bad_number)?;
        let value = TimeSignature {
            numerator,
            denominator,
        };
        if !value.is_valid() {
            return Err(error("MusicXML: métrica fuera de rango"));
        }
        state.time = value;
        bar.time = Some(value);
    }
    if let Some(k) = child(n, "key") {
        if cursor != 0 {
            return Err(error(
                "MusicXML: cambio de tonalidad dentro del compás no admitido",
            ));
        }
        let fifths = i8::try_from(integer(k, "fifths")?).map_err(bad_number)?;
        let mode = match text(k, "mode").unwrap_or("major") {
            "major" => KeyMode::Major,
            "minor" => KeyMode::Minor,
            _ => {
                warnings.add("El modo de la armadura se interpreta como mayor");
                KeyMode::Major
            }
        };
        let key = KeySignature::new(fifths, mode)
            .ok_or_else(|| error("MusicXML: armadura fuera de rango"))?;
        state.key = key;
        bar.key = Some(transpose_key(key, state.transpose[0], state.intervals[0]));
    }
    for c in n.children().filter(|c| c.has_tag_name("clef")) {
        if cursor != 0 {
            return Err(error(
                "MusicXML: cambio de clave dentro del compás no admitido",
            ));
        }
        let staff = (staff_index(c.attribute("number").unwrap_or("1"))? + 1) as u8;
        let line = text(c, "line").unwrap_or("2");
        let clef = match (text(c, "sign"), line) {
            (Some("G"), "2") => Some(Clef::Treble),
            (Some("F"), "4") => Some(Clef::Bass),
            (Some("C"), "3") => Some(Clef::Alto),
            (Some("C"), "4") => Some(Clef::Tenor),
            (Some("percussion"), _) => Some(Clef::Percussion),
            _ => None,
        };
        if let Some(clef) = clef {
            bar.clefs.insert(staff, clef);
        } else {
            warnings.add("La clave especial se redibuja con la clave del instrumento");
        }
        if text(c, "clef-octave-change").is_some() {
            warnings.add("La octava de la clave se conserva mediante las alturas; la clave se redibuja sin indicación de octava");
        }
    }
    for d in n.children().filter(|c| c.has_tag_name("staff-details")) {
        if d.attribute("number") == Some("2")
            && let Some(show) = d.attribute("print-object")
        {
            bar.directions.push(Mark {
                tick: cursor,
                staff: 1,
                voice: None,
                kind: MarkType::Visibility(show != "no"),
            });
        }
    }
    Ok(())
}
fn transpose_key(
    key: KeySignature,
    semitones: i32,
    interval: Option<Transposition>,
) -> KeySignature {
    if let Some(interval) = interval {
        return interval.inverse().written_key(key);
    }
    if semitones.rem_euclid(12) == 0 {
        return key;
    }
    let target = (i32::from(key.fifths()) * 7 + semitones).rem_euclid(12);
    (-7i8..=7)
        .filter(|&f| (i32::from(f) * 7).rem_euclid(12) == target)
        .min_by_key(|f| f.abs())
        .and_then(|f| KeySignature::new(f, key.mode()))
        .unwrap()
}
fn staff_index(s: &str) -> Result<usize, FileError> {
    match s {
        "1" => Ok(0),
        "2" => Ok(1),
        _ => Err(error("MusicXML: pentagrama fuera de rango (1–2)")),
    }
}
fn pitch_value(
    n: Node<'_, '_>,
    transpose: i32,
    interval: Option<Transposition>,
    warnings: &mut Warnings,
) -> Result<Pitch, FileError> {
    let step = Step::from_letter(&text(n, "step").unwrap_or("").to_lowercase())
        .ok_or_else(|| error("MusicXML: altura sin nombre válido"))?;
    let octave = integer(n, "octave")?;
    let alter = text(n, "alter").map_or(Ok(0), |_| integer(n, "alter"))?;
    if !(0..=8).contains(&octave) || !(-2..=2).contains(&alter) {
        return Err(error(
            "MusicXML: altura fuera de rango o microtonalidad no admitida",
        ));
    }
    let midi = octave * 12 + [0, 2, 4, 5, 7, 9, 11][step.index() as usize] + alter + transpose;
    if !(0..=107).contains(&midi) {
        return Err(error(
            "MusicXML: nota transpuesta fuera del registro admitido",
        ));
    }
    if let Some(interval) = interval
        && (-1..=1).contains(&alter)
    {
        let pitch = Pitch::new(step, octave as u8)
            .unwrap()
            .with_accidental(Some(match alter {
                -1 => Accidental::Flat,
                1 => Accidental::Sharp,
                _ => Accidental::Natural,
            }));
        if let Ok(pitches) = interval.pitches(Pitches::single(Some(pitch)), KeySignature::default())
        {
            return Ok(pitches.first().unwrap());
        }
    }
    if transpose == 0 && (-1..=1).contains(&alter) {
        return Ok(Pitch::new(step, octave as u8)
            .unwrap()
            .with_accidental(Some(match alter {
                -1 => Accidental::Flat,
                1 => Accidental::Sharp,
                _ => Accidental::Natural,
            })));
    }
    if alter.abs() == 2 {
        warnings.add("Las dobles alteraciones se convierten a notas enarmónicas equivalentes");
    }
    let (step, alter) = match midi.rem_euclid(12) {
        0 => (Step::C, 0),
        1 => (Step::C, 1),
        2 => (Step::D, 0),
        3 => (Step::E, -1),
        4 => (Step::E, 0),
        5 => (Step::F, 0),
        6 => (Step::F, 1),
        7 => (Step::G, 0),
        8 => (Step::A, -1),
        9 => (Step::A, 0),
        10 => (Step::B, -1),
        _ => (Step::B, 0),
    };
    Ok(Pitch::new(step, (midi / 12) as u8)
        .unwrap()
        .with_accidental(Some(match alter {
            -1 => Accidental::Flat,
            1 => Accidental::Sharp,
            _ => Accidental::Natural,
        })))
}
/// Sung syllables of a note, verse by verse; a rest takes none.
fn read_lyrics(n: Node<'_, '_>, notation: &mut Notation, warnings: &mut Warnings) {
    let rest = child(n, "rest").is_some();
    for (index, lyric) in n.children().filter(|c| c.has_tag_name("lyric")).enumerate() {
        let verse = lyric
            .attribute("number")
            // «1», or the last number of a name such as «part1verse2».
            .and_then(|number| {
                number
                    .split(|c: char| !c.is_ascii_digit())
                    .rfind(|digits| !digits.is_empty())?
                    .parse::<usize>()
                    .ok()
            })
            .filter(|&number| number >= 1)
            .map_or(index, |number| number - 1);
        // An elision joins two syllables on one note.
        let words: Vec<_> = lyric
            .children()
            .filter(|c| c.has_tag_name("text"))
            .filter_map(|t| t.text())
            .collect();
        let mut text = words.join(" ").trim().to_owned();
        if text.is_empty() {
            continue;
        }
        if rest {
            warnings.add("La letra sobre silencios no se importa");
            continue;
        }
        if verse >= MAX_VERSES {
            warnings.add("Solo se importan las cuatro primeras estrofas");
            continue;
        }
        if text.len() > MAX_SYLLABLE_BYTES {
            let mut end = MAX_SYLLABLE_BYTES;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
            warnings.add("Las sílabas de más de 20 caracteres se recortan");
        }
        let join = if child(lyric, "extend").is_some() {
            Join::Melisma
        } else {
            match self::text(lyric, "syllabic") {
                Some("begin" | "middle") => Join::Hyphen,
                _ => Join::Word,
            }
        };
        match Syllable::new(&text, join) {
            Ok(syllable) => {
                notation.lyrics.set(verse, Some(syllable));
            }
            Err(_) => warnings.add("Una sílaba no pudo importarse"),
        }
    }
}

/// A chord symbol over the staff at its offset from the cursor.
fn read_harmony(
    n: Node<'_, '_>,
    cursor: u16,
    state: &State,
    bar: &mut Bar,
    warnings: &mut Warnings,
) -> Result<(), FileError> {
    let offset = text(n, "offset").map_or(Ok(0.0), |s| s.parse::<f64>().map_err(bad_number))?;
    let tickf = f64::from(cursor) + offset * f64::from(TICKS_PER_QUARTER) / state.divisions;
    if !tickf.is_finite() || tickf < 0.0 || (tickf - tickf.round()).abs() > 1e-6 {
        warnings.add("Un cifrado fuera de lugar no se importa");
        return Ok(());
    }
    let staff = (staff_index(text(n, "staff").unwrap_or("1"))? + 1) as u8;
    let read_root = |name: &str| -> Option<ChordRoot> {
        let node = child(n, name)?;
        let step = Step::from_letter(&text(node, &format!("{name}-step"))?.trim().to_lowercase())?;
        let alteration = match text(node, &format!("{name}-alter")) {
            Some(alter) => alter.trim().parse::<f64>().ok()?,
            None => 0.,
        };
        ((-1. ..=1.).contains(&alteration) && alteration.fract() == 0.).then_some(ChordRoot {
            step,
            alteration: alteration as i8,
        })
    };
    let Some(kind) = child(n, "kind") else {
        warnings.add("Un cifrado sin tipo no se importa");
        return Ok(());
    };
    let value = kind.text().unwrap_or("").trim();
    if value == "none" {
        warnings.add("Los cifrados «N.C.» no se importan");
        return Ok(());
    }
    let degree = n
        .children()
        .filter(|c| c.has_tag_name("degree"))
        .find(|d| text(*d, "degree-type") == Some("add"))
        .and_then(|d| {
            Some((
                text(d, "degree-value")?.trim().parse::<u8>().ok()?,
                text(d, "degree-alter").map_or(Some(0), |alter| alter.trim().parse::<i8>().ok())?,
            ))
        });
    let printed = kind
        .attribute("text")
        .filter(|text| !text.trim().is_empty());
    let (Some(root), Some(quality)) = (
        read_root("root"),
        ChordQuality::from_musicxml(value, degree, printed),
    ) else {
        warnings.add("Algunos cifrados no reconocidos no se importan");
        return Ok(());
    };
    let bass = match child(n, "bass") {
        Some(_) => match read_root("bass") {
            Some(bass) => Some(bass),
            None => {
                warnings.add("Algunos bajos de cifrado no se importan");
                None
            }
        },
        None => None,
    };
    bar.directions.push(Mark {
        tick: tickf.round() as u16,
        staff,
        voice: None,
        kind: MarkType::Chord(ChordSymbol {
            root,
            quality,
            bass,
        }),
    });
    Ok(())
}

fn read_note(
    n: Node<'_, '_>,
    onset: u16,
    state: &State,
    warnings: &mut Warnings,
    chord_parent: Option<&RawEvent>,
) -> Result<RawEvent, FileError> {
    let staff = if let Some(s) = text(n, "staff") {
        staff_index(s)?
    } else {
        chord_parent.map_or(0, |e| usize::from(e.staff - 1))
    };
    let voice = text(n, "voice")
        .or_else(|| chord_parent.map(|e| e.voice.as_str()))
        .unwrap_or("1")
        .to_owned();
    if voice.len() > 64 {
        return Err(error("MusicXML: nombre de voz demasiado largo"));
    }
    let length = ticks(number(n, "duration")?, state.divisions)?;
    if length == 0 {
        return Err(error("MusicXML: nota de duración cero"));
    }
    let mut notation = Notation::default();
    let harmonic = n
        .descendants()
        .find(|n| n.has_tag_name("harmonic"))
        .map(|h| {
            let kind = if child(h, "artificial").is_some() {
                Harmonic::Artificial
            } else {
                Harmonic::Natural
            };
            let role = if child(h, "touching-pitch").is_some() {
                "touch"
            } else if child(h, "sounding-pitch").is_some() {
                "sound"
            } else if child(h, "base-pitch").is_some() {
                "base"
            } else {
                "unspecified"
            };
            (kind, role.to_owned())
        });
    let mut pitches = if let Some(p) = child(n, "pitch") {
        vec![Tone {
            pitch: pitch_value(p, state.transpose[staff], state.intervals[staff], warnings)?,
            harmonic,
            diamond: text(n, "notehead") == Some("diamond"),
            ties: [false; 2],
        }]
    } else if let Some(unpitched) = child(n, "unpitched") {
        let step = match text(unpitched, "display-step").unwrap_or("B") {
            "C" => Step::C,
            "D" => Step::D,
            "E" => Step::E,
            "F" => Step::F,
            "G" => Step::G,
            "A" => Step::A,
            _ => Step::B,
        };
        let octave = text(unpitched, "display-octave")
            .unwrap_or("4")
            .parse::<u8>()
            .map_err(bad_number)?;
        vec![Tone {
            pitch: Pitch::new(step, octave).ok_or_else(|| error("Percusión fuera de registro"))?,
            harmonic: None,
            diamond: false,
            ties: [false; 2],
        }]
    } else if child(n, "rest").is_some() {
        Vec::new()
    } else {
        return Err(error("MusicXML: percusión sin altura no admitida"));
    };
    if n.attribute("pizzicato") == Some("yes") {
        notation.technique = Some(PlayingTechnique::Pizzicato);
    } else if n.attribute("pizzicato") == Some("no") {
        notation.technique = Some(PlayingTechnique::Arco);
    }
    if child(n, "cue").is_some() {
        warnings.add("Las notas guía se importan como notas normales");
    }
    read_lyrics(n, &mut notation, warnings);
    let mut endpoints = Vec::new();
    for t in n.children().filter(|t| t.has_tag_name("tie")) {
        if let Some(stop) = match t.attribute("type") {
            Some("start") => Some(false),
            Some("stop") => Some(true),
            _ => None,
        } {
            endpoints.push((ConnectionKind::Tie, stop, "1".into()));
        }
    }
    let mut arpeggio = None;
    if let Some(ns) = child(n, "notations") {
        for m in ns.children().filter(Node::is_element) {
            match m.tag_name().name() {
                "slur" | "tied" => {
                    let kind = if m.has_tag_name("slur") {
                        ConnectionKind::Slur
                    } else {
                        ConnectionKind::Tie
                    };
                    for (typ, stop) in [("stop", true), ("start", false)] {
                        if m.attribute("type") == Some(typ)
                            || m.attribute("type") == Some("continue")
                        {
                            endpoints.push((
                                kind,
                                stop,
                                if kind == ConnectionKind::Tie {
                                    "1"
                                } else {
                                    m.attribute("number").unwrap_or("1")
                                }
                                .into(),
                            ));
                        }
                    }
                }
                "fermata" => notation.fermata = true,
                "articulations" => {
                    for a in m.children().filter(Node::is_element) {
                        let art = match a.tag_name().name() {
                            "staccato" => Some(Articulation::Staccato),
                            "tenuto" => Some(Articulation::Tenuto),
                            "accent" => Some(Articulation::Accent),
                            "strong-accent" => Some(Articulation::Marcato),
                            _ => None,
                        };
                        if art.is_none() || notation.articulation.is_some() {
                            warnings.add(
                                "Algunas articulaciones no admitidas o combinadas no se importan",
                            );
                        }
                        if let Some(a) = art {
                            notation.articulation = Some(a);
                        }
                    }
                }
                "glissando" | "slide" => {
                    if let Some(stop) = match m.attribute("type") {
                        Some("start") => Some(false),
                        Some("stop") => Some(true),
                        _ => None,
                    } {
                        endpoints.push((
                            ConnectionKind::Glissando,
                            stop,
                            m.attribute("number").unwrap_or("1").into(),
                        ));
                    }
                }
                "ornaments" => {
                    let wavy = |typ: &str| {
                        m.children()
                            .find(|o| {
                                o.has_tag_name("wavy-line") && o.attribute("type") == Some(typ)
                            })
                            .map(|o| o.attribute("number").unwrap_or("1").to_owned())
                    };
                    let (start, stop) = (wavy("start"), wavy("stop"));
                    let mark = m.children().any(|o| o.has_tag_name("trill-mark"));
                    if let Some(number) = start.clone().or_else(|| mark.then(|| "1".into())) {
                        endpoints.push((ConnectionKind::Trill, false, number.clone()));
                        // The sign alone trills this note only.
                        if start.is_none() && stop.is_none() {
                            endpoints.push((ConnectionKind::Trill, true, number));
                        }
                    }
                    if let Some(number) = stop {
                        endpoints.push((ConnectionKind::Trill, true, number));
                    }
                    for ornament in m.children().filter(Node::is_element) {
                        if ornament.has_tag_name("trill-mark") || ornament.has_tag_name("wavy-line")
                        {
                            continue;
                        }
                        if ornament.has_tag_name("tremolo") {
                            match ornament.attribute("type") {
                                None | Some("single") => {
                                    if let Some(mark) = ornament
                                        .text()
                                        .and_then(|value| value.trim().parse::<u8>().ok())
                                        .and_then(Tremolo::from_strokes)
                                    {
                                        notation.tremolo = Some(mark);
                                    } else {
                                        warnings.add("Trémolo con un número de barras no admitido");
                                    }
                                }
                                Some("start" | "stop") => {
                                    warnings.add("El trémolo entre dos notas no se importa")
                                }
                                _ => warnings.add("Notación «ornaments» no importada"),
                            }
                        } else {
                            warnings.add("Notación «ornaments» no importada");
                        }
                    }
                }
                "arpeggiate" => {
                    let kind = match m.attribute("direction") {
                        Some("up") => Arpeggio::Up,
                        Some("down") => Arpeggio::Down,
                        _ => Arpeggio::Normal,
                    };
                    arpeggio = Some((m.attribute("number").unwrap_or("1").into(), kind));
                }
                "technical" => {
                    if m.children()
                        .any(|c| c.is_element() && !c.has_tag_name("harmonic"))
                    {
                        warnings.add("Algunas técnicas de ejecución no admitidas no se importan");
                    }
                }
                "tuplet" => {}
                _ => warnings.add(format!("Notación «{}» no importada", m.tag_name().name())),
            }
        }
    }
    for tone in &mut pitches {
        for (kind, stop, _) in &endpoints {
            if *kind == ConnectionKind::Tie {
                tone.ties[usize::from(*stop)] = true;
            }
        }
    }
    let written = value_from_xml(n).filter(|d| d.ticks() == length);
    Ok(RawEvent {
        onset,
        length,
        written,
        staff: (staff + 1) as u8,
        voice,
        pitches,
        notation,
        endpoints,
        arpeggio,
    })
}

fn sound_tempo(n: Node<'_, '_>, warnings: &mut Warnings) -> Result<Option<TempoMark>, FileError> {
    let Some(s) = n.attribute("tempo") else {
        return Ok(None);
    };
    let bpm: f64 = s.parse().map_err(bad_number)?;
    if !bpm.is_finite() || !(10.0..=1200.0).contains(&bpm) {
        return Err(error("MusicXML: tempo fuera del rango representable"));
    }
    let (unit, beat) = if bpm < 20.0 {
        (bpm * 2.0, TempoBeat::Eighth)
    } else if bpm <= 400.0 {
        (bpm, TempoBeat::Quarter)
    } else if bpm <= 800.0 {
        (bpm / 2.0, TempoBeat::Half)
    } else {
        (bpm / 3.0, TempoBeat::DottedHalf)
    };
    if (unit - unit.round()).abs() > 1e-6 {
        warnings.add("Los tempos fraccionarios se redondean al pulso entero más cercano");
    }
    Ok(Some(TempoMark {
        bpm: unit.round() as u16,
        beat,
        text: String::new(),
    }))
}

fn sound_technique(n: Node<'_, '_>) -> Result<Option<PlayingTechnique>, FileError> {
    match n.attribute("pizzicato") {
        Some("yes") => Ok(Some(PlayingTechnique::Pizzicato)),
        Some("no") => Ok(Some(PlayingTechnique::Arco)),
        Some(_) => Err(error("MusicXML: indicación pizzicato inválida")),
        None => Ok(None),
    }
}
fn read_barline(n: Node<'_, '_>, bar: &mut Bar, last: bool, warnings: &mut Warnings) {
    for repeat in n.children().filter(|node| node.has_tag_name("repeat")) {
        match repeat.attribute("direction") {
            Some("forward") => bar.repeat_start = true,
            Some("backward") => {
                let times = repeat
                    .attribute("times")
                    .and_then(|value| value.parse::<i64>().ok())
                    .unwrap_or(2);
                bar.repeat_end = Some(times.clamp(2, i64::from(MAX_REPEAT_TIMES)) as u8);
            }
            _ => {}
        }
    }
    if n.attribute("location") != Some("left") {
        match text(n, "bar-style") {
            Some("light-light") => bar.barline = Some(Barline::Double),
            // A repeat's heavy stroke is implicit; so is the final score bar.
            Some("light-heavy") if bar.repeat_end.is_none() && !last => {
                bar.barline = Some(Barline::Final)
            }
            _ => {}
        }
    }
    for ending in n.children().filter(|node| node.has_tag_name("ending")) {
        match ending.attribute("type") {
            Some("start") => {
                let numbers: Vec<_> = ending
                    .attribute("number")
                    .unwrap_or_default()
                    .split(|character: char| character == ',' || character.is_whitespace())
                    .filter_map(|value| value.parse::<u8>().ok())
                    .filter(|number| (1..=MAX_ENDING_NUMBER).contains(number))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                if numbers.is_empty() {
                    warnings.add("Casilla con números no admitidos");
                } else {
                    bar.ending_start = Some(numbers);
                }
            }
            Some("stop" | "discontinue") => bar.ending_stop = true,
            _ => {}
        }
    }
}

fn navigation_words(words: &str, bar: &mut Bar) -> bool {
    let words = words
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let jump = match words.as_str() {
        "fine" => {
            bar.fine = true;
            return true;
        }
        "to coda" | "al coda" => {
            bar.to_coda = true;
            return true;
        }
        "d.c." | "da capo" => Jump::DaCapo,
        "d.c. al fine" => Jump::DaCapoAlFine,
        "d.c. al coda" => Jump::DaCapoAlCoda,
        "d.s." | "dal segno" => Jump::DalSegno,
        "d.s. al fine" => Jump::DalSegnoAlFine,
        "d.s. al coda" => Jump::DalSegnoAlCoda,
        _ => return false,
    };
    bar.jump = Some(jump);
    bar.jump_from_sound = false;
    true
}

fn navigation_sound(sound: Node<'_, '_>, bar: &mut Bar) {
    let present = |name| {
        sound
            .attribute(name)
            .is_some_and(|value| !value.is_empty() && value != "no")
    };
    bar.segno |= present("segno");
    bar.coda |= present("coda");
    bar.to_coda |= present("tocoda");
    bar.fine |= present("fine");
    if bar.jump.is_none() {
        bar.jump = if present("dalsegno") {
            Some(Jump::DalSegno)
        } else if present("dacapo") {
            Some(Jump::DaCapo)
        } else {
            None
        };
        bar.jump_from_sound = bar.jump.is_some();
    }
}

fn read_direction(
    n: Node<'_, '_>,
    cursor: u16,
    state: &State,
    bar: &mut Bar,
    warnings: &mut Warnings,
) -> Result<(), FileError> {
    let offset = text(n, "offset").map_or(Ok(0.0), |s| s.parse::<f64>().map_err(bad_number))?;
    let tickf = f64::from(cursor) + offset * f64::from(TICKS_PER_QUARTER) / state.divisions;
    if !tickf.is_finite()
        || tickf < 0.0
        || tickf > f64::from(16 * TICKS_PER_WHOLE)
        || (tickf - tickf.round()).abs() > 1e-6
    {
        return Err(error("MusicXML: posición de indicación fuera de rango"));
    }
    let tick = tickf.round() as u16;
    let staff = (staff_index(text(n, "staff").unwrap_or("1"))? + 1) as u8;
    let voice = text(n, "voice").map(str::to_owned);
    let has_metronome = n
        .children()
        .filter(|d| d.has_tag_name("direction-type"))
        .any(|d| child(d, "metronome").is_some());
    let mut tempo = if has_metronome {
        None
    } else {
        child(n, "sound")
            .map(|s| sound_tempo(s, warnings))
            .transpose()?
            .flatten()
    };
    if let Some(technique) = child(n, "sound")
        .map(sound_technique)
        .transpose()?
        .flatten()
    {
        bar.directions.push(Mark {
            tick,
            staff,
            voice: voice.clone(),
            kind: MarkType::Technique(technique),
        });
    }
    let mut free_words = Vec::new();
    for dt in n.children().filter(|d| d.has_tag_name("direction-type")) {
        for d in dt.children().filter(Node::is_element) {
            let kind = match d.tag_name().name() {
                "rehearsal" => {
                    let text = d.text().unwrap_or("").trim();
                    if !text.is_empty() {
                        bar.rehearsal = Some(text.to_owned());
                    }
                    None
                }
                "dynamics" => {
                    let dynamic = d.children().filter(Node::is_element).find_map(|n| {
                        Dynamic::ALL
                            .into_iter()
                            .find(|a| a.label() == n.tag_name().name())
                    });
                    if dynamic.is_none() {
                        warnings.add("Algunas dinámicas no admitidas no se importan");
                    }
                    dynamic.map(MarkType::Dynamic)
                }
                "words" => {
                    let s = d.text().unwrap_or("").trim();
                    let lower = s.to_lowercase();
                    if navigation_words(s, bar) {
                        None
                    } else if lower.starts_with("pizz") {
                        Some(MarkType::Technique(PlayingTechnique::Pizzicato))
                    } else if lower == "arco" {
                        Some(MarkType::Technique(PlayingTechnique::Arco))
                    } else if lower == "div." || lower == "divisi" {
                        Some(MarkType::Division(Division::Divisi))
                    } else if lower == "unis." || lower == "unison" {
                        Some(MarkType::Division(Division::Unison))
                    } else {
                        if !s.is_empty() {
                            free_words.push((s.to_owned(), text_kind(n, d, s)));
                        }
                        None
                    }
                }
                "segno" => {
                    bar.segno = true;
                    None
                }
                "coda" => {
                    if child(n, "sound").is_some_and(|sound| sound.attribute("tocoda").is_some()) {
                        bar.to_coda = true;
                    } else {
                        bar.coda = true;
                    }
                    None
                }
                "wedge" => {
                    let typ = d.attribute("type").unwrap_or("");
                    let kind = match typ {
                        "crescendo" => Some((false, ConnectionKind::Crescendo)),
                        "diminuendo" => Some((false, ConnectionKind::Diminuendo)),
                        "stop" => Some((true, ConnectionKind::Crescendo)),
                        _ => None,
                    };
                    kind.map(|(stop, k)| {
                        MarkType::Wedge(stop, k, d.attribute("number").unwrap_or("1").into())
                    })
                }
                "octave-shift" => {
                    let number = d.attribute("number").unwrap_or("1").to_owned();
                    match (d.attribute("type"), d.attribute("size").unwrap_or("8")) {
                        (Some("stop"), _) => {
                            Some(MarkType::Span(true, ConnectionKind::OttavaAlta, number))
                        }
                        (Some("down"), "8") => {
                            Some(MarkType::Span(false, ConnectionKind::OttavaAlta, number))
                        }
                        (Some("up"), "8") => {
                            Some(MarkType::Span(false, ConnectionKind::OttavaBassa, number))
                        }
                        _ => {
                            warnings.add("Solo se importan las líneas de una octava (8va y 8vb)");
                            None
                        }
                    }
                }
                "pedal" => {
                    let number = d.attribute("number").unwrap_or("1").to_owned();
                    let span = |stop| MarkType::Span(stop, ConnectionKind::Pedal, number.clone());
                    match d.attribute("type") {
                        Some("start") => Some(span(false)),
                        Some("stop") => Some(span(true)),
                        Some("change") => {
                            bar.directions.push(Mark {
                                tick,
                                staff,
                                voice: voice.clone(),
                                kind: span(true),
                            });
                            Some(span(false))
                        }
                        _ => None,
                    }
                }
                "metronome" => {
                    let bpm = number(d, "per-minute")?;
                    let beat = match (text(d, "beat-unit"), child(d, "beat-unit-dot").is_some()) {
                        (Some("half"), false) => TempoBeat::Half,
                        (Some("half"), true) => TempoBeat::DottedHalf,
                        (Some("quarter"), false) => TempoBeat::Quarter,
                        (Some("quarter"), true) => TempoBeat::DottedQuarter,
                        (Some("eighth"), false) => TempoBeat::Eighth,
                        (Some("eighth"), true) => TempoBeat::DottedEighth,
                        _ => return Err(error("MusicXML: unidad de tempo no admitida")),
                    };
                    if !(20.0..=400.0).contains(&bpm) {
                        return Err(error("MusicXML: tempo fuera de rango"));
                    }
                    if (bpm - bpm.round()).abs() > 1e-6 {
                        warnings.add(
                            "Los tempos fraccionarios se redondean al pulso entero más cercano",
                        );
                    }
                    tempo = Some(TempoMark {
                        bpm: bpm.round() as u16,
                        beat,
                        text: String::new(),
                    });
                    None
                }
                _ => {
                    warnings.add(format!("Indicación «{}» no importada", d.tag_name().name()));
                    None
                }
            };
            if let Some(kind) = kind {
                bar.directions.push(Mark {
                    tick,
                    staff,
                    voice: voice.clone(),
                    kind,
                });
            }
        }
    }
    if let Some(sound) = child(n, "sound") {
        navigation_sound(sound, bar);
    }
    if let Some(mut tempo) = tempo {
        if tick == 0 {
            tempo.text = free_words
                .into_iter()
                .map(|(text, _)| text)
                .collect::<Vec<_>>()
                .join(" ");
            bar.tempo = Some(tempo);
        } else {
            warnings.add("Los cambios de tempo dentro del compás no se importan");
        }
    } else {
        let placed = n.attribute("placement").is_some();
        bar.texts.extend(free_words.into_iter().map(|(text, kind)| {
            (
                Mark {
                    tick,
                    staff,
                    voice: voice.clone(),
                    kind: MarkType::Text(kind, text),
                },
                placed,
            )
        }));
    }
    Ok(())
}

/// Below the staff, or in italics without a placement, words are an
/// expression; above, an instruction. «cresc.» and «dim.» are always expressions.
fn text_kind(direction: Node<'_, '_>, words: Node<'_, '_>, text: &str) -> TextKind {
    let swell = Words::new(text)
        .ok()
        .and_then(|words| words.effect(TextKind::Expression))
        .is_some_and(|effect| matches!(effect, TextEffect::Swell(_)));
    match direction.attribute("placement") {
        _ if swell => TextKind::Expression,
        Some("below") => TextKind::Expression,
        Some("above") => TextKind::Technique,
        _ if words.attribute("font-style") == Some("italic") => TextKind::Expression,
        _ => TextKind::Technique,
    }
}

fn value_from_xml(n: Node<'_, '_>) -> Option<Duration> {
    let base = match text(n, "type")? {
        "whole" => Duration::Whole,
        "half" => Duration::Half,
        "quarter" => Duration::Quarter,
        "eighth" => Duration::Eighth,
        "16th" => Duration::Sixteenth,
        "32nd" => Duration::ThirtySecond,
        _ => return None,
    };
    let dots = n.children().filter(|c| c.has_tag_name("dot")).count();
    if dots > 1 {
        return None;
    }
    let triplet = if let Some(m) = child(n, "time-modification") {
        if text(m, "actual-notes") != Some("3") || text(m, "normal-notes") != Some("2") {
            return None;
        }
        true
    } else {
        false
    };
    Some(base.with_dotted(dots == 1).with_triplet(triplet))
}

#[cfg(test)]
mod tests;
