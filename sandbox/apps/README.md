# Sandbox Apps

## Roles

- `apps/web`: BFF + SSR UI routes. Browser entrypoint.
- `apps/backend`: domain/service/repository + REST JSON APIs.

## Request Path

`Browser -> web -> backend -> web -> Browser`

## Ports

- web: `127.0.0.1:3000`
- backend REST: `127.0.0.1:4000`
