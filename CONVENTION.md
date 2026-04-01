# Pilcrow Mandatory App Convention

## Required Project Shape

- `apps/web`: UI routing (SSR/pages/components), DTO mapping, backend API calls.
- `apps/backend`: services, models, repositories, DB, auth, middleware, REST/JSON and gRPC service surface.
- `crates/contracts`: shared request/response contracts.

## Required Framework Usage

- `apps/web` must depend on `pilcrow-web`.
- `apps/backend` must depend on `pilcrow-core`.

## Request Flow (Default)

- Browser -> `apps/web` (BFF route) -> `apps/backend` API -> `apps/web` response.
- Browser direct calls to backend are not part of the default convention.

## Protocol Policy

- REST/JSON is primary for `web` -> `backend`.
- gRPC is available for internal/service usage.

## Enforcement

Run:

```bash
cargo run -p pilcrow-cli -- check-arch
```

The check validates mandatory dependency boundaries.
