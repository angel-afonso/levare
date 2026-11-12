//! Where each part goes in the score. Orchestral order (`Instrument::score_order`)
//! holds for every line-up except three chamber conventions, which follow
//! what is on the page of published scores:
//!
//! - voices accompanied only by keyboards or plucked strings (a song, a
//!   choir with piano) sit above their accompaniment;
//! - in a brass ensemble the trumpets lead, above the horns;
//! - woodwinds with horns (a wind quintet, Harmoniemusik) put the horns
//!   above the bassoons.
//!
//! Adding a part that breaks the line-up brings orchestral order back.

use crate::{Instrument, InstrumentFamily};

/// The key parts are sorted by in a score holding `instruments`.
pub(crate) fn order_key(instruments: &[Instrument]) -> impl Fn(Instrument) -> i32 {
    use InstrumentFamily::*;
    let has = |test: &dyn Fn(Instrument) -> bool| instruments.iter().any(|&i| test(i));
    let all = |test: &dyn Fn(Instrument) -> bool| instruments.iter().all(|&i| test(i));
    let voice = |i: Instrument| i.family() == Voices;
    let horn = |i: Instrument| matches!(i, Instrument::Horn | Instrument::HornF);
    let trumpet = |i: Instrument| {
        matches!(
            i,
            Instrument::Trumpet
                | Instrument::TrumpetBb
                | Instrument::TrumpetD
                | Instrument::PiccoloTrumpet
                | Instrument::Cornet
        )
    };
    let accompanied_voices = has(&voice)
        && has(&|i| !voice(i))
        && all(&|i| matches!(i.family(), Voices | Keyboards | PluckedStrings));
    let brass_ensemble = has(&horn) && has(&trumpet) && all(&|i| i.family() == Brass);
    let wind_ensemble = has(&horn)
        && has(&|i| i.family() == Woodwinds)
        && all(&|i| i.family() == Woodwinds || horn(i));
    let bassoon = i32::from(Instrument::Bassoon.score_order()) * 2;
    move |instrument| {
        let order = i32::from(instrument.score_order()) * 2;
        if (accompanied_voices && voice(instrument)) || (brass_ensemble && trumpet(instrument)) {
            order - 1000
        } else if wind_ensemble && horn(instrument) {
            bassoon - 1
        } else {
            order
        }
    }
}
