use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::Hasher;

fn main() {
    println!("cargo:rerun-if-changed=public/silcrow.js");

    let js = fs::read("public/silcrow.js").expect("silcrow.js not found");
    let mut hasher = DefaultHasher::new();
    hasher.write(&js);
    let hash = format!("{:x}", hasher.finish());
    let short = &hash[..8];

    println!("cargo::rustc-env=SILCROW_JS_HASH={short}");
}
