# Pilcrow Mandatory App Convention

This file is the source of truth for how Pilcrow apps are built.

## Required Project Shape

- `apps/web`: UI routing (SSR/pages/components), request orchestration, backend API calls.
- `apps/backend`: services, models, repositories, DB, auth, middleware, REST/JSON APIs.

## Required Framework Usage

- `apps/web` must depend on `pilcrow-web`.
- `apps/backend` must depend on `pilcrow-core`.

## Dependency Boundaries

- `apps/web` must not depend on backend internals.
- `apps/backend` must not depend on web UI crates/templates.
- Transport DTOs are app-layer concerns and must stay within app boundaries (no shared framework/demo crate required).

## Single Rendering Path (Mandatory)

- UI pages are defined as file-based templates (`pages/components/layouts`).
- Templates must explicitly import PascalCase components/layouts in frontmatter.
- Templates are compiled at build time into Rust render functions.
- `apps/web` handlers call generated render functions and return HTML.

`respond!` is not part of the default app convention for page rendering.

## Request Flow (Mandatory)

`Browser -> apps/web -> apps/backend -> apps/web -> Browser`

- Browser direct calls to backend are not part of the default convention.
- Backend returns JSON APIs to web.

## Enforcement

```bash
cd sandbox
pilcrow-cli check-arch
```

The check validates required dependency boundaries.
