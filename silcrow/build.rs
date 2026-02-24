use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::Hasher;

const MODULES: &[&str] = &[
    "debug",
    "patcher",
    "safety",
    "toasts",
    "navigator",
    "index",
];

fn main() {
    // Rerun if any module file changes
    for name in MODULES {
        println!("cargo:rerun-if-changed=public/silcrow/{name}.js");
    }

    // Concatenate modules into a single IIFE
    let mut bundle = String::from("(function(){\"use strict\";\n");
    for name in MODULES {
        let path = format!("public/silcrow/{name}.js");
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("{path} not found"));
        bundle.push_str(&content);
        bundle.push('\n');
    }
    bundle.push_str("})();");

    fs::write("public/silcrow.js", &bundle).expect("failed to write silcrow.js");

    // Hash the built bundle for cache-busting
    let mut hasher = DefaultHasher::new();
    hasher.write(bundle.as_bytes());
    let hash = format!("{:x}", hasher.finish());
    let short = &hash[..8];

    println!("cargo::rustc-env=SILCROW_JS_HASH={short}");
}
