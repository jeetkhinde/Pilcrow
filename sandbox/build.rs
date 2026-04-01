use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set"));
    let src_root = manifest_dir.join("astro_todo_demo/src");
    if !src_root.exists() {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set"));
    pilcrow_routekit::compile_to_out_dir(&src_root, &out_dir)
        .expect("failed to compile sandbox astro_todo_demo html sources");

    for dir in pilcrow_routekit::watched_source_directories(&src_root) {
        println!("cargo:rerun-if-changed={}", dir.display());
    }
}
