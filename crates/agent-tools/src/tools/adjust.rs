//! Settings with a value: each is one editor command and one undo step.
use super::text::{key_text, place, tempo_text, time_text};
use super::{AjustarArgs, ConnectionClass, TextClass, fold, resolve, staff_of};
use score_core::{
    Clef, Connection, ConnectionKind, Instrument, KeyMode, KeySignature, Score, TempoMark,
    TextKind, TimeSignature,
};
use score_edit::{Command, Editor};

pub(super) fn adjust(editor: &mut Editor, args: AjustarArgs) -> Result<String, String> {
    let score = editor.score();
    // Part numbers move when parts come and go, so those replies list them.
    let renumbers = matches!(
        args,
        AjustarArgs::AnadirPartes { .. } | AjustarArgs::QuitarParte { .. }
    );
    let (command, done) = match args {
        AjustarArgs::CompasTiempo {
            compas,
            numerador,
            denominador,
        } => {
            let measure = score_text::bar_index(score, compas)?;
            let time = TimeSignature {
                numerator: numerador,
                denominator: denominador,
            };
            (
                Command::SetTimeSignatureAt(measure, time),
                format!(
                    "Compás de tiempo {} desde el compás {compas}.",
                    time_text(time)
                ),
            )
        }
        AjustarArgs::Tonalidad {
            compas,
            quintas,
            menor,
        } => {
            let measure = score_text::bar_index(score, compas)?;
            let mode = if menor {
                KeyMode::Minor
            } else {
                KeyMode::Major
            };
            let key = KeySignature::new(quintas, mode).ok_or_else(|| {
                format!("Las quintas van de -7 (siete bemoles) a 7 (siete sostenidos); recibido {quintas}.")
            })?;
            (
                Command::SetKeySignatureAt(measure, key),
                format!("Tonalidad {} desde el compás {compas}.", key_text(key)),
            )
        }
        AjustarArgs::Tempo { compas, bpm, texto } => {
            let measure = score_text::bar_index(score, compas)?;
            // Without `texto`, a mark already written at this bar keeps its words.
            let text = texto.unwrap_or_else(|| {
                score
                    .tempo_change_at(measure)
                    .map(|mark| mark.text.clone())
                    .unwrap_or_default()
            });
            let mark = TempoMark {
                bpm,
                beat: score.tempo_at(measure).beat,
                text: text.trim().to_owned(),
            };
            let done = format!("Tempo {} desde el compás {compas}.", tempo_text(&mark));
            (Command::SetTempoAt(measure, mark), done)
        }
        AjustarArgs::Clave {
            parte,
            pentagrama,
            compas,
            clave,
        } => {
            let part = score_text::resolve_part(score, &parte)?;
            let staff = staff_of(pentagrama)?;
            let measure = score_text::bar_index(score, compas)?;
            let clef = clef(&clave)?;
            (
                Command::SetClefAt(part, staff, measure, clef),
                format!(
                    "Clave de {} en {} desde el compás {compas}.",
                    clef.label().to_lowercase(),
                    score.part_name(part)
                ),
            )
        }
        AjustarArgs::AnadirPartes { instrumentos } => {
            if instrumentos.is_empty() {
                return Err("Indica al menos un instrumento.".into());
            }
            let instruments = instrumentos
                .iter()
                .map(|name| instrument(name))
                .collect::<Result<Vec<_>, _>>()?;
            let names: Vec<_> = instruments.iter().map(|i| i.name()).collect();
            (
                Command::AddParts(instruments),
                format!("Añadidas: {}.", names.join(", ")),
            )
        }
        AjustarArgs::QuitarParte { parte } => {
            let part = score_text::resolve_part(score, &parte)?;
            (
                Command::RemovePartById(part),
                format!("Quitada la parte {}.", score.part_name(part)),
            )
        }
        AjustarArgs::Conexion {
            clase,
            desde,
            hasta,
        } => {
            let connection = Connection {
                start: resolve(score, &desde)?,
                end: resolve(score, &hasta)?,
                kind: match clase {
                    ConnectionClass::Ligadura => ConnectionKind::Slur,
                    ConnectionClass::Union => ConnectionKind::Tie,
                    ConnectionClass::Crescendo => ConnectionKind::Crescendo,
                    ConnectionClass::Diminuendo => ConnectionKind::Diminuendo,
                    ConnectionClass::OctavaAlta => ConnectionKind::OttavaAlta,
                    ConnectionClass::OctavaBaja => ConnectionKind::OttavaBassa,
                    ConnectionClass::Pedal => ConnectionKind::Pedal,
                },
            };
            let done = format!(
                "Añadido: {}, de {} a {}.",
                connection.kind.label(),
                place(score, connection.start),
                place(score, connection.end)
            );
            (Command::AddConnection(connection), done)
        }
        AjustarArgs::Texto {
            posicion,
            clase,
            texto,
        } => {
            let id = resolve(score, &posicion)?;
            let kind = match clase {
                TextClass::Expresion => TextKind::Expression,
                TextClass::Tecnica => TextKind::Technique,
            };
            let texto = texto.trim();
            let done = if texto.is_empty() {
                format!(
                    "Quitado el {} en {}.",
                    kind.label().to_lowercase(),
                    place(score, id)
                )
            } else {
                format!("{} «{texto}» en {}.", kind.label(), place(score, id))
            };
            let words = (!texto.is_empty()).then(|| texto.to_owned());
            (Command::SetText(id, kind, words), done)
        }
        AjustarArgs::Titulo { titulo } => (
            Command::SetHeader(titulo.clone(), score.credits().clone()),
            format!("Título: «{}».", titulo.trim()),
        ),
    };
    match editor.execute(command) {
        Ok(true) if renumbers => Ok(format!("{done}\n{}", parts_list(editor.score()))),
        Ok(true) => Ok(done),
        Ok(false) => Ok(format!("Sin cambios: ya estaba así. ({done})")),
        Err(error) => Err(error.0.to_string()),
    }
}

fn parts_list(score: &Score) -> String {
    let parts: Vec<_> = score
        .parts()
        .iter()
        .enumerate()
        .map(|(index, part)| format!("{} {}", index + 1, score.part_name(part.id())))
        .collect();
    format!("Partes: {}.", parts.join(", "))
}

fn clef(name: &str) -> Result<Clef, String> {
    Ok(match fold(name).as_str() {
        "sol" | "sol en segunda" => Clef::Treble,
        "fa" | "fa en cuarta" => Clef::Bass,
        "do3" | "do en tercera" | "contralto" => Clef::Alto,
        "do4" | "do en cuarta" | "tenor" => Clef::Tenor,
        "percusion" => Clef::Percussion,
        _ => {
            return Err(format!(
                "No conozco la clave «{name}». Usa sol, fa, do3 (contralto), do4 (tenor) o percusión."
            ));
        }
    })
}

/// Compared without accents or spaces, with «bemol» as «b».
fn compact(name: &str) -> String {
    fold(name)
        .replace("bemol", "b")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

/// An instrument by its Spanish name: the exact name, or the only one it
/// begins; otherwise an error with the names it could mean.
fn instrument(query: &str) -> Result<Instrument, String> {
    let wanted = compact(query);
    let names: Vec<(Instrument, String)> = Instrument::ALL
        .into_iter()
        .map(|instrument| (instrument, compact(instrument.name())))
        .collect();
    if let Some((instrument, _)) = names.iter().find(|(_, name)| *name == wanted) {
        return Ok(*instrument);
    }
    let mut similar: Vec<Instrument> = names
        .iter()
        .filter(|(_, name)| !wanted.is_empty() && name.starts_with(&wanted))
        .map(|(instrument, _)| *instrument)
        .collect();
    if let [instrument] = similar[..] {
        return Ok(instrument);
    }
    if similar.is_empty() {
        let words: Vec<String> = fold(query)
            .split_whitespace()
            .filter(|word| word.chars().count() >= 3)
            .map(str::to_owned)
            .collect();
        similar = Instrument::ALL
            .into_iter()
            .filter(|instrument| {
                let name = fold(instrument.name());
                words.iter().any(|word| name.contains(word.as_str()))
            })
            .collect();
    }
    let listed: Vec<_> = if similar.is_empty() {
        Instrument::ALL.iter().map(|i| i.name()).collect()
    } else {
        similar.iter().map(|i| i.name()).collect()
    };
    Err(format!(
        "No reconozco el instrumento «{query}». {}: {}.",
        if similar.is_empty() {
            "Instrumentos disponibles"
        } else {
            "Puede ser"
        },
        listed.join(", ")
    ))
}
