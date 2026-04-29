# Pilcrow Architecture Priorities (DX + DRY)

> Date: 2026-04-29
> Context: codebase review with existing SvelteKit/Next.js comparison docs, including confirmed server hook support.

## What is already strong (and should be treated as baseline)

- **Server hooks already exist** (`handle`, `handle_error`, `init`) and are usable today, so this is not a missing feature. The gap is primarily discoverability + canonical usage in docs/MCP.  
- **Runtime safety defaults are partially in place** (request timeout and HTTP tracing are wired).  
- **Adapter abstraction exists** (`PilcrowAdapter`) and can support non-default deployment targets.  

---

## Critical priority

### 1) MCP/registry discoverability debt for shipped features
**Problem:** Features that already exist (hooks, typed routes/env, CSRF, ISR inspection/export paths) are not consistently surfaced as "canonical" in MCP guidance, so users/agents still generate manual/duplicated patterns.

**Why critical:** This is a multiplier on every generated app and every AI-assisted edit. Missing discoverability creates recurring wrong implementations despite existing primitives.

**Improvements:**
- Add/expand `canonical_usage` in `registry.toml` for hooks-first request lifecycle, typed route helpers, typed env loading, CSRF setup, and ISR inspect/export workflows.
- Add feature aliases/synonyms in MCP search for natural queries ("server hooks", "global error hook", "form actions", "cache tags").
- Add a single migration map section (Astro/SvelteKit/Next style concept -> Pilcrow primitive).

**DX/DRY impact:** Removes repetitive hand-rolled auth/session/env glue and string-literal routing.

### 2) ISR cache key correctness under variation
**Problem:** Any mismatch between cache key derivation and request variation dimensions (headers/cookies/user locale/session) risks stale or cross-context content.

**Why critical:** This is a production correctness/safety class issue, not just ergonomics.

**Improvements:**
- Standardize cache-key composition from path + configured vary dimensions.
- Emit explicit diagnostics when `REVALIDATE` is used with request-dependent values but no vary dimensions configured.
- Add tests that assert per-variant separation.

**DX/DRY impact:** Prevents app teams from reinventing defensive cache wrappers.

---

## High priority

### 3) Action/form ergonomics (`req.form.parse::<T>()` + result helpers)
**Problem:** Handlers still repeat manual field extraction, parsing, and error shaping.

**Improvements:**
- Introduce typed form decode (`parse::<T>()`) plus consistent action outcomes (`ok/fail/redirect` helpers).
- Ship one canonical CRUD example with validation + optimistic update flow.

**DX/DRY impact:** Large line-count reduction in mutation handlers; fewer parsing bugs.

### 4) Hook-centered "golden path" docs and starter scaffolds
**Problem:** Hooks are present but easy to underuse; teams duplicate middleware concerns in route handlers.

**Improvements:**
- Publish a canonical "request enters handle -> locals/session/auth -> action/load" flow.
- Add scaffold options that include a ready-to-edit `hooks.rs` with session/auth/trace/csrf defaults.

**DX/DRY impact:** Centralizes cross-cutting logic and reduces route-level duplication.

### 5) Observability ergonomics beyond trace-on/off
**Problem:** Base tracing exists, but production diagnosis still needs better "what failed and why" affordances.

**Improvements:**
- Standard request IDs in logs/errors.
- Structured revalidation failure reporting + retry status.
- Dev diagnostics page for route map, action dispatch, and cache entry lifecycle.

**DX/DRY impact:** Faster debugging; less ad-hoc logging code in apps.

---

## Medium priority

### 6) Adapter capability matrix + constraints documentation
**Problem:** Adapter API exists, but teams lack clear expectations per target (SSE/WS/ISR/filesystem/streaming support).

**Improvements:**
- Publish target-by-target capability table.
- Add compile-time/runtime warnings for unsupported feature combinations per adapter.

### 7) Testability helpers for request lifecycle
**Problem:** App authors may over-rely on full integration tests for `load()`/action logic.

**Improvements:**
- Provide ergonomic request builders/test constructors for unit-testing handler logic.
- Document hook and locals mocking patterns.

### 8) Optional auth/data scaffolding flags
**Problem:** Common "first 200 lines" are repeatedly re-authored.

**Improvements:**
- CLI flags for secure baseline auth/session/data setup.
- Generated examples that route everything through hooks/middleware.

---

## Recommended execution order

1. **Critical-1 discoverability pass (MCP + registry + canonical docs).**
2. **Critical-2 cache-key correctness + tests.**
3. **High-3 action/form typed ergonomics.**
4. **High-4 hook-first starter templates + docs.**
5. **High-5 observability UX surfaces.**
6. Medium items in parallel by ownership.

---

## Bottom line

The highest ROI is to treat Pilcrow as already strong in primitives (especially hooks) and aggressively close the **knowledge-surface gap** first. After that, the biggest DX/DRY win is typed form/action ergonomics, while cache-key correctness remains the top production safety priority.
