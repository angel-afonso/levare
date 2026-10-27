use super::parts_view::{FamilyRows, parts_view};
use score_core::{Instrument, InstrumentFamily, MAX_PARTS, Score};

fn score(instruments: &[Instrument]) -> Score {
    Score::new("Prueba".into(), instruments, 4).unwrap()
}

fn names(family: &FamilyRows) -> Vec<&str> {
    family.parts.iter().map(|row| row.name.as_str()).collect()
}

#[test]
fn families_keep_score_order_and_count_parts() {
    let score = score(&[
        Instrument::Flute,
        Instrument::ClarinetBb,
        Instrument::HornF,
        Instrument::Piano,
        Instrument::Violin,
        Instrument::Violin,
        Instrument::Cello,
    ]);
    let view = parts_view(&score, 0, "", &[]);
    let families: Vec<_> = view.iter().map(|family| family.family).collect();
    assert_eq!(
        families,
        [
            InstrumentFamily::Woodwinds,
            InstrumentFamily::Brass,
            InstrumentFamily::Keyboards,
            InstrumentFamily::Strings,
        ]
    );
    let labels: Vec<_> = view.iter().map(|family| family.label).collect();
    assert_eq!(labels, ["Maderas", "Metales", "Teclados", "Cuerda"]);
    let counts: Vec<_> = view.iter().map(|family| family.count).collect();
    assert_eq!(counts, [2, 1, 1, 3]);
    assert_eq!(names(&view[0]), ["Flauta", "Clarinete en Si♭"]);
    assert_eq!(names(&view[3]), ["Violín I", "Violín II", "Violoncello"]);
    for family in &view {
        assert!(!family.folded);
        for row in &family.parts {
            assert_eq!(row.name, score.part_name(row.id));
        }
    }
}

#[test]
fn query_ignores_case_and_accents_and_unfolds() {
    let score = score(&[
        Instrument::Flute,
        Instrument::Violin,
        Instrument::Violin,
        Instrument::Viola,
    ]);
    let folded = ["Cuerda".to_owned(), "Maderas".to_owned()];
    let view = parts_view(&score, 0, "  VIOLIN ", &folded);
    assert_eq!(view.len(), 1, "families without a match are left out");
    assert_eq!(view[0].family, InstrumentFamily::Strings);
    assert!(!view[0].folded, "a search shows the families unfolded");
    assert_eq!(view[0].count, 2);
    assert_eq!(names(&view[0]), ["Violín I", "Violín II"]);
    assert_eq!(
        names(&parts_view(&score, 0, "flaUta", &folded)[0]),
        ["Flauta"]
    );
    assert!(parts_view(&score, 0, "tuba", &[]).is_empty());
    // An empty query keeps the folds.
    assert!(
        parts_view(&score, 0, "  ", &folded)
            .iter()
            .all(|f| f.folded)
    );
}

#[test]
fn folded_family_hides_its_rows() {
    let score = score(&[Instrument::Oboe, Instrument::Violin, Instrument::Viola]);
    let view = parts_view(&score, 0, "", &["Cuerda".to_owned()]);
    assert_eq!(view.len(), 2);
    assert!(!view[0].folded);
    assert_eq!(names(&view[0]), ["Oboe"]);
    assert!(view[1].folded);
    assert!(view[1].parts.is_empty());
    assert_eq!(view[1].count, 2, "a folded family still shows its count");
}

#[test]
fn meta_names_clefs_and_transposition() {
    let score = score(&[
        Instrument::ClarinetBb,
        Instrument::HornF,
        Instrument::Piano,
        Instrument::Viola,
        Instrument::DoubleBass,
    ]);
    let view = parts_view(&score, 0, "", &[]);
    let metas: Vec<_> = view
        .iter()
        .flat_map(|family| family.parts.iter())
        .map(|row| (row.name.as_str(), row.meta.as_str()))
        .collect();
    assert_eq!(
        metas,
        [
            ("Clarinete en Si♭", "Si♭ · Sol"),
            ("Corno en Fa", "Fa · Sol"),
            ("Piano", "Sol / Fa"),
            ("Viola", "Do 3.ª"),
            ("Contrabajo", "Fa · 8vb"),
        ]
    );
}

#[test]
fn divisi_bar_names_its_staves() {
    let mut score = score(&[Instrument::Violin]);
    let part = score.parts()[0].id();
    score.set_passage_divisi(part, 1, Some(1), true).unwrap();
    assert_eq!(parts_view(&score, 0, "", &[])[0].parts[0].meta, "Sol");
    assert_eq!(
        parts_view(&score, 1, "", &[])[0].parts[0].meta,
        "Divisi · Sol"
    );
}

#[test]
fn thirty_two_folded_parts_give_one_row_per_family() {
    let catalog = [
        Instrument::Flute,
        Instrument::Oboe,
        Instrument::ClarinetBb,
        Instrument::Bassoon,
        Instrument::HornF,
        Instrument::TrumpetBb,
        Instrument::Trombone,
        Instrument::Tuba,
        Instrument::Piano,
        Instrument::Violin,
        Instrument::Viola,
        Instrument::Cello,
        Instrument::DoubleBass,
    ];
    let instruments: Vec<_> = catalog.iter().copied().cycle().take(32).collect();
    assert!(instruments.len() <= MAX_PARTS);
    let score = score(&instruments);
    let open = parts_view(&score, 0, "", &[]);
    let rows: usize = open.iter().map(|family| family.parts.len()).sum();
    assert_eq!(rows, 32);
    let folded: Vec<String> = open.iter().map(|family| family.label.to_owned()).collect();
    let closed = parts_view(&score, 0, "", &folded);
    let rows: usize = closed.iter().map(|family| 1 + family.parts.len()).sum();
    assert_eq!(rows, closed.len());
    assert_eq!(closed.len(), 4);
    assert_eq!(closed.iter().map(|family| family.count).sum::<usize>(), 32);
}
