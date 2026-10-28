fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    assert_eq!(
        args.len(),
        3,
        "usage: export input.partitura output-directory"
    );
    let score = score_io::load(std::path::Path::new(&args[1])).expect("read score");
    let mut selection = score_export::Selection::all(&score, true);
    selection.pdf = true;
    let files = score_export::export(&score, &selection, std::path::Path::new(&args[2]))
        .expect("export score and parts");
    println!("{} archivos exportados", files.len());
}
