//! The app's answer to the toolbar model: what each command looks like now.
//! One place for "is this option active?", used by the ribbon and File menu.

use super::*;
use score_core::NoteValue;
use score_toolbar::{CommandState, CommandStates};

impl CommandStates for AppView {
    fn state(&self, command: CommandId) -> CommandState {
        let editor = &self.document.editor;
        if let Some(state) = form_state(editor, command) {
            return state;
        }
        let layout_break = |kind| {
            let bar = editor
                .selected_ids()
                .iter()
                .map(|id| id.measure)
                .max()
                .unwrap_or(editor.cursor().measure);
            CommandState::checked(editor.score().layout_break(bar) == Some(kind))
        };
        match command {
            id::SYSTEM_BREAK => return layout_break(score_core::LayoutBreak::System),
            id::PAGE_BREAK => return layout_break(score_core::LayoutBreak::Page),
            _ => {}
        }
        let rehearsal = match command {
            id::REHEARSAL_MARK => Some(
                editor.score().has_rehearsal_mark(
                    editor
                        .selected_ids()
                        .iter()
                        .map(|id| id.measure)
                        .min()
                        .unwrap_or(editor.cursor().measure),
                ),
            ),
            id::REHEARSAL_LETTERS => {
                Some(editor.score().rehearsal_style() == score_core::RehearsalStyle::Letters)
            }
            id::REHEARSAL_NUMBERS => {
                Some(editor.score().rehearsal_style() == score_core::RehearsalStyle::Numbers)
            }
            id::REHEARSAL_BAR_NUMBERS => {
                Some(editor.score().rehearsal_style() == score_core::RehearsalStyle::BarNumbers)
            }
            _ => None,
        };
        if let Some(checked) = rehearsal {
            return CommandState::checked(checked);
        }
        let duration = active_duration(self);
        let value = |note: NoteValue| CommandState::checked(duration.value() == note);
        let accidental = |sign: Accidental| {
            CommandState::checked(
                editor
                    .selected_pitch()
                    .map_or(self.entry_accidental == Some(sign), |pitch| {
                        pitch.accidental() == Some(sign)
                    }),
            )
        };
        let hairpin = |kind: ConnectionKind| {
            let open = self
                .pending_hairpin
                .is_some_and(|(_, pending)| pending == kind);
            CommandState {
                checked: open || selected_connection(self, kind),
                label: open.then(|| format!("Terminar {}", kind.label().to_lowercase())),
                ..CommandState::default()
            }
        };
        let text = |kind: score_core::TextKind| CommandState {
            checked: self.text_editor.as_ref().is_some_and(|e| e.kind == kind)
                || editor
                    .selection()
                    .is_some_and(|id| editor.score().text(id, kind).is_some()),
            enabled: editor.selection().is_some(),
            ..CommandState::default()
        };
        let line = |kind: ConnectionKind| CommandState {
            checked: selected_connection(self, kind),
            enabled: editor.selection().is_some(),
            ..CommandState::default()
        };
        let enabled = |enabled: bool| CommandState {
            enabled,
            ..CommandState::default()
        };
        if let Some(score_core::MarkKind::Tremolo(mark)) = score_toolbar::marks::kind(command) {
            let ids = editor.selected_ids();
            return CommandState {
                checked: !ids.is_empty()
                    && ids.iter().all(|id| {
                        editor.score().event(*id).unwrap().notation.tremolo == Some(mark)
                    }),
                enabled: !ids.is_empty()
                    && ids
                        .iter()
                        .all(|id| !editor.score().event(*id).unwrap().pitches.is_empty()),
                ..Default::default()
            };
        }
        if let Some(score_core::MarkKind::Harmonic(mark)) = score_toolbar::marks::kind(command) {
            let ids = editor.selected_ids();
            let checked = !ids.is_empty()
                && ids
                    .iter()
                    .all(|id| editor.score().event(*id).unwrap().notation.harmonic == Some(mark));
            let enabled = !ids.is_empty()
                && ids.iter().all(|id| {
                    let mut event = *editor.score().event(*id).unwrap();
                    event.notation.harmonic = Some(mark);
                    event
                        .validate_harmonic(editor.score().part(id.part).unwrap().instrument())
                        .is_ok()
                });
            return CommandState {
                checked,
                enabled,
                ..Default::default()
            };
        }
        // Every point mark of the catalog: lit while the selection carries it.
        if let Some(score_core::MarkKind::Division(mark)) = score_toolbar::marks::kind(command)
            && !editor
                .score()
                .part(editor.cursor().part)
                .unwrap()
                .instrument()
                .is_grand_staff()
        {
            return CommandState::checked(
                editor
                    .score()
                    .part(editor.cursor().part)
                    .unwrap()
                    .divisi_change_at_tick(
                        editor.divisi_position().measure,
                        editor.divisi_position().tick,
                    )
                    == Some(mark),
            );
        }
        if let Some(kind) = score_toolbar::marks::kind(command).filter(|kind| kind.is_point()) {
            return CommandState::checked(
                editor
                    .selection()
                    .and_then(|id| editor.score().point_mark(id, kind.family()))
                    == Some(kind),
            );
        }
        match command {
            id::NEW_COMMENT => enabled(editor.selection().is_some()),
            id::SHOW_COMMENTS => CommandState::checked(
                self.comments.open && self.ui.show_inspector && self.ui.inspector == "Revisión",
            ),
            id::RESOLVE_COMMENT => {
                let comment = self
                    .comments
                    .active
                    .and_then(|id| editor.score().comment(id));
                CommandState {
                    enabled: comment.is_some(),
                    label: comment.map(|comment| {
                        if comment.resolved {
                            "Reabrir comentario".into()
                        } else {
                            "Resolver comentario".into()
                        }
                    }),
                    ..Default::default()
                }
            }
            id::CHECK_SELECTION => enabled(editor.selection().is_some()),
            id::REVIEW_PREVIOUS | id::REVIEW_NEXT => enabled(!self.review.filtered().is_empty()),
            id::REVIEW_ISSUES => {
                CommandState::checked(self.ui.show_inspector && self.ui.inspector == "Revisión")
            }
            id::SHOW_RANGES => CommandState::checked(self.review.show_ranges),
            id::EDIT_RANGE => enabled(
                editor
                    .score()
                    .part(editor.cursor().part)
                    .is_some_and(|part| part.sounding_range().is_some()),
            ),
            id::RESET_RANGE => enabled(
                editor
                    .score()
                    .part(editor.cursor().part)
                    .is_some_and(|part| part.range_override().is_some()),
            ),
            id::ARPEGGIO_BOTH_HANDS => {
                let id = editor.selection();
                CommandState {
                    checked: id
                        .and_then(|id| editor.score().arpeggio_at(id))
                        .is_some_and(|mark| mark.other_hand.is_some()),
                    enabled: id.and_then(|id| editor.score().piano_partner(id)).is_some(),
                    ..CommandState::default()
                }
            }
            id::UNDO => enabled(editor.can_undo()),
            id::REDO => enabled(editor.can_redo()),
            id::RESET_POSITION => {
                enabled(editor.selection().is_some() || self.selected_bar_mark().is_some())
            }
            id::REMOVE_INSTRUMENT => enabled(editor.score().parts().len() > 1),

            id::VOICE_1 => CommandState::checked(editor.cursor().voice == score_core::Voice::One),
            id::VOICE_2 => CommandState::checked(editor.cursor().voice == score_core::Voice::Two),
            id::DIVIDE_PART | id::JOIN_PART => {
                let part = editor.score().part(editor.cursor().part).unwrap();
                CommandState {
                    enabled: !part.instrument().is_grand_staff(),
                    ..Default::default()
                }
            }
            id::TOGGLE_NOTE_INPUT => CommandState::checked(self.ui.note_entry),
            id::STEP_TIME => CommandState::checked(true),
            id::CHORD_MODE => CommandState::checked(self.chord_entry),

            id::WHOLE => value(NoteValue::Whole),
            id::HALF => value(NoteValue::Half),
            id::QUARTER => value(NoteValue::Quarter),
            id::EIGHTH => value(NoteValue::Eighth),
            id::SIXTEENTH => value(NoteValue::Sixteenth),
            id::THIRTY_SECOND => value(NoteValue::ThirtySecond),
            id::DOT => CommandState::checked(duration.is_dotted()),
            id::TRIPLET => CommandState::checked(duration.is_triplet()),

            id::NATURAL => accidental(Accidental::Natural),
            id::SHARP => accidental(Accidental::Sharp),
            id::FLAT => accidental(Accidental::Flat),

            id::TIE => CommandState::checked(selected_connection(self, ConnectionKind::Tie)),
            id::SLUR => CommandState {
                checked: self.pending_slur.is_some(),
                label: self
                    .pending_slur
                    .is_some()
                    .then(|| "Terminar expresión".to_owned()),
                ..CommandState::default()
            },
            id::CRESCENDO => hairpin(ConnectionKind::Crescendo),
            id::DIMINUENDO => hairpin(ConnectionKind::Diminuendo),
            id::EXPRESSION_TEXT => text(score_core::TextKind::Expression),
            id::TECHNIQUE_TEXT => text(score_core::TextKind::Technique),
            id::OTTAVA_ALTA => line(ConnectionKind::OttavaAlta),
            id::OTTAVA_BASSA => line(ConnectionKind::OttavaBassa),
            id::PEDAL => line(ConnectionKind::Pedal),
            id::TRILL => line(ConnectionKind::Trill),
            id::GLISSANDO => line(ConnectionKind::Glissando),

            id::CLEF => CommandState::checked(self.clef_picker),
            id::KEY_SIGNATURE => CommandState::checked(self.key_picker),
            id::REPEATS => CommandState::checked(self.playback.follow_repeats),
            id::LOOP => CommandState::checked(self.playback.looping),
            id::HALF_SPEED => CommandState::checked(self.playback.half_speed),
            id::CHORD_PLAYBACK => CommandState::checked(editor.score().chords_sound()),
            id::LYRICS => CommandState {
                checked: self.lyric_editor.is_some()
                    || editor.selection().is_some_and(|id| {
                        editor
                            .score()
                            .event(id)
                            .is_some_and(|event| !event.notation.lyrics.is_empty())
                    }),
                enabled: editor.selection().is_some(),
                ..CommandState::default()
            },
            id::CHORD_SYMBOL => CommandState {
                checked: self.chord_editor.is_some()
                    || editor.selection().is_some_and(|id| {
                        editor
                            .score()
                            .event(id)
                            .is_some_and(|event| event.notation.chord.is_some())
                    }),
                enabled: editor.selection().is_some(),
                ..CommandState::default()
            },
            id::GO_TO_BAR => CommandState::checked(self.go_to_bar.is_some()),
            id::TEMPO_TEXT => CommandState::checked(self.tempo_picker.is_some()),
            id::SCORE_INFO => CommandState::checked(self.header_editor.is_some()),
            id::TIME_SIGNATURE => CommandState::checked(self.time_picker),
            id::ADD_INSTRUMENT => CommandState::checked(self.instrument_picker.is_some()),

            id::PLAY => CommandState {
                checked: self.playback.is_playing(),
                label: self.playback.is_playing().then(|| "Pausar".to_owned()),
                ..CommandState::default()
            },
            id::AUDITION => CommandState::checked(self.playback.audition),
            id::MIXER => {
                CommandState::checked(self.ui.show_inspector && self.ui.inspector == "Mezclador")
            }
            id::SOUNDS => {
                CommandState::checked(self.ui.show_inspector && self.ui.inspector == "Sonidos")
            }

            id::TOGGLE_INSTRUMENTS_PANEL => CommandState::checked(self.ui.show_instruments),
            id::PAGE_VIEW => {
                CommandState::checked(self.ui.view_mode == score_layout::ViewMode::Page)
            }
            id::CONTINUOUS_VIEW => {
                CommandState::checked(self.ui.view_mode == score_layout::ViewMode::Continuous)
            }
            id::SINGLE_SYSTEM_VIEW => {
                CommandState::checked(self.ui.view_mode == score_layout::ViewMode::SingleSystem)
            }
            id::TOGGLE_INSPECTOR => CommandState::checked(self.ui.show_inspector),
            id::TOGGLE_AGENT_PANEL => CommandState::checked(self.ui.show_agent),
            id::TAB_HOME => CommandState::checked(self.ui.tab == score_toolbar::TabId::Home),
            id::TAB_NOTE_INPUT => {
                CommandState::checked(self.ui.tab == score_toolbar::TabId::NoteInput)
            }
            id::TAB_NOTATION => {
                CommandState::checked(self.ui.tab == score_toolbar::TabId::Notation)
            }
            id::TAB_TEXT => CommandState::checked(self.ui.tab == score_toolbar::TabId::Text),
            id::TAB_PLAYBACK => {
                CommandState::checked(self.ui.tab == score_toolbar::TabId::Playback)
            }
            id::TAB_LAYOUT => CommandState::checked(self.ui.tab == score_toolbar::TabId::Layout),
            id::TAB_REVIEW => CommandState::checked(self.ui.tab == score_toolbar::TabId::Review),
            id::TAB_VIEW => CommandState::checked(self.ui.tab == score_toolbar::TabId::View),
            id::TOGGLE_RIBBON => CommandState::checked(self.ui.ribbon_collapsed),
            _ => CommandState::default(),
        }
    }
}

fn form_state(editor: &score_edit::Editor, command: CommandId) -> Option<CommandState> {
    let action = form_command(editor, command)?;
    let (start, end) = form_bounds(editor);
    let first = editor.score().bar_form(start);
    let last = editor.score().bar_form(end);
    let any_ending = || (start..=end).any(|bar| editor.score().ending_at(bar).is_some());
    let checked = match action {
        Command::EditForm(edit) => match edit {
            FormEdit::ToggleRepeatStart => first.repeat_start,
            FormEdit::ToggleRepeatEnd => last.repeat_end.is_some(),
            FormEdit::Barline(_) => {
                last.barline
                    == if command == id::DOUBLE_BARLINE {
                        Barline::Double
                    } else {
                        Barline::Final
                    }
            }
            FormEdit::ToggleSegno => first.segno,
            FormEdit::ToggleCoda => first.coda,
            FormEdit::ToggleToCoda => last.to_coda,
            FormEdit::ToggleFine => last.fine,
            FormEdit::Jump(jump) => last.jump == jump,
            FormEdit::Clear => {
                return Some(CommandState {
                    enabled: any_ending()
                        || (start..=end).any(|bar| !editor.score().bar_form(bar).is_plain()),
                    ..Default::default()
                });
            }
        },
        Command::SetEnding(numbers) => (start..=end).all(|bar| {
            editor
                .score()
                .ending_at(bar)
                .is_some_and(|ending| numbers.iter().all(|number| ending.numbers.contains(number)))
        }),
        Command::RemoveEndings => {
            return Some(CommandState {
                enabled: any_ending(),
                ..Default::default()
            });
        }
        _ => unreachable!("form_command only resolves form operations"),
    };
    Some(CommandState::checked(checked))
}

#[cfg(test)]
mod form_state_tests {
    use super::form_state;
    use crate::form_command;
    use score_core::{
        BarForm, Barline, Duration, Event, Jump, Notation, Pitch, Pitches, Score, Step,
    };
    use score_keymap::id;

    #[test]
    fn form_highlights_follow_the_selected_ranges_actual_targets() {
        let mut score = Score::default();
        let event = Event {
            pitches: Pitches::single(Pitch::new(Step::C, 4)),
            duration: Duration::Quarter,
            notation: Notation::default(),
        };
        let first = score.insert(1, 0, event).unwrap();
        let last = score.insert(3, 0, event).unwrap();
        score
            .set_bar_form(
                1,
                BarForm {
                    repeat_start: true,
                    segno: true,
                    ..Default::default()
                },
            )
            .unwrap();
        score
            .set_bar_form(
                3,
                BarForm {
                    repeat_end: Some(2),
                    barline: Barline::Double,
                    jump: Some(Jump::DalSegnoAlCoda),
                    ..Default::default()
                },
            )
            .unwrap();
        score
            .set_ending(score_core::Ending {
                start: 1,
                end: 3,
                numbers: vec![2],
            })
            .unwrap();
        let mut editor = score_edit::Editor::new(score).unwrap();
        // Cursor remains at the first selected bar; closing actions target the last.
        editor.select(last);
        editor.extend_selection(first);
        assert_eq!(editor.cursor().measure, 1);
        for command in [
            id::REPEAT_START,
            id::SEGNO,
            id::REPEAT_END,
            id::DOUBLE_BARLINE,
            id::DAL_SEGNO_AL_CODA,
            id::ENDING_2,
        ] {
            assert!(form_state(&editor, command).unwrap().checked, "{command}");
        }
        for command in [
            id::FINAL_BARLINE,
            id::DA_CAPO,
            id::ENDING_1,
            id::FINE,
            id::CODA,
        ] {
            assert!(!form_state(&editor, command).unwrap().checked, "{command}");
        }
        assert!(form_state(&editor, id::REMOVE_ENDING).unwrap().enabled);
        assert!(form_state(&editor, id::CLEAR_FORM).unwrap().enabled);
        editor
            .execute(form_command(&editor, id::CLEAR_FORM).unwrap())
            .unwrap();
        assert!(!form_state(&editor, id::REMOVE_ENDING).unwrap().enabled);
        assert!(!form_state(&editor, id::CLEAR_FORM).unwrap().enabled);
    }
}
