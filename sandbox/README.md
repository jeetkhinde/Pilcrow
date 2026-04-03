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

## Configuration

Sandbox runtime config lives in `Pilcrow.toml`:

```toml
[web]
host = "127.0.0.1"
port = 3000
backend_url = "http://127.0.0.1:4000"

[backend]
host = "127.0.0.1"
port = 4000
```

Environment variables override file values:
- `PILCROW_WEB_HOST`
- `PILCROW_WEB_PORT`
- `PILCROW_BACKEND_URL`
- `PILCROW_BACKEND_HOST`
- `PILCROW_BACKEND_PORT`

## Convention Guard

```bash
cargo install --path ../tools/cli --force
pilcrow-cli check-arch
```
