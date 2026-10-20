fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    assert_eq!(
        args.len(),
        3,
        "usage: export_musicxml input.partitura output.musicxml"
    );
    let score = score_io::load(std::path::Path::new(&args[1])).expect("read score");
    score_io::musicxml::export(std::path::Path::new(&args[2]), &score).expect("export MusicXML");
    println!("MusicXML exportado");
}
