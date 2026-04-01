# pilcrow-routekit

Compiler-facing file-based routing primitives for Pilcrow.

## Template behavior

- Known `components/*.html` and `layouts/*.html` are expanded into page templates at build time.
- Paired tags like `<Layout>...</Layout>` inject children through `<slot />` in the target template.
- Named slots are supported via `slot="name"` and `<slot name="name" />`.
- Slot props are supported via `<slot name="item" value={...} />` + `let:value` on slotted nodes.
- Unknown PascalCase tags are preserved as generated Askama component-call syntax.

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
include!(concat!(env!("OUT_DIR"), "/generated_templates.rs"));

// register routes with your framework-specific adapter
let router = register_generated_routes(router, |router, route| {
    // route.pattern, route.symbol, route.render_symbol
    router
});

// compiled Askama render fn
let html = page_index::render_page_index(page_index::Props {
    // ...
})?;
```
