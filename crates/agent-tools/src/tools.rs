//! The tool calls and their arguments, as the MCP server announces them, and
//! `dispatch`, which runs one on the host's editor.
use crate::Host;
use schemars::JsonSchema;
use score_core::{ElementId, PartId, Score, StaffId, Voice};
use score_keymap::Registry;
use serde::Deserialize;

mod adjust;
mod read;
mod select;
mod text;
mod write;

/// A note or rest by part, bar and beat, as a musician says it.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[schemars(description = "Una nota o silencio por parte, compás y tiempo.")]
pub struct Position {
    #[schemars(
        description = "Parte: número (1 es la primera) o nombre, como «violín I» o «violas»."
    )]
    pub parte: String,
    #[schemars(
        description = "Compás, contado desde 1 como lo numera Levare (una anacrusa es el compás 1)."
    )]
    pub compas: u32,
    #[schemars(
        description = "Tiempo del compás desde 1, en unidades del denominador (corcheas en 6/8). Admite decimales: 2.5 es la segunda mitad del tiempo 2."
    )]
    #[serde(default = "first_beat")]
    pub tiempo: f32,
    #[schemars(
        description = "Pentagrama: 1 (superior, por defecto) o 2 (mano izquierda del piano o segundo pentagrama de un divisi)."
    )]
    pub pentagrama: Option<u8>,
    #[schemars(description = "Voz: 1 (por defecto) o 2.")]
    pub voz: Option<u8>,
}

fn first_beat() -> f32 {
    1.0
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct LeerArgs {
    #[schemars(description = "Partes que leer, por número o nombre; todas si se omite.")]
    pub partes: Option<Vec<String>>,
    #[schemars(description = "Primer compás, desde 1.")]
    pub desde: u32,
    #[schemars(description = "Último compás incluido; como mucho 32 compases por llamada.")]
    pub hasta: u32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct SeleccionarArgs {
    #[schemars(description = "Nota o silencio que seleccionar, o el comienzo del pasaje.")]
    pub desde: Position,
    #[schemars(
        description = "Final del pasaje, en la misma parte, pentagrama y voz; omítelo para seleccionar un solo evento."
    )]
    pub hasta: Option<Position>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct ComandosArgs {
    #[schemars(
        description = "Categoría que listar, como «Expresión» o «Compases»; todas si se omite."
    )]
    pub categoria: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct EjecutarArgs {
    #[schemars(
        description = "Id del comando (como «expresion.staccato») o su etiqueta exacta, tal como los lista la herramienta comandos."
    )]
    pub comando: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct EscribirArgs {
    #[schemars(description = "Parte: número o nombre.")]
    pub parte: String,
    #[schemars(description = "Pentagrama: 1 (por defecto) o 2.")]
    pub pentagrama: Option<u8>,
    #[schemars(description = "Voz: 1 (por defecto) o 2.")]
    pub voz: Option<u8>,
    #[schemars(
        description = "Compás donde empieza la escritura, desde 1. El siguiente al último añade compases al final."
    )]
    pub desde_compas: u32,
    #[schemars(
        description = "Un compás completo por cadena, en la notación de texto de Levare; cada uno debe llenar exactamente su compás de tiempo."
    )]
    pub compases: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionClass {
    // Slur.
    Ligadura,
    // Tie.
    Union,
    Crescendo,
    Diminuendo,
    OctavaAlta,
    OctavaBaja,
    Pedal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextClass {
    Expresion,
    Tecnica,
}

/// Settings that take a value, which their ribbon commands would ask for in
/// a picker. A connection's kind is `clase`, since `tipo` names the setting.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(tag = "tipo", rename_all = "snake_case")]
#[schemars(description = "Un ajuste con valor; «tipo» dice cuál.")]
pub enum AjustarArgs {
    #[schemars(description = "Compás de tiempo desde un compás, como 3/4 o 6/8.")]
    CompasTiempo {
        compas: u32,
        numerador: u8,
        denominador: u8,
    },
    #[schemars(
        description = "Tonalidad de la obra (altura real) desde un compás: quintas de -7 (7 bemoles) a 7 (7 sostenidos)."
    )]
    Tonalidad {
        compas: u32,
        quintas: i8,
        menor: bool,
    },
    #[schemars(
        description = "Indicación de tempo desde un compás: pulsos por minuto de 20 a 400 y texto opcional, como «Allegro». Sin texto se conserva el que ya hubiera en ese compás; un texto vacío lo quita."
    )]
    Tempo {
        compas: u32,
        bpm: u16,
        texto: Option<String>,
    },
    #[schemars(description = "Cambio de clave: sol, fa, do3 (contralto), do4 (tenor) o percusión.")]
    Clave {
        parte: String,
        pentagrama: Option<u8>,
        compas: u32,
        clave: String,
    },
    #[schemars(
        description = "Añade partes por nombre de instrumento, como «flauta» o «clarinete en si♭»."
    )]
    AnadirPartes { instrumentos: Vec<String> },
    #[schemars(description = "Quita una parte con toda su música.")]
    QuitarParte { parte: String },
    #[schemars(
        description = "Une dos notas: ligadura (de expresión), union (ligadura de unión), crescendo, diminuendo, octava_alta, octava_baja o pedal."
    )]
    Conexion {
        clase: ConnectionClass,
        desde: Position,
        hasta: Position,
    },
    #[schemars(
        description = "Texto en una nota o silencio: de expresión («dolce») o de técnica («con sord.»). Un texto vacío lo quita."
    )]
    Texto {
        posicion: Position,
        clase: TextClass,
        texto: String,
    },
    #[schemars(description = "Título de la obra.")]
    Titulo { titulo: String },
}

/// One call to one of the agent's tools.
#[derive(Clone, Debug)]
pub enum ToolCall {
    Resumen,
    Leer(LeerArgs),
    Seleccionar(SeleccionarArgs),
    Comandos(ComandosArgs),
    Ejecutar(EjecutarArgs),
    Escribir(EscribirArgs),
    Ajustar(AjustarArgs),
}

impl ToolCall {
    /// The call a tool name and its JSON arguments stand for.
    pub fn parse(name: &str, arguments: serde_json::Value) -> Result<Self, String> {
        fn args<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> Result<T, String> {
            serde_json::from_value(value).map_err(|error| format!("Argumentos no válidos: {error}"))
        }
        Ok(match name {
            "resumen" => Self::Resumen,
            "leer" => Self::Leer(args(arguments)?),
            "seleccionar" => Self::Seleccionar(args(arguments)?),
            "comandos" => Self::Comandos(args(arguments)?),
            "ejecutar" => Self::Ejecutar(args(arguments)?),
            "escribir" => Self::Escribir(args(arguments)?),
            "ajustar" => Self::Ajustar(args(arguments)?),
            _ => return Err(format!("No hay ninguna herramienta «{name}».")),
        })
    }
}

/// Runs one call on the host's editor. The reply and the error are Spanish
/// text for the agent.
pub fn dispatch(
    host: &mut dyn Host,
    registry: &Registry,
    call: ToolCall,
) -> Result<String, String> {
    match call {
        ToolCall::Resumen => Ok(read::summary(host.editor())),
        ToolCall::Leer(args) => read::read(host.editor().score(), &args),
        ToolCall::Comandos(args) => read::commands(registry, &args),
        ToolCall::Seleccionar(args) => select::select(host.editor_mut(), &args),
        ToolCall::Ejecutar(args) => select::execute(host, registry, &args),
        ToolCall::Escribir(args) => write::write(host.editor_mut(), &args),
        ToolCall::Ajustar(args) => adjust::adjust(host.editor_mut(), args),
    }
}

/// Pentagrama 1 or 2 as the agent numbers them.
fn staff_of(number: Option<u8>) -> Result<StaffId, String> {
    match number.unwrap_or(1) {
        1 => Ok(StaffId::Upper),
        2 => Ok(StaffId::Lower),
        other => Err(format!(
            "El pentagrama {other} no existe: usa 1 (superior) o 2 (inferior)."
        )),
    }
}

fn voice_of(number: Option<u8>) -> Result<Voice, String> {
    match number.unwrap_or(1) {
        1 => Ok(Voice::One),
        2 => Ok(Voice::Two),
        other => Err(format!("La voz {other} no existe: usa 1 o 2.")),
    }
}

/// The note or rest at a position, with its part's name in every error.
fn resolve(score: &Score, position: &Position) -> Result<ElementId, String> {
    let part = score_text::resolve_part(score, &position.parte)?;
    let staff = staff_of(position.pentagrama)?;
    let voice = voice_of(position.voz)?;
    let measure = score_text::bar_index(score, position.compas)?;
    let tick = score_text::beat_tick(score, measure, position.tiempo)?;
    score_text::event_at(score, part, staff, voice, measure, tick)
}

/// 1-based number of a part, as the summary lists it.
fn part_number(score: &Score, part: PartId) -> usize {
    score
        .parts()
        .iter()
        .position(|p| p.id() == part)
        .map_or(0, |index| index + 1)
}

/// Lowercase without Spanish accents, with music-font signs as letters, so
/// "Clarinete en Si♭" and "clarinete en sib" compare equal.
fn fold(text: &str) -> String {
    text.trim()
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' | 'ü' => 'u',
            'ñ' => 'n',
            '♭' => 'b',
            '♯' => '#',
            c => c,
        })
        .collect()
}
