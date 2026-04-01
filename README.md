# Pilcrow Monorepo

Pilcrow has one mandatory app architecture.

## App Flow

`Browser -> apps/web -> apps/backend -> apps/web -> Browser`

- `apps/web` handles UI routes and HTML rendering.
- `apps/backend` handles domain logic and JSON APIs.

## Workspace Shape

```text
apps/
  web/
  backend/
crates/
  pilcrow-core/
  pilcrow-web/
  contracts/
  api-client-rest/
  api-client-grpc/
  pilcrow/
  routekit/
  pilcrow-macros/
tools/
  pilcrow-cli/
```

## Mandatory Rendering Path

1. Define UI in file-based templates under web (`pages/components/layouts`).
2. Compile templates with `pilcrow-routekit` in `build.rs`.
3. Use generated Rust render functions in web handlers.
4. Web handlers call backend via API clients.

## Source of Truth

- [CONVENTION.md](CONVENTION.md)
- [apps/README.md](apps/README.md)

## Validate Convention

```bash
cargo run -p pilcrow-cli -- check-arch
```
