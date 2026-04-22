use std::fs;
use std::path::{Path, PathBuf};

pub fn handle_new(args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        return Err("expected: new <dir>".to_string());
    }

    let root = PathBuf::from(&args[0]);
    if root.exists() {
        return Err(format!("destination already exists: {}", root.display()));
    }

    create_scaffold(&root).map_err(|err| err.to_string())?;
    println!("created Pilcrow app at {}", root.display());
    Ok(())
}

fn create_scaffold(root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(root.join("src/pages"))?;
    fs::create_dir_all(root.join("src/ui"))?;
    fs::create_dir_all(root.join("src/api"))?;

    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "app"
version = "0.1.0"
edition = "2024"

[dependencies]
pilcrow-web = { git = "https://github.com/jeetkhinde/Pilcrow" }

[build-dependencies]
pilcrow-routekit = { git = "https://github.com/jeetkhinde/Pilcrow" }
"#,
    )?;

    fs::write(
        root.join("Pilcrow.toml"),
        r#"[web]
host = "127.0.0.1"
port = 3000
"#,
    )?;

    fs::write(
        root.join("build.rs"),
        r#"fn main() {
    let src = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("src");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    pilcrow_routekit::compile_to_out_dir(&src, &out).expect("compile pilcrow html sources");
    for dir in pilcrow_routekit::watched_source_directories(&src) {
        println!("cargo:rerun-if-changed={}", dir.display());
    }
}
"#,
    )?;

    fs::write(
        root.join("src/main.rs"),
        r#"pilcrow_web::pilcrow_app!();

#[tokio::main]
async fn main() {
    pilcrow_web::start(pilcrow_router()).await;
}
"#,
    )?;

    // Root auto-layout: wraps every page automatically (no import needed).
    // Use a named `title` slot so each page can set its own <title>.
    fs::write(
        root.join("src/pages/_layout.html"),
        r#"---
pub struct Props {}
---
<!doctype html>
<html lang="en">
<head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <slot name="title"><title>My App</title></slot>
</head>
<body>
    <slot />
    <script src="{{ pilcrow_web::assets::assets::silcrow_js_path() }}" defer></script>
</body>
</html>
"#,
    )?;

    // Page: show data from load() and a minimal form wired to a named action.
    fs::write(
        root.join("src/pages/index.html"),
        r#"<Fragment slot="title"><title>Home — My App</title></Fragment>
<main>
    <h1>{{ greeting }}</h1>
    <form s-post="?/greet" s-target="main" id="main" method="post" action="?/greet">
        <label>
            Name
            <input type="text" name="name" />
        </label>
        <button type="submit">Say hello</button>
    </form>
</main>
"#,
    )?;

    // Code-behind: demonstrates load() with Req and a named action fn.
    fs::write(
        root.join("src/pages/index.rs"),
        r#"pub struct Props {
    pub greeting: String,
}

pub async fn load(req: Req) -> AppResult<Props> {
    let name = req.query.get("name").unwrap_or("world");
    Ok(Props {
        greeting: format!("Hello, {name}!"),
    })
}

// Named action: POSTs to `?/greet` are dispatched to this function.
pub async fn greet(req: Req) -> ActionResult {
    let name = req.form.get("name").unwrap_or("world").trim().to_owned();
    redirect(&format!("/?name={name}"))
}
"#,
    )?;

    // 404 fallback page.
    fs::write(
        root.join("src/pages/_not_found.html"),
        r#"<Fragment slot="title"><title>404 Not Found — My App</title></Fragment>
<main>
    <h1>404 — Page not found</h1>
    <p><a s-get="/">Go home</a></p>
</main>
"#,
    )?;

    Ok(())
}
