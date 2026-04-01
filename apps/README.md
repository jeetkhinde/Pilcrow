# Apps

This directory follows Pilcrow's mandatory convention.

## Roles

- `apps/web`: BFF + SSR UI routes. Browser entrypoint.
- `apps/backend`: domain/services/repositories/auth + REST/JSON APIs.

## Request Path

`Browser -> web -> backend -> web -> Browser`

## Ports

- web: `127.0.0.1:3000`
- backend REST: `127.0.0.1:4000`

## Run Locally

```bash
cargo run -p pilcrow-backend
cargo run -p pilcrow-web-app
```

## Convention Guard

```bash
cargo run -p pilcrow-cli -- check-arch
```
