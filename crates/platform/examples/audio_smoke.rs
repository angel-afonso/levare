//! Short, audible hardware check, independent of GPUI.
use score_core::{Duration, Event, Pitch, Score, Step};
use score_platform::AudioOutput;
use score_playback::{PlaybackPlan, Tempo};
use std::{
    sync::Arc,
    time::{Duration as WallDuration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut audio = AudioOutput::open()?;
    println!("Salida de audio activa: {} Hz", audio.sample_rate());
    audio.control.preview_chord(
        [Some(61), Some(64), Some(68), None, None, None, None, None],
        score_core::Instrument::Piano,
    )?;
    std::thread::sleep(WallDuration::from_millis(300));
    let mut score = Score::default();
    score.set_key_signature(score_core::KeySignature::new(2, score_core::KeyMode::Major).unwrap());
    for (index, (step, octave)) in [(Step::C, 4), (Step::C, 4), (Step::G, 4), (Step::C, 5)]
        .into_iter()
        .enumerate()
    {
        let id = score.insert(
            0,
            index,
            Event {
                notation: Default::default(),
                pitches: score_core::Pitches::single(Pitch::new(step, octave)),
                duration: Duration::Quarter,
            },
        )?;
        score.add_pitch(id, Pitch::new(step, octave + 1).unwrap())?;
    }
    let left = score.insert_on(
        score.parts()[0].id(),
        score_core::StaffId::Lower,
        0,
        0,
        Event {
            notation: Default::default(),
            pitches: score_core::Pitches::single(Pitch::new(Step::C, 3)),
            duration: Duration::Whole,
        },
    )?;
    score.add_pitch(
        left,
        Pitch::new(Step::E, 3)
            .unwrap()
            .with_accidental(Some(score_core::Accidental::Flat)),
    )?;
    score.add_pitch(left, Pitch::new(Step::G, 3).unwrap())?;
    score.set_accidental(left, 0, Some(score_core::Accidental::Sharp))?;
    let flute = score.add_part(score_core::Instrument::Flute)?;
    let cello = score.add_part(score_core::Instrument::Cello)?;
    score.insert_in(
        flute,
        0,
        0,
        Event {
            notation: Default::default(),
            pitches: score_core::Pitches::single(Pitch::new(Step::G, 5)),
            duration: Duration::Whole,
        },
    )?;
    score.insert_in(
        cello,
        0,
        0,
        Event {
            notation: Default::default(),
            pitches: score_core::Pitches::single(Pitch::new(Step::C, 3)),
            duration: Duration::Whole,
        },
    )?;
    let first = score_core::ElementId {
        part: score.parts()[0].id(),
        staff: score_core::StaffId::Upper,
        measure: 0,
        event: score.measures()[0].id_at(0).unwrap(),
    };
    let second = score.next_event(first).unwrap();
    let third = score.next_event(second).unwrap();
    let fourth = score.next_event(third).unwrap();
    score.set_dynamic(first, Some(score_core::Dynamic::Pp))?;
    score.set_articulation(first, Some(score_core::Articulation::Staccato))?;
    score.set_articulation(third, Some(score_core::Articulation::Accent))?;
    score.set_dynamic(fourth, Some(score_core::Dynamic::F))?;
    score.add_connection(score_core::Connection {
        start: first,
        end: second,
        kind: score_core::ConnectionKind::Tie,
    })?;
    score.add_connection(score_core::Connection {
        start: third,
        end: fourth,
        kind: score_core::ConnectionKind::Slur,
    })?;
    let plan = Arc::new(PlaybackPlan::new(
        &score,
        Tempo::default(),
        audio.sample_rate(),
    )?);
    let selected = plan.spans[1].element;
    let onset = plan.frame_for_element(selected).unwrap();
    audio.play_from(plan, Some(selected))?;
    std::thread::sleep(WallDuration::from_millis(100));
    if audio.control.position().is_none_or(|position| {
        position.frame <= onset
            || position.element != Some(selected)
            || position.elements.into_iter().flatten().count() != 4
    }) {
        return Err("La reproducción no arrancó desde la nota seleccionada".into());
    }
    audio.control.pause();
    std::thread::sleep(WallDuration::from_millis(80));
    let frozen = audio
        .control
        .position()
        .ok_or("No se conservó la posición pausada")?
        .frame;
    std::thread::sleep(WallDuration::from_millis(100));
    if !audio.control.is_paused()
        || audio
            .control
            .position()
            .is_none_or(|position| position.frame != frozen)
    {
        return Err("El reloj musical avanzó durante la pausa".into());
    }
    audio.control.resume();
    let deadline = Instant::now() + WallDuration::from_secs(6);
    let mut progressed = false;
    while audio.control.is_playing() && Instant::now() < deadline {
        if let Some(position) = audio.control.position() {
            progressed |= position.frame > 0;
        }
        std::thread::sleep(WallDuration::from_millis(10));
    }
    if audio.control.failed() || !progressed || audio.control.is_playing() {
        return Err("El dispositivo no completó la prueba de reproducción".into());
    }
    std::thread::sleep(WallDuration::from_millis(200));
    audio.control.stop();
    println!(
        "Ligaduras, dinámicas y articulaciones; piano a dos manos, flauta y violonchelo: selección, pausa, continuación y finalización correctas"
    );
    Ok(())
}
