# Getting Started

This quick start follows the mandatory Pilcrow convention.

## 1. Define file-based pages in web

Example `src/pages/index.html`:

```html
---
pub struct Props {
    pub title: String,
}
---
<AppLayout title={title}>
  <h1 slot="header">{{ title }}</h1>
  <p>Hello from Pilcrow.</p>
</AppLayout>
```

## 2. Compile templates in `build.rs`

```rust
use std::{env, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let src_root = manifest.join("src");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());

    pilcrow_routekit::compile_to_out_dir(&src_root, &out).expect("compile templates");
}
```

## 3. Include generated modules in web runtime

```rust
mod generated_templates {
    include!(concat!(env!("OUT_DIR"), "/generated_templates.rs"));
}
```

## 4. Render compiled template in handler

```rust
async fn index() -> Response {
    let props = generated_templates::page_index::Props {
        title: "Pilcrow".to_string(),
    };

    match generated_templates::page_index::render_page_index(props) {
        Ok(markup) => Html(markup).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, Html(err.to_string())).into_response(),
    }
}
```

## 5. Call backend APIs from web handlers

Web handlers build `Props` using data from backend API clients, not direct DB calls.
