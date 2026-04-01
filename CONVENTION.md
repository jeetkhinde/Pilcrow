# Pilcrow Mandatory App Convention

This file is the source of truth for how Pilcrow apps are built.

## Required Project Shape

- `apps/web`: UI routing (SSR/pages/components), request orchestration, backend API calls.
- `apps/backend`: services, models, repositories, DB, auth, middleware, REST/JSON APIs.
- `crates/contracts`: shared request/response DTOs.

## Required Framework Usage

- `apps/web` must depend on `pilcrow-web`.
- `apps/backend` must depend on `pilcrow-core`.

## Dependency Boundaries

- `apps/web` must not depend on backend internals.
- `apps/backend` must not depend on web UI crates/templates.
- Cross-app DTOs must live in `crates/contracts`.

## Single Rendering Path (Mandatory)

- UI pages are defined as file-based templates (`pages/components/layouts`).
- Templates are compiled at build time into Rust render functions.
- `apps/web` handlers call generated render functions and return HTML.

`respond!` is not part of the default app convention for page rendering.

## Request Flow (Mandatory)

`Browser -> apps/web -> apps/backend -> apps/web -> Browser`

- Browser direct calls to backend are not part of the default convention.
- Backend returns JSON APIs to web.

## Enforcement

```bash
cargo run -p pilcrow-cli -- check-arch
```

The check validates required dependency boundaries.
