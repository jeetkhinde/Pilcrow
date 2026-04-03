use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set"));
    let src_root = manifest_dir.join("src");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set"));

    routekit::compile_to_out_dir(&src_root, &out_dir)
        .expect("failed to compile Pilcrow sources");

    for dir in routekit::watched_source_directories(&src_root) {
        println!("cargo:rerun-if-changed={}", dir.display());
    }
}
