# File-Based Templates

Pilcrow app docs use one template model:

- Astro-style `.html` files with `---` Rust frontmatter
- `pages`, `components`, and `layouts`
- build-time compilation to Rust render functions

## Page Example

```html
---
pub struct Props {
    pub title: String,
    pub done: bool,
}
---
<AppLayout title={title}>
  <h2 slot="header">{{ title }}</h2>
  <TodoCard title={title} done={done} />
</AppLayout>
```

## Layout Example

```html
---
pub struct Props {
    pub title: String,
}
---
<!doctype html>
<html>
<head><title>{{ title }}</title></head>
<body>
  <header><slot name="header" /></header>
  <main><slot /></main>
</body>
</html>
```

## Runtime Usage

```rust
let props = generated_templates::page_todos_id::Props {
    title: "Todo #1".to_string(),
    done: false,
};

let html = generated_templates::page_todos_id::render_page_todos_id(props)?;
```

No alternate template engine path is documented for app developers.
