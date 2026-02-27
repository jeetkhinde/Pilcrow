# WebSocket Multiplex Refactor Plan

## Goal
Move from "one WebSocket per live element" to "one WebSocket per URL per page", while preserving existing public APIs and behavior where possible.

## Current State
- Each `s-live="ws:..."` element opens its own socket.
- Connection state is keyed by element (`liveConnections`) and indexed by URL (`liveConnectionsByUrl`).
- Duplicate sockets are common when multiple elements use the same WS endpoint.

## Target State
- A single socket is shared for each normalized WS URL.
- Multiple elements subscribe to that shared connection.
- Inbound events are dispatched to explicit `target` first, otherwise to subscriber roots.
- Existing APIs continue to work:
  - `s-live="ws:/path"`
  - `Silcrow.send(root, data)`
  - `Silcrow.disconnect(root_or_url)`
  - `Silcrow.reconnect(root_or_url)`

## Non-Goals (Phase 1)
- No Rust protocol changes (`WsEvent` shape remains unchanged).
- No server-side subscription protocol in this phase.
- No breaking changes to `s-live` semantics.

## Design

### Data Model
- Keep `liveConnections: Map<Element, state>` for per-element lookup.
- Replace per-element socket ownership with hub ownership:
  - `wsHubsByUrl: Map<string, WsHub>`
  - `WsHub` fields:
    - `url: string`
    - `socket: WebSocket | null`
    - `subscribers: Set<Element>`
    - `backoff: number`
    - `paused: boolean`
    - `reconnectTimer: number | null`
    - `status: "idle" | "connecting" | "open" | "closed"`
- `state` becomes a lightweight subscription record:
  - `element`
  - `url`
  - `protocol: "ws"`
  - `hub` reference

### Routing Rules
- `patch/html/invalidate`:
  - If message has `target`: resolve selector and apply once.
  - Else: apply to each subscriber element for that URL.
- `navigate`: run once per message (not once per subscriber).
- `custom`: dispatch one document event per message with hub URL + optional root context.

### Lifecycle Rules
- First subscriber to URL creates/connects hub.
- Additional subscribers attach to existing hub without opening a new socket.
- Unsubscribing last element closes socket and removes hub.
- Backoff/reconnect is hub-level, not element-level.

## Implementation Phases

### Phase 1: Introduce WS Hub Layer (No API Changes)
Files:
- `silcrow/ws.js`
- `silcrow/live.js`

Tasks:
- Add `wsHubsByUrl` and hub helpers:
  - `getOrCreateWsHub(url)`
  - `connectWsHub(hub)`
  - `closeWsHub(hub, reason)`
  - `pruneWsHubIfEmpty(hub)`
- Update `openWsLive(root, url)`:
  - Normalize URL.
  - Unsubscribe element from previous hub if needed.
  - Subscribe element to the target hub.
  - Ensure hub is connected.
- Update `pauseLiveState`/`unregisterLiveState` behavior for WS:
  - Remove subscriber from hub instead of closing socket directly.
  - Close hub only when subscriber count reaches zero.

Acceptance:
- Multiple elements with same URL open exactly one socket.
- Replacing one element's URL does not drop other subscribers on original URL.

### Phase 2: Message Dispatch Semantics
Files:
- `silcrow/ws.js`

Tasks:
- Refactor `socket.onmessage` handling to be hub-based.
- Implement helper to iterate current subscribers safely.
- Ensure untargeted payloads fan out to each subscriber root.
- Ensure targeted payloads apply once to resolved selector.
- Keep existing event names (`silcrow:live:connect`, `silcrow:live:disconnect`, `silcrow:ws:*`).

Acceptance:
- Existing message variants still work.
- No duplicate `navigate` actions for multi-subscriber hub.

### Phase 3: Control APIs (`send`, `disconnect`, `reconnect`)
Files:
- `silcrow/ws.js`
- `silcrow/live.js`

Tasks:
- `Silcrow.send(root, data)`:
  - Resolve matching states.
  - Deduplicate hubs before sending (one send per hub).
- `disconnect(root_or_url)`:
  - For element: unsubscribe that element only.
  - For URL: mark hub paused and close socket, retain subscribers.
- `reconnect(root_or_url)`:
  - For element: reconnect its hub.
  - For URL: reconnect that hub and all subscribers.

Acceptance:
- `disconnect("/ws/chat")` affects one shared socket.
- `disconnect("#one")` keeps socket alive if other subscribers remain.

### Phase 4: Cleanup + Docs + Compatibility Notes
Files:
- `SILCROW.md`
- `ws-checklist.md`
- `readme.md` (if WS behavior is described there)

Tasks:
- Document new default: one connection per URL.
- Clarify fan-out rules for untargeted messages.
- Add notes on when to use unique URLs for isolation.

Acceptance:
- Docs match runtime behavior.
- Manual checklist includes multiplex verification.

## Detailed Edge Cases
- Element removed from DOM:
  - If observer-based cleanup exists, ensure it unsubscribes from hub.
  - If not, add cleanup hook (or explicitly document lifecycle responsibility).
- Rapid mount/unmount:
  - Avoid reconnect storms by keeping short reconnect debounce at hub level.
- Mixed protocols:
  - SSE flow remains unchanged.
  - WS-only changes must not affect SSE maps and events.

## Suggested Optional Enhancement
- Add opt-out isolation attribute:
  - `s-ws-isolated` creates a dedicated socket even for same URL.
  - Keep disabled by default.

## Validation Plan

### Automated
- Add JS unit tests (or integration harness) for:
  - one socket for two elements same URL
  - two sockets for two different URLs
  - subscriber removal does not close shared socket until last subscriber
  - `send` deduplicates by hub
  - untargeted patch fans out; targeted patch applies once

### Manual
- Extend `ws-checklist.md` with:
  - [ ] Two elements on same URL -> single Network WS entry
  - [ ] Closing one subscriber keeps socket alive for remaining subscriber
  - [ ] URL-level disconnect/reconnect works for all subscribers

## Rollout Strategy
1. Implement behind internal branch with no public API change.
2. Verify with manual checklist and existing flow.
3. Update docs in same PR.
4. Optionally add `s-ws-isolated` in a follow-up PR, only if needed.

## Risks
- Event fan-out bugs causing duplicate DOM updates.
- Subtle regressions in `disconnect/reconnect` semantics.
- Memory leaks if subscriber cleanup is incomplete.

## Exit Criteria
- Socket count equals unique WS URLs, not element count.
- Existing demos and current WS checklist still pass.
- No API-breaking change required for existing users.
