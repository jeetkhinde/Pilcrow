# pilcrow-routekit

`pilcrow-routekit` is the mandatory web rendering compiler in Pilcrow convention.

## What It Does

- discovers file-based routes from `src/pages`
- composes `layouts` and `components`
- expands slots and component invocations
- emits generated Rust modules:
  - `generated_routes.rs`
  - `generated_templates.rs`

## Explicit Template Imports (Required)

PascalCase template tags are resolved only from explicit frontmatter imports.

```html
---
import MainLayout from "layouts/MainLayout.html";
import StatusBadge from "components/StatusBadge.html";

pub struct Props {
    pub title: String,
}
---
<MainLayout title={title}>
  <StatusBadge text="ready" />
</MainLayout>
```

- Imports must be `src`-root relative and end in `.html`.
- Allowed import roots: `components/...` and `layouts/...`.
- Missing imports are compile errors.

## Common Failure (Missing Import)

If a template uses a PascalCase tag without import, routekit fails with an actionable message, for example:

```text
Pilcrow template compile error
  file: pages/index.html
  error: missing explicit import for component `<StatusBadge>` at template line 25, column 6. Add `import StatusBadge from "components/StatusBadge.html";` in frontmatter.
```

## Required Web Build Integration

```rust
use std::{env, path::PathBuf};

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

No alternate app-level page rendering path is documented.
