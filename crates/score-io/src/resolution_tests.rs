//! Every example score keeps its rhythm when the resolution of time changes.
//! The fixture was recorded at 192 ticks per whole note; each line holds an
//! example's event count, the sum of its onsets and durations, its divisi
//! points and a hash of all of them, in units of that old tick.
use super::*;
use score_core::{Score, TICKS_PER_WHOLE};
use std::fmt::Write as _;

const FIXTURE: &str = "src/resolution_tests.txt";
/// Ticks of today's resolution in one tick of the recorded one.
const LEGACY_SCALE: u64 = TICKS_PER_WHOLE as u64 / 192;

fn legacy(ticks: u64) -> u64 {
    assert_eq!(
        ticks % LEGACY_SCALE,
        0,
        "{ticks} ticks are off the old grid"
    );
    ticks / LEGACY_SCALE
}

fn signature(score: &Score) -> String {
    let (mut events, mut onsets, mut durations, mut points) = (0u64, 0u64, 0u64, 0u64);
    // FNV-1a over every value in the order the score holds them.
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut mix = |value: u64| {
        for byte in value.to_le_bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
    };
    for (index, part) in score.parts().iter().enumerate() {
        for staff in part.staves() {
            for (bar, measure) in staff.measures().iter().enumerate() {
                let start = score.measure_start_tick(bar);
                for (voice, tick, _, event) in measure.timed_iter() {
                    let onset = legacy(start + u64::from(tick));
                    let duration = legacy(u64::from(event.duration.ticks()));
                    events += 1;
                    onsets += onset;
                    durations += duration;
                    for value in [index as u64, bar as u64, voice as u64, onset, duration] {
                        mix(value);
                    }
                }
            }
        }
        for bar in 0..score.measures().len() {
            for (tick, _) in part.divisi_changes_in(bar) {
                let point = legacy(u64::from(tick));
                points += point;
                mix(point);
            }
        }
    }
    format!("{events} {onsets} {durations} {points} {hash:016x}")
}

#[test]
fn example_scores_keep_their_rhythm_at_any_resolution() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut names: Vec<_> = std::fs::read_dir(root.join("../../examples"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "partitura"))
        .collect();
    names.sort();
    let mut actual = String::new();
    for path in names {
        let score = load(&path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy();
        let _ = writeln!(actual, "{name} {}", signature(&score));
    }
    if std::env::var_os("LEVARE_RECORD_RESOLUTION").is_some() {
        std::fs::write(root.join(FIXTURE), &actual).unwrap();
    }
    let expected = std::fs::read_to_string(root.join(FIXTURE)).unwrap();
    assert_eq!(actual, expected);
}
