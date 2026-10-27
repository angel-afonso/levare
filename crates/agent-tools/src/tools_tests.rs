use super::*;
use score_core::{
    Accidental, Articulation, Connection, ConnectionKind, Duration, Event, Instrument, MarkKind,
    Pitch, Pitches, RehearsalStyle, Score, StaffId, Step, Voice,
};
use score_edit::{Command, Editor, FormEdit};
use score_keymap::{CommandId, Registry, id};

/// The app's dispatcher reduced to the ribbon's marks, enough to exercise
/// `ejecutar` on a real editor.
struct FakeHost {
    editor: Editor,
}

impl Host for FakeHost {
    fn editor(&self) -> &Editor {
        &self.editor
    }
    fn editor_mut(&mut self) -> &mut Editor {
        &mut self.editor
    }
    fn run(&mut self, id: CommandId) -> Result<(), String> {
        let command = match id {
            id::REHEARSAL_MARK => Command::ToggleRehearsalMark,
            id::REHEARSAL_NUMBERS => Command::SetRehearsalStyle(RehearsalStyle::Numbers),
            _ => Command::ToggleMark(
                score_toolbar::marks::kind(id)
                    .ok_or_else(|| format!("«{id}» no está disponible en la prueba"))?,
            ),
        };
        self.editor
            .execute(command)
            .map(|_| ())
            .map_err(|error| error.0.to_string())
    }
}

fn note(step: Step) -> Event {
    Event {
        notation: Default::default(),
        pitches: Pitches::single(Some(Pitch::new(step, 4).unwrap())),
        duration: Duration::Quarter,
    }
}

/// A violin with `measures` bars, each one C4 D4 E4 F4 in quarters.
fn violin(measures: usize) -> FakeHost {
    let score = Score::new("Prueba".into(), &[Instrument::Violin], measures).unwrap();
    let mut editor = Editor::new(score).unwrap();
    for measure in 0..measures {
        for (beat, step) in [Step::C, Step::D, Step::E, Step::F].into_iter().enumerate() {
            editor
                .execute(Command::WriteAt {
                    measure,
                    tick: beat as u16 * 48,
                    event: note(step),
                })
                .unwrap();
        }
    }
    FakeHost { editor }
}

fn quartet() -> FakeHost {
    let bytes = include_bytes!("../../../examples/cuarteto-en-sol.partitura");
    let score = score_io::decode(bytes).unwrap();
    FakeHost {
        editor: Editor::new(score).unwrap(),
    }
}

fn at(parte: &str, compas: u32, tiempo: f32) -> Position {
    Position {
        parte: parte.into(),
        compas,
        tiempo,
        pentagrama: None,
        voz: None,
    }
}

fn run(host: &mut FakeHost, call: ToolCall) -> Result<String, String> {
    dispatch(host, &Registry::builtin(), call)
}

fn connections(host: &FakeHost, kind: ConnectionKind) -> Vec<Connection> {
    host.editor
        .score()
        .connections()
        .iter()
        .filter(|c| c.kind == kind)
        .copied()
        .collect()
}

#[test]
fn select_then_toggle_reports_before_after() {
    let mut host = violin(2);
    let selected = run(
        &mut host,
        ToolCall::Seleccionar(SeleccionarArgs {
            desde: at("violín", 1, 2.),
            hasta: None,
        }),
    )
    .unwrap();
    assert!(selected.contains("»D4:4"), "{selected}");
    let staccato = MarkKind::Articulation(Articulation::Staccato).label();
    let toggle = |host: &mut FakeHost| {
        run(
            host,
            ToolCall::Ejecutar(EjecutarArgs {
                comando: id::STACCATO.as_str().into(),
            }),
        )
        .unwrap()
    };
    // The reply's first line names the command, so look past it.
    let sections = |reply: &str| -> (String, String) {
        let (_, rest) = reply.split_once("antes:").expect(reply);
        let (before, after) = rest.split_once("después:").expect(reply);
        (before.into(), after.into())
    };
    let first = toggle(&mut host);
    let (before, after) = sections(&first);
    assert!(!before.contains(staccato), "{first}");
    assert!(after.contains(staccato), "{first}");
    let second = toggle(&mut host);
    let (before, after) = sections(&second);
    assert!(before.contains(staccato), "{second}");
    assert!(!after.contains(staccato), "{second}");
}

#[test]
fn execute_accepts_the_exact_label_and_reports_no_change() {
    let mut host = violin(1);
    run(
        &mut host,
        ToolCall::Seleccionar(SeleccionarArgs {
            desde: at("1", 1, 1.),
            hasta: None,
        }),
    )
    .unwrap();
    let label = Registry::builtin()
        .get(id::ACCENT.as_str())
        .unwrap()
        .label
        .to_string();
    let changed = run(
        &mut host,
        ToolCall::Ejecutar(EjecutarArgs { comando: label }),
    )
    .unwrap();
    assert!(changed.contains("después:"), "{changed}");
    // The host refuses it: the error reaches the agent as it is.
    let refused = run(
        &mut host,
        ToolCall::Ejecutar(EjecutarArgs {
            comando: id::NEXT.as_str().into(),
        }),
    )
    .unwrap_err();
    assert!(
        refused.contains("no está disponible en la prueba"),
        "{refused}"
    );
    let excluded = run(
        &mut host,
        ToolCall::Ejecutar(EjecutarArgs {
            comando: id::UNDO.as_str().into(),
        }),
    )
    .unwrap_err();
    assert!(excluded.contains("edicion.deshacer"), "{excluded}");
    let unknown = run(
        &mut host,
        ToolCall::Ejecutar(EjecutarArgs {
            comando: "nada.de.esto".into(),
        }),
    )
    .unwrap_err();
    assert!(unknown.contains("comandos"), "{unknown}");
}

#[test]
fn write_measures_appends_and_ties() {
    let mut host = violin(2);
    let reply = run(
        &mut host,
        ToolCall::Escribir(EscribirArgs {
            parte: "violín".into(),
            pentagrama: None,
            voz: None,
            desde_compas: 3,
            compases: vec!["E4:2 G4:2~".into(), "G4:4 R:4 [C4 E4]:2".into()],
        }),
    )
    .unwrap();
    let score = host.editor.score();
    assert_eq!(score.measures().len(), 4, "{reply}");
    let ties = connections(&host, ConnectionKind::Tie);
    assert!(
        ties.iter()
            .any(|c| c.start.measure == 2 && c.end.measure == 3),
        "{ties:?}"
    );
    let part = score.parts()[0].id();
    assert_eq!(
        score_text::format_voice(score, part, StaffId::Upper, 3, Voice::One),
        "G4:4 R:4 [C4 E4]:2"
    );
    // The whole call is one undo step.
    assert!(host.editor.undo());
    assert_eq!(host.editor.score().measures().len(), 2);
}

#[test]
fn write_rejects_overfull_without_applying() {
    let mut host = violin(4);
    let before = host.editor.score().clone();
    let error = run(
        &mut host,
        ToolCall::Escribir(EscribirArgs {
            parte: "1".into(),
            pentagrama: None,
            voz: None,
            desde_compas: 1,
            compases: vec!["C5:1".into(), "C5:2 D5:2 E5:8".into(), "C5:1".into()],
        }),
    )
    .unwrap_err();
    assert!(error.contains("compás 2"), "{error}");
    assert!(error.contains("4½"), "{error}");
    assert_eq!(host.editor.score(), &before);
}

#[test]
fn write_rolls_back_when_an_edit_fails_midway() {
    let mut host = violin(3);
    let before = host.editor.score().clone();
    // The tie joins different pitches: refused only after bar 1 was written.
    let error = run(
        &mut host,
        ToolCall::Escribir(EscribirArgs {
            parte: "1".into(),
            pentagrama: None,
            voz: None,
            desde_compas: 1,
            compases: vec!["G4:1~".into(), "A4:1".into()],
        }),
    )
    .unwrap_err();
    assert!(error.contains("compás 1"), "{error}");
    assert_eq!(host.editor.score(), &before);
    assert!(host.editor.undo(), "the setup's own writes stay undoable");
}

#[test]
fn write_past_the_end_is_refused() {
    let mut host = violin(2);
    let error = run(
        &mut host,
        ToolCall::Escribir(EscribirArgs {
            parte: "1".into(),
            pentagrama: None,
            voz: None,
            desde_compas: 4,
            compases: vec!["C4:1".into()],
        }),
    )
    .unwrap_err();
    assert!(error.contains("compás 3"), "{error}");
}

#[test]
fn adjust_connection_slur_beats_1_to_3() {
    let mut host = violin(4);
    run(
        &mut host,
        ToolCall::Ajustar(AjustarArgs::Conexion {
            clase: ConnectionClass::Ligadura,
            desde: at("1", 4, 1.),
            hasta: at("1", 4, 3.),
        }),
    )
    .unwrap();
    let score = host.editor.score();
    let part = score.parts()[0].id();
    let event = |tick| score_text::event_at(score, part, StaffId::Upper, Voice::One, 3, tick);
    assert_eq!(
        connections(&host, ConnectionKind::Slur),
        [Connection {
            start: event(0).unwrap(),
            end: event(96).unwrap(),
            kind: ConnectionKind::Slur,
        }]
    );
}

#[test]
fn adjust_signatures_tempo_and_parts() {
    let mut host = violin(4);
    let adjust = |host: &mut FakeHost, args| run(host, ToolCall::Ajustar(args));
    adjust(
        &mut host,
        AjustarArgs::Tonalidad {
            compas: 3,
            quintas: -3,
            menor: false,
        },
    )
    .unwrap();
    assert_eq!(host.editor.score().key_signature_at(2).fifths(), -3);
    assert!(
        adjust(
            &mut host,
            AjustarArgs::Tonalidad {
                compas: 1,
                quintas: 9,
                menor: false,
            },
        )
        .is_err()
    );
    adjust(
        &mut host,
        AjustarArgs::Tempo {
            compas: 2,
            bpm: 72,
            texto: Some("Adagio".into()),
        },
    )
    .unwrap();
    assert_eq!(host.editor.score().tempo_at(1).bpm, 72);
    adjust(
        &mut host,
        AjustarArgs::AnadirPartes {
            instrumentos: vec!["flauta".into(), "clarinete en sib".into()],
        },
    )
    .unwrap();
    assert_eq!(host.editor.score().parts().len(), 3);
    let unknown = adjust(
        &mut host,
        AjustarArgs::AnadirPartes {
            instrumentos: vec!["clarinete".into()],
        },
    )
    .unwrap_err();
    assert!(unknown.contains("Clarinete en La"), "{unknown}");
    adjust(
        &mut host,
        AjustarArgs::Clave {
            parte: "violín".into(),
            pentagrama: None,
            compas: 2,
            clave: "fa".into(),
        },
    )
    .unwrap();
    let violin = host
        .editor
        .score()
        .parts()
        .iter()
        .find(|p| p.instrument() == Instrument::Violin);
    assert_eq!(
        violin.unwrap().clef_at(StaffId::Upper, 1),
        score_core::Clef::Bass
    );
}

#[test]
fn ajustar_reads_its_tag_from_json() {
    let call = ToolCall::parse(
        "ajustar",
        serde_json::json!({
            "tipo": "conexion",
            "clase": "union",
            "desde": {"parte": "1", "compas": 1, "tiempo": 4},
            "hasta": {"parte": "1", "compas": 2},
        }),
    )
    .unwrap();
    let ToolCall::Ajustar(AjustarArgs::Conexion { clase, hasta, .. }) = call else {
        panic!("{call:?}");
    };
    assert_eq!(clase, ConnectionClass::Union);
    assert_eq!(hasta.tiempo, 1.);
    assert!(matches!(
        ToolCall::parse("resumen", serde_json::json!({})),
        Ok(ToolCall::Resumen)
    ));
    assert!(ToolCall::parse("borrar", serde_json::json!({})).is_err());
    let names: Vec<_> = tool_descriptions().iter().map(|(name, _)| *name).collect();
    assert_eq!(
        names,
        [
            "resumen",
            "leer",
            "seleccionar",
            "comandos",
            "ejecutar",
            "escribir",
            "ajustar"
        ]
    );
}

#[test]
fn summary_lists_parts_and_selection() {
    let mut host = quartet();
    run(
        &mut host,
        ToolCall::Seleccionar(SeleccionarArgs {
            desde: at("viola", 2, 1.),
            hasta: None,
        }),
    )
    .unwrap();
    let summary = run(&mut host, ToolCall::Resumen).unwrap();
    for part in [
        "1 Flauta",
        "2 Violín I",
        "3 Violín II",
        "4 Viola",
        "5 Violoncello",
    ] {
        assert!(summary.contains(part), "{part}: {summary}");
    }
    assert!(summary.contains("8 compases"), "{summary}");
    assert!(summary.contains("Sol mayor"), "{summary}");
    let selection = summary
        .lines()
        .find(|line| line.starts_with("Selección"))
        .expect(&summary);
    assert!(selection.contains("Viola"), "{selection}");
    assert!(selection.contains("compás 2"), "{selection}");
}

#[test]
fn read_states_signatures_and_one_line_per_bar() {
    let mut host = quartet();
    let text = run(
        &mut host,
        ToolCall::Leer(LeerArgs {
            partes: Some(vec!["viola".into(), "1".into()]),
            desde: 1,
            hasta: 2,
        }),
    )
    .unwrap();
    assert!(text.contains("4/4"), "{text}");
    assert!(text.contains("Sol mayor"), "{text}");
    assert!(text.contains("4 Viola"), "{text}");
    assert!(text.contains("1 Flauta"), "{text}");
    assert!(!text.contains("Violoncello"), "{text}");
    let bars = text
        .lines()
        .filter(|line| line.trim_start().starts_with("c"))
        .count();
    assert_eq!(bars, 4, "{text}");
}

#[test]
fn read_caps_range() {
    let mut host = violin(2);
    let error = run(
        &mut host,
        ToolCall::Leer(LeerArgs {
            partes: None,
            desde: 1,
            hasta: 40,
        }),
    )
    .unwrap_err();
    assert!(error.contains("32 compases o menos"), "{error}");
}

#[test]
fn commands_are_grouped_by_category() {
    let mut host = violin(1);
    let all = run(
        &mut host,
        ToolCall::Comandos(ComandosArgs { categoria: None }),
    )
    .unwrap();
    assert!(all.contains("Expresión"), "{all}");
    assert!(all.contains("expresion.staccato — Staccato"), "{all}");
    assert!(!all.contains("archivo.guardar"), "{all}");
    let some = run(
        &mut host,
        ToolCall::Comandos(ComandosArgs {
            categoria: Some("reproduccion".into()),
        }),
    )
    .unwrap();
    assert!(some.contains("reproduccion.alternar"), "{some}");
    assert!(!some.contains("expresion.staccato"), "{some}");
}

#[test]
fn instructions_cover_the_rules() {
    for rule in ["resumen", "armadura", "ambigua", "archivos", "idioma"] {
        assert!(INSTRUCTIONS.contains(rule), "{rule}");
    }
    let (_, write) = tool_descriptions()
        .into_iter()
        .find(|(name, _)| *name == "escribir")
        .unwrap();
    assert!(write.contains("armadura"), "{write}");
    // Accidentals do not carry through the bar.
    assert!(INSTRUCTIONS.contains("F#4 G4 F#4"), "{INSTRUCTIONS}");
    assert!(write.contains("F#4 G4 F#4"), "{write}");
    let (_, read) = tool_descriptions()
        .into_iter()
        .find(|(name, _)| *name == "leer")
        .unwrap();
    assert!(
        read.contains("lee solo las partes y compases que necesites"),
        "{read}"
    );
}

#[test]
fn each_note_keeps_its_own_accidental() {
    let mut host = violin(1);
    run(
        &mut host,
        ToolCall::Escribir(EscribirArgs {
            parte: "1".into(),
            pentagrama: None,
            voz: None,
            desde_compas: 1,
            compases: vec!["F#4:4 G4:4 F#4:4 G4:4".into()],
        }),
    )
    .unwrap();
    let score = host.editor.score();
    let part = score.parts()[0].id();
    let bar = &score.parts()[0].staff(StaffId::Upper).unwrap().measures()[0];
    let sharps = bar
        .voice_iter(Voice::One)
        .filter_map(|(_, event)| event.pitches.first())
        .filter(|pitch| pitch.step == Step::F && pitch.accidental() == Some(Accidental::Sharp))
        .count();
    assert_eq!(sharps, 2);
    assert_eq!(
        score_text::format_voice(score, part, StaffId::Upper, 0, Voice::One),
        "F#4:4 G4:4 F#4:4 G4:4"
    );
    let read = run(
        &mut host,
        ToolCall::Leer(LeerArgs {
            partes: None,
            desde: 1,
            hasta: 1,
        }),
    )
    .unwrap();
    assert_eq!(read.matches("F#4").count(), 2, "{read}");
}

#[test]
fn read_refuses_ranges_that_would_overflow() {
    let mut host = violin(2);
    let leer = |host: &mut FakeHost, desde, hasta| {
        run(
            host,
            ToolCall::Leer(LeerArgs {
                partes: None,
                desde,
                hasta,
            }),
        )
    };
    let error = leer(&mut host, 0, u32::MAX).unwrap_err();
    assert!(error.contains("desde 1"), "{error}");
    let error = leer(&mut host, 1, u32::MAX).unwrap_err();
    assert!(error.contains("32 compases o menos"), "{error}");
    let error = leer(&mut host, u32::MAX, 1).unwrap_err();
    assert!(error.contains("al revés"), "{error}");
}

#[test]
fn adjust_connection_trill_and_glissando() {
    let mut host = violin(1);
    for (clase, kind, hasta) in [
        (ConnectionClass::Trino, ConnectionKind::Trill, 2.),
        (ConnectionClass::Glissando, ConnectionKind::Glissando, 4.),
    ] {
        let reply = run(
            &mut host,
            ToolCall::Ajustar(AjustarArgs::Conexion {
                clase,
                desde: at("1", 1, 1.),
                hasta: at("1", 1, hasta),
            }),
        )
        .unwrap();
        assert!(reply.contains(kind.label()), "{reply}");
        assert_eq!(connections(&host, kind).len(), 1, "{reply}");
    }
    let call = ToolCall::parse(
        "ajustar",
        serde_json::json!({
            "tipo": "conexion",
            "clase": "trino",
            "desde": {"parte": "1", "compas": 1, "tiempo": 3},
            "hasta": {"parte": "1", "compas": 1, "tiempo": 3},
        }),
    )
    .unwrap();
    assert!(matches!(
        call,
        ToolCall::Ajustar(AjustarArgs::Conexion {
            clase: ConnectionClass::Trino,
            ..
        })
    ));
}

#[test]
fn rehearsal_marks_and_their_style_show_in_replies() {
    let mut host = violin(3);
    let summary = run(&mut host, ToolCall::Resumen).unwrap();
    assert!(
        summary.contains("Marcas de ensayo: ninguna (estilo: Letras)"),
        "{summary}"
    );
    run(
        &mut host,
        ToolCall::Seleccionar(SeleccionarArgs {
            desde: at("1", 2, 1.),
            hasta: None,
        }),
    )
    .unwrap();
    let ejecutar = |host: &mut FakeHost, command: CommandId| {
        run(
            host,
            ToolCall::Ejecutar(EjecutarArgs {
                comando: command.as_str().into(),
            }),
        )
        .unwrap()
    };
    let added = ejecutar(&mut host, id::REHEARSAL_MARK);
    let (_, after) = added.split_once("después:").expect(&added);
    assert!(after.contains("ensayo A"), "{added}");
    let restyled = ejecutar(&mut host, id::REHEARSAL_NUMBERS);
    let (before, after) = restyled.split_once("después:").expect(&restyled);
    assert!(before.contains("ensayo A"), "{restyled}");
    assert!(after.contains("ensayo 1"), "{restyled}");
    assert!(
        restyled.contains("Estilo de las marcas de ensayo: Números"),
        "{restyled}"
    );
    let summary = run(&mut host, ToolCall::Resumen).unwrap();
    assert!(
        summary.contains("Marcas de ensayo (estilo: Números): 1 en el compás 2"),
        "{summary}"
    );
    // A style change shows even where no mark is in view.
    run(
        &mut host,
        ToolCall::Seleccionar(SeleccionarArgs {
            desde: at("1", 3, 1.),
            hasta: None,
        }),
    )
    .unwrap();
    host.editor
        .execute(Command::SetRehearsalStyle(RehearsalStyle::Letters))
        .unwrap();
    let restyled = ejecutar(&mut host, id::REHEARSAL_NUMBERS);
    assert!(
        restyled.contains("Estilo de las marcas de ensayo: Números"),
        "{restyled}"
    );
}

#[test]
fn bar_headers_show_the_form() {
    let mut host = violin(3);
    run(
        &mut host,
        ToolCall::Seleccionar(SeleccionarArgs {
            desde: at("1", 2, 1.),
            hasta: None,
        }),
    )
    .unwrap();
    for edit in [FormEdit::ToggleRepeatStart, FormEdit::ToggleSegno] {
        host.editor.execute(Command::EditForm(edit)).unwrap();
    }
    let read = run(
        &mut host,
        ToolCall::Leer(LeerArgs {
            partes: None,
            desde: 1,
            hasta: 3,
        }),
    )
    .unwrap();
    let header = read
        .lines()
        .find(|line| line.starts_with("Compás 2:"))
        .expect(&read);
    assert!(header.contains("inicio de repetición"), "{read}");
    assert!(header.contains("segno"), "{read}");
}
