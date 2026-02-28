# Implementation Plan: System Fixes & Hardening

## Overview

Address critical programmatic API breakages in WebSocket controls, resolve documentation/header naming mismatches, improve Rust API flexibility, and fix ambiguous payload detection in live streams.

## Files Touched

* `silcrow/live.js` — fix WS URL normalization and ambiguous envelope logic
* `silcrow/ws.js` — fix ambiguous envelope logic in WS dispatch
* `src/response.rs` — update `with_header` to accept dynamic values
* `SILCROW.md` — sync header documentation

---

## Step 1: Fix WebSocket URL normalization

**File:** `silcrow/live.js`
**What:** Update `resolveLiveStates` to correctly normalize URLs for WebSocket hubs. Programs calling `Silcrow.send("/path", ...)` currently fail because the lookup key in `liveConnectionsByUrl` uses the `ws:` scheme, but `resolveLiveStates` generates `http:`.

```js
// Inside resolveLiveStates(root):
if (
  root.startsWith("/") ||
  root.startsWith("http://") ||
  root.startsWith("https://")
) {
  let fullUrl = new URL(root, location.origin).href;
  
  // Normalize protocol to match wsHubs keys if it's a known WS path
  // This ensures Silcrow.send("/ws/chat", ...) matches the ws://... key
  if (root.startsWith("ws:") || root.includes("/ws/")) {
    fullUrl = fullUrl.replace(/^http/, "ws");
  }
  
  const states = liveConnectionsByUrl.get(fullUrl);
  return states ? Array.from(states) : [];
}

```

---

## Step 2: Fix Ambiguous Envelope Detection

**File:** `silcrow/live.js` and `silcrow/ws.js`
**What:** Change the envelope detection logic to check for the `target` property instead of `data`. This prevents Silcrow from accidentally "unwrapping" legitimate data payloads that happen to have a top-level `data` key.

**In `silcrow/live.js` (patch event listener):**

```js
// Change line 214:
if (
  payload &&
  typeof payload === "object" &&
  !Array.isArray(payload) &&
  Object.prototype.hasOwnProperty.call(payload, "target") // Changed from "data"
) {
  data = payload.data;
  if (payload.target) {
    const selected = document.querySelector(payload.target);
    if (selected) target = selected;
  }
}

```

**In `silcrow/ws.js` (dispatchWsMessage):**

```js
// Update patch/html/invalidate blocks to prioritize envelope target 
// over the subscriber-fan-out if a target is present in the payload.

```

---

## Step 3: Update Rust `with_header` flexibility

**File:** `src/response.rs`
**What:** Update the `with_header` modifier in the `ResponseExt` trait to accept `impl Into<String>` for values, allowing users to pass dynamic strings (e.g., from format macros) instead of only `'static str`.

```rust
// Inside trait ResponseExt:
fn with_header(mut self, key: &'static str, value: impl Into<String>) -> Self {
    if let Ok(val) = HeaderValue::from_str(&value.into()) {
        self.base_mut().headers.insert(key, val);
    }
    self
}

```

---

## Step 4: Sync Documentation for History Header

**File:** `SILCROW.md`
**What:** Update the documentation table to match the implemented header name `silcrow-push`.

```markdown
// Change line 206:
| `silcrow-push` | Override the URL pushed to browser history. |

```

---

## Commit Plan

```text
Branch: fix/system-hardening

Commits:
  1. fix(silcrow): normalize WS schemes in resolveLiveStates for programmatic API
     (Step 1: Fixes Silcrow.send/disconnect lookup bugs)

  2. fix(silcrow): use target property for envelope detection to avoid payload collisions
     (Step 2: Security/Logic fix for SSE/WS patching)

  3. refactor(response): allow dynamic string values in with_header
     (Step 3: Improves DX for dynamic headers)

  4. docs: sync silcrow-push header name in SILCROW.md
     (Step 4: Doc correction)

PR title: fix: WebSocket API normalization and payload detection hardening
```
