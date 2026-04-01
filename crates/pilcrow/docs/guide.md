# Pilcrow Guide (Single Path)

This guide documents one app path only.

## Mandatory Architecture

- `apps/web`: file-based UI routes and SSR rendering
- `apps/backend`: services/models/repositories/auth + REST/JSON APIs
- request flow: `Browser -> web -> backend -> web -> Browser`

## Reading Order

1. [Getting Started](getting-started.md)
2. [File-Based Templates](templates.md)
3. [Forms & Mutations](forms-and-mutations.md)
4. [Partials & Targets](partials-and-targets.md)
5. [Server-Sent Events](sse-guide.md)
6. [WebSocket](ws-guide.md)
7. [Response Modifiers](response-modifiers.md)

All examples assume web calls backend APIs and never touches DB directly.
