me: can we improve this ugly syntax?

{% for item in items %} {{ Card { title: item.title, body: item.body } }} {% endfor %}
AI: Astro uses JSX-like component syntax:

```astro
{items.map(item => <Card title={item.title} body={item.body} />)}
```

**For Pilcrow, we could do:**

```html
<Card :for="item in items" title="{{ item.title }}" body="{{ item.body }}" />
```

Or even cleaner — treat components exactly like HTML elements:

```html
{% for item in items %}
    <Card title="{{ item.title }}" body="{{ item.body }}" />
{% endfor %}
```

**The problem:** Askama doesn't support this. To get this syntax we need a custom template parser.

**This is the fork in the road:**

- **Stick with Askama** → component syntax stays ugly, everything else works today
- **Custom template parser in `build.rs`** → clean component syntax, but now we're building a template engine

And a custom template parser inside `page!{}` proc-macro brings us right back to the `.pilcrow` file discussion — because at that point the HTML block inside `page!{}` IS a custom language.

**Honest take:** the clean component syntax `<Card />` is only achievable with either:

1. `.pilcrow` files + custom parser
2. A full template engine built for Pilcrow

The ugly syntax is the Askama tax. Which path do you want?

me:
{% for item in items %} <Card title="{{ item.title }}" body="{{ item.body }}" /> {% endfor %}

when we see this we can convert to  {% for item in items %} {{ Card { title: item.title, body: item.body } }} {% endfor %} ?? or use for!()?

me: more chat.
AI:
**The full picture:**

```
pages/news.rs
─────────────────────────────────────────
page!(News {
    layout: Dashboard,
    title: String,
    items: Vec<NewsItem>,

    async fn load(State(db): State<Db>) -> Self {
        Self { title: "News".into(), items: db.get_news().await? }
    }

    async fn create(Form(input): Form<CreateNews>) -> Response {
        db.create(input).await?;
        navigate("/news").with_toast("Created!", "success")
    }
    ---
    {% for item in items %}
        <Card title="{{ item.title }}" body="{{ item.body }}" />
    {% endfor %}
})
```

**What `page!{}` expands to internally:**

1. Askama `#[derive(Template)]` struct with owned fields
2. `load()` → GET Axum handler with `respond!` baked in
3. `create()` → POST Axum handler
4. `build.rs` preprocessor transforms `<Card />` → Askama calls
5. Layout inheritance via Askama `{% extends %}`

**The layers:**

```
dev writes     →  page!{} + <Component /> syntax
macro expands  →  Rust struct + Askama template
build.rs       →  transforms <Card /> → Askama calls
Askama         →  compile-time validated HTML
Pilcrow        →  respond!, content negotiation, SSE, WS
Axum           →  routing, extractors, middleware
```

**What the dev never sees:**

- Lifetimes
- `respond!` macro
- `IntoResponse`
- Askama derive boilerplate
- `SilcrowRequest` extractor

**What stays explicit:**

- Rust types (unavoidable, but just `String`, `Vec<T>`, `bool`)
- Axum extractors in `load()` / `create()` (familiar, documented)
- Route registration (one line per page)

**The honest gap vs Astro:**

- Route registration is still manual (`.page::<News>("/news")`)
- `String` instead of inferred JS types
- No hot reload without recompile

**My take:** this is achievable, coherent, and closer to Astro than anything in the Rust ecosystem today. The pieces are all proven — proc-macros, Askama, `build.rs` preprocessing. Nothing experimental.

Should this be `pilcrow-pages` as a separate opt-in crate, or core?


Me:
how silcrow compliments it?

ai:
**Without Silcrow — traditional SSR (like Astro static):**

```rust
page!(News {
    items: Vec<NewsItem>,
    async fn load(State(db): State<Db>) -> Self { ... }
    ---
    {% for item in items %}
        <Card title="{{ item.title }}" body="{{ item.body }}" />
    {% endfor %}
})
```

Full page reload on every interaction. That's it.

**With Silcrow — SSR + reactivity without writing JS:**

```rust
page!(News {
    items: Vec<NewsItem>,
    
    async fn load(State(db): State<Db>) -> Self { ... }
    
    async fn create(Form(input): Form<CreateNews>) -> Response {
        db.create(input).await?;
        navigate("/news").with_toast("Created!", "success")
    }

    async fn delete(Path(id): Path<i64>) -> Response {
        db.delete(id).await?;
        // no redirect — Silcrow patches the DOM
        json(&serde_json::json!({ "items": { "id": id, "_remove": true } }))
    }
    ---
    <ul s-list="items" s-template="item-tpl">
        {% for item in items %}
            <Card id="{{ item.id }}" title="{{ item.title }}" />
        {% endfor %}
    </ul>

    <template id="item-tpl">
        <Card s-key=".id" title="{{ item.title }}" />
    </template>
})
```

**What Silcrow adds on top of `page!{}`:**

|Feature|Without Silcrow|With Silcrow|
|---|---|---|
|Form submit|Full page reload|Partial DOM update|
|Delete item|Full page reload|`_remove` tombstone, no reload|
|Live updates|Impossible|`s-live` + SSE baked into page|
|Optimistic UI|Impossible|`Silcrow.optimistic()`|
|Navigation|Full reload|Client-side, cached|
|Toasts|Cookie on reload|Instant, no reload|

**The real power — real-time page with zero JS written:**

```rust
page!(Dashboard {
    layout: Base,
    stats: Stats,

    async fn load(State(db): State<Db>, Extension(sse): Extension<SseRoute>) -> Self {
        Self { stats: db.get_stats().await? }
    }
    ---
    <div s-live="/events/dashboard">
        <Stats total="{{ stats.total }}" active="{{ stats.active }}" />
    </div>
})
```

Server pushes updates via SSE → Silcrow patches `<Stats />` in place → zero JS written by the dev.

**The mental model:**

- `page!{}` = Astro (SSR, structure, data fetching)
- Silcrow = Astro Islands (reactivity, live updates, partial DOM)
- Together = full Astro experience without leaving Rust

The dev writes HTML + Rust types. Silcrow handles everything the browser needs to feel fast and reactive.


me: write full specs

ai:
# Pilcrow Pages — Full Specification

---

## 1. Overview

`pilcrow-pages` is an optional crate that adds Astro-like DX to Pilcrow. It provides file-based conventions, collocated templates, and automatic route wiring — without leaving Rust or sacrificing type safety.

**Stack:**

```
page!{} / component!{} / layout!{}  ← dev-facing macros
        ↓
Askama (compile-time templates)      ← template engine
        ↓
build.rs preprocessor                ← <Component /> → Askama
        ↓
Pilcrow (respond!, SSE, WS)          ← response layer
        ↓
Axum (routing, extractors)           ← HTTP layer
        ↓
Silcrow.js (DOM, SSE, WS client)     ← browser runtime
```

---

## 2. Crate Structure

```
pilcrow-pages/
  src/
    lib.rs          — re-exports
    page.rs         — page!{} macro
    layout.rs       — layout!{} macro
    component.rs    — component!{} macro
    router.rs       — RouterExt trait (.page::<T>())
    render.rs       — PilcrowRender trait
  build.rs          — <Component /> preprocessor
```

**Cargo.toml:**

```toml
[dependencies]
pilcrow = { path = "../core" }
askama = "0.12"
axum = "0.7"
serde = { features = ["derive"] }
serde_json = "1"
```

---

## 3. Macros

### 3.1 `layout!{}`

```rust
layout!(Base {
    title: String,
    ---
    <!DOCTYPE html>
    <html>
    <head>
        <title>{{ title }}</title>
        {{ pilcrow_scripts }}
    </head>
    <body>
        {% block content %}{% endblock %}
    </body>
    </html>
})
```

**Rules:**

- Must contain `{% block content %}{% endblock %}`
- `{{ pilcrow_scripts }}` injects `pilcrow::assets::script_tag()`
- No `load()` — layouts receive props from pages only
- Fields are owned types only (`String`, `Vec<T>`, `bool`, etc.)
- No `'a` lifetimes permitted

**Expands to:**

```rust
#[derive(askama::Template)]
#[template(source = "...", ext = "html")]
struct Base {
    title: String,
    pilcrow_scripts: String,
}

impl Base {
    fn new(title: String) -> Self {
        Self {
            title,
            pilcrow_scripts: pilcrow::assets::script_tag(),
        }
    }
}
```

---

### 3.2 `component!{}`

```rust
component!(Card {
    title: String,
    body: String,
    active: bool,
    ---
    <div class="card {% if active %}active{% endif %}">
        <h2>{{ title }}</h2>
        <p>{{ body }}</p>
    </div>
})
```

**Rules:**

- No `load()` — props only, no async
- No layout
- Renderable standalone as a partial response
- All fields owned types
- Implements `PilcrowRender` trait

**Expands to:**

```rust
#[derive(askama::Template, serde::Serialize)]
#[template(source = "...", ext = "html")]
struct Card {
    title: String,
    body: String,
    active: bool,
}

impl PilcrowRender for Card {}
```

**Standalone partial use:**

```rust
async fn update_card(req: SilcrowRequest) -> Result<Response, Response> {
    let card = Card {
        title: "Updated".into(),
        body: "New body".into(),
        active: true,
    };
    respond!(req, {
        html => card.render()?,
        json => json(&card),
    })
}
```

---

### 3.3 `page!{}`

```rust
page!(News {
    // Layout (optional)
    layout: Base,

    // Fields — all owned types
    title: String,
    items: Vec<NewsItem>,

    // GET handler (optional — defaults to 404 if omitted)
    async fn load(State(db): State<Db>) -> Self {
        let items = db.get_news().await?;
        Self {
            title: "News".into(),
            items,
        }
    }

    // POST handler (optional)
    async fn create(
        State(db): State<Db>,
        Form(input): Form<CreateNews>,
    ) -> Response {
        db.create(input).await?;
        navigate("/news").with_toast("Created!", "success")
    }

    // PUT/PATCH handler (optional)
    async fn update(
        State(db): State<Db>,
        Path(id): Path<i64>,
        Form(input): Form<UpdateNews>,
    ) -> Response {
        db.update(id, input).await?;
        navigate("/news").with_toast("Updated!", "success")
    }

    // DELETE handler (optional)
    async fn delete(
        State(db): State<Db>,
        Path(id): Path<i64>,
    ) -> Response {
        db.delete(id).await?;
        json(&serde_json::json!({ "items": { "id": id, "_remove": true } }))
    }

    // Private helper — not exposed as route
    fn format_date(date: &str) -> String {
        // ...
    }
    ---
    <form s-action="/news" POST>
        <input name="title" placeholder="Title" />
        <button type="submit">Create</button>
    </form>

    <ul s-list="items" s-template="news-tpl">
        {% for item in items %}
            <Card
                title="{{ item.title }}"
                body="{{ item.body }}"
                active="{{ item.active }}"
            />
        {% endfor %}
    </ul>

    <template id="news-tpl">
        <Card
            s-key=".id"
            title=".title"
            body=".body"
            active=".active"
        />
    </template>
})
```

**Handler convention:**

|Method name|HTTP method|Route|
|---|---|---|
|`load()`|GET|`/path`|
|`create()`|POST|`/path`|
|`update()`|PUT/PATCH|`/path/:id`|
|`delete()`|DELETE|`/path/:id`|
|anything else|—|private helper|

**`load()` return type rules:**

- Must return `Self` or `Result<Self, Response>`
- Axum extractors injected automatically
- `?` works — errors become `500` or whatever `IntoResponse` the error type produces

**Expands to:**

```rust
// 1. Template struct
#[derive(askama::Template, serde::Serialize)]
#[template(source = "...", ext = "html")]
struct NewsTemplate {
    title: String,
    items: Vec<NewsItem>,
    pilcrow_scripts: String,
}

// 2. Page struct (public)
struct News;

impl News {
    async fn load_handler(
        req: SilcrowRequest,
        State(db): State<Db>,
    ) -> Result<Response, Response> {
        let page = NewsTemplate::load(State(db)).await?;
        respond!(req, {
            html => html(page.render()?),
            json => json(&page),
        })
    }

    async fn create_handler(
        State(db): State<Db>,
        Form(input): Form<CreateNews>,
    ) -> Response {
        // body of create()
    }
}

// 3. PilcrowPage trait impl
impl PilcrowPage for News {
    fn get_handler() -> MethodRouter { get(Self::load_handler) }
    fn post_handler() -> MethodRouter { post(Self::create_handler) }
}
```

---

## 4. `PilcrowRender` Trait

```rust
pub trait PilcrowRender: askama::Template + serde::Serialize {
    fn render(&self) -> Result<HtmlResponse, Response> {
        askama::Template::render(self)
            .map(pilcrow::html)
            .map_err(|e| {
                tracing::error!("Template render failed: {e}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            })
    }
}
```

Both `page!{}` and `component!{}` implement this. Layouts do not — they're only used internally by pages.

---

## 5. `RouterExt` Trait

```rust
pub trait RouterExt {
    fn page<P: PilcrowPage>(self, path: &str) -> Self;
    fn page_with_id<P: PilcrowPage>(self, path: &str) -> Self;
}

impl RouterExt for Router {
    fn page<P: PilcrowPage>(self, path: &str) -> Self {
        self.route(path, P::get_handler())
            .route(path, P::post_handler())
    }

    fn page_with_id<P: PilcrowPage>(self, path: &str) -> Self {
        let id_path = format!("{path}/:id");
        self.route(path, P::get_handler())
            .route(path, P::post_handler())
            .route(&id_path, P::put_handler())
            .route(&id_path, P::delete_handler())
    }
}
```

**Usage:**

```rust
let app = Router::new()
    .page::<Index>("/")
    .page::<News>("/news")
    .page_with_id::<News>("/news")  // adds /news/:id routes
    .page::<Dashboard>("/dashboard");
```

---

## 6. `build.rs` Preprocessor

Runs before Askama sees any template. Two transforms:

### 6.1 Component tag transform

**Input:**

```html
<Card title="{{ item.title }}" body="{{ item.body }}" active="{{ item.active }}" />
```

**Output:**

```
{{ Card { title: item.title, body: item.body, active: item.active } }}
```

**Rule:** `<[A-Z][a-zA-Z]*` → component call. Lowercase tags pass through untouched.

### 6.2 Self-closing component with s-key (for templates)

**Input:**

```html
<Card s-key=".id" title=".title" body=".body" />
```

**Output:**

```html
<div s-key=".id">
    <!-- rendered by Silcrow client-side via s-bind -->
    <h2 s-bind=".title"></h2>
    <p s-bind=".body"></p>
</div>
```

This allows components to work both server-side (in `{% for %}` loops) and client-side (in `<template>` tags for Silcrow's `s-list`).

---

## 7. Template Syntax Reference

Everything Askama supports, plus component tags:

```html
<!-- components -->
<Card title="{{ item.title }}" />

<!-- conditionals -->
{% if user.is_admin %}
    <AdminPanel name="{{ user.name }}" />
{% else %}
    <UserPanel name="{{ user.name }}" />
{% endif %}

<!-- match -->
{% match status %}
    {% when Status::Active %}
        <ActiveBadge />
    {% when Status::Inactive %}
        <InactiveBadge />
{% endmatch %}

<!-- loops -->
{% for item in items %}
    <Card title="{{ item.title }}" />
{% endfor %}

<!-- loop with filter -->
{% for item in items if item.active %}
    <Card title="{{ item.title }}" />
{% endfor %}

<!-- let binding -->
{% let count = items.len() %}
<p>{{ count }} items</p>

<!-- layout block -->
{% block content %}{% endblock %}

<!-- includes -->
{% include "partials/footer.html" %}

<!-- silcrow reactive list -->
<ul s-list="items" s-template="item-tpl">
    {% for item in items %}
        <Card title="{{ item.title }}" />
    {% endfor %}
</ul>

<template id="item-tpl">
    <Card s-key=".id" title=".title" body=".body" />
</template>
```

---

## 8. Silcrow Integration

Pages and components are Silcrow-aware by default:

### 8.1 Reactive page with SSE

```rust
page!(Dashboard {
    layout: Base,
    stats: Stats,

    async fn load(
        State(db): State<Db>,
        Extension(sse_route): Extension<SseRoute>,
    ) -> Self {
        Self { stats: db.get_stats().await? }
    }
    ---
    <div id="stats" s-live="/events/dashboard">
        <StatCard label="Total" value="{{ stats.total }}" />
        <StatCard label="Active" value="{{ stats.active }}" />
    </div>
})
```

### 8.2 Optimistic updates

```html
<button
    s-action="/news/{{ item.id }}/like"
    POST
    onclick="Silcrow.optimistic('#item-{{ item.id }}', { likes: {{ item.likes }} + 1 })"
>
    Like
</button>
```

### 8.3 Partial component response

```rust
// returns just the Card HTML, Silcrow patches it into the DOM
async fn refresh_card(
    req: SilcrowRequest,
    Path(id): Path<i64>,
    State(db): State<Db>,
) -> Result<Response, Response> {
    let item = db.get_item(id).await?;
    let card = Card {
        title: item.title,
        body: item.body,
        active: item.active,
    };
    respond!(req, {
        html => card.render()?,
        json => json(&card),
    })
}
```

---

## 9. Type Constraints

All page/component/layout fields must satisfy:

```rust
// required
T: Clone + Serialize + 'static

// forbidden in field position
&'a T       // no references
Arc<T>      // no shared ownership
Mutex<T>    // no locks
Box<dyn T>  // no trait objects
```

If a field needs shared state, it comes via Axum extractors in `load()` — not as a field on the struct.

---

## 10. Error Handling

```rust
// load() can return Result
async fn load(State(db): State<Db>) -> Result<Self, Response> {
    let items = db.get_news().await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
    })?;
    Ok(Self { items })
}

// or use AppError pattern
async fn load(State(db): State<Db>) -> Result<Self, AppError> {
    Ok(Self { items: db.get_news().await? })
}
```

Any error type implementing `IntoResponse` works. Pilcrow never forces a specific error type.

---

## 11. What the Dev Never Sees

|Hidden|Why|
|---|---|
|`SilcrowRequest` extractor|Injected by `page!{}` macro|
|`respond!` macro|Generated inside `load_handler`|
|`#[derive(Template)]`|Generated by `page!{}`|
|`pilcrow_scripts` field|Injected by `layout!{}`|
|`PilcrowPage` trait impl|Generated by `page!{}`|
|`<Card />` → Askama transform|`build.rs` preprocessor|
|Lifetime annotations|Forbidden by type constraints|

---

## 12. What Stays Explicit

|Explicit|Why|
|---|---|
|Rust types (`String`, `Vec<T>`)|Unavoidable, but simple|
|Axum extractors in handlers|Familiar, documented, powerful|
|Route registration|One line per page, intentional|
|`State<T>` for shared state|Axum convention, unchanged|

---

## 13. File Convention (Recommended)

```
src/
  pages/
    index.rs          → /
    news.rs           → /news
    dashboard.rs      → /dashboard
  layouts/
    base.rs
    dashboard.rs
  components/
    card.rs
    stat_card.rs
    nav.rs
  main.rs             → Router wiring
```

---

## 14. Full Example

```rust
// components/card.rs
component!(Card {
    title: String,
    body: String,
    ---
    <div class="card">
        <h2>{{ title }}</h2>
        <p>{{ body }}</p>
    </div>
})

// layouts/base.rs
layout!(Base {
    title: String,
    ---
    <!DOCTYPE html>
    <html>
    <head>
        <title>{{ title }}</title>
        {{ pilcrow_scripts }}
    </head>
    <body>
        <nav>
            <a s-action="/">Home</a>
            <a s-action="/news">News</a>
        </nav>
        {% block content %}{% endblock %}
    </body>
    </html>
})

// pages/news.rs
page!(News {
    layout: Base,
    title: String,
    items: Vec<NewsItem>,

    async fn load(State(db): State<Db>) -> Result<Self, AppError> {
        Ok(Self {
            title: "News".into(),
            items: db.get_news().await?,
        })
    }

    async fn create(
        State(db): State<Db>,
        Form(input): Form<CreateNews>,
    ) -> Response {
        db.create(input).await?;
        navigate("/news").with_toast("Created!", "success")
    }

    async fn delete(
        State(db): State<Db>,
        Path(id): Path<i64>,
    ) -> Response {
        db.delete(id).await?;
        json(&serde_json::json!({ "items": { "id": id, "_remove": true } }))
    }
    ---
    <form s-action="/news" POST>
        <input name="title" placeholder="What's the news?" />
        <button type="submit">Post</button>
    </form>

    <ul s-list="items" s-template="news-tpl">
        {% for item in items %}
            <Card title="{{ item.title }}" body="{{ item.body }}" />
        {% endfor %}
    </ul>

    <template id="news-tpl">
        <Card s-key=".id" title=".title" body=".body" />
    </template>
})

// main.rs
#[tokio::main]
async fn main() {
    let db = Db::connect().await;

    let app = Router::new()
        .page::<Index>("/")
        .page_with_id::<News>("/news")
        .page::<Dashboard>("/dashboard")
        .route(DASH_EVENTS.path(), get(dashboard_stream))
        .with_state(db);

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

---

## 15. Out of Scope (for now)

|Feature|Why deferred|
|---|---|
|File-based routing|Requires `build.rs` directory scanning + significant codegen|
|Hot reload|Requires file watcher, separate tooling concern|
|Image optimization|Separate concern, use existing crates|
|MDX / markdown|Additive, not core|
|`.pilcrow` file format|Deferred until LSP is feasible|
|i18n|Additive, not core|

---

**Implementation order:**

1. `PilcrowRender` trait + `component!{}`
2. `layout!{}`
3. `page!{}` with `load()` only
4. `RouterExt` trait
5. `build.rs` `<Component />` preprocessor
6. `page!{}` mutation handlers (`create`, `update`, `delete`)
7. Silcrow integration (`s-list` template components)

me:
loooks good. will rust allows ---? or cargo check or rust analyser?

ai:
No. `---` is not valid Rust syntax. The proc-macro receives a `TokenStream` — the Rust tokenizer runs first, before the macro sees anything. It will fail at the tokenizer level, not even reaching the macro.

**What the tokenizer allows as separators:**

```rust
// ❌ --- invalid token
page!(News {
    title: String,
    ---
    <h1>{{ title }}</h1>
})

// ✅ => valid, already used in respond!
page!(News {
    title: String,
    =>
    <h1>{{ title }}</h1>
})

// ✅ --- as a string literal
page!(News {
    title: String,
    "---"
    <h1>{{ title }}</h1>
})

// ✅ custom keyword
page!(News {
    title: String,
    template {
        "<h1>{{ title }}</h1>"
    }
})

// ✅ HTML as string literal
page!(News {
    title: String,
    template: "
        <h1>{{ title }}</h1>
    "
})
```

**My recommendation — `template {}` block:**

```rust
page!(News {
    layout: Base,
    title: String,
    items: Vec<NewsItem>,

    async fn load(State(db): State<Db>) -> Self { ... }

    template {
        <ul>
            {% for item in items %}
                <Card title="{{ item.title }}" />
            {% endfor %}
        </ul>
    }
})
```

Clean, readable, valid Rust token. `rust-analyzer` won't understand the HTML inside but won't error on the structure either.


me: 

ai: 
**`.html` file:**

```html
---
layout: Base
fields:
  title: String
  items: Vec<NewsItem>
---

<ul>
  {% for item in items %}
    <Card title="{{ item.title }}" />
  {% endfor %}
</ul>
```

- VS Code HTML syntax highlighting works **out of the box**
- Askama already uses `.html` files
- The `---` frontmatter is exactly Jekyll/Astro convention
- Devs immediately recognize it

**`.yaml` frontmatter for handlers is the problem:**

```yaml
---
layout: Base
fields:
  title: String
  items: Vec<NewsItem>
load: |
  let items = db.get_news().await?;
  Self { title: "News".into(), items }
---
```

Rust code inside YAML strings is terrible. No syntax highlighting, no type checking, indentation sensitive.

**Hybrid — `.html` with typed frontmatter:**

```html
---
layout: Base
title: String
items: Vec<NewsItem>
---
<!-- handlers stay in a companion .rs file -->

<ul>
  {% for item in items %}
    <Card title="{{ item.title }}" />
  {% endfor %}
</ul>
```

Companion `.rs` file for handlers:

```rust
// pages/news.rs
impl News {
    async fn load(State(db): State<Db>) -> Self {
        Self { title: "News".into(), items: db.get_news().await? }
    }
}
```

**My recommendation: `.html` with YAML frontmatter for fields/layout, companion `.rs` for handlers.**

- HTML file → perfect syntax highlighting today
- YAML frontmatter → fields and layout only, no Rust code in YAML
- `.rs` companion → full `rust-analyzer` for handlers

```
pages/
  news.html    ← template + field declarations
  news.rs      ← handlers (optional, only if mutations needed)
```


ai:
Astro keeps everything in one `.astro` file because JavaScript can live inline:

```astro
---
// this is just JS/TS — any editor understands it
import Card from '../components/Card.astro';
const { title } = Astro.props;
const items = await db.getNews();
---

<ul>
  {items.map(item => <Card title={item.title} />)}
</ul>
```

**Why Astro can do this and we can't:**

- JS/TS has no compiler step — editors understand it inline anywhere
- No type system enforcement at file boundaries
- Vite handles the file splitting at build time invisibly

**The Rust constraint we can't escape:**

- `rustc` must see valid `.rs` files
- Handlers need the full Rust compiler (types, borrows, async)
- You can't inline real Rust inside HTML without a compiler pipeline

**So the honest comparison:**

||Astro|Pilcrow best case|
|---|---|---|
|Template|`.astro` (one file)|`.html` file|
|Handler|Same `.astro` file|companion `.rs` file|
|Fields|Inferred from JS|YAML frontmatter|
|Components|`import Card`|auto-discovered by convention|
|Tooling|Full VSCode support|HTML highlighting + rust-analyzer|

**The unavoidable Rust tax is exactly one extra file per page.**

`.html` + `.rs` companion is the closest we can get to Astro without building a full compiler pipeline. Two files instead of one — that's the real cost.

Is that acceptable?

Yes. That's the `.pilcrow` approach but without a new extension.

```
pages/
  news.html          ← template + frontmatter
  news.rs            ← handlers (optional)
  index.html
  dashboard.html
  dashboard.rs

components/
  card.html
  stat_card.html

layouts/
  base.html
  dashboard.html
```

**`build.rs` scans these directories and generates valid Rust:**

```
pages/news.html + pages/news.rs
        ↓ build.rs
src/generated/pages/news.rs  ← valid Rust, never touched by dev
```

**`news.html`:**

```html
---
layout: Base
title: String
items: Vec<NewsItem>
---

<ul>
  {% for item in items %}
    <Card title="{{ item.title }}" />
  {% endfor %}
</ul>
```

**`news.rs` (optional — only for handlers):**

```rust
impl News {
    async fn load(State(db): State<Db>) -> Self {
        Self { title: "News".into(), items: db.get_news().await? }
    }

    async fn create(Form(input): Form<CreateNews>) -> Response {
        navigate("/news").with_toast("Created!", "success")
    }
}
```

**`build.rs` generates:**

```rust
// generated — never edit this file
#[derive(askama::Template, serde::Serialize)]
#[template(path = "pages/news.html")]
struct NewsTemplate {
    title: String,
    items: Vec<NewsItem>,
    pilcrow_scripts: String,
}

struct News;
impl News {
    async fn load_handler(req: SilcrowRequest, ...) -> Result<Response, Response> {
        let page = News::load(...).await?;
        respond!(req, {
            html => html(page.render()?),
            json => json(&page),
        })
    }
}

impl PilcrowPage for News { ... }
```

**`main.rs` — router auto-generated too:**

```rust
// option 1 — fully auto
let app = pilcrow_pages::router(); // build.rs generated all routes

// option 2 — explicit, dev controls order/middleware
let app = Router::new()
    .page::<News>("/news")
    .page::<Dashboard>("/dashboard");
```

**What VS Code sees:**

- `.html` files → HTML syntax highlighting, Askama highlighting with extension
- `.rs` files → full `rust-analyzer`
- Generated files → hidden in `target/`, never visible

**The full pipeline:**

```
pages/*.html + pages/*.rs
      ↓
build.rs (parse frontmatter, generate structs, wire routes)
      ↓
Askama (compile-time template validation)
      ↓
rustc (full type checking on generated + handler code)
      ↓
Pilcrow + Axum (runtime)
```

**What the dev never touches:**

- Generated Rust structs
- `respond!` macro
- `SilcrowRequest` extractor
- Route registration (if using auto-router)
- `PilcrowPage` trait

**What stays in dev's hands:**

- `.html` files (fields + template)
- `.rs` companion (handlers only when needed)
- `main.rs` (app config, state, middleware)

This is genuinely close to Astro. One directory convention, two file types devs already know. Should we update the full spec with this approach?

me: Why can't handlers live in html file  inside --- here ----

ai:
They can. `build.rs` sees the raw file before `rustc` — it can parse anything between `---` delimiters.

```html
---
layout: Base
title: String
items: Vec<NewsItem>

async fn load(State(db): State<Db>) -> Self {
    Self { 
        title: "News".into(), 
        items: db.get_news().await? 
    }
}

async fn create(Form(input): Form<CreateNews>) -> Response {
    db.create(input).await?;
    navigate("/news").with_toast("Created!", "success")
}
---

<ul>
  {% for item in items %}
    <Card title="{{ item.title }}" />
  {% endfor %}
</ul>
```

`build.rs` splits on `---`, parses the frontmatter section as:

1. YAML-style field declarations (`title: String`)
2. Rust `fn` blocks (anything starting with `async fn` or `fn`)

Generates valid `.rs` file from both. `rustc` never sees the `.html` file.

**This is identical to Astro's mental model:**

|Astro|Pilcrow|
|---|---|
|`---` JS/TS block|`---` Rust block|
|HTML below|HTML below|
|One file|One file|
|Vite parses it|`build.rs` parses it|

**The only remaining question:** does `rust-analyzer` work inside the `---` block?

No — not without an LSP. But `build.rs` can emit errors with file + line number pointing back to the `.html` file, so `cargo check` at least gives useful errors.

This is the cleanest design. One file, one convention, closest to Astro. Update the full spec?

me:
correct. we can spit one file to two files, one rust another askama using build.rs file - if i am right. but you're rust expert here. you can guide better.

ai:
You're exactly right. That's the correct mental model.

**What `build.rs` does:**

```
pages/news.html
      ↓ build.rs splits it
      
OUT_DIR/pages/news.rs        ← pure Rust (struct + handlers)
OUT_DIR/pages/news_template.html  ← pure Askama template
```

**The split:**

```html
<!-- pages/news.html — dev writes this -->
---
layout: Base
title: String
items: Vec<NewsItem>

async fn load(State(db): State<Db>) -> Self {
    Self { title: "News".into(), items: db.get_news().await? }
}
---

<ul>
  {% for item in items %}
    <Card title="{{ item.title }}" />
  {% endfor %}
</ul>
```

**`build.rs` emits two files:**

```rust
// OUT_DIR/news.rs — valid Rust
#[derive(askama::Template, Serialize)]
#[template(path = "news_template.html")]
struct NewsTemplate {
    title: String,
    items: Vec<NewsItem>,
    pilcrow_scripts: String,
}

impl NewsTemplate {
    async fn load(State(db): State<Db>) -> Self {
        Self { title: "News".into(), items: db.get_news().await? }
    }
}
```

```html
<!-- OUT_DIR/news_template.html — valid Askama -->
{% extends "base.html" %}
{% block content %}
<ul>
  {% for item in items %}
    {{ Card { title: item.title } }}
  {% endfor %}
</ul>
{% endblock %}
```

**Then `lib.rs` includes the generated file:**

```rust
include!(concat!(env!("OUT_DIR"), "/news.rs"));
```

**The pipeline:**

```
pages/news.html
      ↓
build.rs — splits frontmatter + HTML
      ↓
OUT_DIR/news.rs          → rustc → full type checking
OUT_DIR/news_template.html → Askama → compile-time template validation
      ↓
cargo check catches errors in both
      ↓
error messages mapped back to news.html line numbers
```

**The one hard part:** mapping `rustc` errors from `OUT_DIR/news.rs` back to `pages/news.html` line numbers. This requires emitting `#[track_caller]` or procedural spans — doable but non-trivial.

**Everything else is straightforward `build.rs` string processing.** No proc-macros needed for the core pipeline. `build.rs` is just Rust code that reads files and writes files.

Should we spec the `build.rs` parser in detail?

me:
why will it error when we emited valid rust?

ai:
You're right. It won't.

me:
how about if build.rs just compiles to rust code using askana - the askana part and remaing part to pure rust. I mean if we convert to pure rust then i dn't think there won't be any problem

ai:
Exactly right. Skip Askama entirely.

`build.rs` converts everything to pure Rust:

```html
<!-- pages/news.html — dev writes -->
---
layout: Base
title: String
items: Vec<NewsItem>

async fn load(State(db): State<Db>) -> Self {
    Self { title: "News".into(), items: db.get_news().await? }
}
---

<ul>
  {% for item in items %}
    <Card title="{{ item.title }}" />
  {% endfor %}
</ul>
```

**`build.rs` converts entire file to pure Rust:**

```rust
// OUT_DIR/news.rs — pure Rust, no Askama
struct NewsTemplate {
    title: String,
    items: Vec<NewsItem>,
}

impl NewsTemplate {
    async fn load(State(db): State<Db>) -> Self {
        Self { title: "News".into(), items: db.get_news().await? }
    }

    fn render(&self) -> String {
        let mut buf = String::new();
        buf.push_str("<ul>");
        for item in &self.items {
            buf.push_str(&Card {
                title: item.title.clone(),
            }.render());
        }
        buf.push_str("</ul>");
        buf
    }
}
```

**What `build.rs` translates:**

|Template syntax|Pure Rust|
|---|---|
|`{% for item in items %}`|`for item in &self.items {`|
|`{% if condition %}`|`if condition {`|
|`{% match val %}`|`match val {`|
|`{{ title }}`|`buf.push_str(&self.title)`|
|`<Card title="{{ item.title }}" />`|`buf.push_str(&Card { title: item.title.clone() }.render())`|
|`{% block content %}`|function call into layout|

**Benefits:**

- `rustc` sees pure Rust — full type checking
- `cargo check` works perfectly
- Errors point to generated Rust — one level removed but fully readable
- No Askama dependency at all
- No template engine at runtime
- Zero overhead — just string concatenation compiled by Rust

**The only dependency is `build.rs` itself** — a file parser + Rust code generator. No external crates needed.

This is essentially what Maud does — but generated from HTML syntax instead of Rust macro syntax.

Spec the `build.rs` parser and code generator in detail?

me:
that is also correct but I was only saying to this way.

what is inside --- converts to pure rust ---- what is after --- build step convert to askana and then askana converts to pure rust.



ai:
Perfect. That's cleaner — let each tool do what it's best at.

```
pages/news.html
      ↓
build.rs splits on ---

ABOVE ---     → pure Rust (already valid, just extracted)
BELOW ---     → Askama template → Askama compiles to pure Rust
      ↓
single OUT_DIR/news.rs — pure Rust, full type checking
```

**The split is trivial:**

```rust
// build.rs
let content = fs::read_to_string("pages/news.html")?;
let parts: Vec<&str> = content.splitn(3, "---").collect();

let frontmatter = parts[1]; // fields + handlers → pure Rust
let template    = parts[2]; // HTML → Askama
```

**Each tool does one job:**

- `build.rs` — splits the file, nothing more
- Askama — compiles HTML template to Rust (what it already does)
- `rustc` — compiles everything to binary

**Error experience is now perfect:**

- Handler errors → `rustc` on extracted Rust code
- Template errors → Askama on extracted HTML
- Frontmatter field errors → `build.rs` on `.html` file directly

No custom template engine. No string concatenation. Just two proven tools doing what they already do.

Simplest possible architecture. Spec the `build.rs` in detail?


me:
yes.

ai:
# `build.rs` Parser — Full Specification

---

## 1. What it does

```
Input:  pages/*.html, components/*.html, layouts/*.html
Output: OUT_DIR/generated.rs  ← included by lib.rs
        OUT_DIR/templates/*   ← Askama template files
```

---

## 2. File Format

```html
---
layout: Base
title: String
items: Vec<NewsItem>

async fn load(State(db): State<Db>) -> Self {
    Self { title: "News".into(), items: db.get_news().await? }
}

async fn create(Form(input): Form<CreateNews>) -> Response {
    navigate("/news").with_toast("Created!", "success")
}
---

<ul>
  {% for item in items %}
    <Card title="{{ item.title }}" />
  {% endfor %}
</ul>
```

**Split rule:** `content.splitn(3, "---")` gives:

- `parts[0]` → empty (before first `---`)
- `parts[1]` → frontmatter (fields + handlers)
- `parts[2]` → HTML template

---

## 3. Frontmatter Parser

Frontmatter has two sections — field declarations and handler functions:

```
layout: Base          ← layout declaration (optional)
title: String         ← field declaration
items: Vec<NewsItem>  ← field declaration
                      ← blank line separator
async fn load(...) {  ← handler block
    ...
}
```

**Parser logic:**

```rust
struct Frontmatter {
    layout: Option<String>,
    fields: Vec<Field>,
    handlers: Vec<Handler>,
}

struct Field {
    name: String,   // "title"
    ty: String,     // "String"
}

struct Handler {
    raw: String,    // entire "async fn load(...) { ... }" verbatim
    method: HttpMethod,  // GET, POST, PUT, DELETE, private
}

enum HttpMethod {
    Get,     // load()
    Post,    // create()
    Put,     // update()
    Delete,  // delete()
    Private, // anything else
}
```

**Parsing steps:**

```rust
fn parse_frontmatter(input: &str) -> Frontmatter {
    let mut layout = None;
    let mut fields = vec![];
    let mut handlers = vec![];

    let mut lines = input.lines().peekable();

    while let Some(line) = lines.peek() {
        let trimmed = line.trim();

        // layout declaration
        if trimmed.starts_with("layout:") {
            layout = Some(trimmed["layout:".len()..].trim().to_string());
            lines.next();
        }
        // field declaration — "name: Type"
        else if looks_like_field(trimmed) {
            let (name, ty) = parse_field(trimmed);
            fields.push(Field { name, ty });
            lines.next();
        }
        // handler — "async fn" or "fn"
        else if trimmed.starts_with("async fn") || trimmed.starts_with("fn") {
            let raw = collect_fn_block(&mut lines); // collects until balanced }
            let method = infer_method(&raw);
            handlers.push(Handler { raw, method });
        }
        else {
            lines.next(); // skip blank lines, comments
        }
    }

    Frontmatter { layout, fields, handlers }
}

fn looks_like_field(s: &str) -> bool {
    // "title: String" — has colon, no parens, no braces
    s.contains(':') && !s.contains('(') && !s.contains('{')
}

fn infer_method(raw: &str) -> HttpMethod {
    if raw.contains("fn load") { HttpMethod::Get }
    else if raw.contains("fn create") { HttpMethod::Post }
    else if raw.contains("fn update") { HttpMethod::Put }
    else if raw.contains("fn delete") { HttpMethod::Delete }
    else { HttpMethod::Private }
}

fn collect_fn_block(lines: &mut Peekable<Lines>) -> String {
    let mut buf = String::new();
    let mut depth = 0;
    for line in lines.by_ref() {
        depth += line.chars().filter(|&c| c == '{').count();
        depth -= line.chars().filter(|&c| c == '}').count();
        buf.push_str(line);
        buf.push('\n');
        if depth == 0 && !buf.trim().is_empty() { break; }
    }
    buf
}
```

---

## 4. Template Preprocessor

Before passing to Askama, transform `<Component />` tags:

```rust
fn preprocess_template(input: &str) -> String {
    // matches <Card title="{{ item.title }}" body="{{ item.body }}" />
    let component_re = Regex::new(r#"<([A-Z][a-zA-Z]*)\s*([^>]*?)\s*/>"#).unwrap();
    
    component_re.replace_all(input, |caps: &Captures| {
        let name = &caps[1];           // "Card"
        let attrs = &caps[2];          // `title="{{ item.title }}" body="{{ item.body }}"`
        let fields = parse_attrs(attrs); // → "title: item.title, body: item.body"
        format!("{{{{ {name} {{ {fields} }} }}}}")
    }).to_string()
}

fn parse_attrs(attrs: &str) -> String {
    // title="{{ item.title }}" body="{{ item.body }}"
    // →  title: item.title, body: item.body
    let attr_re = Regex::new(r#"(\w+)="\{\{\s*(.+?)\s*\}\}""#).unwrap();
    attr_re.captures_iter(attrs)
        .map(|cap| format!("{}: {}", &cap[1], &cap[2]))
        .collect::<Vec<_>>()
        .join(", ")
}
```

---

## 5. Code Generator

From parsed `Frontmatter`, generate complete Rust:

```rust
fn generate_rust(
    name: &str,           // "News"
    fm: &Frontmatter,
    template_path: &str,  // "OUT_DIR/templates/news.html"
) -> String {
    let struct_fields = fm.fields.iter()
        .map(|f| format!("    pub {}: {},", f.name, f.ty))
        .collect::<Vec<_>>()
        .join("\n");

    let layout_extends = fm.layout.as_ref()
        .map(|l| format!(r#"{{% extends "{}.html" %}}{{% block content %}}"#, 
            l.to_lowercase()))
        .unwrap_or_default();

    let layout_end = fm.layout.as_ref()
        .map(|_| "{% endblock %}".to_string())
        .unwrap_or_default();

    let handlers = fm.handlers.iter()
        .map(|h| h.raw.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");

    let route_methods = generate_route_methods(name, &fm.handlers);
    let template_name = format!("{}Template", name);

    format!(r#"
// ── {name} ─────────────────────────────────────────────────
// generated from pages/{lowercase}.html — do not edit

#[derive(askama::Template, serde::Serialize)]
#[template(path = "{template_path}")]
pub struct {template_name} {{
{struct_fields}
    pub pilcrow_scripts: String,
}}

pub struct {name};

impl {template_name} {{
{handlers}
}}

impl pilcrow_pages::PilcrowPage for {name} {{
{route_methods}
}}
"#,
        name = name,
        lowercase = name.to_lowercase(),
        template_name = template_name,
        struct_fields = struct_fields,
        template_path = template_path,
        handlers = handlers,
        route_methods = route_methods,
    )
}

fn generate_route_methods(name: &str, handlers: &[Handler]) -> String {
    let template_name = format!("{}Template", name);
    let mut methods = vec![];

    for handler in handlers {
        match handler.method {
            HttpMethod::Get => methods.push(format!(r#"
    fn get_handler() -> axum::routing::MethodRouter {{
        axum::routing::get(|req: pilcrow::SilcrowRequest, state| async move {{
            let page = {template_name}::load(state).await?;
            pilcrow::respond!(req, {{
                html => pilcrow::html(askama::Template::render(&page)?),
                json => pilcrow::json(&page),
            }})
        }})
    }}"#, template_name = template_name)),

            HttpMethod::Post => methods.push(format!(r#"
    fn post_handler() -> axum::routing::MethodRouter {{
        axum::routing::post({template_name}::create)
    }}"#, template_name = template_name)),

            HttpMethod::Put => methods.push(format!(r#"
    fn put_handler() -> axum::routing::MethodRouter {{
        axum::routing::put({template_name}::update)
    }}"#, template_name = template_name)),

            HttpMethod::Delete => methods.push(format!(r#"
    fn delete_handler() -> axum::routing::MethodRouter {{
        axum::routing::delete({template_name}::delete)
    }}"#, template_name = template_name)),

            HttpMethod::Private => {} // not exposed
        }
    }

    methods.join("\n")
}
```

---

## 6. Template Writer

Write preprocessed HTML to `OUT_DIR` for Askama:

```rust
fn write_template(
    name: &str,
    template_html: &str,
    fm: &Frontmatter,
    out_dir: &Path,
) -> PathBuf {
    let preprocessed = preprocess_template(template_html);

    // wrap with layout extends if needed
    let final_template = match &fm.layout {
        Some(layout) => format!(
            "{{% extends \"{}.html\" %}}\n{{% block content %}}\n{}\n{{% endblock %}}",
            layout.to_lowercase(),
            preprocessed
        ),
        None => preprocessed,
    };

    let path = out_dir
        .join("templates")
        .join(format!("{}.html", name.to_lowercase()));

    fs::write(&path, final_template).unwrap();
    path
}
```

---

## 7. Main `build.rs` Entry Point

```rust
fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let templates_dir = out_dir.join("templates");
    fs::create_dir_all(&templates_dir).unwrap();

    let mut generated = String::new();

    // Process all directories
    for dir in &["pages", "components", "layouts"] {
        let path = PathBuf::from(dir);
        if !path.exists() { continue; }

        for entry in fs::read_dir(&path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();

            if path.extension().and_then(|e| e.to_str()) != Some("html") {
                continue;
            }

            // tell cargo to rerun if file changes
            println!("cargo:rerun-if-changed={}", path.display());

            let content = fs::read_to_string(&path).unwrap();
            let name = path.file_stem().unwrap().to_str().unwrap();
            let name = to_pascal_case(name); // "news" → "News"

            // split on ---
            let parts: Vec<&str> = content.splitn(3, "---").collect();
            if parts.len() != 3 {
                panic!("{}:  missing --- delimiter", path.display());
            }

            let frontmatter = parse_frontmatter(parts[1]);
            let template_path = write_template(&name, parts[2], &frontmatter, &out_dir);
            let rust_code = generate_rust(&name, &frontmatter, 
                template_path.to_str().unwrap());

            generated.push_str(&rust_code);
        }
    }

    // write single generated file
    fs::write(out_dir.join("generated.rs"), generated).unwrap();
}

fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().to_string() + c.as_str(),
            }
        })
        .collect()
}
```

---

## 8. `lib.rs` Inclusion

```rust
// pilcrow-pages/src/lib.rs
include!(concat!(env!("OUT_DIR"), "/generated.rs"));
```

---

## 9. Error Reporting

`build.rs` panics with file + line number on parse errors:

```rust
fn parse_field(line: &str, file: &str, line_no: usize) -> Field {
    let parts: Vec<&str> = line.splitn(2, ':').collect();
    if parts.len() != 2 {
        panic!("{}:{}: invalid field declaration: `{}`", file, line_no, line);
    }
    Field {
        name: parts[0].trim().to_string(),
        ty: parts[1].trim().to_string(),
    }
}
```

`cargo check` output:

```
error: pages/news.html:3: invalid field declaration: `titlee String`
```

---

## 10. `PilcrowPage` Trait

```rust
// pilcrow-pages/src/lib.rs
pub trait PilcrowPage {
    fn get_handler() -> axum::routing::MethodRouter {
        axum::routing::get(|| async { StatusCode::METHOD_NOT_ALLOWED })
    }
    fn post_handler() -> axum::routing::MethodRouter {
        axum::routing::post(|| async { StatusCode::METHOD_NOT_ALLOWED })
    }
    fn put_handler() -> axum::routing::MethodRouter {
        axum::routing::put(|| async { StatusCode::METHOD_NOT_ALLOWED })
    }
    fn delete_handler() -> axum::routing::MethodRouter {
        axum::routing::delete(|| async { StatusCode::METHOD_NOT_ALLOWED })
    }
}

pub trait RouterExt {
    fn page<P: PilcrowPage>(self, path: &str) -> Self;
    fn page_with_id<P: PilcrowPage>(self, path: &str) -> Self;
}

impl RouterExt for Router {
    fn page<P: PilcrowPage>(self, path: &str) -> Self {
        self.route(path, P::get_handler())
            .route(path, P::post_handler())
    }

    fn page_with_id<P: PilcrowPage>(self, path: &str) -> Self {
        let id_path = format!("{path}/:id");
        self.route(path, P::get_handler())
            .route(path, P::post_handler())
            .route(&id_path, P::put_handler())
            .route(&id_path, P::delete_handler())
    }
}
```

---

## 11. Implementation Order

1. `build.rs` file splitter (`splitn(3, "---")`)
2. Frontmatter parser (fields + layout)
3. Template writer (pass-through to Askama)
4. Struct generator (fields → Rust struct)
5. `PilcrowPage` trait + `RouterExt`
6. Handler extraction + route method generation
7. `<Component />` preprocessor
8. Error reporting with file + line numbers
9. `component!{}` and `layout!{}` support
10. Auto-router (`pilcrow_pages::router()`)

---

**Branch:** `feat/pilcrow-pages`

Approve to start Phase 1 (file splitter + frontmatter parser)?