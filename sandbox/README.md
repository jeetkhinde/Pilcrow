# Sandbox Workspace

This workspace is the canonical Pilcrow consumer app.

## Structure

```text
sandbox/
  apps/
    web/       # BFF + SSR + generated pages/api routes
    backend/   # in-memory todo service + REST API
  crates/
    contracts/
    api-client-rest/
    api-client-grpc/
```

## Run

```bash
cargo run -p pilcrow-backend
cargo run -p web
```

## Convention Guard

```bash
cargo run -p pilcrow-cli -- check-arch
```
