# Template Engines

Pilcrow is template-agnostic — `html()` accepts any `impl Into<String>`. This guide shows how to integrate two popular engines: **Maud** (compile-time macros) and **Askama** (file-based templates).

## Maud

[Maud](https://maud.lambda.xyz/) uses Rust macros to generate HTML at compile time. No template files, no runtime parsing.

### Setup

```toml
# Cargo.toml
[dependencies]
maud = "0.26"
```

### Layout Function

Create a reusable layout that wraps page content:

```rust
use maud::{html, Markup, DOCTYPE};
use pilcrow::assets;

fn layout(title: &str, content: Markup) -> String {
    html! {
        (DOCTYPE)
        html {
            head {
                title { (title) " — MyApp" }
                (maud::PreEscaped(assets::script_tag()))
            }
            body {
                nav {
                    a s-action="/" { "Home" }
                    " | "
                    a s-action="/items" { "Items" }
                }
                main { (content) }
            }
        }
    }
    .into_string()
}
```

> **Key detail:** `assets::script_tag()` returns raw HTML, so wrap it in `maud::PreEscaped()` to avoid double-escaping.

### Full-Page Handler

```rust
use pilcrow::*;

async fn item_page(req: SilcrowRequest) -> Result<Response, Response> {
    let item = db.get_item(42).await?;

    let content = maud::html! {
        div.item-detail {
            h1 { (item.name) }
            p.price { "$" (item.price) }
            p.description { (item.description) }
        }
    };

    respond!(req, {
        html => pilcrow::html(layout("Item", content)),
        json => json(&item),
    })
}
```

### Partial (Fragment) for Targeted Updates

When Silcrow.js navigates with `s-target`, you only need the inner content — skip the layout:

```rust
async fn item_partial(req: SilcrowRequest) -> Result<Response, Response> {
    let item = db.get_item(42).await?;

    // Just the fragment — no <html>, <head>, layout
    let fragment = maud::html! {
        div.item-detail {
            h1 { (item.name) }
            p.price { "$" (item.price) }
        }
    }
    .into_string();

    respond!(req, {
        html => pilcrow::html(fragment),
        json => json(&item),
    })
}
```

Silcrow.js swaps this fragment into the `s-target` element. See [Partials & Targets](partials-and-targets.md) for details.

---

## Askama

[Askama](https://github.com/djc/askama) uses Jinja2-style `.html` template files with compile-time checking.

### Setup

```toml
# Cargo.toml
[dependencies]
askama = "0.12"
```

### Templates Directory

```
templates/
├── base.html           # Layout with block inheritance
├── item.html           # Full page (extends base)
└── item_fragment.html  # Partial for targeted updates
```

### Base Template

```html
{# templates/base.html #}
<!DOCTYPE html>
<html>
<head>
  <title>{% block title %}MyApp{% endblock %}</title>
  {{ script_tag }}
</head>
<body>
  <nav>
    <a s-action="/">Home</a> |
    <a s-action="/items">Items</a>
  </nav>
  <main>{% block content %}{% endblock %}</main>
</body>
</html>
```

### Full-Page Template

```html
{# templates/item.html #}
{% extends "base.html" %}

{% block title %}{{ item.name }} — MyApp{% endblock %}

{% block content %}
<div class="item-detail">
  <h1>{{ item.name }}</h1>
  <p class="price">${{ item.price }}</p>
  <p>{{ item.description }}</p>
</div>
{% endblock %}
```

### Fragment Template

```html
{# templates/item_fragment.html #}
<div class="item-detail">
  <h1>{{ item.name }}</h1>
  <p class="price">${{ item.price }}</p>
</div>
```

### Handler

```rust
use askama::Template;
use pilcrow::*;

#[derive(Template)]
#[template(path = "item.html")]
struct ItemPage<'a> {
    item: &'a Item,
    script_tag: String,
}

#[derive(Template)]
#[template(path = "item_fragment.html")]
struct ItemFragment<'a> {
    item: &'a Item,
}

async fn item_page(req: SilcrowRequest) -> Result<Response, Response> {
    let item = db.get_item(42).await?;

    let markup = ItemPage {
        item: &item,
        script_tag: assets::script_tag(),
    }
    .render()
    .unwrap();

    respond!(req, {
        html => html(markup),
        json => json(&item),
    })
}
```

---

## Choosing Between Full-Page and Partial

The same handler often needs to return either a full page or a fragment, depending on how the request arrived:

| Request Source | What to Return |
| --- | --- |
| Browser address bar (first load) | Full page with layout |
| Silcrow.js `s-action` click (no `s-target`) | Full page (replaces `<body>`) |
| Silcrow.js `s-action` with `s-target="#main"` | Partial fragment only |
| API client (`Accept: application/json`) | JSON (no HTML needed) |

You can check `req.is_silcrow` to decide:

```rust
async fn items(req: SilcrowRequest) -> Result<Response, Response> {
    let items = db.list_items().await?;

    let markup = if req.is_silcrow {
        // Silcrow.js request — return fragment
        render_items_fragment(&items)
    } else {
        // Browser or first load — return full page
        layout("Items", render_items_fragment(&items))
    };

    respond!(req, {
        html => html(markup),
        json => raw items,
    })
}
```

> **Tip:** With Askama's block inheritance, you can use `{% extends %}` for full pages and render the `{% block content %}` alone for fragments.

## Sharing Data Between Arms

Both HTML and JSON arms often need the same data. Fetch it **before** `respond!`:

```rust
async fn dashboard(req: SilcrowRequest) -> Result<Response, Response> {
    // Data fetch runs once, shared by both arms
    let stats = db.get_stats().await?;
    let recent = db.recent_activity(10).await?;

    let markup = render_dashboard(&stats, &recent);

    respond!(req, {
        html => html(layout("Dashboard", markup)),
        json => json(&serde_json::json!({
            "stats": stats,
            "recent": recent,
        })),
    })
}
```

## Next Steps

- [Forms & Mutations](forms-and-mutations.md) — handle POST requests with templates
- [Partials & Targets](partials-and-targets.md) — deep dive into partial rendering
