//./build.rs
use crc32fast::Hasher;
use std::fs;

const MODULES: &[&str] = &[
    "debug",
    "url-safety",
    "patcher",
    "safety",
    "toasts",
    "navigator",
    "live",
    "ws",
    "optimistic",
    "index",
];

fn main() {
    for name in MODULES {
        println!("cargo:rerun-if-changed=silcrow/{name}.js");
    }

    // Concatenate modules into a single IIFE
    let mut raw = String::from("(function(){\"use strict\";\n");
    for name in MODULES {
        let path = format!("silcrow/{name}.js");
        let content = fs::read_to_string(&path).unwrap_or_else(|_| panic!("{path} not found"));
        raw.push_str(&content);
        raw.push('\n');
    }
    raw.push_str("})();");

    // Minify in release builds
    let bundle = if std::env::var("PROFILE").unwrap() == "release" {
        minifier::js::minify(&raw).to_string()
    } else {
        raw
    };

    // Write built bundle
    fs::create_dir_all("public").expect("failed to create public/");
    fs::write("public/silcrow.js", &bundle).expect("failed to write silcrow.js");

    // Hash for cache-busting
    let mut hasher = Hasher::new();
    hasher.update(bundle.as_bytes());
    let hash = format!("{:08x}", hasher.finalize());

    println!("cargo::rustc-env=SILCROW_JS_HASH={hash}");
}
