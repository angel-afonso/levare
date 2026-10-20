fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    assert_eq!(
        args.len(),
        3,
        "usage: import_musicxml input.musicxml output.partitura"
    );
    let input = std::path::Path::new(&args[1]);
    let output = std::path::Path::new(&args[2]);
    assert!(
        input.canonicalize().ok() != output.canonicalize().ok() || !output.exists(),
        "input and output must differ"
    );
    let imported = score_io::musicxml::load(input).expect("import MusicXML");
    for warning in imported.warnings {
        eprintln!("Aviso: {warning}");
    }
    score_io::save(output, &imported.score).expect("save editable score");
    println!(
        "MusicXML importado · {} instrumentos · {} compases",
        imported.score.parts().len(),
        imported.score.measures().len()
    );
}
