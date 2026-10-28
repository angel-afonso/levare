use super::*;
use score_core::{Event, Notation, Pitch, Pitches, Step};
#[test]
fn transposed_keys_percussion_and_harp_engrave_in_all_views_without_cropping() {
    let instruments = [
        Instrument::Piccolo,
        Instrument::Flute,
        Instrument::Oboe,
        Instrument::EnglishHorn,
        Instrument::ClarinetBb,
        Instrument::BassClarinet,
        Instrument::Bassoon,
        Instrument::Contrabassoon,
        Instrument::HornF,
        Instrument::TrumpetBb,
        Instrument::Trombone,
        Instrument::Tuba,
        Instrument::Timpani,
        Instrument::SnareDrum,
        Instrument::Triangle,
        Instrument::Glockenspiel,
        Instrument::Harp,
        Instrument::Celesta,
        Instrument::Violin,
        Instrument::Viola,
        Instrument::Cello,
        Instrument::DoubleBass,
    ];
    let mut score = Score::new("Orquesta".into(), &instruments, 2).unwrap();
    let parts: Vec<_> = score.parts().iter().map(|p| p.id()).collect();
    for part in parts {
        score
            .insert_on(
                part,
                StaffId::Upper,
                0,
                0,
                Event {
                    pitches: Pitches::single(Pitch::new(Step::C, 4)),
                    duration: Duration::Whole,
                    notation: Notation::default(),
                },
            )
            .unwrap();
    }
    let face =
        ttf_parser::Face::parse(include_bytes!("../../../assets/fonts/Bravura.otf"), 0).unwrap();
    assert!(
        face.glyph_bounding_box(face.glyph_index('\u{e069}').unwrap())
            .is_some()
    );
    for mode in [ViewMode::Page, ViewMode::Continuous, ViewMode::SingleSystem] {
        let layout = LayoutEngine::default().layout_view(&score, 1600., mode);
        if mode == ViewMode::Page {
            assert!(layout.pages[0].width > 1600.);
            assert!(layout.pages[0].height / layout.pages[0].width < 1.5);
        }
        for (index, instrument) in instruments.iter().enumerate() {
            let bar = layout
                .measures
                .iter()
                .find(|b| {
                    b.part == score.parts()[index].id() && b.staff == StaffId::Upper && b.index == 0
                })
                .unwrap();
            let key = score.written_key_at(bar.part, 0);
            let sign = if key.fifths() < 0 {
                "\u{e260}"
            } else {
                "\u{e262}"
            };
            assert_eq!(
                bar.content
                    .glyphs
                    .iter()
                    .filter(|g| g.element.is_none() && g.text == sign)
                    .count(),
                key.count()
            );
            if instrument.percussion_key().is_some() {
                assert!(bar.content.glyphs.iter().any(|g| g.text == "\u{e069}"));
            }
        }
        assert!(layout.pages.iter().all(|p| {
            layout
                .systems
                .iter()
                .filter(|s| s.first_measure >= p.first_measure && s.last_measure <= p.last_measure)
                .all(|s| s.bottom <= p.top + p.height)
        }));
    }
}

#[test]
fn large_orchestra_page_reuses_the_resolved_width_without_reengraving_unchanged_music() {
    let score = Score::new("Caché".into(), &Instrument::ORCHESTRA, 32).unwrap();
    let mut engine = LayoutEngine::default();
    let first = engine.layout_view(&score, 1000., ViewMode::Page);
    assert!(first.width > 1600.);
    for width in [700., 1400., 1000.] {
        let next = engine.layout_view(&score, width, ViewMode::Page);
        assert_eq!(next.width, first.width);
        assert_eq!(engine.recomputed_measures, 0);
        assert_eq!(next.pages.len(), first.pages.len());
    }
}

#[test]
fn one_note_edit_on_a_large_orchestra_page_engraves_only_that_bar_again() {
    let mut score = Score::new("Edición".into(), &Instrument::ORCHESTRA, 16).unwrap();
    let parts: Vec<_> = score.parts().iter().map(|p| p.id()).collect();
    let mut edited = None;
    for &part in &parts {
        for bar in 0..16 {
            for (index, step) in [Step::C, Step::E, Step::G, Step::E].into_iter().enumerate() {
                let id = score
                    .insert_on(
                        part,
                        StaffId::Upper,
                        bar,
                        index,
                        Event {
                            pitches: Pitches::single(Pitch::new(step, 4)),
                            duration: Duration::Quarter,
                            notation: Notation::default(),
                        },
                    )
                    .unwrap();
                if part == parts[parts.len() - 3] && bar == 9 && index == 1 {
                    edited = Some(id);
                }
            }
        }
    }
    let mut engine = LayoutEngine::default();
    engine.layout_view(&score, 1000., ViewMode::Page);
    score.transpose(edited.unwrap(), 1).unwrap();
    let layout = engine.layout_view(&score, 1000., ViewMode::Page);
    let staves: usize = score
        .parts()
        .iter()
        .map(|p| p.display_staves(9).count())
        .sum();
    assert!(
        (1..=staves).contains(&engine.recomputed_measures),
        "one edited note engraved {} bars again; bar 10 has {staves} staves",
        engine.recomputed_measures
    );
    // Starting from the width already resolved gives the page a fresh
    // layout, such as the PDF export's, would give.
    let fresh = LayoutEngine::default().layout_view(&score, 1000., ViewMode::Page);
    assert_eq!(format!("{layout:?}"), format!("{fresh:?}"));
}

#[test]
fn an_edit_that_shrinks_the_tallest_system_gives_back_the_narrower_page() {
    let mut score = Score::new("Registros".into(), &Instrument::ORCHESTRA, 8).unwrap();
    let violin = score.parts()[score.parts().len() - 5].id();
    let high = score
        .insert_on(
            violin,
            StaffId::Upper,
            2,
            0,
            Event {
                pitches: Pitches::single(Pitch::new(Step::C, 8)),
                duration: Duration::Whole,
                notation: Notation::default(),
            },
        )
        .unwrap();
    let mut engine = LayoutEngine::default();
    let tall = engine.layout_view(&score, 1000., ViewMode::Page);
    score.transpose(high, -21).unwrap();
    let layout = engine.layout_view(&score, 1000., ViewMode::Page);
    let fresh = LayoutEngine::default().layout_view(&score, 1000., ViewMode::Page);
    assert!(fresh.width < tall.width);
    assert_eq!(format!("{layout:?}"), format!("{fresh:?}"));
}

#[test]
fn dense_orchestra_resolves_extreme_register_reflow_on_the_first_page_layout() {
    let mut score = Score::new("Registros".into(), &[Instrument::Violin; 34], 8).unwrap();
    let parts: Vec<_> = score.parts().iter().map(|p| p.id()).collect();
    for part in parts {
        for bar in 0..8 {
            for index in 0..16 {
                let octave = match bar {
                    0 | 4 => 7,
                    3 | 7 => 1,
                    _ => 4,
                };
                score
                    .insert_on(
                        part,
                        StaffId::Upper,
                        bar,
                        index,
                        Event {
                            pitches: Pitches::single(Pitch::new(Step::C, octave)),
                            duration: Duration::Sixteenth,
                            notation: Notation::default(),
                        },
                    )
                    .unwrap();
            }
        }
    }
    let mut engine = LayoutEngine::default();
    let first = engine.layout_view(&score, 1000., ViewMode::Page);
    assert!(first.pages.iter().all(|p| p.height / p.width < 1.42));
    let again = engine.layout_view(&score, 1000., ViewMode::Page);
    assert_eq!(first.width, again.width);
    assert_eq!(engine.recomputed_measures, 0);
}
