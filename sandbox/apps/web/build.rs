fn main() {
    if let Err(err) = routekit::compile_current_crate_sources() {
        eprintln!("Pilcrow template compile error:");
        eprintln!("{err}");
        std::process::exit(1);
    }
}
