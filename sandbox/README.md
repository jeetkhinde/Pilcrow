# Sandbox Workspace

This workspace is the canonical Pilcrow consumer app.

## Structure

```text
sandbox/
  apps/
    web/       # BFF + SSR + generated pages/api routes
    backend/   # in-memory todo service + REST API
```

## Run

```bash
cargo run -p pilcrow-backend
cargo run -p web
```

## Convention Guard

```bash
cargo install --path ../tools/cli --force
pilcrow-cli check-arch
```
