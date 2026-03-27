# pilcrow-routekit

Compiler-facing file-based routing primitives for Pilcrow.

## Build script integration

```rust
// build.rs
use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let src_root = manifest_dir.join("src");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    pilcrow_routekit::compile_to_out_dir(&src_root, &out_dir)
        .expect("compile pilcrow html sources");

    for dir in pilcrow_routekit::watched_source_directories(&src_root) {
        println!("cargo:rerun-if-changed={}", dir.display());
    }
}
```

```rust
// runtime
include!(concat!(env!("OUT_DIR"), "/generated_routes.rs"));

// register routes with your framework-specific adapter
let router = register_generated_routes(router, |router, route| {
    // route.pattern, route.template_path, route.symbol
    router
});
```
