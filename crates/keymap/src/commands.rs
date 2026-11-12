//! The command registry: every action a shortcut, button or menu can run.

use std::fmt;

/// Stable identifier. It is stored in the user's shortcut file, so an id is
/// never renamed once published; only its label may change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommandId(pub &'static str);

impl CommandId {
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    File,
    Edit,
    NoteInput,
    Duration,
    Accidentals,
    Expression,
    Notation,
    Bars,
    Instruments,
    Navigation,
    Playback,
    Review,
    View,
}

impl Category {
    pub const ALL: [Self; 13] = [
        Self::File,
        Self::Edit,
        Self::NoteInput,
        Self::Duration,
        Self::Accidentals,
        Self::Expression,
        Self::Notation,
        Self::Bars,
        Self::Instruments,
        Self::Navigation,
        Self::Playback,
        Self::Review,
        Self::View,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::File => "Archivo",
            Self::Edit => "Edición",
            Self::NoteInput => "Entrada de notas",
            Self::Duration => "Duración",
            Self::Accidentals => "Alteraciones",
            Self::Expression => "Expresión",
            Self::Notation => "Notación",
            Self::Bars => "Compases",
            Self::Instruments => "Instrumentos",
            Self::Navigation => "Navegación",
            Self::Playback => "Reproducción",
            Self::Review => "Revisión",
            Self::View => "Vista",
        }
    }
}

/// Where a shortcut is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    /// Anywhere in the window, also over dialogs (usually ⌘ combinations).
    Global,
    /// While working on the score: single keys that would otherwise type text.
    Score,
    /// Only while a modal panel, such as Preferences, is open.
    Modal,
}

impl Scope {
    /// GPUI key-context predicate. The window root declares `Levare`, adds
    /// `Modal` while a panel is open, and text fields add `TextInput`.
    pub fn predicate(self) -> &'static str {
        match self {
            Self::Global => "Levare",
            Self::Score => "Levare && !Modal && !TextInput",
            Self::Modal => "Levare && Modal",
        }
    }
    pub fn overlaps(self, other: Self) -> bool {
        self == other || self == Self::Global || other == Self::Global
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ready,
    /// Reserved: the key does what Sibelius users expect once implemented and
    /// meanwhile reports that the feature is pending.
    Pending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandSpec {
    pub id: CommandId,
    pub category: Category,
    pub label: &'static str,
    pub scope: Scope,
    /// Whether holding the key repeats the command. Toggles never repeat.
    pub repeat: bool,
    pub status: Status,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    Duplicate(CommandId),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate(id) => write!(f, "el comando «{id}» ya está registrado"),
        }
    }
}

impl std::error::Error for RegistryError {}

/// Ordered set of commands. Built-ins come first; extensions may register
/// more at startup (ids must be unique and are usually namespaced, e.g.
/// `"plugin.mi-extension.accion"`).
#[derive(Clone, Debug)]
pub struct Registry {
    commands: Vec<CommandSpec>,
}

impl Registry {
    pub fn builtin() -> Self {
        Self {
            commands: BUILTIN.to_vec(),
        }
    }
    pub fn register(&mut self, spec: CommandSpec) -> Result<(), RegistryError> {
        if self.get(spec.id.as_str()).is_some() {
            return Err(RegistryError::Duplicate(spec.id));
        }
        self.commands.push(spec);
        Ok(())
    }
    pub fn get(&self, id: &str) -> Option<&CommandSpec> {
        self.commands.iter().find(|spec| spec.id.as_str() == id)
    }
    pub fn iter(&self) -> impl Iterator<Item = &CommandSpec> {
        self.commands.iter()
    }
    pub fn in_category(&self, category: Category) -> impl Iterator<Item = &CommandSpec> {
        self.iter().filter(move |spec| spec.category == category)
    }
}

pub mod id {
    use super::CommandId;
    macro_rules! ids {
        ($($name:ident = $text:literal;)*) => {
            $(pub const $name: CommandId = CommandId($text);)*
        };
    }
    ids! {
        NEW = "archivo.nueva";
        OPEN = "archivo.abrir";
        SAVE = "archivo.guardar";
        SAVE_AS = "archivo.guardar-como";
        CLOSE = "archivo.cerrar";
        QUIT = "archivo.salir";
        PRINT = "archivo.imprimir";
        EXPORT = "archivo.exportar";
        EXPORT_MUSICXML = "archivo.exportar_musicxml";
        SCORE_INFO = "archivo.informacion";
        PREFERENCES = "archivo.preferencias";
        CLEAR_RECENT = "archivo.borrar-recientes";
        CHECK_FOR_UPDATES = "archivo.buscar-actualizaciones";
        RESTART_TO_UPDATE = "archivo.reiniciar-para-actualizar";
        AUTOMATIC_UPDATES = "archivo.actualizaciones-automaticas";
        ABOUT = "archivo.acerca-de";
        SHOW_DIAGNOSTICS = "archivo.registros-y-fallos";
        REPORT_PROBLEM = "archivo.informar-problema";
        CLOSE_PANEL = "panel.cerrar";

        UNDO = "edicion.deshacer";
        REDO = "edicion.rehacer";
        CUT = "edicion.cortar";
        COPY = "edicion.copiar";
        PASTE = "edicion.pegar";
        SELECT_ALL = "edicion.seleccionar-todo";
        DELETE = "edicion.eliminar";
        REMOVE_TIME = "edicion.eliminar-tiempo";
        RESET_POSITION = "edicion.restablecer-posicion";
        ESCAPE = "edicion.escape";
        REPEAT = "edicion.repetir";
        TRANSPOSE = "edicion.transportar";
        FLIP = "edicion.invertir";
        RESPELL = "edicion.enarmonizar";
        CROSS_STAFF_UP = "edicion.pentagrama-superior";
        CROSS_STAFF_DOWN = "edicion.pentagrama-inferior";

        TOGGLE_NOTE_INPUT = "entrada.alternar";
        NOTE_C = "entrada.nota.c";
        NOTE_D = "entrada.nota.d";
        NOTE_E = "entrada.nota.e";
        NOTE_F = "entrada.nota.f";
        NOTE_G = "entrada.nota.g";
        NOTE_A = "entrada.nota.a";
        NOTE_B = "entrada.nota.b";
        ADD_C = "entrada.agregar.c";
        ADD_D = "entrada.agregar.d";
        ADD_E = "entrada.agregar.e";
        ADD_F = "entrada.agregar.f";
        ADD_G = "entrada.agregar.g";
        ADD_A = "entrada.agregar.a";
        ADD_B = "entrada.agregar.b";
        INTERVAL_1 = "entrada.intervalo.1";
        INTERVAL_2 = "entrada.intervalo.2";
        INTERVAL_3 = "entrada.intervalo.3";
        INTERVAL_4 = "entrada.intervalo.4";
        INTERVAL_5 = "entrada.intervalo.5";
        INTERVAL_6 = "entrada.intervalo.6";
        INTERVAL_7 = "entrada.intervalo.7";
        INTERVAL_8 = "entrada.intervalo.8";
        INTERVAL_9 = "entrada.intervalo.9";
        REST = "entrada.silencio";
        CHORD_MODE = "entrada.modo-acorde";
        REMOVE_HIGHEST = "entrada.quitar-nota-aguda";
        ENTRY_OCTAVE_UP = "entrada.octava-mas";
        ENTRY_OCTAVE_DOWN = "entrada.octava-menos";
        DIVISI = "notacion.divisi";
        UNISON = "notacion.unisono";
        DIVIDE_PART = "instrumentos.dividir";
        JOIN_PART = "instrumentos.reunir";
        VOICE_1 = "entrada.voz.1";
        VOICE_2 = "entrada.voz.2";
        VOICE_3 = "entrada.voz.3";
        VOICE_4 = "entrada.voz.4";
        GRACE_NOTE = "entrada.nota-de-adorno";
        FLEXI_TIME = "entrada.tiempo-real";
        STEP_TIME = "entrada.paso-a-paso";
        MIDI_INPUT = "entrada.teclado-midi";

        SIXTY_FOURTH = "duracion.semifusa";
        THIRTY_SECOND = "duracion.fusa";
        SIXTEENTH = "duracion.semicorchea";
        EIGHTH = "duracion.corchea";
        QUARTER = "duracion.negra";
        HALF = "duracion.blanca";
        WHOLE = "duracion.redonda";
        DOT = "duracion.puntillo";
        DOUBLE_DOT = "duracion.doble-puntillo";
        DUPLET = "duracion.dosillo";
        TRIPLET = "duracion.tresillo";
        QUADRUPLET = "duracion.cuatrillo";
        QUINTUPLET = "duracion.quintillo";
        SEXTUPLET = "duracion.seisillo";
        SEPTUPLET = "duracion.septillo";
        OCTUPLET = "duracion.octillo";
        NONUPLET = "duracion.nonillo";
        OTHER_TUPLET = "duracion.grupillo";

        NATURAL = "alteracion.becuadro";
        SHARP = "alteracion.sostenido";
        FLAT = "alteracion.bemol";
        FOLLOW_KEY = "alteracion.armadura";
        DOUBLE_SHARP = "alteracion.doble-sostenido";
        DOUBLE_FLAT = "alteracion.doble-bemol";

        TIE = "expresion.ligadura-union";
        SLUR = "expresion.ligadura-expresion";
        REMOVE_TIE = "expresion.quitar-union";
        REMOVE_SLUR = "expresion.quitar-expresion";
        STACCATO = "expresion.staccato";
        TENUTO = "expresion.tenuto";
        ACCENT = "expresion.acento";
        MARCATO = "expresion.marcato";
        STACCATISSIMO = "expresion.staccatissimo";
        MORDENT = "notacion.mordente";
        INVERTED_MORDENT = "notacion.mordente-superior";
        TURN = "notacion.grupeto";
        BREATH = "notacion.respiracion";
        CAESURA = "notacion.cesura";
        CRESCENDO = "expresion.crescendo";
        DIMINUENDO = "expresion.diminuendo";
        REMOVE_CRESCENDO = "expresion.quitar-crescendo";
        REMOVE_DIMINUENDO = "expresion.quitar-diminuendo";
        OTTAVA_ALTA = "notacion.8va";
        OTTAVA_BASSA = "notacion.8vb";
        PEDAL = "notacion.pedal";
        TRILL = "notacion.trino";
        GLISSANDO = "notacion.glissando";
        PIZZICATO = "expresion.pizzicato";
        ARCO = "expresion.arco";
        HARMONIC_NATURAL = "expresion.armonico-natural";
        ACCIACCATURA = "notacion.acciaccatura";
        APPOGGIATURA = "notacion.appoggiatura";
        HARMONIC_ARTIFICIAL = "expresion.armonico-artificial";
        CLEAR_TECHNIQUE = "expresion.sin-tecnica";
        CLEAR_ARTICULATION = "expresion.sin-articulacion";
        ARPEGGIO_BOTH_HANDS = "expresion.arpegio-ambas-manos";
        ARPEGGIO = "expresion.arpegio";
        ARPEGGIO_UP = "expresion.arpegio-ascendente";
        ARPEGGIO_DOWN = "expresion.arpegio-descendente";
        FERMATA = "expresion.calderon";
        TREMOLO_1 = "expresion.tremolo-1";
        TREMOLO_2 = "expresion.tremolo-2";
        TREMOLO_3 = "expresion.tremolo-3";
        NOTEHEAD_CROSS = "notacion.cabeza-x";
        NOTEHEAD_DIAMOND = "notacion.cabeza-rombo";
        NOTEHEAD_SLASH = "notacion.cabeza-diagonal";
        BEAM_START = "notacion.barra-corchea.iniciar";
        BEAM_JOIN = "notacion.barra-corchea.unir";
        BEAM_OFF = "notacion.barra-corchea.sin";
        DYNAMIC_PPP = "expresion.dinamica.ppp";
        DYNAMIC_PP = "expresion.dinamica.pp";
        DYNAMIC_P = "expresion.dinamica.p";
        DYNAMIC_MP = "expresion.dinamica.mp";
        DYNAMIC_MF = "expresion.dinamica.mf";
        DYNAMIC_F = "expresion.dinamica.f";
        DYNAMIC_FF = "expresion.dinamica.ff";
        DYNAMIC_FFF = "expresion.dinamica.fff";
        DYNAMIC_SFZ = "expresion.dinamica.sfz";
        DYNAMIC_RFZ = "expresion.dinamica.rfz";
        DYNAMIC_FP = "expresion.dinamica.fp";
        DYNAMIC_SFP = "expresion.dinamica.sfp";
        DYNAMIC_NIENTE = "expresion.dinamica.niente";
        CLEAR_DYNAMIC = "expresion.sin-dinamica";
        EXPRESSION_TEXT = "expresion.texto";
        TECHNIQUE_TEXT = "expresion.tecnica";
        TEMPO_TEXT = "expresion.tempo";
        LYRICS = "expresion.letra";
        CHORD_SYMBOL = "expresion.cifrado";
        REHEARSAL_MARK = "expresion.ensayo";
        REHEARSAL_LETTERS = "expresion.ensayo-letras";
        REHEARSAL_NUMBERS = "expresion.ensayo-numeros";
        REHEARSAL_BAR_NUMBERS = "expresion.ensayo-numeros-de-compas";
        CHECK_SCORE = "revision.comprobar-obra";
        CHECK_SELECTION = "revision.comprobar-seleccion";
        REVIEW_ISSUES = "revision.mostrar-avisos";
        REVIEW_PREVIOUS = "revision.aviso-anterior";
        REVIEW_NEXT = "revision.aviso-siguiente";
        SHOW_RANGES = "revision.mostrar-rangos";
        EDIT_RANGE = "revision.editar-rango";
        RESET_RANGE = "revision.restablecer-rango";
        NEW_COMMENT = "revision.nuevo-comentario";
        SHOW_COMMENTS = "revision.mostrar-comentarios";
        RESOLVE_COMMENT = "revision.resolver-comentario";

        KEY_SIGNATURE = "notacion.tonalidad";
        TIME_SIGNATURE = "notacion.compas-de-tiempo";
        CLEF = "notacion.clave";
        LINES = "notacion.lineas";
        SYMBOL = "notacion.simbolo";

        BAR_AT_END = "compases.al-final";
        SINGLE_BAR = "compases.uno";
        DELETE_BAR = "compases.eliminar";
        GO_TO_BAR = "compases.ir-a";
        REPEAT_START = "compases.inicio-repeticion";
        REPEAT_END = "compases.fin-repeticion";
        ENDING_1 = "compases.primera-casilla";
        ENDING_2 = "compases.segunda-casilla";
        REMOVE_ENDING = "compases.quitar-casilla";
        DOUBLE_BARLINE = "compases.doble-barra";
        FINAL_BARLINE = "compases.barra-final";
        DASHED_BARLINE = "compases.barra-discontinua";
        DOTTED_BARLINE = "compases.barra-punteada";
        TICK_BARLINE = "compases.barra-de-marca";
        SHORT_BARLINE = "compases.barra-corta";
        INVISIBLE_BARLINE = "compases.barra-invisible";
        BAR_REPEAT = "compases.repetir-compas";
        SEGNO = "compases.segno";
        CODA = "compases.coda";
        TO_CODA = "compases.al-coda";
        FINE = "compases.fine";
        DA_CAPO = "compases.da-capo";
        DA_CAPO_AL_FINE = "compases.da-capo-al-fine";
        DA_CAPO_AL_CODA = "compases.da-capo-al-coda";
        DAL_SEGNO = "compases.dal-segno";
        DAL_SEGNO_AL_FINE = "compases.dal-segno-al-fine";
        DAL_SEGNO_AL_CODA = "compases.dal-segno-al-coda";
        CLEAR_FORM = "compases.quitar-forma";
        SYSTEM_BREAK = "compases.salto-de-sistema";
        PAGE_BREAK = "compases.salto-de-pagina";
        BAR_NUMBERS_SYSTEM = "compases.numeros.sistema";
        BAR_NUMBERS_EVERY_BAR = "compases.numeros.cada-compas";
        BAR_NUMBERS_EVERY_5 = "compases.numeros.cada-5";
        BAR_NUMBERS_EVERY_10 = "compases.numeros.cada-10";
        BAR_NUMBERS_HIDDEN = "compases.numeros.ninguno";
        PAGE_NUMBERS_FOOTER = "compases.paginas.pie";
        PAGE_NUMBERS_CORNER = "compases.paginas.esquina";
        PAGE_NUMBERS_HIDDEN = "compases.paginas.ninguno";
        PAPER_AUTOMATIC = "pagina.papel.automatico";
        PAPER_A4 = "pagina.papel.a4";
        PAPER_LETTER = "pagina.papel.carta";
        PAPER_A3 = "pagina.papel.a3";
        PAPER_TABLOID = "pagina.papel.tabloide";
        LANDSCAPE = "pagina.horizontal";
        HIDE_EMPTY_STAVES = "pagina.ocultar-pentagramas-vacios";
        MULTIRESTS = "pagina.silencios-de-varios-compases";
        MARGINS_NORMAL = "pagina.margenes.normales";
        MARGINS_WIDE = "pagina.margenes.anchos";
        MARGINS_EXTRA_WIDE = "pagina.margenes.muy-anchos";
        STAFF_SIZE_AUTOMATIC = "pagina.pentagrama.automatico";
        STAFF_SIZE_7 = "pagina.pentagrama.7";
        STAFF_SIZE_6 = "pagina.pentagrama.6";
        STAFF_SIZE_5 = "pagina.pentagrama.5";
        STAFF_SIZE_4 = "pagina.pentagrama.4";

        ADD_INSTRUMENT = "instrumentos.anadir";
        ADD_PIANO = "instrumentos.piano";
        OSSIA = "instrumentos.ossia";
        REMOVE_INSTRUMENT = "instrumentos.quitar";
        NEXT_INSTRUMENT = "instrumentos.siguiente";
        PREVIOUS_INSTRUMENT = "instrumentos.anterior";
        RIGHT_HAND = "instrumentos.mano-derecha";
        LEFT_HAND = "instrumentos.mano-izquierda";

        EXTEND_PREVIOUS = "navegacion.ampliar-anterior";
        EXTEND_NEXT = "navegacion.ampliar-siguiente";
        PREVIOUS = "navegacion.anterior";
        NEXT = "navegacion.siguiente";
        STEP_UP = "navegacion.grado-arriba";
        STEP_DOWN = "navegacion.grado-abajo";
        OCTAVE_UP = "navegacion.octava-arriba";
        OCTAVE_DOWN = "navegacion.octava-abajo";
        UPPER_NOTE_OR_STAFF = "navegacion.arriba";
        LOWER_NOTE_OR_STAFF = "navegacion.abajo";
        BAR_START = "navegacion.inicio-compas";
        NEXT_BAR = "navegacion.compas-siguiente";

        PLAY = "reproduccion.alternar";
        PLAY_FROM_SELECTION = "reproduccion.desde-seleccion";
        PLAY_FROM_START = "reproduccion.desde-inicio";
        STOP = "reproduccion.detener";
        PLAYLINE_TO_START = "reproduccion.linea-al-inicio";
        LOOP = "reproduccion.bucle";
        REPLAY = "reproduccion.repetir";
        HALF_SPEED = "reproduccion.mitad-velocidad";
        METRONOME = "reproduccion.metronomo";
        COUNT_IN = "reproduccion.cuenta-previa";
        CHORD_PLAYBACK = "reproduccion.cifrados";
        MIXER = "reproduccion.mezclador";
        MUTE_PART = "reproduccion.silenciar-parte";
        SOLO_PART = "reproduccion.solo-parte";
        CLEAR_MUTE_SOLO = "reproduccion.quitar-silencios-solos";
        SOUNDS = "reproduccion.sonidos";
        AUDITION = "reproduccion.audicion";
        TEMPO_UP = "reproduccion.tempo-mas";
        TEMPO_DOWN = "reproduccion.tempo-menos";
        REPEATS = "reproduccion.repeticiones";

        ZOOM_IN = "vista.acercar";
        ZOOM_OUT = "vista.alejar";
        ZOOM_ACTUAL = "vista.tamano-real";
        ZOOM_FIT = "vista.ajustar-pagina";
        FULL_SCREEN = "vista.pantalla-completa";
        TOGGLE_INSTRUMENTS_PANEL = "vista.panel-instrumentos";
        TOGGLE_INSPECTOR = "vista.panel-propiedades";
        TOGGLE_AGENT_PANEL = "vista.panel-agente";
        PAGE_VIEW = "vista.pagina";
        CONTINUOUS_VIEW = "vista.continua";
        SINGLE_SYSTEM_VIEW = "vista.pentagrama";
        CONCERT_PITCH = "vista.altura-de-concierto";
        COMMAND_SEARCH = "vista.buscar-comandos";
        TAB_HOME = "vista.pestana.inicio";
        TAB_NOTE_INPUT = "vista.pestana.entrada";
        TAB_NOTATION = "vista.pestana.notacion";
        TAB_DYNAMICS = "vista.pestana.dinamicas";
        TAB_TEXT = "vista.pestana.texto";
        TAB_PLAYBACK = "vista.pestana.reproduccion";
        TAB_LAYOUT = "vista.pestana.disposicion";
        TAB_REVIEW = "vista.pestana.revision";
        TAB_VIEW = "vista.pestana.vista";
        TOGGLE_RIBBON = "vista.plegar-cinta";
    }
}

const fn spec(
    id: CommandId,
    category: Category,
    label: &'static str,
    scope: Scope,
    repeat: bool,
    status: Status,
) -> CommandSpec {
    CommandSpec {
        id,
        category,
        label,
        scope,
        repeat,
        status,
    }
}

use Category as C;
use Scope::{Global as G, Modal as M, Score as S};
use Status::{Pending as P, Ready as R};

#[rustfmt::skip]
const BUILTIN: &[CommandSpec] = &[
    spec(id::NEW, C::File, "Nueva partitura", G, false, R),
    spec(id::OPEN, C::File, "Abrir…", G, false, R),
    spec(id::SAVE, C::File, "Guardar", G, false, R),
    spec(id::SAVE_AS, C::File, "Guardar como…", G, false, R),
    spec(id::CLOSE, C::File, "Cerrar ventana", G, false, R),
    spec(id::QUIT, C::File, "Salir de Levare", G, false, R),
    spec(id::PRINT, C::File, "Imprimir…", G, false, P),
    spec(id::EXPORT, C::File, "Exportar partitura y partes…", G, false, R),
    spec(id::EXPORT_MUSICXML, C::File, "Exportar MusicXML…", G, false, R),
    spec(id::SCORE_INFO, C::File, "Información de la partitura…", G, false, R),
    spec(id::PREFERENCES, C::File, "Preferencias de atajos…", G, false, R),
    spec(id::CLEAR_RECENT, C::File, "Borrar la lista de recientes", G, false, R),
    spec(id::CHECK_FOR_UPDATES, C::File, "Buscar actualizaciones…", G, false, R),
    spec(id::RESTART_TO_UPDATE, C::File, "Reiniciar para actualizar", G, false, R),
    spec(id::AUTOMATIC_UPDATES, C::File, "Activar o desactivar la búsqueda automática de actualizaciones", G, false, R),
    spec(id::ABOUT, C::File, "Acerca de Levare…", G, false, R),
    spec(id::SHOW_DIAGNOSTICS, C::File, "Mostrar registros e informes de fallos", G, false, R),
    spec(id::REPORT_PROBLEM, C::File, "Informar de un problema…", G, false, R),
    spec(id::CLOSE_PANEL, C::File, "Cerrar el panel abierto", M, false, R),

    spec(id::UNDO, C::Edit, "Deshacer", G, true, R),
    spec(id::REDO, C::Edit, "Rehacer", G, true, R),
    spec(id::CUT, C::Edit, "Cortar", G, false, R),
    spec(id::COPY, C::Edit, "Copiar", G, false, R),
    spec(id::PASTE, C::Edit, "Pegar", G, true, R),
    spec(id::SELECT_ALL, C::Edit, "Seleccionar todo el pentagrama", G, false, R),
    spec(id::DELETE, C::Edit, "Convertir en silencios", S, true, R),
    spec(id::REMOVE_TIME, C::Edit, "Eliminar notas y su tiempo", G, false, R),
    spec(id::RESET_POSITION, C::Edit, "Restablecer posición", S, false, R),
    spec(id::ESCAPE, C::Edit, "Detener, cancelar o quitar la selección", S, false, R),
    spec(id::REPEAT, C::Edit, "Repetir la nota, acorde o silencio", S, true, R),
    spec(id::TRANSPOSE, C::Edit, "Transportar…", S, false, R),
    spec(id::FLIP, C::Edit, "Invertir plica o ligadura", S, false, R),
    spec(id::RESPELL, C::Edit, "Enarmonizar", S, false, R),
    spec(id::CROSS_STAFF_UP, C::Edit, "Pasar al pentagrama superior", S, false, R),
    spec(id::CROSS_STAFF_DOWN, C::Edit, "Pasar al pentagrama inferior", S, false, R),

    spec(id::TOGGLE_NOTE_INPUT, C::NoteInput, "Activar o desactivar la entrada de notas", S, false, R),
    spec(id::NOTE_C, C::NoteInput, "Escribir Do", S, true, R),
    spec(id::NOTE_D, C::NoteInput, "Escribir Re", S, true, R),
    spec(id::NOTE_E, C::NoteInput, "Escribir Mi", S, true, R),
    spec(id::NOTE_F, C::NoteInput, "Escribir Fa", S, true, R),
    spec(id::NOTE_G, C::NoteInput, "Escribir Sol", S, true, R),
    spec(id::NOTE_A, C::NoteInput, "Escribir La", S, true, R),
    spec(id::NOTE_B, C::NoteInput, "Escribir Si", S, true, R),
    spec(id::ADD_C, C::NoteInput, "Añadir Do encima", S, false, R),
    spec(id::ADD_D, C::NoteInput, "Añadir Re encima", S, false, R),
    spec(id::ADD_E, C::NoteInput, "Añadir Mi encima", S, false, R),
    spec(id::ADD_F, C::NoteInput, "Añadir Fa encima", S, false, R),
    spec(id::ADD_G, C::NoteInput, "Añadir Sol encima", S, false, R),
    spec(id::ADD_A, C::NoteInput, "Añadir La encima", S, false, R),
    spec(id::ADD_B, C::NoteInput, "Añadir Si encima", S, false, R),
    spec(id::INTERVAL_1, C::NoteInput, "Añadir unísono", S, false, R),
    spec(id::INTERVAL_2, C::NoteInput, "Añadir una segunda encima", S, false, R),
    spec(id::INTERVAL_3, C::NoteInput, "Añadir una tercera encima", S, false, R),
    spec(id::INTERVAL_4, C::NoteInput, "Añadir una cuarta encima", S, false, R),
    spec(id::INTERVAL_5, C::NoteInput, "Añadir una quinta encima", S, false, R),
    spec(id::INTERVAL_6, C::NoteInput, "Añadir una sexta encima", S, false, R),
    spec(id::INTERVAL_7, C::NoteInput, "Añadir una séptima encima", S, false, R),
    spec(id::INTERVAL_8, C::NoteInput, "Añadir una octava encima", S, false, R),
    spec(id::INTERVAL_9, C::NoteInput, "Añadir una novena encima", S, false, R),
    spec(id::REST, C::NoteInput, "Escribir un silencio", S, true, R),
    spec(id::CHORD_MODE, C::NoteInput, "Modo acorde", S, false, R),
    spec(id::REMOVE_HIGHEST, C::NoteInput, "Quitar la nota más aguda del acorde", S, false, R),
    spec(id::ENTRY_OCTAVE_UP, C::NoteInput, "Subir la octava de entrada", S, false, R),
    spec(id::ENTRY_OCTAVE_DOWN, C::NoteInput, "Bajar la octava de entrada", S, false, R),
    spec(id::DIVISI, C::Notation, "Indicación div.", S, false, R),
    spec(id::UNISON, C::Notation, "Indicación unis.", S, false, R),
    spec(id::DIVIDE_PART, C::Instruments, "Dividir el pasaje en dos pentagramas", S, false, R),
    spec(id::JOIN_PART, C::Instruments, "Volver a un pentagrama desde esta posición", S, false, R),
    spec(id::VOICE_1, C::NoteInput, "Voz 1", S, false, R),
    spec(id::VOICE_2, C::NoteInput, "Voz 2", S, false, R),
    spec(id::VOICE_3, C::NoteInput, "Voz 3", S, false, R),
    spec(id::VOICE_4, C::NoteInput, "Voz 4", S, false, R),
    spec(id::GRACE_NOTE, C::NoteInput, "Añadir nota de adorno", S, false, R),
    spec(id::ACCIACCATURA, C::NoteInput, "Acciaccatura", S, false, R),
    spec(id::APPOGGIATURA, C::NoteInput, "Appoggiatura", S, false, R),
    spec(id::FLEXI_TIME, C::NoteInput, "Entrada en tiempo real", G, false, P),
    spec(id::STEP_TIME, C::NoteInput, "Entrada paso a paso", S, false, R),
    spec(id::MIDI_INPUT, C::NoteInput, "Entrada con teclado MIDI", G, false, P),

    spec(id::SIXTY_FOURTH, C::Duration, "Semifusa", S, false, R),
    spec(id::THIRTY_SECOND, C::Duration, "Fusa", S, false, R),
    spec(id::SIXTEENTH, C::Duration, "Semicorchea", S, false, R),
    spec(id::EIGHTH, C::Duration, "Corchea", S, false, R),
    spec(id::QUARTER, C::Duration, "Negra", S, false, R),
    spec(id::HALF, C::Duration, "Blanca", S, false, R),
    spec(id::WHOLE, C::Duration, "Redonda", S, false, R),
    spec(id::DUPLET, C::Duration, "Dosillo (2:3)", S, false, R),
    spec(id::TRIPLET, C::Duration, "Tresillo (3:2)", S, false, R),
    spec(id::QUADRUPLET, C::Duration, "Cuatrillo (4:3)", S, false, R),
    spec(id::QUINTUPLET, C::Duration, "Quintillo (5:4)", S, false, R),
    spec(id::SEXTUPLET, C::Duration, "Seisillo (6:4)", S, false, R),
    spec(id::SEPTUPLET, C::Duration, "Septillo (7:4)", S, false, R),
    spec(id::OCTUPLET, C::Duration, "Octillo (8:6)", S, false, R),
    spec(id::NONUPLET, C::Duration, "Nonillo (9:8)", S, false, R),
    spec(id::OTHER_TUPLET, C::Duration, "Otro grupillo…", S, false, R),
    spec(id::DOT, C::Duration, "Puntillo", S, false, R),
    spec(id::DOUBLE_DOT, C::Duration, "Doble puntillo", S, false, R),

    spec(id::NATURAL, C::Accidentals, "Becuadro", S, false, R),
    spec(id::SHARP, C::Accidentals, "Sostenido", S, false, R),
    spec(id::FLAT, C::Accidentals, "Bemol", S, false, R),
    spec(id::DOUBLE_SHARP, C::Accidentals, "Doble sostenido", S, false, R),
    spec(id::DOUBLE_FLAT, C::Accidentals, "Doble bemol", S, false, R),
    spec(id::FOLLOW_KEY, C::Accidentals, "Seguir la armadura", S, false, R),

    spec(id::TIE, C::Expression, "Ligadura de unión con la siguiente", S, false, R),
    spec(id::SLUR, C::Expression, "Inicio o fin de ligadura de expresión", S, false, R),
    spec(id::REMOVE_TIE, C::Expression, "Quitar ligadura de unión", S, false, R),
    spec(id::REMOVE_SLUR, C::Expression, "Quitar ligadura de expresión", S, false, R),
    spec(id::STACCATO, C::Expression, "Staccato", S, false, R),
    spec(id::TENUTO, C::Expression, "Tenuto", S, false, R),
    spec(id::ACCENT, C::Expression, "Acento", S, false, R),
    spec(id::MARCATO, C::Expression, "Marcato", S, false, R),
    spec(id::STACCATISSIMO, C::Expression, "Staccatissimo", S, false, R),
    spec(id::MORDENT, C::Notation, "Mordente", S, false, R),
    spec(id::INVERTED_MORDENT, C::Notation, "Mordente superior", S, false, R),
    spec(id::TURN, C::Notation, "Grupeto", S, false, R),
    spec(id::BREATH, C::Notation, "Respiración", S, false, R),
    spec(id::CAESURA, C::Notation, "Cesura", S, false, R),
    spec(id::CRESCENDO, C::Expression, "Crescendo: inicio o fin", S, false, R),
    spec(id::DIMINUENDO, C::Expression, "Diminuendo: inicio o fin", S, false, R),
    spec(id::REMOVE_CRESCENDO, C::Expression, "Quitar crescendo", S, false, R),
    spec(id::REMOVE_DIMINUENDO, C::Expression, "Quitar diminuendo", S, false, R),
    spec(id::OTTAVA_ALTA, C::Notation, "Octava alta 8va", S, false, R),
    spec(id::OTTAVA_BASSA, C::Notation, "Octava baja 8vb", S, false, R),
    spec(id::PEDAL, C::Notation, "Pedal de resonancia", S, false, R),
    spec(id::TRILL, C::Notation, "Trino", S, false, R),
    spec(id::GLISSANDO, C::Notation, "Glissando hasta la nota siguiente", S, false, R),
    spec(id::PIZZICATO, C::Expression, "Pizzicato", S, false, R),
    spec(id::ARCO, C::Expression, "Arco", S, false, R),
    spec(id::HARMONIC_NATURAL, C::Expression, "Armónico natural", S, false, R),
    spec(id::HARMONIC_ARTIFICIAL, C::Expression, "Armónico artificial", S, false, R),
    spec(id::CLEAR_TECHNIQUE, C::Expression, "Quitar indicación de técnica", S, false, R),
    spec(id::CLEAR_ARTICULATION, C::Expression, "Quitar articulación", S, false, R),
    spec(id::ARPEGGIO_BOTH_HANDS, C::Expression, "Arpegio en ambas manos", S, false, R),
    spec(id::ARPEGGIO, C::Expression, "Arpegio", S, false, R),
    spec(id::ARPEGGIO_UP, C::Expression, "Arpegio ascendente", S, false, R),
    spec(id::ARPEGGIO_DOWN, C::Expression, "Arpegio descendente", S, false, R),
    spec(id::FERMATA, C::Expression, "Calderón", S, false, R),
    spec(id::TREMOLO_1, C::Expression, "Trémolo de una barra", S, false, R),
    spec(id::TREMOLO_2, C::Expression, "Trémolo de dos barras", S, false, R),
    spec(id::TREMOLO_3, C::Expression, "Trémolo de tres barras", S, false, R),
    spec(id::NOTEHEAD_CROSS, C::Notation, "Cabeza en x", S, false, R),
    spec(id::NOTEHEAD_DIAMOND, C::Notation, "Cabeza de rombo", S, false, R),
    spec(id::NOTEHEAD_SLASH, C::Notation, "Cabeza diagonal", S, false, R),
    spec(id::BEAM_START, C::Notation, "Iniciar barra de corchea", S, false, R),
    spec(id::BEAM_JOIN, C::Notation, "Unir a la barra anterior", S, false, R),
    spec(id::BEAM_OFF, C::Notation, "Sin barra de corchea", S, false, R),
    spec(id::DYNAMIC_PPP, C::Expression, "Dinámica ppp", S, false, R),
    spec(id::DYNAMIC_PP, C::Expression, "Dinámica pp", S, false, R),
    spec(id::DYNAMIC_P, C::Expression, "Dinámica p", S, false, R),
    spec(id::DYNAMIC_MP, C::Expression, "Dinámica mp", S, false, R),
    spec(id::DYNAMIC_MF, C::Expression, "Dinámica mf", S, false, R),
    spec(id::DYNAMIC_F, C::Expression, "Dinámica f", S, false, R),
    spec(id::DYNAMIC_FF, C::Expression, "Dinámica ff", S, false, R),
    spec(id::DYNAMIC_FFF, C::Expression, "Dinámica fff", S, false, R),
    spec(id::DYNAMIC_SFZ, C::Expression, "Dinámica sfz", S, false, R),
    spec(id::DYNAMIC_RFZ, C::Expression, "Dinámica rfz", S, false, R),
    spec(id::DYNAMIC_FP, C::Expression, "Dinámica fp", S, false, R),
    spec(id::DYNAMIC_SFP, C::Expression, "Dinámica sfp", S, false, R),
    spec(id::DYNAMIC_NIENTE, C::Expression, "Dinámica niente", S, false, R),
    spec(id::CLEAR_DYNAMIC, C::Expression, "Quitar dinámica", S, false, R),
    spec(id::EXPRESSION_TEXT, C::Expression, "Texto de expresión…", S, false, R),
    spec(id::TECHNIQUE_TEXT, C::Expression, "Texto de técnica…", S, false, R),
    spec(id::TEMPO_TEXT, C::Expression, "Indicación de tempo…", G, false, R),
    spec(id::LYRICS, C::Expression, "Letra…", S, false, R),
    spec(id::CHORD_SYMBOL, C::Expression, "Cifrado de acordes…", S, false, R),
    spec(id::REHEARSAL_MARK, C::Expression, "Marca de ensayo", G, false, R),
    spec(id::REHEARSAL_LETTERS, C::Expression, "Marcas de ensayo con letras", G, false, R),
    spec(id::REHEARSAL_NUMBERS, C::Expression, "Marcas de ensayo con números", G, false, R),
    spec(id::REHEARSAL_BAR_NUMBERS, C::Expression, "Marcas de ensayo con números de compás", G, false, R),

    spec(id::KEY_SIGNATURE, C::Notation, "Tonalidad…", S, false, R),
    spec(id::TIME_SIGNATURE, C::Notation, "Compás de tiempo…", S, false, R),
    spec(id::CLEF, C::Notation, "Clave…", G, false, R),
    spec(id::LINES, C::Notation, "Líneas…", S, false, P),
    spec(id::SYMBOL, C::Notation, "Símbolo…", S, false, P),

    spec(id::BAR_AT_END, C::Bars, "Añadir un compás al final", G, true, R),
    spec(id::SINGLE_BAR, C::Bars, "Insertar un compás antes del actual", G, false, R),
    spec(id::DELETE_BAR, C::Bars, "Eliminar compases seleccionados", G, false, R),
    spec(id::GO_TO_BAR, C::Bars, "Ir al compás…", G, false, R),
    spec(id::REPEAT_START, C::Bars, "Inicio de repetición", G, false, R),
    spec(id::REPEAT_END, C::Bars, "Fin de repetición", G, false, R),
    spec(id::ENDING_1, C::Bars, "Primera casilla", G, false, R),
    spec(id::ENDING_2, C::Bars, "Segunda casilla", G, false, R),
    spec(id::REMOVE_ENDING, C::Bars, "Quitar casilla", G, false, R),
    spec(id::DOUBLE_BARLINE, C::Bars, "Doble barra", G, false, R),
    spec(id::FINAL_BARLINE, C::Bars, "Barra final", G, false, R),
    spec(id::DASHED_BARLINE, C::Bars, "Barra discontinua", G, false, R),
    spec(id::DOTTED_BARLINE, C::Bars, "Barra punteada", G, false, R),
    spec(id::TICK_BARLINE, C::Bars, "Barra de marca", G, false, R),
    spec(id::SHORT_BARLINE, C::Bars, "Barra corta", G, false, R),
    spec(id::INVISIBLE_BARLINE, C::Bars, "Barra invisible", G, false, R),
    spec(id::BAR_REPEAT, C::Bars, "Compás de repetición (%)", S, false, R),
    spec(id::SEGNO, C::Bars, "Segno", G, false, R),
    spec(id::CODA, C::Bars, "Coda", G, false, R),
    spec(id::TO_CODA, C::Bars, "To Coda", G, false, R),
    spec(id::FINE, C::Bars, "Fine", G, false, R),
    spec(id::DA_CAPO, C::Bars, "D.C.", G, false, R),
    spec(id::DA_CAPO_AL_FINE, C::Bars, "D.C. al Fine", G, false, R),
    spec(id::DA_CAPO_AL_CODA, C::Bars, "D.C. al Coda", G, false, R),
    spec(id::DAL_SEGNO, C::Bars, "D.S.", G, false, R),
    spec(id::DAL_SEGNO_AL_FINE, C::Bars, "D.S. al Fine", G, false, R),
    spec(id::DAL_SEGNO_AL_CODA, C::Bars, "D.S. al Coda", G, false, R),
    spec(id::CLEAR_FORM, C::Bars, "Quitar repeticiones y saltos", G, false, R),
    spec(id::SYSTEM_BREAK, C::Bars, "Salto de sistema tras el compás", G, false, R),
    spec(id::PAGE_BREAK, C::Bars, "Salto de página tras el compás", G, false, R),
    spec(id::BAR_NUMBERS_SYSTEM, C::Bars, "Número de compás en cada sistema", G, false, R),
    spec(id::BAR_NUMBERS_EVERY_BAR, C::Bars, "Número en cada compás", G, false, R),
    spec(id::BAR_NUMBERS_EVERY_5, C::Bars, "Número de compás cada 5 compases", G, false, R),
    spec(id::BAR_NUMBERS_EVERY_10, C::Bars, "Número de compás cada 10 compases", G, false, R),
    spec(id::BAR_NUMBERS_HIDDEN, C::Bars, "Sin números de compás", G, false, R),
    spec(id::PAGE_NUMBERS_FOOTER, C::Bars, "Número de página al pie", G, false, R),
    spec(id::PAGE_NUMBERS_CORNER, C::Bars, "Número de página en la esquina", G, false, R),
    spec(id::PAGE_NUMBERS_HIDDEN, C::Bars, "Sin números de página", G, false, R),
    spec(id::PAPER_AUTOMATIC, C::Bars, "Papel automático", G, false, R),
    spec(id::PAPER_A4, C::Bars, "Papel A4", G, false, R),
    spec(id::PAPER_LETTER, C::Bars, "Papel carta", G, false, R),
    spec(id::PAPER_A3, C::Bars, "Papel A3", G, false, R),
    spec(id::PAPER_TABLOID, C::Bars, "Papel tabloide", G, false, R),
    spec(id::LANDSCAPE, C::Bars, "Página horizontal", G, false, R),
    spec(id::HIDE_EMPTY_STAVES, C::Bars, "Ocultar pentagramas vacíos", G, false, R),
    spec(id::MULTIRESTS, C::Bars, "Silencios de varios compases", G, false, R),
    spec(id::MARGINS_NORMAL, C::Bars, "Márgenes normales", G, false, R),
    spec(id::MARGINS_WIDE, C::Bars, "Márgenes anchos", G, false, R),
    spec(id::MARGINS_EXTRA_WIDE, C::Bars, "Márgenes muy anchos", G, false, R),
    spec(id::STAFF_SIZE_AUTOMATIC, C::Bars, "Tamaño de pentagrama automático", G, false, R),
    spec(id::STAFF_SIZE_7, C::Bars, "Pentagrama de 7 mm", G, false, R),
    spec(id::STAFF_SIZE_6, C::Bars, "Pentagrama de 6 mm", G, false, R),
    spec(id::STAFF_SIZE_5, C::Bars, "Pentagrama de 5 mm", G, false, R),
    spec(id::STAFF_SIZE_4, C::Bars, "Pentagrama de 4 mm", G, false, R),

    spec(id::ADD_INSTRUMENT, C::Instruments, "Añadir instrumentos…", S, false, R),
    spec(id::ADD_PIANO, C::Instruments, "Añadir piano a dos manos", G, false, R),
    spec(id::OSSIA, C::Instruments, "Ossia", G, false, P),
    spec(id::REMOVE_INSTRUMENT, C::Instruments, "Eliminar el instrumento seleccionado", G, false, R),
    spec(id::NEXT_INSTRUMENT, C::Instruments, "Instrumento siguiente", G, true, R),
    spec(id::PREVIOUS_INSTRUMENT, C::Instruments, "Instrumento anterior", G, true, R),
    spec(id::RIGHT_HAND, C::Instruments, "Piano: mano derecha", G, false, R),
    spec(id::LEFT_HAND, C::Instruments, "Piano: mano izquierda", G, false, R),

    spec(id::EXTEND_PREVIOUS, C::Navigation, "Ampliar selección hacia atrás", S, true, R),
    spec(id::EXTEND_NEXT, C::Navigation, "Ampliar selección hacia delante", S, true, R),
    spec(id::PREVIOUS, C::Navigation, "Elemento anterior", S, true, R),
    spec(id::NEXT, C::Navigation, "Elemento siguiente", S, true, R),
    spec(id::STEP_UP, C::Navigation, "Subir un grado", S, true, R),
    spec(id::STEP_DOWN, C::Navigation, "Bajar un grado", S, true, R),
    spec(id::OCTAVE_UP, C::Navigation, "Subir una octava", S, true, R),
    spec(id::OCTAVE_DOWN, C::Navigation, "Bajar una octava", S, true, R),
    spec(id::UPPER_NOTE_OR_STAFF, C::Navigation, "Nota superior del acorde o pentagrama de arriba", S, true, R),
    spec(id::LOWER_NOTE_OR_STAFF, C::Navigation, "Nota inferior del acorde o pentagrama de abajo", S, true, R),
    spec(id::BAR_START, C::Navigation, "Inicio del compás", S, true, R),
    spec(id::NEXT_BAR, C::Navigation, "Compás siguiente", S, true, R),

    spec(id::PLAY, C::Playback, "Reproducir, pausar o continuar", S, false, R),
    spec(id::PLAY_FROM_SELECTION, C::Playback, "Reproducir desde la selección", S, false, R),
    spec(id::PLAY_FROM_START, C::Playback, "Reproducir desde el inicio", G, false, R),
    spec(id::STOP, C::Playback, "Detener la reproducción", G, false, R),
    spec(id::PLAYLINE_TO_START, C::Playback, "Llevar la línea de reproducción al inicio", G, false, R),
    spec(id::LOOP, C::Playback, "Reproducción en bucle", S, false, R),
    spec(id::REPLAY, C::Playback, "Repetir la última reproducción", S, false, R),
    spec(id::HALF_SPEED, C::Playback, "Reproducir a media velocidad", S, false, R),
    spec(id::METRONOME, C::Playback, "Metrónomo", G, false, R),
    spec(id::COUNT_IN, C::Playback, "Cuenta previa de un compás", G, false, R),
    spec(id::CHORD_PLAYBACK, C::Playback, "Hacer sonar los cifrados", S, false, R),
    spec(id::MIXER, C::Playback, "Mezcla de las partes", S, false, R),
    spec(id::MUTE_PART, C::Playback, "Silenciar parte", S, false, R),
    spec(id::SOLO_PART, C::Playback, "Solo de la parte", S, false, R),
    spec(id::CLEAR_MUTE_SOLO, C::Playback, "Quitar silencios y solos", S, false, R),
    spec(id::SOUNDS, C::Playback, "Mostrar el sonido", S, false, R),
    spec(id::AUDITION, C::Playback, "Audición al escribir", G, false, R),
    spec(id::TEMPO_UP, C::Playback, "Subir el tempo", G, true, R),
    spec(id::TEMPO_DOWN, C::Playback, "Bajar el tempo", G, true, R),
    spec(id::REPEATS, C::Playback, "Reproducir repeticiones", G, false, R),

    spec(id::ZOOM_IN, C::View, "Acercar", G, true, R),
    spec(id::CHECK_SCORE, C::Review, "Comprobar toda la partitura", G, false, R),
    spec(id::NEW_COMMENT, C::Review, "Nuevo comentario sobre la selección…", G, false, R),
    spec(id::SHOW_COMMENTS, C::Review, "Mostrar comentarios", G, false, R),
    spec(id::RESOLVE_COMMENT, C::Review, "Resolver o reabrir el comentario activo", G, false, R),
    spec(id::CHECK_SELECTION, C::Review, "Comprobar la selección", G, false, R),
    spec(id::REVIEW_ISSUES, C::Review, "Mostrar avisos de revisión", G, false, R),
    spec(id::REVIEW_PREVIOUS, C::Review, "Aviso anterior", G, true, R),
    spec(id::REVIEW_NEXT, C::Review, "Aviso siguiente", G, true, R),
    spec(id::SHOW_RANGES, C::Review, "Mostrar notas fuera de rango en rojo", G, false, R),
    spec(id::EDIT_RANGE, C::Review, "Editar rango de la parte activa…", G, false, R),
    spec(id::RESET_RANGE, C::Review, "Restablecer rango de la parte activa", G, false, R),
    spec(id::ZOOM_OUT, C::View, "Alejar", G, true, R),
    spec(id::ZOOM_ACTUAL, C::View, "Tamaño real (100 %)", G, false, R),
    spec(id::ZOOM_FIT, C::View, "Ajustar a la página", G, false, R),
    spec(id::FULL_SCREEN, C::View, "Pantalla completa", G, false, R),
    spec(id::TOGGLE_INSTRUMENTS_PANEL, C::View, "Mostrar u ocultar las partes", G, false, R),
    spec(id::TOGGLE_INSPECTOR, C::View, "Mostrar u ocultar el panel derecho", G, false, R),
    spec(id::TOGGLE_AGENT_PANEL, C::View, "Panel de agente", G, false, R),
    spec(id::PAGE_VIEW, C::View, "Vista de página", G, false, R),
    spec(id::CONTINUOUS_VIEW, C::View, "Vista continua", G, false, R),
    spec(id::SINGLE_SYSTEM_VIEW, C::View, "Vista de pentagrama", G, false, R),
    spec(id::CONCERT_PITCH, C::View, "Partitura en Do (altura de concierto)", G, false, R),
    spec(id::COMMAND_SEARCH, C::View, "Buscar comandos…", G, false, R),
    spec(id::TAB_HOME, C::View, "Pestaña Inicio", G, false, R),
    spec(id::TAB_NOTE_INPUT, C::View, "Pestaña Entrada de notas", G, false, R),
    spec(id::TAB_NOTATION, C::View, "Pestaña Notaciones", G, false, R),
    spec(id::TAB_TEXT, C::View, "Pestaña Texto", G, false, R),
    spec(id::TAB_DYNAMICS, C::View, "Pestaña Texto (dinámicas)", G, false, R),
    spec(id::TAB_PLAYBACK, C::View, "Pestaña Reproducción", G, false, R),
    spec(id::TAB_LAYOUT, C::View, "Pestaña Diseño", G, false, R),
    spec(id::TAB_REVIEW, C::View, "Pestaña Revisión", G, false, R),
    spec(id::TAB_VIEW, C::View, "Pestaña Vista", G, false, R),
    spec(id::TOGGLE_RIBBON, C::View, "Plegar o desplegar la cinta", G, false, R),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_ids_are_unique_and_registration_rejects_duplicates() {
        let mut registry = Registry::builtin();
        let mut ids: Vec<_> = registry.iter().map(|spec| spec.id).collect();
        let count = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), count);
        let extension = spec(
            CommandId("plugin.prueba.saludar"),
            Category::Edit,
            "Saludar",
            Scope::Score,
            false,
            Status::Ready,
        );
        registry.register(extension).unwrap();
        assert_eq!(
            registry.register(extension),
            Err(RegistryError::Duplicate(extension.id))
        );
        assert!(registry.get("plugin.prueba.saludar").is_some());
    }

    #[test]
    fn musical_clipboard_commands_are_ready_and_paste_allows_repeated_pastes() {
        let registry = Registry::builtin();
        for command in [
            id::COPY,
            id::CUT,
            id::PASTE,
            id::SELECT_ALL,
            id::EXTEND_NEXT,
            id::EXTEND_PREVIOUS,
        ] {
            assert_eq!(
                registry.get(command.as_str()).unwrap().status,
                Status::Ready
            );
        }
        assert!(registry.get(id::PASTE.as_str()).unwrap().repeat);
        assert!(!registry.get(id::CUT.as_str()).unwrap().repeat);
    }

    #[test]
    fn every_category_has_commands_and_scopes_overlap_as_documented() {
        let registry = Registry::builtin();
        for category in Category::ALL {
            assert!(
                registry.in_category(category).next().is_some(),
                "{category:?}"
            );
        }
        assert!(Scope::Global.overlaps(Scope::Score));
        assert!(Scope::Modal.overlaps(Scope::Global));
        assert!(!Scope::Score.overlaps(Scope::Modal));
    }
}
