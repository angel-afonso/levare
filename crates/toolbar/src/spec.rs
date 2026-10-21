//! What the ribbon contains: tabs, groups and the commands of each control.
//! Pure data. Labels, keys, state and the pending mark come from elsewhere.

use score_keymap::{CommandId, id};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TabId {
    Home,
    NoteInput,
    Notation,
    Text,
    Playback,
    Review,
    View,
}

impl TabId {
    pub fn label(self) -> &'static str {
        match self {
            Self::Home => "Inicio",
            Self::NoteInput => "Entrada",
            Self::Notation => "Notación",
            Self::Text => "Texto",
            Self::Playback => "Reproducción",
            Self::Review => "Revisión",
            Self::View => "Vista",
        }
    }

    /// The command that shows this tab.
    pub fn command(self) -> CommandId {
        match self {
            Self::Home => id::TAB_HOME,
            Self::NoteInput => id::TAB_NOTE_INPUT,
            Self::Notation => id::TAB_NOTATION,
            Self::Text => id::TAB_TEXT,
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
    /// Switches something on or off.
    Toggle,
    /// Does something once.
    Action,
}

/// Every control of a size measures the same, which keeps groups aligned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlSize {
    /// Icon over label, full height.
    Large,
    /// One row of a three-row column: icon and label.
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

const fn large(command: CommandId, visual: Visual, label: &'static str) -> Control {
    Control {
        command,
        size: ControlSize::Large,
        role: Role::Action,
        visual,
        symbol_only: false,
        label: Some(label),
    }
}
const fn small(command: CommandId, visual: Visual, label: &'static str) -> Control {
    Control {
        command,
        size: ControlSize::Small,
        role: Role::Action,
        visual,
        symbol_only: false,
        label: Some(label),
    }
}
/// `label` names the tile when the group has room to show it.
const fn tile(command: CommandId, visual: Visual, label: &'static str) -> Control {
    Control {
        command,
        size: ControlSize::Tile,
        role: Role::Action,
        visual,
        symbol_only: false,
        label: Some(label),
    }
}

const fn symbol(control: Control) -> Control {
    Control {
        symbol_only: true,
        ..control
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

use Visual::{Icon, Label, Music, None as Plain, Text};
use score_core::{
    Arpeggio as R, Articulation as A, ConnectionKind as K, Dynamic as D, MarkKind,
    PlayingTechnique as T,
};

/// A catalog mark: its command, symbol and name come from the catalog.
const fn mark_visual(kind: MarkKind) -> Visual {
    if let MarkKind::Harmonic(harmonic) = kind {
        return Visual::Harmonic(harmonic);
    }
    match kind.symbol() {
        Some(symbol) => Music(symbol),
        None => Text(kind.label()),
    }
}
const fn mark_tile(kind: MarkKind) -> Control {
    tile(crate::marks::command(kind), mark_visual(kind), kind.name())
}
const fn mark_small(kind: MarkKind, label: &'static str) -> Control {
    small(crate::marks::command(kind), mark_visual(kind), label)
}

const CLIPBOARD: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Portapapeles",
    visual: Icon("clipboard"),
    priority: 1,
    controls: &[
        large(id::PASTE, Icon("clipboard"), "Pegar"),
        small(id::COPY, Icon("copy"), "Copiar"),
        small(id::CUT, Icon("cut"), "Cortar"),
        small(id::REPEAT, Icon("repeat"), "Repetir"),
        small(id::SELECT_ALL, Plain, "Seleccionar todo"),
    ],
};
const INSTRUMENTS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Instrumentos",
    visual: Visual::Staff,
    priority: 3,
    controls: &[
        large(id::ADD_INSTRUMENT, Visual::Staff, "Añadir"),
        small(id::ADD_PIANO, Visual::Staff, "Piano a dos manos"),
        small(id::OSSIA, Visual::Staff, "Ossia"),
    ],
};
const BARS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Compases",
    visual: Visual::Bar,
    priority: 2,
    controls: &[
        large(id::BAR_AT_END, Visual::Bar, "Añadir al final"),
        small(id::SINGLE_BAR, Visual::Bar, "Insertar antes"),
        small(id::DELETE_BAR, Visual::Bar, "Eliminar compases"),
    ],
};
const TREMOLOS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Trémolo",
    visual: Music("\u{e221}"),
    priority: 2,
    controls: &[
        symbol(toggle(mark_small(
            MarkKind::Tremolo(score_core::Tremolo::One),
            "1",
        ))),
        symbol(toggle(mark_small(
            MarkKind::Tremolo(score_core::Tremolo::Two),
            "2",
        ))),
        symbol(toggle(mark_small(
            MarkKind::Tremolo(score_core::Tremolo::Three),
            "3",
        ))),
    ],
};
/// One place for divisi: `div.` opens two staves from the selection, `unis.` closes them.
const DIVISI: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Divisi",
    visual: Text("div."),
    priority: 3,
    controls: &[
        toggle(small(id::DIVISI, Text("div."), "Dividir (div.)")),
        toggle(small(id::UNISON, Text("unis."), "Unir (unis.)")),
    ],
};
const REPEATS: GroupSpec = GroupSpec {
    dropdown: true,
    title: "Repeticiones",
    visual: Music("\u{e041}"),
    priority: 2,
    controls: &[
        toggle(small(id::REPEAT_START, Music("\u{e040}"), "Inicio")),
        toggle(small(id::REPEAT_END, Music("\u{e041}"), "Fin")),
        toggle(small(id::ENDING_1, Label("1."), "1.ª")),
        toggle(small(id::ENDING_2, Label("2."), "2.ª")),
        toggle(small(id::DOUBLE_BARLINE, Music("\u{e031}"), "Doble")),
        toggle(small(id::FINAL_BARLINE, Music("\u{e032}"), "Final")),
    ],
};
const JUMPS: GroupSpec = GroupSpec {
    dropdown: true,
    title: "Saltos",
    visual: Music("\u{e047}"),
    priority: 2,
    controls: &[
        toggle(small(id::SEGNO, Music("\u{e047}"), "Segno")),
        toggle(small(id::CODA, Music("\u{e048}"), "Coda")),
        toggle(small(id::FINE, Text("Fine"), "Fine")),
        toggle(small(id::TO_CODA, Text("To Coda"), "To Coda")),
        toggle(small(id::DA_CAPO, Text("D.C."), "D.C.")),
        toggle(small(id::DA_CAPO_AL_FINE, Text("D.C."), "D.C. al Fine")),
        toggle(small(id::DA_CAPO_AL_CODA, Text("D.C."), "D.C. al Coda")),
        toggle(small(id::DAL_SEGNO, Text("D.S."), "D.S.")),
        toggle(small(id::DAL_SEGNO_AL_FINE, Text("D.S."), "D.S. al Fine")),
        toggle(small(id::DAL_SEGNO_AL_CODA, Text("D.S."), "D.S. al Coda")),
    ],
};
/// Sibelius' Notations › Common: what starts or changes the staff itself.
const COMMON: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Comunes",
    visual: Music("\u{e050}"),
    priority: 5,
    controls: &[
        small(id::CLEF, Visual::Clef(score_core::Clef::Treble), "Clave"),
        small(id::KEY_SIGNATURE, Music("\u{e262}"), "Tonalidad"),
        small(id::TIME_SIGNATURE, Music("\u{e084}"), "Compás de tiempo"),
    ],
};
const INPUT_MODE: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Modo de entrada",
    visual: Music("\u{e1d7}"),
    priority: 4,
    controls: &[
        toggle(large(id::TOGGLE_NOTE_INPUT, Music("\u{e1d7}"), "Notas")),
        small(id::ENTRY_OCTAVE_UP, Label("↑"), "Subir octava"),
        small(id::ENTRY_OCTAVE_DOWN, Label("↓"), "Bajar octava"),
    ],
};
const DURATION: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Duración",
    visual: Music("\u{e1d5}"),
    priority: 9,
    controls: &[
        choice(tile(id::WHOLE, Music("\u{e1d2}"), "Redonda")),
        choice(tile(id::HALF, Music("\u{e1d3}"), "Blanca")),
        choice(tile(id::QUARTER, Music("\u{e1d5}"), "Negra")),
        choice(tile(id::EIGHTH, Music("\u{e1d7}"), "Corchea")),
        choice(tile(id::SIXTEENTH, Music("\u{e1d9}"), "Semicorchea")),
        choice(tile(id::THIRTY_SECOND, Music("\u{e1db}"), "Fusa")),
        toggle(tile(id::DOT, Music("\u{e1e7}"), "Puntillo")),
        toggle(tile(id::TRIPLET, Music("\u{e883}"), "Tresillo")),
        tile(id::REST, Music("\u{e4e5}"), "Silencio"),
    ],
};
const ACCIDENTALS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Alteraciones",
    visual: Music("\u{e262}"),
    priority: 8,
    controls: &[
        choice(tile(id::NATURAL, Music("\u{e261}"), "Becuadro")),
        choice(tile(id::SHARP, Music("\u{e262}"), "Sostenido")),
        choice(tile(id::FLAT, Music("\u{e260}"), "Bemol")),
        choice(tile(id::FOLLOW_KEY, Label("arm."), "Según armadura")),
    ],
};
/// Chord mode and the intervals that stack a note above the selected one.
const CHORDS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Acordes",
    visual: Visual::Chord,
    priority: 2,
    controls: &[
        toggle(large(id::CHORD_MODE, Visual::Chord, "Modo acorde")),
        tile(id::INTERVAL_2, Label("2ª"), "Segunda"),
        tile(id::INTERVAL_3, Label("3ª"), "Tercera"),
        tile(id::INTERVAL_4, Label("4ª"), "Cuarta"),
        tile(id::INTERVAL_5, Label("5ª"), "Quinta"),
        tile(id::INTERVAL_6, Label("6ª"), "Sexta"),
        tile(id::INTERVAL_7, Label("7ª"), "Séptima"),
        tile(id::INTERVAL_8, Label("8ª"), "Octava"),
    ],
};
const VOICES: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Voces",
    visual: Label("1"),
    priority: 2,
    controls: &[
        choice(tile(id::VOICE_1, Label("1"), "Voz 1")),
        choice(tile(id::VOICE_2, Label("2"), "Voz 2")),
    ],
};
/// Like MuseScore's note-input bar and Sibelius' keypad: what is written
/// with each note sits beside its duration and accidental.
const ARTICULATIONS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Articulaciones y ligaduras",
    visual: Music("\u{e4a0}"),
    priority: 7,
    controls: &[
        toggle(mark_tile(MarkKind::Articulation(A::Staccato))),
        toggle(mark_tile(MarkKind::Articulation(A::Tenuto))),
        toggle(mark_tile(MarkKind::Articulation(A::Accent))),
        toggle(mark_tile(MarkKind::Articulation(A::Marcato))),
        toggle(mark_tile(MarkKind::Fermata)),
        toggle(tile(id::TIE, Visual::Tie, "Unión")),
        toggle(tile(id::SLUR, Visual::Slur, "Expresión")),
    ],
};
const ARPEGGIOS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Arpegios",
    visual: Music("\u{e63c}"),
    priority: 6,
    controls: &[
        toggle(mark_tile(MarkKind::Arpeggio(R::Normal))),
        toggle(mark_tile(MarkKind::Arpeggio(R::Up))),
        toggle(mark_tile(MarkKind::Arpeggio(R::Down))),
        toggle(large(
            id::ARPEGGIO_BOTH_HANDS,
            Music("\u{e63c}"),
            "Ambas manos",
        )),
    ],
};
const DYNAMICS: GroupSpec = GroupSpec {
    dropdown: true,
    title: "Dinámicas",
    visual: Music("\u{e521}\u{e522}"),
    priority: 6,
    controls: &[
        symbol(mark_tile(MarkKind::Dynamic(D::Pp))),
        symbol(mark_tile(MarkKind::Dynamic(D::P))),
        symbol(mark_tile(MarkKind::Dynamic(D::Mp))),
        symbol(mark_tile(MarkKind::Dynamic(D::Mf))),
        symbol(mark_tile(MarkKind::Dynamic(D::F))),
        symbol(mark_tile(MarkKind::Dynamic(D::Ff))),
        symbol(mark_small(MarkKind::Dynamic(D::Ppp), "ppp")),
        symbol(mark_small(MarkKind::Dynamic(D::Fff), "fff")),
    ],
};
const HAIRPINS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Reguladores",
    visual: Music("\u{e53e}"),
    priority: 4,
    controls: &[
        toggle(mark_small(MarkKind::Connection(K::Crescendo), "Crescendo")),
        toggle(mark_small(
            MarkKind::Connection(K::Diminuendo),
            "Diminuendo",
        )),
    ],
};
const TECHNIQUE: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Técnica de cuerda",
    visual: Text("pizz."),
    priority: 3,
    controls: &[
        symbol(toggle(mark_small(
            MarkKind::Technique(T::Pizzicato),
            "Pizzicato",
        ))),
        symbol(toggle(mark_small(MarkKind::Technique(T::Arco), "Arco"))),
        symbol(toggle(mark_small(
            MarkKind::Harmonic(score_core::Harmonic::Natural),
            "Armónico natural",
        ))),
        symbol(toggle(mark_small(
            MarkKind::Harmonic(score_core::Harmonic::Artificial),
            "Armónico artificial",
        ))),
    ],
};
const HEADER: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Título",
    visual: Label("Aa"),
    priority: 4,
    controls: &[large(id::SCORE_INFO, Label("Aa"), "Título y créditos")],
};
/// The written tempo: the mark itself and quick ±5 changes to it.
const TEMPO: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Tempo",
    visual: Text("Allegro"),
    priority: 5,
    controls: &[
        large(id::TEMPO_TEXT, Text("Allegro"), "Tempo"),
        small(id::TEMPO_UP, Icon("tempo-up"), "Subir tempo"),
        small(id::TEMPO_DOWN, Icon("tempo-down"), "Bajar tempo"),
    ],
};
/// Write a mark, then choose how every mark of the score is numbered.
const REHEARSAL: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Marcas de ensayo",
    visual: Label("A"),
    priority: 3,
    controls: &[
        toggle(large(id::REHEARSAL_MARK, Label("A"), "Ensayo")),
        choice(tile(id::REHEARSAL_LETTERS, Label("A"), "Letras")),
        choice(tile(id::REHEARSAL_NUMBERS, Label("1"), "Números")),
        choice(tile(id::REHEARSAL_BAR_NUMBERS, Label("12"), "Nº de compás")),
    ],
};
const TRANSPORT: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Transporte",
    visual: Icon("play"),
    priority: 5,
    controls: &[
        large(id::PLAY, Icon("play"), "Reproducir"),
        small(id::PLAY_FROM_SELECTION, Icon("play"), "Desde la selección"),
        small(id::PLAY_FROM_START, Icon("first"), "Desde el inicio"),
        small(id::STOP, Icon("stop"), "Detener"),
    ],
};
const SOUND: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Sonido",
    visual: Icon("speaker"),
    priority: 4,
    controls: &[
        toggle(small(id::AUDITION, Icon("speaker"), "Audición al escribir")),
        toggle(small(id::SOUNDS, Icon("speaker"), "Sonidos")),
        toggle(large(id::MIXER, Icon("mixer"), "Mezclador")),
    ],
};
const INTERPRETATION: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Interpretación",
    visual: Icon("loop"),
    priority: 2,
    controls: &[
        toggle(small(id::LOOP, Icon("loop"), "Bucle")),
        toggle(small(id::REPEATS, Icon("repeats"), "Repeticiones")),
        toggle(small(id::HALF_SPEED, Icon("half-speed"), "Media velocidad")),
    ],
};
const VIEW_MODES: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Modo de vista",
    visual: Icon("page"),
    priority: 4,
    controls: &[
        choice(large(id::PAGE_VIEW, Icon("page"), "Página")),
        choice(large(id::CONTINUOUS_VIEW, Icon("continuous"), "Continua")),
        choice(large(
            id::SINGLE_SYSTEM_VIEW,
            Icon("single-system"),
            "Pentagrama",
        )),
    ],
};
const REVIEW_CHECKS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Comprobar",
    visual: Icon("search"),
    priority: 5,
    controls: &[
        large(id::CHECK_SCORE, Icon("search"), "Partitura"),
        small(id::CHECK_SELECTION, Icon("search"), "Selección"),
    ],
};
const REVIEW_NOTICES: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Avisos",
    visual: Icon("right-panel"),
    priority: 4,
    controls: &[
        toggle(large(
            id::REVIEW_ISSUES,
            Icon("right-panel"),
            "Mostrar avisos",
        )),
        small(id::REVIEW_PREVIOUS, Label("‹"), "Anterior"),
        small(id::REVIEW_NEXT, Label("›"), "Siguiente"),
    ],
};
const REVIEW_RANGES: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Rangos",
    visual: Icon("single-system"),
    priority: 3,
    controls: &[
        toggle(small(
            id::SHOW_RANGES,
            Icon("single-system"),
            "Notas en rojo",
        )),
        small(id::EDIT_RANGE, Icon("mixer"), "Editar rango…"),
        small(id::RESET_RANGE, Icon("undo"), "Restablecer"),
    ],
};
const REVIEW_COMMENTS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Comentarios",
    visual: Icon("right-panel"),
    priority: 4,
    controls: &[
        large(id::NEW_COMMENT, Label("+"), "Nuevo comentario"),
        toggle(small(id::SHOW_COMMENTS, Icon("right-panel"), "Mostrar")),
        small(id::RESOLVE_COMMENT, Label("✓"), "Resolver / reabrir"),
    ],
};
const ZOOM: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Zoom",
    visual: Icon("search"),
    priority: 4,
    controls: &[
        small(id::ZOOM_IN, Icon("zoom-in"), "Acercar"),
        small(id::ZOOM_OUT, Icon("zoom-out"), "Alejar"),
        small(id::ZOOM_ACTUAL, Icon("zoom-actual"), "Tamaño real"),
        small(id::ZOOM_FIT, Icon("fit-page"), "Ajustar a la página"),
    ],
};
const PANELS: GroupSpec = GroupSpec {
    dropdown: false,
    title: "Paneles",
    visual: Icon("left-panel"),
    priority: 3,
    controls: &[
        toggle(small(
            id::TOGGLE_INSTRUMENTS_PANEL,
            Icon("left-panel"),
            "Instrumentos",
        )),
        toggle(small(
            id::TOGGLE_INSPECTOR,
            Icon("right-panel"),
            "Propiedades",
        )),
        toggle(small(
            id::FULL_SCREEN,
            Icon("full-screen"),
            "Pantalla completa",
        )),
        toggle(small(
            id::TOGGLE_RIBBON,
            Icon("full-screen"),
            "Plegar cinta",
        )),
    ],
};

/// Levare's ribbon, after Sibelius' tabs. File is not a tab: it is the
/// dropdown in `menu`. Undo and redo live in the title bar's quick access.
/// There are no remove buttons: select the mark's note in the score and
/// press its active control again, or delete the selection.
/// A tab without real functions is not shown: Layout and Parts return when
/// their functions exist.
pub const TABS: &[TabSpec] = &[
    TabSpec {
        id: TabId::Home,
        groups: &[CLIPBOARD, INSTRUMENTS, BARS],
    },
    TabSpec {
        id: TabId::NoteInput,
        groups: &[
            INPUT_MODE,
            DURATION,
            ACCIDENTALS,
            ARTICULATIONS,
            VOICES,
            CHORDS,
        ],
    },
    TabSpec {
        id: TabId::Notation,
        groups: &[
            COMMON, DYNAMICS, HAIRPINS, ARPEGGIOS, TECHNIQUE, TREMOLOS, DIVISI, REPEATS, JUMPS,
        ],
    },
    TabSpec {
        id: TabId::Text,
        groups: &[HEADER, TEMPO, REHEARSAL],
    },
    TabSpec {
        id: TabId::Playback,
        groups: &[TRANSPORT, SOUND, INTERPRETATION],
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
        groups: &[VIEW_MODES, ZOOM, PANELS],
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
            COMMON,
            DURATION,
            ACCIDENTALS,
            CHORDS,
            ARTICULATIONS,
            ARPEGGIOS,
            DYNAMICS,
            HAIRPINS,
            TECHNIQUE,
            TREMOLOS,
            DIVISI,
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
        assert!(matches!(INPUT_MODE.controls[0].visual, Visual::Music(_)));
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
            TabId::Home,
            TabId::NoteInput,
            TabId::Notation,
            TabId::Text,
            TabId::Playback,
            TabId::Review,
            TabId::View,
        ] {
            assert_eq!(tab(id).id, id);
            assert!(!tab(id).groups.is_empty(), "{id:?}");
        }
        assert_eq!(TABS.len(), 7);
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
                REHEARSAL
                    .controls
                    .iter()
                    .any(|control| control.command == command)
            );
        }
    }
}
