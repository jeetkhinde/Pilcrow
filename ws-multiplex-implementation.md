# WebSocket Multiplex Implementation Plan

## Overview
Refactor from "one WebSocket per element" to "one WebSocket per URL per page."
JS-only change. No Rust changes. No public API breaking changes.

## Files Touched
- `silcrow/ws.js` — primary changes (hub layer, dispatch, send)
- `silcrow/live.js` — minor changes (cleanup integration, openWsLive delegation)
- `silcrow/index.js` — minor (MutationObserver for DOM cleanup)
- `ws-checklist.md` — updated verification steps
- `SILCROW.md` — document multiplex behavior

## Terminology
- **Hub**: A shared WebSocket connection for a given normalized URL. Owns the socket, backoff, and reconnect timer.
- **Subscriber**: An element registered to receive messages from a hub. Lightweight record.
- **Unsubscribe**: Remove an element from a hub. If last subscriber, close the hub.
- **Disconnect**: Close the hub's socket (all subscribers stop receiving). Subscribers retained for reconnect.

---

## Step 1: Add WsHub data structure

**File:** `silcrow/ws.js`
**What:** Add `wsHubs` map and the `WsHub` factory at the TOP of the file (after `normalizeWsEndpoint`).
**Do NOT** modify any existing functions yet.

```js
// Add after normalizeWsEndpoint function:

const wsHubs = new Map(); // normalized URL → hub object

function createWsHub(url) {
  return {
    url,
    socket: null,
    subscribers: new Set(),
    backoff: 1000,
    paused: false,
    reconnectTimer: null,
  };
}

function getOrCreateWsHub(url) {
  let hub = wsHubs.get(url);
  if (!hub) {
    hub = createWsHub(url);
    wsHubs.set(url, hub);
  }
  return hub;
}

function removeWsHub(hub) {
  if (hub.subscribers.size > 0) return; // safety: don't remove if subscribers exist
  if (hub.reconnectTimer) {
    clearTimeout(hub.reconnectTimer);
    hub.reconnectTimer = null;
  }
  if (hub.socket) {
    hub.socket.close();
    hub.socket = null;
  }
  wsHubs.delete(hub.url);
}
```

**Verify:** No behavior change. Existing code still works. New code is dead (unreachable).

---

## Step 2: Add hub connect function

**File:** `silcrow/ws.js`
**What:** Add `connectWsHub(hub)` function below the hub helpers from Step 1.
**Do NOT** modify existing `connectWS` yet. The new function coexists.

```js
function connectWsHub(hub) {
  if (hub.paused) return;
  if (hub.socket && hub.socket.readyState <= WebSocket.OPEN) return; // already connected/connecting

  const socket = new WebSocket(hub.url);
  hub.socket = socket;

  socket.onopen = function () {
    hub.backoff = 1000;
    document.dispatchEvent(
      new CustomEvent("silcrow:live:connect", {
        bubbles: true,
        detail: {
          url: hub.url,
          protocol: "ws",
          subscribers: Array.from(hub.subscribers),
        },
      })
    );
  };

  socket.onmessage = function (e) {
    dispatchWsMessage(hub, e.data);
  };

  socket.onclose = function () {
    hub.socket = null;
    if (hub.paused) return;
    if (hub.subscribers.size === 0) {
      removeWsHub(hub);
      return;
    }

    const reconnectIn = hub.backoff;

    document.dispatchEvent(
      new CustomEvent("silcrow:live:disconnect", {
        bubbles: true,
        detail: {
          url: hub.url,
          protocol: "ws",
          reconnectIn,
          subscribers: Array.from(hub.subscribers),
        },
      })
    );

    hub.reconnectTimer = setTimeout(function () {
      hub.reconnectTimer = null;
      connectWsHub(hub);
    }, reconnectIn);

    hub.backoff = Math.min(hub.backoff * 2, MAX_BACKOFF);
  };

  socket.onerror = function () {
    // onerror is always followed by onclose per spec
  };
}
```

**Verify:** No behavior change. New function is dead code.

---

## Step 3: Add hub message dispatch

**File:** `silcrow/ws.js`
**What:** Add `dispatchWsMessage(hub, rawData)` function. This replaces the inline `socket.onmessage` logic with hub-aware routing.

```js
function dispatchWsMessage(hub, rawData) {
  try {
    const msg = JSON.parse(rawData);
    const type = msg && msg.type;

    // Targeted messages: resolve selector, apply once
    if (type === "patch") {
      if (msg.target) {
        const target = document.querySelector(msg.target);
        if (target && msg.data !== undefined) patch(msg.data, target);
      } else {
        // Untargeted: fan out to all subscribers
        for (const el of hub.subscribers) {
          if (msg.data !== undefined) patch(msg.data, el);
        }
      }
    } else if (type === "html") {
      if (msg.target) {
        const target = document.querySelector(msg.target);
        if (target) safeSetHTML(target, msg.markup == null ? "" : String(msg.markup));
      } else {
        for (const el of hub.subscribers) {
          safeSetHTML(el, msg.markup == null ? "" : String(msg.markup));
        }
      }
    } else if (type === "invalidate") {
      if (msg.target) {
        const target = document.querySelector(msg.target);
        if (target) invalidate(target);
      } else {
        for (const el of hub.subscribers) {
          invalidate(el);
        }
      }
    } else if (type === "navigate") {
      // Navigate runs once, not per subscriber
      if (msg.path) {
        navigate(msg.path.trim(), {trigger: "ws"});
      }
    } else if (type === "custom") {
      // Custom event dispatched once on document
      document.dispatchEvent(
        new CustomEvent("silcrow:ws:" + (msg.event || "message"), {
          bubbles: true,
          detail: {url: hub.url, data: msg.data},
        })
      );
    } else {
      warn("Unknown WS event type: " + type);
    }
  } catch (err) {
    warn("Failed to parse WS message: " + err.message);
  }
}
```

**Verify:** No behavior change. New function is dead code.

---

## Step 4: Rewrite `openWsLive` to use hubs

**File:** `silcrow/ws.js`
**What:** Replace the existing `openWsLive` function. This is the switchover point.

**Before (current):**
```js
function openWsLive(root, url) {
  const element = typeof root === "string" ? document.querySelector(root) : root;
  if (!element) {
    warn("WS live root not found: " + root);
    return;
  }

  const fullUrl = normalizeWsEndpoint(url);
  if (!fullUrl) return;

  const existing = liveConnections.get(element);
  if (existing) {
    pauseLiveState(existing);
    unregisterLiveState(existing);
  }

  const state = {
    es: null,
    socket: null,
    url: fullUrl,
    element,
    backoff: 1000,
    paused: false,
    reconnectTimer: null,
    protocol: "ws",
  };
  registerLiveState(state);

  connectWS(fullUrl, state);
}
```

**After (new):**
```js
function openWsLive(root, url) {
  const element = typeof root === "string" ? document.querySelector(root) : root;
  if (!element) {
    warn("WS live root not found: " + root);
    return;
  }

  const fullUrl = normalizeWsEndpoint(url);
  if (!fullUrl) return;

  // Unsubscribe from previous hub if switching URLs
  const existing = liveConnections.get(element);
  if (existing && existing.protocol === "ws") {
    unsubscribeWs(element);
  } else if (existing) {
    // Was SSE — use existing SSE cleanup
    pauseLiveState(existing);
    unregisterLiveState(existing);
  }

  // Subscribe to hub
  const hub = getOrCreateWsHub(fullUrl);
  hub.subscribers.add(element);

  // Register in liveConnections for compatibility with disconnect/reconnect APIs
  const state = {
    es: null,
    socket: null,
    url: fullUrl,
    element,
    backoff: 0,       // backoff is hub-level now
    paused: false,
    reconnectTimer: null,
    protocol: "ws",
    hub,               // reference to shared hub
  };
  registerLiveState(state);

  // Connect hub if not already connected
  connectWsHub(hub);
}
```

**Verify:**
- Single element with `s-live="ws:/path"` still works.
- Two elements with same URL open ONE socket (check Network tab).
- `silcrow:live:connect` fires once.

---

## Step 5: Add `unsubscribeWs` helper

**File:** `silcrow/ws.js`
**What:** Add helper to cleanly remove an element from its hub. Place before `openWsLive`.

```js
function unsubscribeWs(element) {
  const state = liveConnections.get(element);
  if (!state || state.protocol !== "ws") return;

  const hub = state.hub;
  if (hub) {
    hub.subscribers.delete(element);
    if (hub.subscribers.size === 0) {
      removeWsHub(hub);
    }
  }

  unregisterLiveState(state);
}
```

**Verify:** Removing one subscriber keeps socket alive for remaining subscribers.

---

## Step 6: Update `sendWs` to deduplicate by hub

**File:** `silcrow/ws.js`
**What:** Replace existing `sendWs` function.

**Before (current):**
```js
function sendWs(root, data) {
  const states = resolveLiveStates(root);
  if (!states.length) {
    warn("No live connection found for send target");
    return;
  }

  for (const state of states) {
    if (state.protocol !== "ws") {
      warn("Cannot send on SSE connection — use WS for bidirectional");
      continue;
    }
    if (!state.socket || state.socket.readyState !== WebSocket.OPEN) {
      warn("WebSocket not open for send");
      continue;
    }
    try {
      const payload = typeof data === "string" ? data : JSON.stringify(data);
      state.socket.send(payload);
    } catch (err) {
      warn("WS send failed: " + err.message);
    }
  }
}
```

**After (new):**
```js
function sendWs(root, data) {
  const states = resolveLiveStates(root);
  if (!states.length) {
    warn("No live connection found for send target");
    return;
  }

  // Deduplicate: send once per hub, not once per subscriber
  const sentHubs = new Set();

  for (const state of states) {
    if (state.protocol !== "ws") {
      warn("Cannot send on SSE connection — use WS for bidirectional");
      continue;
    }

    const hub = state.hub;
    if (!hub || sentHubs.has(hub)) continue;
    sentHubs.add(hub);

    if (!hub.socket || hub.socket.readyState !== WebSocket.OPEN) {
      warn("WebSocket not open for send");
      continue;
    }

    try {
      const payload = typeof data === "string" ? data : JSON.stringify(data);
      hub.socket.send(payload);
    } catch (err) {
      warn("WS send failed: " + err.message);
    }
  }
}
```

**Verify:** `Silcrow.send("/ws/chat", data)` sends one frame, not N.

---

## Step 7: Update `pauseLiveState` for WS protocol

**File:** `silcrow/live.js`
**What:** Modify `pauseLiveState` to handle WS hub-based connections.

**Current code:**
```js
function pauseLiveState(state) {
  state.paused = true;
  if (state.reconnectTimer) {
    clearTimeout(state.reconnectTimer);
    state.reconnectTimer = null;
  }
  if (state.es) {
    state.es.close();
    state.es = null;
  }
  if (state.socket) {
    state.socket.close();
    state.socket = null;
  }
}
```

**New code:**
```js
function pauseLiveState(state) {
  state.paused = true;
  if (state.reconnectTimer) {
    clearTimeout(state.reconnectTimer);
    state.reconnectTimer = null;
  }
  if (state.es) {
    state.es.close();
    state.es = null;
  }
  // For WS: unsubscribe from hub instead of closing socket directly
  if (state.protocol === "ws" && state.hub) {
    state.hub.subscribers.delete(state.element);
    if (state.hub.subscribers.size === 0) {
      removeWsHub(state.hub);
    }
    state.hub = null;
  } else if (state.socket) {
    state.socket.close();
    state.socket = null;
  }
}
```

**Verify:** `Silcrow.disconnect("#one")` unsubscribes element. Socket stays open if `#two` is still subscribed.

---

## Step 8: Update disconnect/reconnect for hub semantics

**File:** `silcrow/live.js`
**What:** Modify `disconnectLive` and `reconnectLive` to handle URL-level hub operations.

**`disconnectLive` — no change needed.** It calls `pauseLiveState` which now handles hubs (Step 7).

**`reconnectLive` — update:**

**Current code:**
```js
function reconnectLive(root) {
  const states = resolveLiveStates(root);
  if (!states.length) return;

  for (const state of states) {
    state.paused = false;
    state.backoff = 1000;
    if (state.reconnectTimer) {
      clearTimeout(state.reconnectTimer);
      state.reconnectTimer = null;
    }
    connectSSE(state.url, state);
  }
}
```

**New code:**
```js
function reconnectLive(root) {
  const states = resolveLiveStates(root);
  if (!states.length) return;

  const reconnectedHubs = new Set();

  for (const state of states) {
    state.paused = false;

    if (state.protocol === "ws") {
      // Re-subscribe to hub
      const hub = getOrCreateWsHub(state.url);
      hub.subscribers.add(state.element);
      state.hub = hub;

      if (!reconnectedHubs.has(hub)) {
        reconnectedHubs.add(hub);
        hub.paused = false;
        hub.backoff = 1000;
        if (hub.reconnectTimer) {
          clearTimeout(hub.reconnectTimer);
          hub.reconnectTimer = null;
        }
        connectWsHub(hub);
      }
    } else {
      // SSE: existing behavior
      state.backoff = 1000;
      if (state.reconnectTimer) {
        clearTimeout(state.reconnectTimer);
        state.reconnectTimer = null;
      }
      connectSSE(state.url, state);
    }
  }
}
```

**Verify:**
- `Silcrow.disconnect("/ws/chat")` pauses hub, all subscribers stop.
- `Silcrow.reconnect("/ws/chat")` resumes hub, all subscribers receive again.
- `Silcrow.disconnect("#one")` then `Silcrow.reconnect("#one")` works for single element.

---

## Step 9: Update `destroyAllLive` for hub cleanup

**File:** `silcrow/live.js`
**What:** Ensure `destroyAllLive` cleans up all hubs.

**Current code:**
```js
function destroyAllLive() {
  for (const state of liveConnections.values()) {
    pauseLiveState(state);
  }
  liveConnections.clear();
  liveConnectionsByUrl.clear();
}
```

**New code:**
```js
function destroyAllLive() {
  for (const state of liveConnections.values()) {
    pauseLiveState(state);
  }
  liveConnections.clear();
  liveConnectionsByUrl.clear();

  // Clean up any remaining WS hubs
  for (const hub of wsHubs.values()) {
    if (hub.reconnectTimer) {
      clearTimeout(hub.reconnectTimer);
      hub.reconnectTimer = null;
    }
    if (hub.socket) {
      hub.socket.close();
      hub.socket = null;
    }
  }
  wsHubs.clear();
}
```

**Verify:** `Silcrow.destroy()` closes all WS connections, no leaks.

---

## Step 10: Add MutationObserver for DOM removal cleanup

**File:** `silcrow/index.js`
**What:** Add a MutationObserver in `init()` that unsubscribes elements removed from DOM.

**Add inside `init()` after `initLiveElements()`:**

```js
  // Observe DOM for removed live elements
  const liveObserver = new MutationObserver(function (mutations) {
    for (const mutation of mutations) {
      for (const removed of mutation.removedNodes) {
        if (removed.nodeType !== 1) continue;

        // Check the removed node itself
        const state = liveConnections.get(removed);
        if (state) {
          if (state.protocol === "ws") {
            unsubscribeWs(removed);
          } else {
            pauseLiveState(state);
          }
          unregisterLiveState(state);
        }

        // Check descendants of removed node
        if (removed.querySelectorAll) {
          for (const child of removed.querySelectorAll("[s-live]")) {
            const childState = liveConnections.get(child);
            if (childState) {
              if (childState.protocol === "ws") {
                unsubscribeWs(child);
              } else {
                pauseLiveState(childState);
              }
              unregisterLiveState(childState);
            }
          }
        }
      }
    }
  });

  liveObserver.observe(document.body, {childList: true, subtree: true});
```

**Also update `destroy()` to disconnect the observer:**

```js
// Store observer reference at module scope (add near top of index.js):
let liveObserver = null;

// In init(), assign:
liveObserver = new MutationObserver(function (mutations) { ... });
liveObserver.observe(document.body, {childList: true, subtree: true});

// In destroy(), add before responseCache.clear():
if (liveObserver) {
  liveObserver.disconnect();
  liveObserver = null;
}
```

**Verify:**
- Remove a `[s-live="ws:/path"]` element from DOM → subscriber count drops.
- If last subscriber removed → socket closes.
- `Silcrow.destroy()` disconnects observer.

---

## Step 11: Remove dead code

**File:** `silcrow/ws.js`
**What:** Remove the old `connectWS(url, state)` function that is no longer called.

**Verify:** `grep -r "connectWS" silcrow/` returns zero hits (only `connectWsHub` remains).

---

## Step 12: Update ws-checklist.md

**File:** `ws-checklist.md`
**What:** Add multiplex verification items.

Add after existing items:
```md
## Multiplex Verification
- [ ] Two elements with `s-live="ws:/same/path"` → ONE WebSocket in Network tab
- [ ] Two elements with different WS URLs → TWO WebSockets
- [ ] Remove one element from DOM → socket stays open for remaining subscriber
- [ ] Remove last element from DOM → socket closes
- [ ] `Silcrow.send("/ws/path", data)` sends one frame, not N
- [ ] `Silcrow.disconnect("#one")` keeps socket alive if `#two` subscribed to same URL
- [ ] `Silcrow.disconnect("/ws/path")` pauses hub, all subscribers stop
- [ ] `Silcrow.reconnect("/ws/path")` resumes hub for all subscribers
- [ ] `Silcrow.destroy()` closes all hubs, no leaked sockets
```

---

## Step 13: Update SILCROW.md

**File:** `SILCROW.md`
**What:** Add note in the Live section about WS multiplexing.

Add after the WebSocket connection description:
```md
### Connection Sharing

When multiple elements connect to the same WebSocket URL, Silcrow opens a single
shared connection. Messages with an explicit `target` selector are applied once to
the matching element. Messages without a target fan out to all subscribed elements.

This is automatic — no configuration needed. If you need isolated connections to the
same URL (rare), use distinct query parameters: `ws:/ws/chat?room=1` vs `ws:/ws/chat?room=2`.
```

---

## Commit Plan

```
Branch: refactor/ws-multiplex

Commits:
  1. refactor(silcrow): add WsHub data model and connect/dispatch functions
     (Steps 1-3: dead code, no behavior change)

  2. refactor(silcrow): switch openWsLive to hub-based connections
     (Steps 4-5: the switchover)

  3. refactor(silcrow): update send/disconnect/reconnect for hub semantics
     (Steps 6-8)

  4. refactor(silcrow): add MutationObserver cleanup and destroy hub teardown
     (Steps 9-10)

  5. chore(silcrow): remove dead connectWS function
     (Step 11)

  6. docs: update ws-checklist and SILCROW.md for multiplex behavior
     (Steps 12-13)

PR title: refactor: WebSocket connection multiplexing (one socket per URL)
```

---

## Agent Instructions

- Complete steps in order. Each step builds on the previous.
- After each step, verify the "Verify" condition before proceeding.
- Do NOT modify any function until the step says to modify it.
- Steps 1-3 add new code only. No existing code changes.
- Step 4 is the switchover. If anything breaks, revert Step 4 only.
- Steps that say "no behavior change" must pass all existing manual checks.
- `wsHubs` is defined in `ws.js`. Functions in `live.js` call it — this works because `ws.js` loads before `live.js` is used (both are in the same IIFE, `ws.js` is concatenated before `index.js`).
- Build order in `build.rs`: debug → patcher → safety → toasts → navigator → live → ws → optimistic → index. Functions defined in `ws.js` are available in `live.js` and `index.js`.

**IMPORTANT:** `ws.js` loads AFTER `live.js` in build order. This means functions defined in `ws.js` (like `unsubscribeWs`, `connectWsHub`, `getOrCreateWsHub`, `removeWsHub`) are NOT available inside `live.js` at parse time. However, they ARE available at call time because all code runs inside a single IIFE and these functions are hoisted. Since `live.js` functions that reference `ws.js` functions are only called at runtime (not during module evaluation), this is safe. Verify by checking: no top-level code in `live.js` calls `ws.js` functions directly — only function bodies do.
