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
    Layout,
    Appearance,
    Parts,
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
            Self::Layout => "Disposición",
            Self::Appearance => "Apariencia",
            Self::Parts => "Partes",
            Self::Review => "Revisión",
            Self::View => "Vista",
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
    /// Plain or italic text, such as `pizz.`.
    Text(&'static str),
    None,
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
    pub visual: Visual,
    /// Short text for the toolbar; the registry's label is used when `None`.
    pub label: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroupSpec {
    pub title: &'static str,
    pub visual: Visual,
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
        visual,
        label: Some(label),
    }
}
const fn small(command: CommandId, visual: Visual, label: &'static str) -> Control {
    Control {
        command,
        size: ControlSize::Small,
        visual,
        label: Some(label),
    }
}
/// `label` names the tile when the group has room to show it.
const fn tile(command: CommandId, visual: Visual, label: &'static str) -> Control {
    Control {
        command,
        size: ControlSize::Tile,
        visual,
        label: Some(label),
    }
}

use Visual::{Icon, Music, None as Plain, Text};
use score_core::{
    Articulation as A, ConnectionKind as K, Dynamic as D, MarkKind, PlayingTechnique as T,
};

/// A catalog mark: its command, symbol and name come from the catalog.
const fn mark_visual(kind: MarkKind) -> Visual {
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
    title: "Portapapeles",
    visual: Icon("clipboard"),
    priority: 1,
    controls: &[
        large(id::PASTE, Icon("clipboard"), "Pegar"),
        small(id::COPY, Icon("copy"), "Copiar"),
        small(id::CUT, Icon("cut"), "Cortar"),
        small(id::REPEAT, Icon("repeat"), "Repetir"),
    ],
};
const INSTRUMENTS: GroupSpec = GroupSpec {
    title: "Instrumentos",
    visual: Icon("add-staff"),
    priority: 3,
    controls: &[
        large(id::ADD_INSTRUMENT, Icon("add-staff"), "Añadir"),
        small(id::ADD_PIANO, Icon("piano"), "Piano a dos manos"),
        small(id::OSSIA, Icon("ossia"), "Ossia"),
    ],
};
const BARS: GroupSpec = GroupSpec {
    title: "Compases",
    visual: Icon("add-bar"),
    priority: 2,
    controls: &[large(id::BAR_AT_END, Icon("add-bar"), "Añadir compás")],
};
const SELECTION: GroupSpec = GroupSpec {
    title: "Selección",
    visual: Plain,
    priority: 0,
    controls: &[small(id::SELECT_ALL, Plain, "Seleccionar todo")],
};
/// Sibelius' Notations › Common: what starts or changes the staff itself.
const COMMON: GroupSpec = GroupSpec {
    title: "Comunes",
    visual: Music("\u{e050}"),
    priority: 5,
    controls: &[
        small(id::CLEF, Music("\u{e050}"), "Clave"),
        small(id::KEY_SIGNATURE, Music("\u{e262}"), "Tonalidad"),
        small(id::TIME_SIGNATURE, Music("\u{e084}"), "Compás de tiempo"),
    ],
};
const INPUT_MODE: GroupSpec = GroupSpec {
    title: "Modo de entrada",
    visual: Music("\u{e1d7}"),
    priority: 4,
    controls: &[
        large(id::TOGGLE_NOTE_INPUT, Music("\u{e1d7}"), "Notas"),
        small(id::STEP_TIME, Icon("steps"), "Paso a paso"),
        small(id::FLEXI_TIME, Icon("clock"), "Tiempo real"),
        small(id::MIDI_INPUT, Icon("keyboard"), "Teclado MIDI"),
    ],
};
const DURATION: GroupSpec = GroupSpec {
    title: "Duración",
    visual: Music("\u{e1d5}"),
    priority: 9,
    controls: &[
        tile(id::WHOLE, Music("\u{e1d2}"), "Redonda"),
        tile(id::HALF, Music("\u{e1d3}"), "Blanca"),
        tile(id::QUARTER, Music("\u{e1d5}"), "Negra"),
        tile(id::EIGHTH, Music("\u{e1d7}"), "Corchea"),
        tile(id::SIXTEENTH, Music("\u{e1d9}"), "Semicorchea"),
        tile(id::THIRTY_SECOND, Music("\u{e1db}"), "Fusa"),
        tile(id::DOT, Music("\u{e1e7}"), "Puntillo"),
        tile(id::TRIPLET, Music("\u{e883}"), "Tresillo"),
    ],
};
const ACCIDENTALS: GroupSpec = GroupSpec {
    title: "Alteraciones",
    visual: Music("\u{e262}"),
    priority: 8,
    controls: &[
        tile(id::NATURAL, Music("\u{e261}"), "Becuadro"),
        tile(id::SHARP, Music("\u{e262}"), "Sostenido"),
        tile(id::FLAT, Music("\u{e260}"), "Bemol"),
    ],
};
/// Chord mode and the intervals that stack a note above the selected one.
const CHORDS: GroupSpec = GroupSpec {
    title: "Acordes",
    visual: Icon("chord"),
    priority: 2,
    controls: &[
        large(id::CHORD_MODE, Icon("chord"), "Modo acorde"),
        tile(id::INTERVAL_2, Text("2ª"), "Segunda"),
        tile(id::INTERVAL_3, Text("3ª"), "Tercera"),
        tile(id::INTERVAL_4, Text("4ª"), "Cuarta"),
        tile(id::INTERVAL_5, Text("5ª"), "Quinta"),
        tile(id::INTERVAL_6, Text("6ª"), "Sexta"),
        tile(id::INTERVAL_7, Text("7ª"), "Séptima"),
        tile(id::INTERVAL_8, Text("8ª"), "Octava"),
    ],
};
const VOICES: GroupSpec = GroupSpec {
    title: "Voces",
    visual: Text("1"),
    priority: 2,
    controls: &[
        tile(id::VOICE_1, Text("1"), "Voz 1"),
        tile(id::VOICE_2, Text("2"), "Voz 2"),
        tile(id::VOICE_3, Text("3"), "Voz 3"),
        tile(id::VOICE_4, Text("4"), "Voz 4"),
    ],
};
const ELEMENTS: GroupSpec = GroupSpec {
    title: "Elementos",
    visual: Music("\u{e050}"),
    priority: 2,
    controls: &[
        small(id::LINES, Plain, "Líneas"),
        small(id::SYMBOL, Plain, "Símbolo"),
        small(id::GRACE_NOTE, Plain, "Nota de adorno"),
    ],
};
/// Like MuseScore's note-input bar and Sibelius' keypad: what is written
/// with each note sits beside its duration and accidental.
const ARTICULATIONS: GroupSpec = GroupSpec {
    title: "Articulaciones y ligaduras",
    visual: Music("\u{e4a0}"),
    priority: 7,
    controls: &[
        mark_tile(MarkKind::Articulation(A::Staccato)),
        mark_tile(MarkKind::Articulation(A::Tenuto)),
        mark_tile(MarkKind::Articulation(A::Accent)),
        mark_tile(MarkKind::Articulation(A::Marcato)),
        mark_tile(MarkKind::Fermata),
        tile(id::TIE, Icon("tie"), "Unión"),
        tile(id::SLUR, Icon("slur"), "Expresión"),
    ],
};
const DYNAMICS: GroupSpec = GroupSpec {
    title: "Dinámicas",
    visual: Music("\u{e521}\u{e522}"),
    priority: 6,
    controls: &[
        mark_tile(MarkKind::Dynamic(D::Pp)),
        mark_tile(MarkKind::Dynamic(D::P)),
        mark_tile(MarkKind::Dynamic(D::Mp)),
        mark_tile(MarkKind::Dynamic(D::Mf)),
        mark_tile(MarkKind::Dynamic(D::F)),
        mark_tile(MarkKind::Dynamic(D::Ff)),
        mark_small(MarkKind::Dynamic(D::Ppp), "ppp"),
        mark_small(MarkKind::Dynamic(D::Fff), "fff"),
    ],
};
const HAIRPINS: GroupSpec = GroupSpec {
    title: "Reguladores",
    visual: Music("\u{e53e}"),
    priority: 4,
    controls: &[
        mark_small(MarkKind::Connection(K::Crescendo), "Crescendo"),
        mark_small(MarkKind::Connection(K::Diminuendo), "Diminuendo"),
    ],
};
const TECHNIQUE: GroupSpec = GroupSpec {
    title: "Técnica de cuerda",
    visual: Text("pizz."),
    priority: 3,
    controls: &[
        mark_small(MarkKind::Technique(T::Pizzicato), "Pizzicato"),
        mark_small(MarkKind::Technique(T::Arco), "Arco"),
    ],
};
const TEXT_EXPRESSION: GroupSpec = GroupSpec {
    title: "Texto musical",
    visual: Text("Aa"),
    priority: 2,
    controls: &[
        small(id::EXPRESSION_TEXT, Text("espr."), "Expresión"),
        small(id::TECHNIQUE_TEXT, Text("pizz."), "Técnica"),
        small(id::TEMPO_TEXT, Text("Allegro"), "Tempo"),
        small(id::LYRICS, Text("la"), "Letra"),
        small(id::CHORD_SYMBOL, Text("C7"), "Cifrado"),
        small(id::REHEARSAL_MARK, Text("A"), "Ensayo"),
    ],
};
const TRANSPORT: GroupSpec = GroupSpec {
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
    title: "Sonido",
    visual: Icon("speaker"),
    priority: 4,
    controls: &[
        small(id::AUDITION, Icon("speaker"), "Audición al escribir"),
        small(id::TEMPO_UP, Plain, "Subir tempo"),
        small(id::TEMPO_DOWN, Plain, "Bajar tempo"),
        large(id::MIXER, Icon("mixer"), "Mezclador"),
    ],
};
const INTERPRETATION: GroupSpec = GroupSpec {
    title: "Interpretación",
    visual: Icon("loop"),
    priority: 2,
    controls: &[
        small(id::LOOP, Icon("loop"), "Bucle"),
        small(id::REPEATS, Plain, "Repeticiones"),
        small(id::HALF_SPEED, Plain, "Media velocidad"),
    ],
};
const ZOOM: GroupSpec = GroupSpec {
    title: "Zoom",
    visual: Icon("search"),
    priority: 4,
    controls: &[
        small(id::ZOOM_IN, Plain, "Acercar"),
        small(id::ZOOM_OUT, Plain, "Alejar"),
        small(id::ZOOM_ACTUAL, Plain, "Tamaño real"),
    ],
};
const PANELS: GroupSpec = GroupSpec {
    title: "Paneles",
    visual: Icon("left-panel"),
    priority: 3,
    controls: &[
        small(
            id::TOGGLE_INSTRUMENTS_PANEL,
            Icon("left-panel"),
            "Instrumentos",
        ),
        small(id::TOGGLE_INSPECTOR, Icon("right-panel"), "Propiedades"),
        small(id::FULL_SCREEN, Plain, "Pantalla completa"),
    ],
};
const PAGE: GroupSpec = GroupSpec {
    title: "Página",
    visual: Icon("page"),
    priority: 2,
    controls: &[
        small(id::PAGE_VIEW, Icon("page"), "Vista de página"),
        small(id::ZOOM_FIT, Plain, "Ajustar a la página"),
    ],
};

/// Partitura's ribbon, after Sibelius' tabs. File is not a tab: it is the
/// dropdown in `menu`. Undo and redo live in the title bar's quick access.
/// There are no remove buttons: select the mark's note in the score and
/// press its active control again, or delete the selection.
pub const TABS: &[TabSpec] = &[
    TabSpec {
        id: TabId::Home,
        groups: &[CLIPBOARD, INSTRUMENTS, BARS, SELECTION],
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
        groups: &[COMMON, DYNAMICS, HAIRPINS, TECHNIQUE, ELEMENTS],
    },
    TabSpec {
        id: TabId::Text,
        groups: &[TEXT_EXPRESSION],
    },
    TabSpec {
        id: TabId::Playback,
        groups: &[TRANSPORT, SOUND, INTERPRETATION],
    },
    TabSpec {
        id: TabId::Layout,
        groups: &[],
    },
    TabSpec {
        id: TabId::Appearance,
        groups: &[],
    },
    TabSpec {
        id: TabId::Parts,
        groups: &[],
    },
    TabSpec {
        id: TabId::Review,
        groups: &[],
    },
    TabSpec {
        id: TabId::View,
        groups: &[ZOOM, PANELS, PAGE],
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
    fn every_tab_has_a_spec() {
        for id in [
            TabId::Home,
            TabId::NoteInput,
            TabId::Notation,
            TabId::Text,
            TabId::Playback,
            TabId::Layout,
            TabId::Appearance,
            TabId::Parts,
            TabId::Review,
            TabId::View,
        ] {
            assert_eq!(tab(id).id, id);
        }
        assert_eq!(TABS.len(), 10);
    }
}
