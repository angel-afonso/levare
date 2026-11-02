//! What the ribbon contains: tabs, groups and the commands of each control.
//! Pure data. Keys, state and the pending mark come from elsewhere.

use score_keymap::{CommandId, id};

/// The tabs of Sibelius that Levare has functions for, in Sibelius' order,
/// which ⌥⌘1–8 (Ctrl+Mayús+1–8 on Windows and Linux) follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TabId {
    Home,
    NoteInput,
    Notations,
    Text,
    Playback,
    Layout,
    Review,
    View,
}

impl TabId {
    pub fn label(self) -> &'static str {
        match self {
            Self::Home => "Inicio",
            Self::NoteInput => "Entrada de notas",
            Self::Notations => "Notaciones",
            Self::Text => "Texto",
            Self::Playback => "Reproducción",
            Self::Layout => "Diseño",
            Self::Review => "Revisión",
            Self::View => "Vista",
        }
    }

    /// The command that shows this tab. `TAB_DYNAMICS`, published when
    /// Dinámicas was a tab, opens Texto, where Sibelius writes dynamics.
    pub fn command(self) -> CommandId {
        match self {
            Self::Home => id::TAB_HOME,
            Self::NoteInput => id::TAB_NOTE_INPUT,
            Self::Notations => id::TAB_NOTATION,
            Self::Text => id::TAB_TEXT,
            Self::Playback => id::TAB_PLAYBACK,
            Self::Layout => id::TAB_LAYOUT,
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
    /// Eighth notes drawn with Bravura heads: `flagged` ones with their own
    /// flags, then `beamed` ones under one beam.
    Beam {
        flagged: u8,
        beamed: u8,
    },
    /// Bravura staff, optionally combined with a barline.
    Staff,
    Bar,
    /// Musical text as it appears in a score, such as `pizz.`: italic serif.
    Text(&'static str),
    /// Interface text, such as a voice number or an interval. Never italic.
    Label(&'static str),
    None,
}

/// What pressing a control does. Every role is drawn alike: bare at rest,
/// lit while on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// One option among its neighbours: consecutive choice tiles share a track.
    Choice,
    /// An on/off state.
    Toggle,
    /// Runs once.
    Action,
}

/// Every control is a tile: glyph over its name and key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlSize {
    /// A group's main control: a wider tile while it shows its name.
    Large,
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
    Harmonic as H, MarkKind, Ornament as O, Pause as P, PlayingTechnique as T, Tremolo as M,
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

/// Same sentence for both hairpins: they are written in two clicks.
const HAIRPIN_HINT: &str =
    "Selecciona un pasaje, o marca inicio y final con el mismo botón. Esc cancela.";
const VOICE_HINT: &str = "Hasta cuatro voces: plicas arriba para las voces 1 y 3, abajo para la 2 y la 4. Cada una conserva su propio ritmo.";
const DIVISI_HINT: &str = "div. abre dos pentagramas desde la posición elegida; unis. vuelve a uno. También dentro de un compás.";
const HARMONIC_HINT: &str = "Armónicos: natural = altura sonora; artificial = nota pisada + rombo automático a la cuarta. Vuelve a pulsar para quitarlo.";

// Entrada de notas

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
            "Activa la entrada de notas. Con ella encendida, las letras A–G escriben notas. Esc sale de la entrada para corregir notas existentes.",
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
    priority: 7,
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
            "Tres figuras en el tiempo de dos. Sobre una nota empieza un tresillo y rellena el grupo con silencios; otra vez, convierte el grupo en silencios.",
        )),
        tile(
            id::REST,
            Music("\u{e4e5}"),
            "Silencio",
            "Escribe un silencio de la duración elegida.",
        ),
    ],
};
/// The rarer values, from one dropdown next to Modificar, as Sibelius keeps
/// them on its second keypad layout. Each tuplet acts like Tresillo.
const TUPLET_HINT: &str = "Sobre una nota empieza el grupillo y rellena el grupo con silencios; otra vez, convierte el grupo en silencios.";
const MORE_VALUES: GroupSpec = GroupSpec {
    dropdown: true,
    title: "Más figuras",
    visual: Music("\u{e885}"),
    priority: 6,
    controls: &[
        toggle(tile(
            id::SIXTY_FOURTH,
            Music("\u{e1dd}"),
            "Semifusa",
            "Un dieciseisavo de tiempo.",
        )),
        toggle(tile(
            id::DOUBLE_DOT,
            Music("\u{e1e7}\u{e1e7}"),
            "Doble puntillo",
            "Alarga la figura tres cuartos de su valor.",
        )),
        toggle(tile(id::DUPLET, Music("\u{e882}"), "Dosillo", TUPLET_HINT)),
        toggle(tile(
            id::QUADRUPLET,
            Music("\u{e884}"),
            "Cuatrillo",
            TUPLET_HINT,
        )),
        toggle(tile(
            id::QUINTUPLET,
            Music("\u{e885}"),
            "Quintillo",
            TUPLET_HINT,
        )),
        toggle(tile(
            id::SEXTUPLET,
            Music("\u{e886}"),
            "Seisillo",
            TUPLET_HINT,
        )),
        toggle(tile(
            id::SEPTUPLET,
            Music("\u{e887}"),
            "Septillo",
            TUPLET_HINT,
        )),
        toggle(tile(
            id::OCTUPLET,
            Music("\u{e888}"),
            "Octillo",
            TUPLET_HINT,
        )),
        toggle(tile(
            id::NONUPLET,
            Music("\u{e889}"),
            "Nonillo",
            TUPLET_HINT,
        )),
        toggle(tile(
            id::OTHER_TUPLET,
            Label("n:m"),
            "Otro grupillo",
            "Elige cuántas notas y en el tiempo de cuántas, en la pestaña Selección.",
        )),
    ],
};
const ACCIDENTALS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Alteración",
    visual: Music("\u{e262}"),
    priority: 8,
    controls: &[
        choice(tile(
            id::NATURAL,
            Music("\u{e261}"),
            "Becuadro",
            "Anula la alteración de la armadura. Pulsa otra vez para quitarlo.",
        )),
        choice(tile(
            id::SHARP,
            Music("\u{e262}"),
            "Sostenido",
            "Sube la nota un semitono. Pulsa otra vez para quitarlo.",
        )),
        choice(tile(
            id::FLAT,
            Music("\u{e260}"),
            "Bemol",
            "Baja la nota un semitono. Pulsa otra vez para quitarlo.",
        )),
        choice(tile(
            id::DOUBLE_SHARP,
            Music("\u{e263}"),
            "Doble ♯",
            "Sube la nota dos semitonos. Pulsa otra vez para quitarlo.",
        )),
        choice(tile(
            id::DOUBLE_FLAT,
            Music("\u{e264}"),
            "Doble ♭",
            "Baja la nota dos semitonos. Pulsa otra vez para quitarlo.",
        )),
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
    priority: 5,
    controls: &[
        toggle(tile(
            id::TIE,
            Visual::Tie,
            "Unión",
            "Une la nota con la siguiente de la misma altura: notas o acordes iguales y consecutivos.",
        )),
        toggle(tile(
            id::SLUR,
            Visual::Slur,
            "Expresión",
            "Elige inicio y final: pulsa en la primera nota y otra vez en la última. Esc cancela.",
        )),
    ],
};
const VOICES: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Voz",
    visual: Label("1"),
    priority: 6,
    controls: &[
        choice(tile(id::VOICE_1, Label("1"), "Voz 1", VOICE_HINT)),
        choice(tile(id::VOICE_2, Label("2"), "Voz 2", VOICE_HINT)),
        choice(tile(id::VOICE_3, Label("3"), "Voz 3", VOICE_HINT)),
        choice(tile(id::VOICE_4, Label("4"), "Voz 4", VOICE_HINT)),
    ],
};
/// Both ways of building a chord: stacking the notes you write, or adding an
/// interval above the selected one. The split button lights up while the chord
/// mode is on.
const CHORDS: GroupSpec = GroupSpec {
    dropdown: true,
    title: "Acorde",
    visual: Visual::Chord,
    priority: 3,
    controls: &[
        toggle(tile(
            id::CHORD_MODE,
            Visual::Chord,
            "Acorde",
            "Las notas que escribas se apilan en la misma posición.",
        )),
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

// Notaciones

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
            MarkKind::Articulation(A::Staccatissimo),
            "Staccatissimo",
            "Más corta que el staccato.",
        )),
        toggle(mark(
            MarkKind::Fermata,
            "Calderón",
            "Mantiene la nota a criterio del intérprete.",
        )),
    ],
};
/// Ornaments and pauses, from one dropdown as Sibelius keeps them in its
/// Symbols gallery.
const SYMBOLS: GroupSpec = GroupSpec {
    dropdown: true,
    title: "Símbolos",
    visual: Music("\u{e56c}"),
    priority: 3,
    controls: &[
        toggle(mark(
            MarkKind::Ornament(O::Mordent),
            "Mordente",
            "La nota, la de abajo y otra vez la nota, rápido.",
        )),
        toggle(mark(
            MarkKind::Ornament(O::InvertedMordent),
            "Mordente superior",
            "La nota, la de arriba y otra vez la nota, rápido.",
        )),
        toggle(mark(
            MarkKind::Ornament(O::Turn),
            "Grupeto",
            "La nota de arriba, la nota, la de abajo y la nota.",
        )),
        toggle(mark(
            MarkKind::Pause(P::Breath),
            "Respiración",
            "Corta un poco la nota antes de la siguiente.",
        )),
        toggle(mark(
            MarkKind::Pause(P::Caesura),
            "Cesura",
            "Una pausa clara después de la nota.",
        )),
    ],
};
const GRACES: GroupSpec = GroupSpec {
    dropdown: true,
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
            // Two appoggiaturas: one more grace note.
            Music("\u{e562}\u{e562}"),
            "Añadir otra",
            "Añade otra nota al adorno de la nota seleccionada.",
        ),
    ],
};
const ARPEGGIOS: GroupSpec = GroupSpec {
    dropdown: true,
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
            "Arriba",
            "Arpegio con flecha hacia arriba.",
        )),
        toggle(mark(
            MarkKind::Arpeggio(R::Down),
            "Abajo",
            "Arpegio con flecha hacia abajo.",
        )),
        toggle(tile(
            id::ARPEGGIO_BOTH_HANDS,
            // The piano's brace: across both staves.
            Music("\u{e000}"),
            "Ambas manos",
            "Extiende el arpegio a los dos pentagramas del piano.",
        )),
    ],
};
const TREMOLOS: GroupSpec = GroupSpec {
    dropdown: true,
    title: "Trémolo",
    visual: Music("\u{e221}"),
    priority: 2,
    controls: &[
        toggle(mark(
            MarkKind::Tremolo(M::One),
            "1 barra",
            "Repite la nota en corcheas.",
        )),
        toggle(mark(
            MarkKind::Tremolo(M::Two),
            "2 barras",
            "Repite la nota en semicorcheas.",
        )),
        toggle(mark(
            MarkKind::Tremolo(M::Three),
            "3 barras",
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
            "Pellizcando la cuerda. Se mantiene hasta la siguiente indicación.",
        )),
        toggle(mark(
            MarkKind::Technique(T::Arco),
            "Arco",
            "Vuelve a tocar con arco. Se mantiene hasta la siguiente indicación.",
        )),
        toggle(mark(
            MarkKind::Harmonic(H::Natural),
            "Armónico",
            HARMONIC_HINT,
        )),
        toggle(mark(
            MarkKind::Harmonic(H::Artificial),
            "Artificial",
            HARMONIC_HINT,
        )),
    ],
};

// Dynamics go to Texto, hairpins and lines to Notaciones, divisi to Inicio

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
/// Accents and sudden changes, from one dropdown beside the levels.
const MORE_DYNAMICS: GroupSpec = GroupSpec {
    dropdown: true,
    title: "Más dinámicas",
    visual: Music("\u{e524}\u{e522}\u{e525}"),
    priority: 3,
    controls: &[
        mark(
            MarkKind::Dynamic(D::Sfz),
            "sfz",
            "Acento fuerte en la nota; luego sigue el nivel de antes.",
        ),
        mark(
            MarkKind::Dynamic(D::Rfz),
            "rfz",
            "Refuerza la nota; luego sigue el nivel de antes.",
        ),
        mark(
            MarkKind::Dynamic(D::Fp),
            "fp",
            "Ataque forte y enseguida piano.",
        ),
        mark(
            MarkKind::Dynamic(D::Sfp),
            "sfp",
            "Sforzando y enseguida piano.",
        ),
        mark(
            MarkKind::Dynamic(D::Niente),
            "niente",
            "Desde o hasta el silencio, al principio o al final de un regulador.",
        ),
    ],
};
/// Hairpins and lines, from one dropdown as Sibelius' Lines gallery.
const LINES: GroupSpec = GroupSpec {
    dropdown: true,
    title: "Líneas",
    visual: Music("\u{e53e}"),
    priority: 4,
    controls: &[
        toggle(mark(
            MarkKind::Connection(K::Crescendo),
            "Crescendo",
            HAIRPIN_HINT,
        )),
        toggle(mark(
            MarkKind::Connection(K::Diminuendo),
            "Diminuendo",
            HAIRPIN_HINT,
        )),
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
        toggle(tile(id::DIVISI, Text("div."), "Dividir", DIVISI_HINT)),
        toggle(tile(id::UNISON, Text("unis."), "Unir", DIVISI_HINT)),
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
        "Título…",
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
    visual: Icon("rehearsal-mark"),
    priority: 4,
    controls: &[toggle(tile(
        id::REHEARSAL_MARK,
        Icon("rehearsal-mark"),
        "Añadir",
        "Añade la siguiente marca en el compás seleccionado, o la quita.",
    ))],
};
/// How every rehearsal mark of the score is numbered.
const REHEARSAL_STYLE: GroupSpec = GroupSpec {
    dropdown: true,
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

// Inicio

/// Sibelius opens Inicio with the clipboard; Repetir sits beside it as the
/// one-key copy and paste.
const CLIPBOARD: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Portapapeles",
    visual: Icon("clipboard"),
    priority: 5,
    controls: &[
        large(
            id::PASTE,
            Icon("clipboard"),
            "Pegar",
            "Pega lo copiado a partir de la selección.",
        ),
        tile(id::COPY, Icon("copy"), "Copiar", "Copia la selección."),
        tile(
            id::CUT,
            Icon("cut"),
            "Cortar",
            "Copia la selección y la quita de la partitura.",
        ),
        tile(
            id::REPEAT,
            Icon("repeat"),
            "Repetir",
            "Repite justo después la nota, el acorde o el silencio seleccionado.",
        ),
    ],
};
const SELECT: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Seleccionar",
    visual: Visual::Staff,
    priority: 3,
    controls: &[
        tile(
            id::SELECT_ALL,
            Visual::Staff,
            "Todo",
            "Selecciona el pentagrama entero.",
        ),
        tile(
            id::GO_TO_BAR,
            Label("#"),
            "Ir al compás…",
            "Lleva la selección al compás que indiques.",
        ),
    ],
};

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
        tile(id::ADD_PIANO, Icon("piano"), "Piano", "Piano a dos manos."),
        tile(
            id::OSSIA,
            Icon("ossia"),
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
            Icon("bar-at-end"),
            "Al final",
            "Añade un compás al final de la obra.",
        ),
        tile(
            id::SINGLE_BAR,
            Icon("bar-before"),
            "Antes",
            "Inserta un compás antes del seleccionado.",
        ),
        tile(
            id::DELETE_BAR,
            Icon("delete-bar"),
            "Eliminar",
            "Elimina los compases seleccionados.",
        ),
    ],
};

// Clefs and barlines go to Notaciones, Fine and the jumps to Texto

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
/// Repeats, endings and barlines from one dropdown, like Sibelius' barline
/// gallery.
/// Sibelius' barline gallery: the repeat barlines, the endings and the line
/// that closes the selected bar.
const REPEATS: GroupSpec = GroupSpec {
    dropdown: true,
    title: "Barra",
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
        toggle(tile(
            id::DASHED_BARLINE,
            Music("\u{e036}"),
            "Discontinua",
            "Barra discontinua al final del compás: una división secundaria.",
        )),
        toggle(tile(
            id::DOTTED_BARLINE,
            Music("\u{e037}"),
            "Punteada",
            "Barra punteada al final del compás: una división secundaria.",
        )),
        toggle(tile(
            id::TICK_BARLINE,
            Music("\u{e039}"),
            "Marca",
            "Solo un trazo sobre la línea superior, como en la música antigua.",
        )),
        toggle(tile(
            id::SHORT_BARLINE,
            Music("\u{e038}"),
            "Corta",
            "Un trazo en el centro del pentagrama.",
        )),
        toggle(tile(
            id::INVISIBLE_BARLINE,
            Label("inv."),
            "Invisible",
            "Sin barra: el compás sigue dividiendo el tiempo.",
        )),
    ],
};
/// The signs and the jumps back to them, from one dropdown as Sibelius
/// keeps its repeat text in one gallery.
const SIGNS: GroupSpec = GroupSpec {
    dropdown: true,
    title: "D.C. / D.S.",
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
// Diseño

/// The user's own system and page ends, after the selected bar.
const PAPER_HINT: &str = "El papel de la vista de página y del PDF. Automático: A3 para doce instrumentos o más, A4 si no.";
/// Sibelius' Document Setup, reduced to the paper and its orientation.
const PAPER: GroupSpec = GroupSpec {
    dropdown: true,
    title: "Papel",
    visual: Icon("page"),
    priority: 2,
    controls: &[
        choice(tile(
            id::PAPER_AUTOMATIC,
            Label("auto"),
            "Automático",
            PAPER_HINT,
        )),
        choice(tile(id::PAPER_A4, Label("A4"), "A4", PAPER_HINT)),
        choice(tile(id::PAPER_LETTER, Label("8½"), "Carta", PAPER_HINT)),
        choice(tile(id::PAPER_A3, Label("A3"), "A3", PAPER_HINT)),
        choice(tile(
            id::PAPER_TABLOID,
            Label("Tab."),
            "Tabloide",
            PAPER_HINT,
        )),
        toggle(tile(
            id::LANDSCAPE,
            Label("↔"),
            "Horizontal",
            "Gira la página: más compases por sistema.",
        )),
    ],
};
const STAFF_SIZE_HINT: &str = "Altura del pentagrama impreso, de la primera a la quinta línea. Automático: según el número de instrumentos.";
const STAFF_SIZE: GroupSpec = GroupSpec {
    dropdown: true,
    title: "Pentagrama",
    visual: Visual::Staff,
    priority: 2,
    controls: &[
        choice(tile(
            id::STAFF_SIZE_AUTOMATIC,
            Label("auto"),
            "Automático",
            STAFF_SIZE_HINT,
        )),
        choice(tile(id::STAFF_SIZE_7, Label("7"), "7 mm", STAFF_SIZE_HINT)),
        choice(tile(id::STAFF_SIZE_6, Label("6"), "6 mm", STAFF_SIZE_HINT)),
        choice(tile(id::STAFF_SIZE_5, Label("5"), "5 mm", STAFF_SIZE_HINT)),
        choice(tile(id::STAFF_SIZE_4, Label("4"), "4 mm", STAFF_SIZE_HINT)),
    ],
};
const BAR_NUMBERS_HINT: &str = "Qué compases llevan su número, sobre el pentagrama superior de cada sistema. El primero nunca.";
/// Sibelius' bar number frequencies; the first bar is never numbered.
const BAR_NUMBERS: GroupSpec = GroupSpec {
    dropdown: true,
    title: "Números de compás",
    visual: Label("12"),
    priority: 2,
    controls: &[
        choice(tile(
            id::BAR_NUMBERS_SYSTEM,
            Label("|12"),
            "Sistema",
            BAR_NUMBERS_HINT,
        )),
        choice(tile(
            id::BAR_NUMBERS_EVERY_BAR,
            Label("123"),
            "Cada compás",
            BAR_NUMBERS_HINT,
        )),
        choice(tile(
            id::BAR_NUMBERS_EVERY_5,
            Label("5"),
            "Cada 5",
            BAR_NUMBERS_HINT,
        )),
        choice(tile(
            id::BAR_NUMBERS_EVERY_10,
            Label("10"),
            "Cada 10",
            BAR_NUMBERS_HINT,
        )),
        choice(tile(
            id::BAR_NUMBERS_HIDDEN,
            Label("–"),
            "Ninguno",
            BAR_NUMBERS_HINT,
        )),
    ],
};
const PAGE_NUMBERS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Números de página",
    visual: Label("1/3"),
    priority: 2,
    controls: &[
        choice(tile(
            id::PAGE_NUMBERS_FOOTER,
            Label("1/3"),
            "Al pie",
            "«2 / 5» centrado al pie de cada página.",
        )),
        choice(tile(
            id::PAGE_NUMBERS_CORNER,
            Label("2"),
            "Esquina",
            "El número arriba, en la esquina exterior, desde la segunda página, como en Sibelius.",
        )),
        choice(tile(
            id::PAGE_NUMBERS_HIDDEN,
            Label("no"),
            "Ninguno",
            "Páginas sin número.",
        )),
    ],
};
/// Noteheads change only the drawing: the note keeps its pitch and sound.
const NOTEHEADS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Cabeza",
    visual: Music("\u{e0a9}"),
    priority: 2,
    controls: &[
        toggle(tile(
            id::NOTEHEAD_CROSS,
            Music("\u{e0a9}"),
            "En x",
            "Cabeza en x: notas habladas, sin altura definida o fantasma. Suenan igual.",
        )),
        toggle(tile(
            id::NOTEHEAD_DIAMOND,
            Music("\u{e0db}"),
            "Rombo",
            "Cabeza de rombo, abierta en blancas y redondas. Suena igual.",
        )),
        toggle(tile(
            id::NOTEHEAD_SLASH,
            Music("\u{e101}"),
            "Diagonal",
            "Cabeza diagonal (slash): el ritmo sobre el que improvisar. Suena igual.",
        )),
    ],
};
/// Beam overrides at a note, as Sibelius' keypad and MuseScore's beam
/// properties offer; the automatic beaming applies everywhere else.
const BEAMS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Barra de corchea",
    visual: Visual::Beam {
        flagged: 0,
        beamed: 2,
    },
    priority: 2,
    controls: &[
        toggle(tile(
            id::BEAM_START,
            Visual::Beam {
                flagged: 1,
                beamed: 2,
            },
            "Iniciar",
            "La barra de corchea empieza en esta nota, aunque esté dentro de un tiempo.",
        )),
        toggle(tile(
            id::BEAM_JOIN,
            Visual::Beam {
                flagged: 0,
                beamed: 3,
            },
            "Unir",
            "La nota se une a la barra de la anterior, aunque cruce el tiempo.",
        )),
        toggle(tile(
            id::BEAM_OFF,
            Visual::Beam {
                flagged: 2,
                beamed: 0,
            },
            "Sin barra",
            "La nota lleva su propio corchete, sin barra.",
        )),
    ],
};
const BREAKS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Sistema y página",
    visual: Icon("continuous"),
    priority: 2,
    controls: &[
        toggle(tile(
            id::SYSTEM_BREAK,
            Icon("continuous"),
            "Nuevo sistema",
            "El siguiente compás empieza un sistema nuevo.",
        )),
        toggle(tile(
            id::PAGE_BREAK,
            Icon("page"),
            "Nueva página",
            "El siguiente compás empieza una página nueva.",
        )),
        toggle(tile(
            id::HIDE_EMPTY_STAVES,
            Visual::Staff,
            "Ocultar vacíos",
            "En la vista de página y el PDF, tras el primer sistema, quita los pentagramas de los instrumentos que no tocan en ese sistema.",
        )),
    ],
};

const POSITION: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Posición",
    visual: Icon("undo"),
    priority: 1,
    controls: &[tile(
        id::RESET_POSITION,
        Icon("undo"),
        "Restablecer",
        "Devuelve a su sitio lo que has movido a mano.",
    )],
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
            Icon("play-from-selection"),
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
            "Mezcla",
            "Volumen, posición y Sección | Solista de cada parte.",
        )),
        toggle(tile(
            id::SOUNDS,
            Icon("sound-bank"),
            "Sonido",
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
    visual: Icon("flag"),
    priority: 4,
    controls: &[
        toggle(tile(
            id::REVIEW_ISSUES,
            Icon("flag"),
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

/// Levare's ribbon, laid out as Sibelius': Inicio, Entrada de notas,
/// Notaciones, Texto, Reproducción, Diseño, Revisión and Vista. File is not
/// a tab: it is the dropdown in `menu`. Undo and redo live in the title
/// bar's quick access, the panels in the header. Durations, accidentals and
/// voices stay in Entrada de notas, where MuseScore keeps its note input
/// toolbar, instead of Sibelius' separate keypad. There are no remove
/// buttons: select the mark's note in the score and press its active control
/// again, or delete the selection. Apariencia and Partes return when their
/// functions exist.
pub const TABS: &[TabSpec] = &[
    TabSpec {
        id: TabId::Home,
        groups: &[CLIPBOARD, INSTRUMENTS, DIVISI, BARS, SELECT],
    },
    TabSpec {
        id: TabId::NoteInput,
        groups: &[
            ENTRY,
            DURATION,
            MODIFY,
            MORE_VALUES,
            ACCIDENTALS,
            TIES,
            VOICES,
            CHORDS,
        ],
    },
    TabSpec {
        id: TabId::Notations,
        groups: &[
            CHANGES,
            REPEATS,
            ARTICULATIONS,
            LINES,
            TECHNIQUE,
            SYMBOLS,
            GRACES,
            ARPEGGIOS,
            TREMOLOS,
        ],
    },
    TabSpec {
        id: TabId::Text,
        groups: &[
            HEADER,
            TEMPO,
            DYNAMICS,
            MORE_DYNAMICS,
            STAFF_TEXT,
            LYRICS,
            REHEARSAL,
            REHEARSAL_STYLE,
            SIGNS,
        ],
    },
    TabSpec {
        id: TabId::Playback,
        groups: &[TRANSPORT, PRACTICE, SOUND],
    },
    TabSpec {
        id: TabId::Layout,
        groups: &[
            PAPER,
            STAFF_SIZE,
            BREAKS,
            BAR_NUMBERS,
            PAGE_NUMBERS,
            NOTEHEADS,
            BEAMS,
            POSITION,
        ],
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

    /// Without their names (narrow windows) two tiles of a tab must still
    /// look different. A `ControlSize::Large` control keeps its name, so it
    /// may share its glyph: Comprobar's two tiles are one check on two
    /// ranges, the whole score (large) and the selection. A designed dropdown
    /// lists its options by name and never draws them as tiles.
    #[test]
    fn no_tab_draws_two_tiles_alike() {
        for tab in TABS {
            let mut seen = Vec::new();
            for group in tab.groups.iter().filter(|group| !group.dropdown) {
                for control in group
                    .controls
                    .iter()
                    .filter(|control| control.size != ControlSize::Large)
                {
                    assert!(
                        !seen.contains(&control.visual),
                        "{:?} → {}: {:?}",
                        tab.id,
                        group.title,
                        control.visual
                    );
                    seen.push(control.visual);
                }
            }
        }
        let checks = REVIEW_CHECKS.controls;
        assert_eq!(checks[0].size, ControlSize::Large);
        assert_eq!(checks[0].visual, checks[1].visual);
    }

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

    /// Outline drawings of notation itself, for the score's staves and bars.
    const NOTATION_ICONS: [&str; 5] = ["piano", "ossia", "bar-at-end", "bar-before", "delete-bar"];

    #[test]
    fn notation_groups_never_fall_back_to_utility_icons() {
        for group in [
            INSTRUMENTS,
            BARS,
            CHANGES,
            DURATION,
            MODIFY,
            MORE_VALUES,
            ACCIDENTALS,
            TIES,
            VOICES,
            CHORDS,
            ARTICULATIONS,
            GRACES,
            ARPEGGIOS,
            DYNAMICS,
            LINES,
            TECHNIQUE,
            TREMOLOS,
            DIVISI,
            REPEATS,
            SIGNS,
            NOTEHEADS,
            BEAMS,
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
                            | Visual::Beam { .. }
                            | Visual::Staff
                            | Visual::Bar
                            | Visual::Text(_)
                            | Visual::Label(_)
                    ) || matches!(visual, Icon(name) if NOTATION_ICONS.contains(&name)),
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
            id::SIXTY_FOURTH,
            id::DOUBLE_DOT,
            id::OTHER_TUPLET,
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
            TabId::Home,
            TabId::NoteInput,
            TabId::Notations,
            TabId::Text,
            TabId::Playback,
            TabId::Layout,
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
    use score_keymap::{KeySequence, Platform, Profile, Registry};

    /// Sibelius' order, without Apariencia and Partes.
    const ORDER: [(TabId, &str); 8] = [
        (TabId::Home, "Inicio"),
        (TabId::NoteInput, "Entrada de notas"),
        (TabId::Notations, "Notaciones"),
        (TabId::Text, "Texto"),
        (TabId::Playback, "Reproducción"),
        (TabId::Layout, "Diseño"),
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

    /// Left the ribbon for the app's native Vista menu
    /// (`app-gpui/src/shortcuts.rs`, which checks they are still there).
    const NATIVE_MENUS: &[CommandId] = &[
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
    fn tabs_follow_sibelius_order() {
        let tabs: Vec<_> = TABS.iter().map(|tab| (tab.id, tab.id.label())).collect();
        assert_eq!(tabs, ORDER);
    }

    #[test]
    fn every_ribbon_control_is_a_named_tile_with_a_hint() {
        for group in TABS.iter().flat_map(|tab| tab.groups) {
            for control in group.controls {
                let what = format!("{} → {}", group.title, control.command);
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
    fn more_values_holds_the_rare_rhythms_and_every_tuplet_but_the_triplet() {
        let commands: Vec<_> = MORE_VALUES.controls.iter().map(|c| c.command).collect();
        assert_eq!(
            commands,
            [
                id::SIXTY_FOURTH,
                id::DOUBLE_DOT,
                id::DUPLET,
                id::QUADRUPLET,
                id::QUINTUPLET,
                id::SEXTUPLET,
                id::SEPTUPLET,
                id::OCTUPLET,
                id::NONUPLET,
                id::OTHER_TUPLET,
            ]
        );
        assert!(MODIFY.controls.iter().any(|c| c.command == id::TRIPLET));
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

    /// Sibelius' Inicio, with the clipboard and the selection: the commands
    /// that once left the ribbon for Edición are back. Silencio returns to
    /// Entrada de notas, beside the dot, as on MuseScore's toolbar.
    #[test]
    fn home_and_note_input_hold_the_editing_tools_of_sibelius() {
        let commands = |id| -> Vec<_> {
            tab(id)
                .groups
                .iter()
                .flat_map(|group| group.controls)
                .map(|control| control.command)
                .collect()
        };
        let home = commands(TabId::Home);
        for command in [
            id::PASTE,
            id::COPY,
            id::CUT,
            id::REPEAT,
            id::SELECT_ALL,
            id::GO_TO_BAR,
            id::ADD_INSTRUMENT,
            id::DELETE_BAR,
            id::DIVISI,
        ] {
            assert!(home.contains(&command), "{command}");
        }
        let note_input = commands(TabId::NoteInput);
        assert!(note_input.contains(&id::REST));
        assert!(commands(TabId::Text).contains(&id::DYNAMIC_MF));
        assert!(commands(TabId::Notations).contains(&id::CRESCENDO));
        assert!(commands(TabId::Layout).contains(&id::RESET_POSITION));
    }

    #[test]
    fn old_tab_shortcuts_open_a_tab() {
        let registry = Registry::builtin();
        assert!(registry.get(id::TAB_DYNAMICS.as_str()).is_some());
        for (platform, modifier) in [
            (Platform::MacOs, "alt"),
            (Platform::Windows, "shift"),
            (Platform::Linux, "shift"),
        ] {
            let profile = Profile::sibelius_laptop_for(platform);
            for (index, (tab, _)) in ORDER.iter().enumerate() {
                let keys =
                    KeySequence::parse(&format!("{modifier}-secondary-{}", index + 1)).unwrap();
                assert_eq!(profile.keys_for(tab.command()), vec![keys], "{tab:?}");
            }
            assert!(profile.keys_for(id::TAB_DYNAMICS).is_empty());
        }
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
