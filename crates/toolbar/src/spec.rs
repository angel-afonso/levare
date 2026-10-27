//! What the ribbon contains: tabs, groups and the commands of each control.
//! Pure data. Keys, state and the pending mark come from elsewhere.

use score_keymap::{CommandId, id};

/// One tab per task, in the order they appear and ⌥⌘1–8 open them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TabId {
    Write,
    Articulations,
    Dynamics,
    Text,
    Score,
    Playback,
    Review,
    View,
}

impl TabId {
    pub fn label(self) -> &'static str {
        match self {
            Self::Write => "Escribir",
            Self::Articulations => "Articulaciones",
            Self::Dynamics => "Dinámicas",
            Self::Text => "Texto",
            Self::Score => "Partitura",
            Self::Playback => "Reproducción",
            Self::Review => "Revisión",
            Self::View => "Vista",
        }
    }

    /// The command that shows this tab. Published ids keep their old names:
    /// `TAB_NOTE_INPUT` opens Escribir, `TAB_NOTATION` Articulaciones and
    /// `TAB_HOME` Partitura.
    pub fn command(self) -> CommandId {
        match self {
            Self::Write => id::TAB_NOTE_INPUT,
            Self::Articulations => id::TAB_NOTATION,
            Self::Dynamics => id::TAB_DYNAMICS,
            Self::Text => id::TAB_TEXT,
            Self::Score => id::TAB_HOME,
            Self::Playback => id::TAB_PLAYBACK,
            Self::Review => id::TAB_REVIEW,
            Self::View => id::TAB_VIEW,
        }
    }
}

/// How a control is drawn. The renderer maps each variant to a glyph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visual {
    /// A vector icon of the app, by name (`"clipboard"`, `"add-staff"`…).
    Icon(&'static str),
    /// Characters of the music font (SMuFL code points).
    Music(&'static str),
    /// A clef fitted to its icon frame using the font's visible bounds.
    Clef(score_core::Clef),
    /// Small harmonic marks enlarged to a readable ink height.
    Harmonic(score_core::Harmonic),
    /// A triad composed from the music font's quarter notes on a shared stem.
    Chord,
    /// Music-font noteheads joined by the font's tie contour.
    Tie,
    Slur,
    /// Bravura staff, optionally combined with a barline.
    Staff,
    Bar,
    /// Musical text as it appears in a score, such as `pizz.`: italic serif.
    Text(&'static str),
    /// Interface text, such as a voice number or an interval. Never italic.
    Label(&'static str),
    None,
}

/// What pressing a control does; the ribbon draws each kind differently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// One option among its neighbours: consecutive choice tiles form a segment.
    Choice,
    /// An on/off state; drawn with an outline while off.
    Toggle,
    /// Runs once; drawn without an outline.
    Action,
}

/// Every control of a size measures the same, which keeps groups aligned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlSize {
    /// Icon over label, full height.
    Large,
    /// One row of a three-row column: icon and label. The ribbon no longer
    /// uses it; every control there is a tile or a large button.
    Small,
    /// Glyph, full height, fixed width; its shortcut appears on hover.
    Tile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Control {
    pub command: CommandId,
    pub size: ControlSize,
    pub role: Role,
    pub visual: Visual,
    /// Keep only the font symbol in the ribbon, including expanded groups.
    pub symbol_only: bool,
    /// Short text for the toolbar; the registry's label is used when `None`.
    pub label: Option<&'static str>,
    /// One sentence on what the control does, for its tooltip.
    pub hint: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroupSpec {
    pub title: &'static str,
    pub visual: Visual,
    /// Keep these choices in a dropdown even when the ribbon has spare room.
    pub dropdown: bool,
    /// Narrow windows fold groups into a single button, lowest priority first.
    pub priority: u8,
    pub controls: &'static [Control],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TabSpec {
    pub id: TabId,
    pub groups: &'static [GroupSpec],
}

const fn large(
    command: CommandId,
    visual: Visual,
    label: &'static str,
    hint: &'static str,
) -> Control {
    Control {
        command,
        size: ControlSize::Large,
        role: Role::Action,
        visual,
        symbol_only: false,
        label: Some(label),
        hint,
    }
}
/// `label` names the tile when the group has room to show it.
const fn tile(
    command: CommandId,
    visual: Visual,
    label: &'static str,
    hint: &'static str,
) -> Control {
    Control {
        command,
        size: ControlSize::Tile,
        role: Role::Action,
        visual,
        symbol_only: false,
        label: Some(label),
        hint,
    }
}

const fn choice(control: Control) -> Control {
    Control {
        role: Role::Choice,
        ..control
    }
}
const fn toggle(control: Control) -> Control {
    Control {
        role: Role::Toggle,
        ..control
    }
}

use Visual::{Icon, Label, Music, Text};
use score_core::{
    Arpeggio as R, Articulation as A, ConnectionKind as K, Dynamic as D, GraceKind as G,
    Harmonic as H, MarkKind, PlayingTechnique as T, Tremolo as M,
};

/// A catalog mark: its command and symbol come from the catalog.
const fn mark_visual(kind: MarkKind) -> Visual {
    if let MarkKind::Harmonic(harmonic) = kind {
        return Visual::Harmonic(harmonic);
    }
    match kind.symbol() {
        Some(symbol) => Music(symbol),
        None => Text(kind.label()),
    }
}
const fn mark(kind: MarkKind, label: &'static str, hint: &'static str) -> Control {
    tile(crate::marks::command(kind), mark_visual(kind), label, hint)
}

/// Same sentence for every hairpin and slur: they are written in two clicks.
const TWO_CLICKS: &str = "Pulsa en la primera nota y otra vez en la última.";
const TOGGLE_SIGN: &str = "Pulsa otra vez para quitarlo.";

// Escribir

const ENTRY: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Entrada",
    visual: Music("\u{e1d7}"),
    priority: 9,
    controls: &[
        toggle(large(
            id::TOGGLE_NOTE_INPUT,
            Music("\u{e1d7}"),
            "Escribir",
            "Activa la entrada de notas. Con ella encendida, las letras A–G escriben notas.",
        )),
        tile(
            id::ENTRY_OCTAVE_UP,
            Label("↑"),
            "Octava",
            "Sube una octava las notas que escribas.",
        ),
        tile(
            id::ENTRY_OCTAVE_DOWN,
            Label("↓"),
            "Octava",
            "Baja una octava las notas que escribas.",
        ),
    ],
};
const DURATION: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Duración",
    visual: Music("\u{e1d5}"),
    priority: 10,
    controls: &[
        choice(tile(
            id::WHOLE,
            Music("\u{e1d2}"),
            "Redonda",
            "Cuatro tiempos en 4/4.",
        )),
        choice(tile(
            id::HALF,
            Music("\u{e1d3}"),
            "Blanca",
            "Dos tiempos en 4/4.",
        )),
        choice(tile(
            id::QUARTER,
            Music("\u{e1d5}"),
            "Negra",
            "Un tiempo en 4/4.",
        )),
        choice(tile(
            id::EIGHTH,
            Music("\u{e1d7}"),
            "Corchea",
            "Medio tiempo.",
        )),
        choice(tile(
            id::SIXTEENTH,
            Music("\u{e1d9}"),
            "Semicorchea",
            "Un cuarto de tiempo.",
        )),
        choice(tile(
            id::THIRTY_SECOND,
            Music("\u{e1db}"),
            "Fusa",
            "Un octavo de tiempo.",
        )),
    ],
};
const MODIFY: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Modificar",
    visual: Music("\u{e1e7}"),
    priority: 8,
    controls: &[
        toggle(tile(
            id::DOT,
            Music("\u{e1e7}"),
            "Puntillo",
            "Alarga la figura la mitad de su valor.",
        )),
        toggle(tile(
            id::TRIPLET,
            Music("\u{e883}"),
            "Tresillo",
            "Tres figuras en el tiempo de dos.",
        )),
        tile(
            id::REST,
            Music("\u{e4e5}"),
            "Silencio",
            "Escribe un silencio con la duración elegida.",
        ),
    ],
};
const ACCIDENTALS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Alteración",
    visual: Music("\u{e262}"),
    priority: 7,
    controls: &[
        choice(tile(
            id::NATURAL,
            Music("\u{e261}"),
            "Becuadro",
            TOGGLE_SIGN,
        )),
        choice(tile(id::SHARP, Music("\u{e262}"), "Sostenido", TOGGLE_SIGN)),
        choice(tile(id::FLAT, Music("\u{e260}"), "Bemol", TOGGLE_SIGN)),
        choice(tile(
            id::FOLLOW_KEY,
            Label("arm."),
            "Armadura",
            "La nota sigue la armadura: sin alteración propia.",
        )),
    ],
};
const TIES: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Ligadura",
    visual: Visual::Tie,
    priority: 6,
    controls: &[
        toggle(tile(
            id::TIE,
            Visual::Tie,
            "De unión",
            "Une la nota con la siguiente de la misma altura.",
        )),
        toggle(tile(id::SLUR, Visual::Slur, "De expresión", TWO_CLICKS)),
    ],
};
const VOICES: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Voz",
    visual: Label("1"),
    priority: 5,
    controls: &[
        choice(tile(
            id::VOICE_1,
            Label("1"),
            "Voz 1",
            "Plicas hacia arriba.",
        )),
        choice(tile(
            id::VOICE_2,
            Label("2"),
            "Voz 2",
            "Plicas hacia abajo, con su propio ritmo.",
        )),
    ],
};
const CHORDS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Acorde",
    visual: Visual::Chord,
    priority: 4,
    controls: &[toggle(tile(
        id::CHORD_MODE,
        Visual::Chord,
        "Modo acorde",
        "Las notas que escribas se apilan en la misma posición.",
    ))],
};
/// The intervals that stack a note above the selected one.
const INTERVALS: GroupSpec = GroupSpec {
    dropdown: true,
    title: "Intervalo",
    visual: Label("3ª"),
    priority: 3,
    controls: &[
        tile(
            id::INTERVAL_2,
            Label("2ª"),
            "Segunda",
            "Añade una segunda encima de la nota.",
        ),
        tile(
            id::INTERVAL_3,
            Label("3ª"),
            "Tercera",
            "Añade una tercera encima de la nota.",
        ),
        tile(
            id::INTERVAL_4,
            Label("4ª"),
            "Cuarta",
            "Añade una cuarta encima de la nota.",
        ),
        tile(
            id::INTERVAL_5,
            Label("5ª"),
            "Quinta",
            "Añade una quinta encima de la nota.",
        ),
        tile(
            id::INTERVAL_6,
            Label("6ª"),
            "Sexta",
            "Añade una sexta encima de la nota.",
        ),
        tile(
            id::INTERVAL_7,
            Label("7ª"),
            "Séptima",
            "Añade una séptima encima de la nota.",
        ),
        tile(
            id::INTERVAL_8,
            Label("8ª"),
            "Octava",
            "Añade una octava encima de la nota.",
        ),
    ],
};

// Articulaciones

const ARTICULATIONS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Articulación",
    visual: Music("\u{e4a0}"),
    priority: 6,
    controls: &[
        toggle(mark(
            MarkKind::Articulation(A::Staccato),
            "Staccato",
            "Nota corta y separada.",
        )),
        toggle(mark(
            MarkKind::Articulation(A::Tenuto),
            "Tenuto",
            "Nota sostenida en todo su valor.",
        )),
        toggle(mark(
            MarkKind::Articulation(A::Accent),
            "Acento",
            "Ataque más fuerte.",
        )),
        toggle(mark(
            MarkKind::Articulation(A::Marcato),
            "Marcato",
            "Acento fuerte y marcado.",
        )),
        toggle(mark(
            MarkKind::Fermata,
            "Calderón",
            "Mantiene la nota a criterio del intérprete.",
        )),
    ],
};
const GRACES: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Nota de adorno",
    visual: Music("\u{e560}"),
    priority: 5,
    controls: &[
        toggle(mark(
            MarkKind::Grace(G::Acciaccatura),
            "Acciaccatura",
            "Adorno rápido, tachado, antes de la nota.",
        )),
        toggle(mark(
            MarkKind::Grace(G::Appoggiatura),
            "Appoggiatura",
            "Adorno que toma tiempo de la nota principal.",
        )),
        tile(
            id::GRACE_NOTE,
            Music("\u{e562}"),
            "Añadir otra",
            "Añade otra nota al adorno de la nota seleccionada.",
        ),
    ],
};
const ARPEGGIOS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Arpegio",
    visual: Music("\u{e63c}"),
    priority: 3,
    controls: &[
        toggle(mark(
            MarkKind::Arpeggio(R::Normal),
            "Arpegio",
            "Toca el acorde de abajo arriba, sin flecha.",
        )),
        toggle(mark(
            MarkKind::Arpeggio(R::Up),
            "Hacia arriba",
            "Arpegio con flecha hacia arriba.",
        )),
        toggle(mark(
            MarkKind::Arpeggio(R::Down),
            "Hacia abajo",
            "Arpegio con flecha hacia abajo.",
        )),
        toggle(tile(
            id::ARPEGGIO_BOTH_HANDS,
            Music("\u{e63c}"),
            "Ambas manos",
            "Extiende el arpegio a los dos pentagramas del piano.",
        )),
    ],
};
const TREMOLOS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Trémolo",
    visual: Music("\u{e221}"),
    priority: 2,
    controls: &[
        toggle(mark(
            MarkKind::Tremolo(M::One),
            "Una barra",
            "Repite la nota en corcheas.",
        )),
        toggle(mark(
            MarkKind::Tremolo(M::Two),
            "Dos barras",
            "Repite la nota en semicorcheas.",
        )),
        toggle(mark(
            MarkKind::Tremolo(M::Three),
            "Tres barras",
            "Repite la nota en fusas.",
        )),
    ],
};
const TECHNIQUE: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Técnica de cuerda",
    visual: Text("pizz."),
    priority: 4,
    controls: &[
        toggle(mark(
            MarkKind::Technique(T::Pizzicato),
            "Pizzicato",
            "Pellizcando la cuerda.",
        )),
        toggle(mark(
            MarkKind::Technique(T::Arco),
            "Arco",
            "Vuelve a tocar con arco.",
        )),
        toggle(mark(
            MarkKind::Harmonic(H::Natural),
            "Armónico",
            "Armónico natural sobre la nota.",
        )),
        toggle(mark(
            MarkKind::Harmonic(H::Artificial),
            "Artificial",
            "Armónico artificial sobre la nota.",
        )),
    ],
};

// Dinámicas

const DYNAMICS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Dinámica",
    visual: Music("\u{e521}\u{e522}"),
    priority: 6,
    controls: &[
        mark(
            MarkKind::Dynamic(D::Ppp),
            "ppp",
            "Escribe ppp bajo la nota seleccionada.",
        ),
        mark(
            MarkKind::Dynamic(D::Pp),
            "pp",
            "Escribe pp bajo la nota seleccionada.",
        ),
        mark(
            MarkKind::Dynamic(D::P),
            "p",
            "Escribe p bajo la nota seleccionada.",
        ),
        mark(
            MarkKind::Dynamic(D::Mp),
            "mp",
            "Escribe mp bajo la nota seleccionada.",
        ),
        mark(
            MarkKind::Dynamic(D::Mf),
            "mf",
            "Escribe mf bajo la nota seleccionada.",
        ),
        mark(
            MarkKind::Dynamic(D::F),
            "f",
            "Escribe f bajo la nota seleccionada.",
        ),
        mark(
            MarkKind::Dynamic(D::Ff),
            "ff",
            "Escribe ff bajo la nota seleccionada.",
        ),
        mark(
            MarkKind::Dynamic(D::Fff),
            "fff",
            "Escribe fff bajo la nota seleccionada.",
        ),
    ],
};
const HAIRPINS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Regulador",
    visual: Music("\u{e53e}"),
    priority: 5,
    controls: &[
        toggle(mark(
            MarkKind::Connection(K::Crescendo),
            "Crescendo",
            TWO_CLICKS,
        )),
        toggle(mark(
            MarkKind::Connection(K::Diminuendo),
            "Diminuendo",
            TWO_CLICKS,
        )),
    ],
};
const LINES: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Líneas",
    visual: Music("\u{e511}"),
    priority: 4,
    controls: &[
        toggle(mark(
            MarkKind::Connection(K::OttavaAlta),
            "8va",
            "Suena una octava más aguda.",
        )),
        toggle(mark(
            MarkKind::Connection(K::OttavaBassa),
            "8vb",
            "Suena una octava más grave.",
        )),
        toggle(mark(
            MarkKind::Connection(K::Pedal),
            "Pedal",
            "Pedal de resonancia del piano.",
        )),
        toggle(mark(
            MarkKind::Connection(K::Trill),
            "Trino",
            "Alterna rápido con la nota superior.",
        )),
        toggle(mark(
            MarkKind::Connection(K::Glissando),
            "Glissando",
            "Desliza de una nota a la siguiente.",
        )),
    ],
};
/// One place for divisi: `div.` opens two staves from the selection, `unis.` closes them.
const DIVISI: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Divisi",
    visual: Text("div."),
    priority: 2,
    controls: &[
        toggle(tile(
            id::DIVISI,
            Text("div."),
            "Dividir",
            "La sección se divide en dos pentagramas.",
        )),
        toggle(tile(
            id::UNISON,
            Text("unis."),
            "Unir",
            "La sección vuelve a tocar al unísono.",
        )),
    ],
};

// Texto

const HEADER: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Partitura",
    visual: Label("Aa"),
    priority: 3,
    controls: &[large(
        id::SCORE_INFO,
        Label("Aa"),
        "Título y créditos",
        "Título, subtítulo, compositor, arreglista y demás créditos.",
    )],
};
/// The written tempo: the mark itself and quick ±5 changes to it.
const TEMPO: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Tempo",
    visual: Text("Allegro"),
    priority: 6,
    controls: &[
        tile(
            id::TEMPO_TEXT,
            Text("Allegro"),
            "Indicación",
            "Escribe un tempo y su metrónomo en el compás.",
        ),
        tile(
            id::TEMPO_UP,
            Icon("tempo-up"),
            "Más rápido",
            "Sube el metrónomo del tempo en el compás actual.",
        ),
        tile(
            id::TEMPO_DOWN,
            Icon("tempo-down"),
            "Más lento",
            "Baja el metrónomo del tempo en el compás actual.",
        ),
    ],
};
/// Words at the selected note: an expression below, an instruction above.
const STAFF_TEXT: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Sobre el pentagrama",
    visual: Text("dolce"),
    priority: 5,
    controls: &[
        toggle(tile(
            id::EXPRESSION_TEXT,
            Text("dolce"),
            "Expresión",
            "Texto de carácter bajo el pentagrama.",
        )),
        toggle(tile(
            id::TECHNIQUE_TEXT,
            Text("con sord."),
            "Técnica",
            "Instrucción de técnica sobre el pentagrama.",
        )),
    ],
};
/// Sung words under the notes and chord symbols above them.
const LYRICS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Letra y cifrado",
    visual: Text("la-la"),
    priority: 5,
    controls: &[
        toggle(tile(
            id::LYRICS,
            Text("la-la"),
            "Letra",
            "Escribe sílabas bajo las notas.",
        )),
        toggle(tile(
            id::CHORD_SYMBOL,
            Text("C7"),
            "Cifrado",
            "Escribe acordes sobre el pentagrama.",
        )),
    ],
};
const REHEARSAL: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Marca de ensayo",
    visual: Label("A"),
    priority: 4,
    controls: &[toggle(tile(
        id::REHEARSAL_MARK,
        Label("A"),
        "Añadir",
        "Añade la siguiente marca en el compás seleccionado, o la quita.",
    ))],
};
/// How every rehearsal mark of the score is numbered.
const REHEARSAL_STYLE: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Estilo de ensayo",
    visual: Label("A"),
    priority: 2,
    controls: &[
        choice(tile(
            id::REHEARSAL_LETTERS,
            Label("A"),
            "Letras",
            "Las marcas van por letras: A, B, C.",
        )),
        choice(tile(
            id::REHEARSAL_NUMBERS,
            Label("1"),
            "Números",
            "Las marcas van por números: 1, 2, 3.",
        )),
        choice(tile(
            id::REHEARSAL_BAR_NUMBERS,
            Label("12"),
            "Nº de compás",
            "Cada marca lleva el número del compás donde está.",
        )),
    ],
};

// Partitura

const INSTRUMENTS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Instrumentos",
    visual: Visual::Staff,
    priority: 6,
    controls: &[
        large(
            id::ADD_INSTRUMENT,
            Visual::Staff,
            "Añadir…",
            "Elige instrumentos de la orquesta, por familias.",
        ),
        tile(id::ADD_PIANO, Visual::Staff, "Piano", "Piano a dos manos."),
        tile(
            id::OSSIA,
            Visual::Staff,
            "Ossia",
            "Un pentagrama pequeño encima para una alternativa.",
        ),
    ],
};
const BARS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Compases",
    visual: Visual::Bar,
    priority: 6,
    controls: &[
        tile(
            id::BAR_AT_END,
            Visual::Bar,
            "Al final",
            "Añade un compás al final de la obra.",
        ),
        tile(
            id::SINGLE_BAR,
            Visual::Bar,
            "Antes",
            "Inserta un compás antes del seleccionado.",
        ),
        tile(
            id::DELETE_BAR,
            Visual::Bar,
            "Eliminar",
            "Elimina los compases seleccionados.",
        ),
    ],
};
/// What starts or changes the staff itself.
const CHANGES: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Cambios",
    visual: Music("\u{e050}"),
    priority: 7,
    controls: &[
        tile(
            id::CLEF,
            Visual::Clef(score_core::Clef::Treble),
            "Clave…",
            "Cambia la clave desde la nota seleccionada.",
        ),
        tile(
            id::KEY_SIGNATURE,
            Music("\u{e262}"),
            "Tonalidad…",
            "Cambia la armadura desde el compás seleccionado.",
        ),
        tile(
            id::TIME_SIGNATURE,
            Music("\u{e084}"),
            "Compás…",
            "Cambia el compás de tiempo.",
        ),
    ],
};
const REPEATS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Repeticiones",
    visual: Music("\u{e041}"),
    priority: 4,
    controls: &[
        toggle(tile(
            id::REPEAT_START,
            Music("\u{e040}"),
            "Inicio",
            "Barra de inicio de repetición.",
        )),
        toggle(tile(
            id::REPEAT_END,
            Music("\u{e041}"),
            "Fin",
            "Barra de fin de repetición.",
        )),
        toggle(tile(
            id::ENDING_1,
            Label("1."),
            "1.ª vez",
            "Casilla de primera vez.",
        )),
        toggle(tile(
            id::ENDING_2,
            Label("2."),
            "2.ª vez",
            "Casilla de segunda vez.",
        )),
        toggle(tile(
            id::DOUBLE_BARLINE,
            Music("\u{e031}"),
            "Doble barra",
            "Doble barra al final del compás.",
        )),
        toggle(tile(
            id::FINAL_BARLINE,
            Music("\u{e032}"),
            "Final",
            "Barra final.",
        )),
    ],
};
const SIGNS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Da capo y segno",
    visual: Music("\u{e047}"),
    priority: 3,
    controls: &[
        toggle(tile(
            id::SEGNO,
            Music("\u{e047}"),
            "Segno",
            "Punto al que vuelve un D.S.",
        )),
        toggle(tile(
            id::CODA,
            Music("\u{e048}"),
            "Coda",
            "Comienzo de la coda.",
        )),
        toggle(tile(
            id::FINE,
            Text("Fine"),
            "Fine",
            "Donde termina la obra tras volver.",
        )),
    ],
};
/// The jumps back, chosen from a dropdown next to their signs.
const JUMPS: GroupSpec = GroupSpec {
    dropdown: true,
    title: "D.C. / D.S.",
    visual: Text("D.C."),
    priority: 3,
    controls: &[
        toggle(tile(
            id::DA_CAPO,
            Text("D.C."),
            "D.C.",
            "Vuelve al principio.",
        )),
        toggle(tile(
            id::DA_CAPO_AL_FINE,
            Text("D.C."),
            "D.C. al Fine",
            "Vuelve al principio y termina en Fine.",
        )),
        toggle(tile(
            id::DA_CAPO_AL_CODA,
            Text("D.C."),
            "D.C. al Coda",
            "Vuelve al principio y salta a la coda.",
        )),
        toggle(tile(
            id::DAL_SEGNO,
            Text("D.S."),
            "D.S.",
            "Vuelve al segno.",
        )),
        toggle(tile(
            id::DAL_SEGNO_AL_FINE,
            Text("D.S."),
            "D.S. al Fine",
            "Vuelve al segno y termina en Fine.",
        )),
        toggle(tile(
            id::DAL_SEGNO_AL_CODA,
            Text("D.S."),
            "D.S. al Coda",
            "Vuelve al segno y salta a la coda.",
        )),
        toggle(tile(
            id::TO_CODA,
            Text("To Coda"),
            "To Coda",
            "Desde aquí, al volver, se salta a la coda.",
        )),
    ],
};
/// The user's own system and page ends, after the selected bar.
const BREAKS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Sistema y página",
    visual: Icon("continuous"),
    priority: 2,
    controls: &[
        toggle(tile(
            id::SYSTEM_BREAK,
            Icon("continuous"),
            "Salto de sistema",
            "El siguiente compás empieza un sistema nuevo.",
        )),
        toggle(tile(
            id::PAGE_BREAK,
            Icon("page"),
            "Salto de página",
            "El siguiente compás empieza una página nueva.",
        )),
    ],
};

// Reproducción

const TRANSPORT: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Reproducir",
    visual: Icon("play"),
    priority: 6,
    controls: &[
        large(
            id::PLAY,
            Icon("play"),
            "Reproducir",
            "Reproduce, pausa o continúa.",
        ),
        tile(
            id::PLAY_FROM_SELECTION,
            Icon("play"),
            "Desde la selección",
            "Empieza en la nota seleccionada.",
        ),
        tile(
            id::PLAY_FROM_START,
            Icon("first"),
            "Desde el inicio",
            "Empieza en el primer compás.",
        ),
        tile(id::STOP, Icon("stop"), "Detener", "Para la reproducción."),
    ],
};
const PRACTICE: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Práctica",
    visual: Icon("loop"),
    priority: 4,
    controls: &[
        toggle(tile(
            id::LOOP,
            Icon("loop"),
            "Bucle",
            "Repite el pasaje seleccionado.",
        )),
        toggle(tile(
            id::HALF_SPEED,
            Icon("half-speed"),
            "Media velocidad",
            "Reproduce a la mitad del tempo.",
        )),
        toggle(tile(
            id::REPEATS,
            Icon("repeats"),
            "Repeticiones",
            "Sigue las barras de repetición al reproducir.",
        )),
    ],
};
const SOUND: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Sonido",
    visual: Icon("speaker"),
    priority: 3,
    controls: &[
        toggle(tile(
            id::AUDITION,
            Icon("speaker"),
            "Oír al escribir",
            "Cada nota que escribes suena.",
        )),
        toggle(tile(
            id::CHORD_PLAYBACK,
            Text("C7"),
            "Cifrados",
            "Los cifrados suenan al reproducir.",
        )),
        toggle(tile(
            id::MIXER,
            Icon("mixer"),
            "Mezclador",
            "Volumen y posición de cada instrumento.",
        )),
        toggle(tile(
            id::SOUNDS,
            Icon("speaker"),
            "Sonidos",
            "Elige el banco de sonidos e instala más.",
        )),
    ],
};

// Revisión

const REVIEW_CHECKS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Comprobar",
    visual: Icon("search"),
    priority: 5,
    controls: &[
        large(
            id::CHECK_SCORE,
            Icon("search"),
            "Toda la partitura",
            "Busca notas fuera de registro y repeticiones o saltos incompletos.",
        ),
        tile(
            id::CHECK_SELECTION,
            Icon("search"),
            "Selección",
            "Comprueba solo lo seleccionado.",
        ),
    ],
};
const REVIEW_NOTICES: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Avisos",
    visual: Icon("right-panel"),
    priority: 4,
    controls: &[
        toggle(tile(
            id::REVIEW_ISSUES,
            Icon("right-panel"),
            "Mostrar",
            "Muestra los avisos en el panel de revisión.",
        )),
        tile(
            id::REVIEW_PREVIOUS,
            Label("‹"),
            "Anterior",
            "Va al aviso anterior.",
        ),
        tile(
            id::REVIEW_NEXT,
            Label("›"),
            "Siguiente",
            "Va al aviso siguiente.",
        ),
    ],
};
const REVIEW_RANGES: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Registro del instrumento",
    visual: Icon("single-system"),
    priority: 3,
    controls: &[
        toggle(tile(
            id::SHOW_RANGES,
            Icon("single-system"),
            "Mostrar",
            "Marca en rojo las notas fuera del registro del instrumento.",
        )),
        tile(
            id::EDIT_RANGE,
            Icon("mixer"),
            "Editar…",
            "Ajusta el registro del instrumento activo.",
        ),
        tile(
            id::RESET_RANGE,
            Icon("undo"),
            "Restablecer",
            "Vuelve al registro habitual del instrumento.",
        ),
    ],
};
const REVIEW_COMMENTS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Comentarios",
    visual: Icon("right-panel"),
    priority: 2,
    controls: &[
        tile(
            id::NEW_COMMENT,
            Label("+"),
            "Nuevo",
            "Comenta la selección.",
        ),
        toggle(tile(
            id::SHOW_COMMENTS,
            Icon("right-panel"),
            "Mostrar",
            "Muestra los comentarios en el panel.",
        )),
        tile(
            id::RESOLVE_COMMENT,
            Label("✓"),
            "Resolver",
            "Resuelve o reabre el comentario.",
        ),
    ],
};

// Vista

const VIEW_MODES: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Modo",
    visual: Icon("page"),
    priority: 5,
    controls: &[
        choice(tile(
            id::PAGE_VIEW,
            Icon("page"),
            "Página",
            "Páginas como se imprimirán.",
        )),
        choice(tile(
            id::CONTINUOUS_VIEW,
            Icon("continuous"),
            "Continua",
            "Sistemas al ancho de la ventana, sin páginas.",
        )),
        choice(tile(
            id::SINGLE_SYSTEM_VIEW,
            Icon("single-system"),
            "Un sistema",
            "Toda la obra en un solo sistema hacia la derecha.",
        )),
    ],
};
const ZOOM: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Zoom",
    visual: Icon("search"),
    priority: 4,
    controls: &[
        tile(id::ZOOM_OUT, Icon("zoom-out"), "Alejar", "Reduce el zoom."),
        tile(id::ZOOM_IN, Icon("zoom-in"), "Acercar", "Aumenta el zoom."),
        tile(
            id::ZOOM_ACTUAL,
            Icon("zoom-actual"),
            "Tamaño real",
            "Zoom al 100 %.",
        ),
        tile(
            id::ZOOM_FIT,
            Icon("fit-page"),
            "Ajustar",
            "La página entera en la ventana.",
        ),
    ],
};
const RIBBON: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Cinta",
    visual: Icon("full-screen"),
    priority: 2,
    controls: &[toggle(tile(
        id::TOGGLE_RIBBON,
        Icon("full-screen"),
        "Plegar",
        "Oculta la cinta; las pestañas la muestran al pulsarlas.",
    ))],
};

/// Levare's ribbon, one tab per task. File is not a tab: it is the
/// dropdown in `menu`. Undo and redo live in the title bar's quick access;
/// the clipboard lives in Edición and its keys, the panels in the header.
/// There are no remove buttons: select the mark's note in the score and
/// press its active control again, or delete the selection.
/// A tab without real functions is not shown: Parts returns when its
/// functions exist.
pub const TABS: &[TabSpec] = &[
    TabSpec {
        id: TabId::Write,
        groups: &[
            ENTRY,
            DURATION,
            MODIFY,
            ACCIDENTALS,
            TIES,
            VOICES,
            CHORDS,
            INTERVALS,
        ],
    },
    TabSpec {
        id: TabId::Articulations,
        groups: &[ARTICULATIONS, GRACES, ARPEGGIOS, TREMOLOS, TECHNIQUE],
    },
    TabSpec {
        id: TabId::Dynamics,
        groups: &[DYNAMICS, HAIRPINS, LINES, DIVISI],
    },
    TabSpec {
        id: TabId::Text,
        groups: &[
            HEADER,
            TEMPO,
            STAFF_TEXT,
            LYRICS,
            REHEARSAL,
            REHEARSAL_STYLE,
        ],
    },
    TabSpec {
        id: TabId::Score,
        groups: &[INSTRUMENTS, BARS, CHANGES, REPEATS, SIGNS, JUMPS, BREAKS],
    },
    TabSpec {
        id: TabId::Playback,
        groups: &[TRANSPORT, PRACTICE, SOUND],
    },
    TabSpec {
        id: TabId::Review,
        groups: &[
            REVIEW_CHECKS,
            REVIEW_NOTICES,
            REVIEW_RANGES,
            REVIEW_COMMENTS,
        ],
    },
    TabSpec {
        id: TabId::View,
        groups: &[VIEW_MODES, ZOOM, RIBBON],
    },
];

pub fn tab(id: TabId) -> &'static TabSpec {
    TABS.iter()
        .find(|tab| tab.id == id)
        .expect("every TabId has a spec")
}

#[cfg(test)]
mod tests {
    use super::*;
    use score_keymap::Registry;

    #[test]
    fn every_tab_has_its_own_command() {
        let registry = Registry::builtin();
        let mut seen = std::collections::HashSet::new();
        for tab in TABS {
            let command = tab.id.command();
            assert!(seen.insert(command), "{command:?} repeated");
            assert!(
                registry.get(command.as_str()).is_some(),
                "{command:?} unregistered"
            );
        }
    }

    #[test]
    fn notation_groups_never_fall_back_to_utility_icons() {
        for group in [
            INSTRUMENTS,
            BARS,
            CHANGES,
            DURATION,
            MODIFY,
            ACCIDENTALS,
            TIES,
            VOICES,
            CHORDS,
            INTERVALS,
            ARTICULATIONS,
            GRACES,
            ARPEGGIOS,
            DYNAMICS,
            HAIRPINS,
            LINES,
            TECHNIQUE,
            TREMOLOS,
            DIVISI,
            REPEATS,
            SIGNS,
            JUMPS,
        ] {
            for visual in
                std::iter::once(group.visual).chain(group.controls.iter().map(|c| c.visual))
            {
                assert!(
                    matches!(
                        visual,
                        Visual::Music(_)
                            | Visual::Clef(_)
                            | Visual::Harmonic(_)
                            | Visual::Chord
                            | Visual::Tie
                            | Visual::Slur
                            | Visual::Staff
                            | Visual::Bar
                            | Visual::Text(_)
                            | Visual::Label(_)
                    ),
                    "{}: {visual:?}",
                    group.title
                );
            }
        }
        assert!(matches!(ENTRY.controls[0].visual, Visual::Music(_)));
    }

    #[test]
    fn choices_are_tiles_and_never_sit_alone() {
        // A segment of one would look like a stuck button.
        for group in TABS.iter().flat_map(|tab| tab.groups) {
            let tiles: Vec<_> = group
                .controls
                .iter()
                .filter(|c| c.role == Role::Choice && c.size == ControlSize::Tile)
                .collect();
            assert!(tiles.is_empty() || tiles.len() >= 2, "{}", group.title);
        }
    }

    #[test]
    fn every_control_runs_a_registered_command_once_per_tab() {
        let registry = Registry::builtin();
        for tab in TABS {
            let mut seen = Vec::new();
            for group in tab.groups {
                assert!(!group.controls.is_empty(), "{}", group.title);
                for control in group.controls {
                    assert!(
                        registry.get(control.command.as_str()).is_some(),
                        "{} → {}",
                        group.title,
                        control.command
                    );
                    assert!(!seen.contains(&control.command), "{}", control.command);
                    seen.push(control.command);
                }
            }
        }
    }

    #[test]
    fn the_ribbon_has_no_remove_buttons() {
        let registry = Registry::builtin();
        for group in TABS.iter().flat_map(|tab| tab.groups) {
            for control in group.controls {
                let label = registry.get(control.command.as_str()).unwrap().label;
                assert!(
                    !label.starts_with("Quitar") && control.command != id::DELETE,
                    "{}: {label}",
                    group.title
                );
            }
        }
    }

    #[test]
    fn the_view_tab_switches_every_view_mode_and_zoom() {
        // The score has no floating view controls: this tab carries them all.
        let commands: Vec<_> = tab(TabId::View)
            .groups
            .iter()
            .flat_map(|group| group.controls)
            .map(|control| control.command)
            .collect();
        for command in [
            id::PAGE_VIEW,
            id::CONTINUOUS_VIEW,
            id::SINGLE_SYSTEM_VIEW,
            id::ZOOM_IN,
            id::ZOOM_OUT,
            id::ZOOM_ACTUAL,
            id::ZOOM_FIT,
        ] {
            assert!(commands.contains(&command), "{command}");
        }
    }

    #[test]
    fn every_group_has_a_ready_command() {
        // A group of pending commands only promises; it stays out of the ribbon.
        let registry = Registry::builtin();
        for group in TABS.iter().flat_map(|tab| tab.groups) {
            assert!(
                group.controls.iter().any(|control| {
                    registry.get(control.command.as_str()).unwrap().status
                        == score_keymap::Status::Ready
                }),
                "{}",
                group.title
            );
        }
    }

    #[test]
    fn each_command_appears_once_in_the_whole_ribbon() {
        let mut seen = std::collections::HashSet::new();
        for group in TABS.iter().flat_map(|tab| tab.groups) {
            for control in group.controls {
                assert!(
                    seen.insert(control.command),
                    "{} → {}",
                    group.title,
                    control.command
                );
            }
        }
    }

    #[test]
    fn divisi_has_a_single_home_and_basic_entry_tools_are_present() {
        let commands: Vec<_> = TABS
            .iter()
            .flat_map(|tab| tab.groups)
            .flat_map(|group| group.controls)
            .map(|control| control.command)
            .collect();
        for absent in [
            id::DIVIDE_PART,
            id::JOIN_PART,
            id::STEP_TIME,
            id::FLEXI_TIME,
            id::MIDI_INPUT,
        ] {
            assert!(!commands.contains(&absent), "{absent}");
        }
        for present in [
            id::REST,
            id::FOLLOW_KEY,
            id::ENTRY_OCTAVE_UP,
            id::ENTRY_OCTAVE_DOWN,
            id::SCORE_INFO,
            id::SOUNDS,
            id::DA_CAPO,
            id::DA_CAPO_AL_CODA,
            id::DAL_SEGNO,
            id::DAL_SEGNO_AL_FINE,
        ] {
            assert!(commands.contains(&present), "{present}");
        }
    }

    #[test]
    fn every_tab_has_a_spec() {
        for id in [
            TabId::Write,
            TabId::Articulations,
            TabId::Dynamics,
            TabId::Text,
            TabId::Score,
            TabId::Playback,
            TabId::Review,
            TabId::View,
        ] {
            assert_eq!(tab(id).id, id);
            assert!(!tab(id).groups.is_empty(), "{id:?}");
        }
        assert_eq!(TABS.len(), 8);
    }
    #[test]
    fn every_form_command_is_ready_for_menu_and_custom_shortcuts() {
        let registry = Registry::builtin();
        for command in [
            id::REPEAT_START,
            id::REPEAT_END,
            id::ENDING_1,
            id::ENDING_2,
            id::REMOVE_ENDING,
            id::DOUBLE_BARLINE,
            id::FINAL_BARLINE,
            id::SEGNO,
            id::CODA,
            id::TO_CODA,
            id::FINE,
            id::DA_CAPO,
            id::DA_CAPO_AL_FINE,
            id::DA_CAPO_AL_CODA,
            id::DAL_SEGNO,
            id::DAL_SEGNO_AL_FINE,
            id::DAL_SEGNO_AL_CODA,
            id::CLEAR_FORM,
        ] {
            let spec = registry.get(command.as_str()).unwrap();
            assert_eq!(spec.status, score_keymap::Status::Ready, "{command}");
            assert_eq!(spec.category, score_keymap::Category::Bars);
            assert_eq!(spec.scope, score_keymap::Scope::Global);
            assert!(!spec.repeat);
        }
    }
}

#[cfg(test)]
mod task_tests {
    use super::*;
    use crate::menu::{BAR_MARK_CONTEXT_MENU, FILE_MENU, MARK_CONTEXT_MENU, NOTE_CONTEXT_MENU};
    use score_keymap::{KeySequence, Profile, Registry};

    const ORDER: [(TabId, &str); 8] = [
        (TabId::Write, "Escribir"),
        (TabId::Articulations, "Articulaciones"),
        (TabId::Dynamics, "Dinámicas"),
        (TabId::Text, "Texto"),
        (TabId::Score, "Partitura"),
        (TabId::Playback, "Reproducción"),
        (TabId::Review, "Revisión"),
        (TabId::View, "Vista"),
    ];

    /// Every command the ribbon had before it was organised by task.
    const RIBBON_BEFORE: &[CommandId] = &[
        id::PASTE,
        id::COPY,
        id::CUT,
        id::REPEAT,
        id::SELECT_ALL,
        id::ADD_INSTRUMENT,
        id::ADD_PIANO,
        id::OSSIA,
        id::BAR_AT_END,
        id::SINGLE_BAR,
        id::DELETE_BAR,
        id::TOGGLE_NOTE_INPUT,
        id::ENTRY_OCTAVE_UP,
        id::ENTRY_OCTAVE_DOWN,
        id::WHOLE,
        id::HALF,
        id::QUARTER,
        id::EIGHTH,
        id::SIXTEENTH,
        id::THIRTY_SECOND,
        id::DOT,
        id::TRIPLET,
        id::REST,
        id::NATURAL,
        id::SHARP,
        id::FLAT,
        id::FOLLOW_KEY,
        id::STACCATO,
        id::TENUTO,
        id::ACCENT,
        id::MARCATO,
        id::FERMATA,
        id::TIE,
        id::SLUR,
        id::ACCIACCATURA,
        id::APPOGGIATURA,
        id::GRACE_NOTE,
        id::VOICE_1,
        id::VOICE_2,
        id::CHORD_MODE,
        id::INTERVAL_2,
        id::INTERVAL_3,
        id::INTERVAL_4,
        id::INTERVAL_5,
        id::INTERVAL_6,
        id::INTERVAL_7,
        id::INTERVAL_8,
        id::CLEF,
        id::KEY_SIGNATURE,
        id::TIME_SIGNATURE,
        id::DYNAMIC_PP,
        id::DYNAMIC_P,
        id::DYNAMIC_MP,
        id::DYNAMIC_MF,
        id::DYNAMIC_F,
        id::DYNAMIC_FF,
        id::DYNAMIC_PPP,
        id::DYNAMIC_FFF,
        id::CRESCENDO,
        id::DIMINUENDO,
        id::OTTAVA_ALTA,
        id::OTTAVA_BASSA,
        id::PEDAL,
        id::TRILL,
        id::GLISSANDO,
        id::ARPEGGIO,
        id::ARPEGGIO_UP,
        id::ARPEGGIO_DOWN,
        id::ARPEGGIO_BOTH_HANDS,
        id::PIZZICATO,
        id::ARCO,
        id::HARMONIC_NATURAL,
        id::HARMONIC_ARTIFICIAL,
        id::TREMOLO_1,
        id::TREMOLO_2,
        id::TREMOLO_3,
        id::DIVISI,
        id::UNISON,
        id::REPEAT_START,
        id::REPEAT_END,
        id::ENDING_1,
        id::ENDING_2,
        id::DOUBLE_BARLINE,
        id::FINAL_BARLINE,
        id::SEGNO,
        id::CODA,
        id::FINE,
        id::TO_CODA,
        id::DA_CAPO,
        id::DA_CAPO_AL_FINE,
        id::DA_CAPO_AL_CODA,
        id::DAL_SEGNO,
        id::DAL_SEGNO_AL_FINE,
        id::DAL_SEGNO_AL_CODA,
        id::SCORE_INFO,
        id::TEMPO_TEXT,
        id::TEMPO_UP,
        id::TEMPO_DOWN,
        id::EXPRESSION_TEXT,
        id::TECHNIQUE_TEXT,
        id::LYRICS,
        id::CHORD_SYMBOL,
        id::REHEARSAL_MARK,
        id::REHEARSAL_LETTERS,
        id::REHEARSAL_NUMBERS,
        id::REHEARSAL_BAR_NUMBERS,
        id::PLAY,
        id::PLAY_FROM_SELECTION,
        id::PLAY_FROM_START,
        id::STOP,
        id::AUDITION,
        id::SOUNDS,
        id::MIXER,
        id::LOOP,
        id::REPEATS,
        id::HALF_SPEED,
        id::CHORD_PLAYBACK,
        id::SYSTEM_BREAK,
        id::PAGE_BREAK,
        id::CHECK_SCORE,
        id::CHECK_SELECTION,
        id::REVIEW_ISSUES,
        id::REVIEW_PREVIOUS,
        id::REVIEW_NEXT,
        id::SHOW_RANGES,
        id::EDIT_RANGE,
        id::RESET_RANGE,
        id::NEW_COMMENT,
        id::SHOW_COMMENTS,
        id::RESOLVE_COMMENT,
        id::PAGE_VIEW,
        id::CONTINUOUS_VIEW,
        id::SINGLE_SYSTEM_VIEW,
        id::ZOOM_IN,
        id::ZOOM_OUT,
        id::ZOOM_ACTUAL,
        id::ZOOM_FIT,
        id::TOGGLE_INSTRUMENTS_PANEL,
        id::TOGGLE_INSPECTOR,
        id::FULL_SCREEN,
        id::TOGGLE_RIBBON,
    ];

    /// Left the ribbon for the app's native Edición and Vista menus
    /// (`app-gpui/src/shortcuts.rs`, which checks they are still there).
    const NATIVE_MENUS: &[CommandId] = &[
        id::SELECT_ALL,
        id::TOGGLE_INSTRUMENTS_PANEL,
        id::TOGGLE_INSPECTOR,
        id::FULL_SCREEN,
    ];

    fn ribbon() -> impl Iterator<Item = &'static Control> {
        TABS.iter()
            .flat_map(|tab| tab.groups)
            .flat_map(|group| group.controls)
    }

    #[test]
    fn tabs_follow_the_task_order() {
        let tabs: Vec<_> = TABS.iter().map(|tab| (tab.id, tab.id.label())).collect();
        assert_eq!(tabs, ORDER);
    }

    #[test]
    fn every_ribbon_control_is_a_named_tile_with_a_hint() {
        for group in TABS.iter().flat_map(|tab| tab.groups) {
            for control in group.controls {
                let what = format!("{} → {}", group.title, control.command);
                assert_ne!(control.size, ControlSize::Small, "{what}");
                assert!(!control.symbol_only, "{what}");
                assert!(
                    control.label.is_some_and(|label| !label.is_empty()),
                    "{what}"
                );
                assert!(!control.hint.is_empty(), "{what}");
                assert!(control.hint.ends_with('.'), "{what}: {}", control.hint);
            }
        }
    }

    #[test]
    fn nothing_leaves_the_app() {
        let registry = Registry::builtin();
        let menus = [
            FILE_MENU,
            NOTE_CONTEXT_MENU,
            MARK_CONTEXT_MENU,
            BAR_MARK_CONTEXT_MENU,
        ];
        for command in RIBBON_BEFORE {
            assert!(registry.get(command.as_str()).is_some(), "{command}");
            let in_ribbon = ribbon().any(|control| control.command == *command);
            let in_menu = menus
                .iter()
                .any(|menu| menu.contains(&crate::MenuEntry::Command(*command)));
            assert!(
                in_ribbon || in_menu || NATIVE_MENUS.contains(command),
                "{command} left the app"
            );
        }
    }

    #[test]
    fn old_tab_shortcuts_open_a_tab() {
        let registry = Registry::builtin();
        for command in [
            id::TAB_HOME,
            id::TAB_LAYOUT,
            id::TAB_NOTATION,
            id::TAB_DYNAMICS,
        ] {
            assert!(registry.get(command.as_str()).is_some(), "{command}");
        }
        let profile = Profile::sibelius_laptop();
        for (index, (tab, _)) in ORDER.iter().enumerate() {
            let keys = KeySequence::parse(&format!("alt-cmd-{}", index + 1)).unwrap();
            assert_eq!(profile.keys_for(tab.command()), vec![keys], "{tab:?}");
        }
        assert!(profile.keys_for(id::TAB_LAYOUT).is_empty());
    }
}

#[cfg(test)]
mod rehearsal_tests {
    use super::*;
    #[test]
    fn rehearsal_commands_are_ready_and_available_in_text_ribbon() {
        let registry = score_keymap::Registry::builtin();
        for command in [
            id::REHEARSAL_MARK,
            id::REHEARSAL_LETTERS,
            id::REHEARSAL_NUMBERS,
            id::REHEARSAL_BAR_NUMBERS,
        ] {
            assert_eq!(
                registry.get(command.as_str()).unwrap().status,
                score_keymap::Status::Ready
            );
            assert!(
                tab(TabId::Text)
                    .groups
                    .iter()
                    .flat_map(|group| group.controls)
                    .any(|control| control.command == command)
            );
        }
    }
}
