fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    assert_eq!(args.len(), 3, "usage: export input.partitura output.pdf");
    let score = score_io::load(std::path::Path::new(&args[1])).expect("read score");
    let pages = score_pdf::export(&score, std::path::Path::new(&args[2])).expect("export PDF");
    println!("Exported {pages} pages");
}
