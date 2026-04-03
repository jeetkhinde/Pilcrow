# Pilcrow Monorepo

Pilcrow has one mandatory app architecture.

## App Flow

`Browser -> apps/web -> apps/backend -> apps/web -> Browser`

- `apps/web` handles UI routes and HTML rendering.
- `apps/backend` handles domain logic and JSON APIs.

## Workspace Shape

```text
crates/
  pilcrow-core/
  pilcrow-web/
  pilcrow-runtime/
  routekit/
  pilcrow-macros/
tools/
  pilcrow-cli/
sandbox/
  apps/
    web/
    backend/
```

## Mandatory Rendering Path

1. Define UI in file-based templates under web (`pages/components/layouts`) with explicit component/layout imports in frontmatter.
2. Compile templates with `pilcrow-routekit` in `build.rs` via `routekit::compile_current_crate_sources()`.
3. Use generated Rust render functions in web handlers.
4. Web handlers call backend via API clients.

## Runtime Config

Use a workspace `Pilcrow.toml` for bind host/port and backend URL. Apps can override via:
- `PILCROW_WEB_HOST`
- `PILCROW_WEB_PORT`
- `PILCROW_BACKEND_URL`
- `PILCROW_BACKEND_HOST`
- `PILCROW_BACKEND_PORT`

## Source of Truth

- [CONVENTION.md](CONVENTION.md)
- [sandbox/README.md](sandbox/README.md)

## Validate Convention

```bash
cargo install --path tools/cli --force
cd sandbox
pilcrow-cli check-arch
```
