# Silcrow.js

A lightweight client-side runtime for building hypermedia-driven applications. Silcrow handles DOM patching, client-side navigation, response caching, and server-driven UI orchestration — all from declarative HTML attributes.

Silcrow.js is the frontend counterpart to [Pilcrow](readme.md) but operates independently as a standalone library. Any backend that speaks HTTP and returns HTML or JSON can drive it.

## Loading

Silcrow.js is a single self-executing IIFE with no dependencies. Include it in your page:

```html
<script src="/_silcrow/silcrow.js" defer></script>
```

If using Pilcrow on the backend, use the `script_tag()` helper which returns a fingerprinted URL with immutable caching.

Enable debug mode by adding `s-debug` to the body:

```html
<body s-debug>
```

This enables console warnings and throws on template validation errors.

## Two Systems

Silcrow.js has two independent systems exposed through a single `window.Silcrow` API:

1. **Runtime** — reactive data binding and DOM patching via `s-bind` and `s-list` attributes
2. **Navigator** — client-side routing, history management, and response caching via `s-action` attributes

---

## Runtime: Data Binding & DOM Patching

### Scalar Binding with `s-bind`

Bind any element to a data path. The format is `s-bind="path"` for text content or `s-bind="path:property"` for element properties.

```html
<h1 s-bind="user.name"></h1>
<input s-bind="user.email:value" />
<img s-bind="user.avatar:src" />
<button s-bind="user.banned:disabled"></button>
```

Patch data into the DOM:

```js
Silcrow.patch({
  user: { name: "Alice", email: "a@b.com", avatar: "/img/alice.png", banned: false }
}, "#app");
```

The second argument is a root — either a CSS selector string or a DOM `Element`. Silcrow only patches bindings within that root.

**Known properties** (`value`, `checked`, `disabled`, `selected`, `src`, `href`, `selectedIndex`) are set as DOM properties. Everything else is set as an attribute. `null` or `undefined` values reset properties to their type default or remove attributes.

**Security:** Binding to event handler attributes (`onclick`, `onload`, etc.) is rejected. Text content is set via `textContent`, never `innerHTML`.

### Collection Rendering with `s-list`

Render arrays of keyed objects into a container. Each item **must** have a `key` property.

```html
<ul s-list="todos" s-template="todo-tpl">
</ul>

<template id="todo-tpl">
  <li>
    <span s-bind=".text"></span>
    <input type="checkbox" s-bind=".done:checked" />
  </li>
</template>
```

```js
Silcrow.patch({
  todos: [
    { key: "1", text: "Buy milk", done: false },
    { key: "2", text: "Write docs", done: true },
  ]
}, "#app");
```

**Local bindings** use a leading dot (`.text`, `.done`) — they bind to fields on the individual item, not the global data object.

**Reconciliation:** Silcrow uses keyed reconciliation. Existing DOM nodes are reused by key, new items are created from the template, removed items are deleted, and order is maintained by repositioning. Duplicate keys are rejected.

**Template resolution order:**

1. Item key prefix — if `key` is `"special#3"`, Silcrow looks for `<template id="special">`
2. `s-template` attribute on the container
3. Inline `<template>` child of the container

**Template rules:** Templates must contain exactly one element child. Scripts and event handler attributes inside templates are rejected during validation.

### `Silcrow.patch(data, root, options?)`

The core patching function. Options:

- `invalidate: true` — rebuilds the binding map from scratch (use after DOM mutations)
- `silent: true` — suppresses the `silcrow:patched` custom event

After each patch, a `silcrow:patched` event fires on the root with `detail.paths` listing all bound paths.

### `Silcrow.invalidate(root)`

Clears the cached binding map and template validations for a root. Call this when you've added or removed `s-bind` / `s-list` elements dynamically.

### `Silcrow.stream(root)`

Returns a microtask-batched update function. Multiple calls within the same microtask are coalesced — only the last data wins.

```js
const update = Silcrow.stream("#dashboard");
update({ count: 1 });
update({ count: 2 });
update({ count: 3 }); // only this patch executes
```

### Path Resolution

Dot-separated paths resolve into nested objects: `"user.profile.name"` reads `data.user.profile.name`. Prototype pollution paths (`__proto__`, `constructor`, `prototype`) are blocked and return `undefined`.

---

## Navigator: Client-Side Routing

### Declarative Navigation with `s-action`

Add `s-action` to any element to make it navigate on click:

```html
<a s-action="/dashboard">Dashboard</a>
<button s-action="/api/save" POST>Save</button>
<button s-action="/items/5" DELETE s-target="#item-5">Remove</button>
```

### Attributes

| Attribute | Purpose | Default |
| --- | --- | --- |
| `s-action` | URL to request | *(required)* |
| `s-target` | CSS selector — swap response into this element | The triggering element itself |
| `s-html` | Request `text/html` instead of `application/json` | JSON |
| `s-skip-history` | Don't push to browser history | Push for full-page GETs |
| `s-preload` | Preload on mouse hover | Off |
| `s-timeout` | Request timeout in ms | `30000` |
| `GET`, `POST`, `PUT`, `PATCH`, `DELETE` | HTTP method (as attribute) | `GET` (or `POST` for forms) |

### Forms

Forms with `s-action` are intercepted automatically. `GET` forms append `FormData` as query params. Other methods send `FormData` as the body.

```html
<form s-action="/search" GET s-target="#results">
  <input name="q" />
  <button>Search</button>
</form>
```

### Programmatic Navigation

```js
Silcrow.go("/dashboard");
Silcrow.go("/api/items", { method: "POST", body: { name: "New" }, target: "#list" });
```

### Response Processing

The navigator reads the `Content-Type` header to decide how to handle the response:

- **JSON** (`application/json`) — parsed and passed to `Silcrow.patch()` on the target element
- **HTML** (`text/html`) — sanitized and swapped into the target element's `innerHTML`

For HTML responses, if the response is a full page (`<!DOCTYPE` or `<html`), Silcrow extracts the `<title>` and the matching `s-target` selector content (or `<body>` as fallback).

**HTML sanitization:** Silcrow uses the Sanitizer API (`el.setHTML()`) when available. When it isn't, a DOMParser fallback strips all `<script>` elements and event handler attributes (`on*`) before insertion.

### Server-Driven Headers

The backend can control Silcrow's behavior through response headers:

| Header | Effect |
| --- | --- |
| `silcrow-trigger` | Fire custom DOM events. JSON object `{"event-name": detail}` or a plain event name string. |
| `silcrow-retarget` | CSS selector — override where the response is swapped into. |
| `silcrow-push-url` | Override the URL pushed to browser history. |
| `silcrow-cache` | Set to `no-cache` to prevent this response from being cached. |

### Caching

GET responses are cached in-memory for 5 minutes (max 50 entries). Any mutation request (`POST`, `PUT`, `PATCH`, `DELETE`) clears the entire cache. The server can opt out per-response with the `silcrow-cache: no-cache` header.

```js
Silcrow.cache.has("/dashboard");  // check cache
Silcrow.cache.clear("/dashboard"); // clear one entry
Silcrow.cache.clear();             // clear all
```

### Preloading

Elements with `s-preload` fire a background fetch on `mouseenter`. The response is cached so the subsequent click is instant.

```html
<a s-action="/settings" s-preload>Settings</a>
```

### History & Scroll

Full-page GET navigations push to `history.pushState`. On popstate (back/forward), Silcrow re-fetches the URL and restores the saved scroll position. Partial updates (those with `s-target`) skip history by default.

### Loading States

During requests, Silcrow adds `silcrow-loading` CSS class and `aria-busy="true"` to the target element. Style it however you want:

```css
.silcrow-loading { opacity: 0.5; pointer-events: none; }
```

### Abort & Timeout

Navigating to the same target while a GET is in-flight aborts the previous request. Mutation requests are never aborted. Timeout defaults to 30 seconds and can be set per-element with `s-timeout`.

---

## Toast System

Register a toast handler to receive toast notifications from both JSON payloads and cookie-based HTML responses:

```js
Silcrow.onToast((message, level) => {
  showNotification(message, level); // your UI
});
```

**JSON responses:** Toasts are read from the `_toasts` array in the payload, then removed before patching. If the payload was wrapped by the server (non-object root with toasts), Silcrow unwraps it.

**HTML/redirect responses:** Toasts are read from the `silcrow_toasts` cookie (URL-encoded JSON array), then the cookie is immediately cleared.

---

## Events

All events bubble and are dispatched on `document` (except `silcrow:patched` which fires on the root element).

| Event | Detail | Cancelable | When |
| --- | --- | --- | --- |
| `silcrow:navigate` | `{url, method, trigger, target}` | Yes | Before any fetch |
| `silcrow:before-swap` | `{url, target, content, isJSON, proceed}` | Yes | After fetch, before DOM update |
| `silcrow:load` | `{url, target, redirected}` | No | After successful swap |
| `silcrow:error` | `{error, url}` | No | On fetch error or timeout |
| `silcrow:patched` | `{paths}` | No | After `patch()` completes |

**Transition hook:** Listen to `silcrow:before-swap` and call `event.detail.proceed()` manually to control when the DOM update happens (e.g., after a CSS transition). If no listener calls `proceed()`, Silcrow executes it automatically.

---

## Lifecycle

```js
// Register handlers (chainable)
Silcrow
  .onToast((msg, level) => { /* ... */ })
  .onRoute(({ url, finalUrl, redirected, method, response, contentType, target }) => {
    // Return false to prevent the default swap
  })
  .onError((err, { url, method, trigger, target }) => {
    // Custom error handling
  });

// Teardown — removes all event listeners and clears caches
Silcrow.destroy();
```

---

## API Reference

### Runtime

| Method | Description |
| --- | --- |
| `Silcrow.patch(data, root, options?)` | Patch data into bound elements under root |
| `Silcrow.invalidate(root)` | Clear cached binding maps for root |
| `Silcrow.stream(root)` | Returns microtask-batched updater function |

### Navigation

| Method | Description |
| --- | --- |
| `Silcrow.go(path, options?)` | Programmatic navigation |
| `Silcrow.cache.has(path)` | Check if a path is cached |
| `Silcrow.cache.clear(path?)` | Clear one or all cache entries |

### Lifecycle Table

| Method | Description |
| --- | --- |
| `Silcrow.onToast(handler)` | Register toast callback (chainable) |
| `Silcrow.onRoute(handler)` | Register route middleware (chainable) |
| `Silcrow.onError(handler)` | Register error handler (chainable) |
| `Silcrow.destroy()` | Teardown all listeners and caches |

`window.SilcrowNavigate` is available as a backward-compatible alias for `window.Silcrow`.

---

## Compatibility

Silcrow.js requires a modern browser with support for `fetch`, `URL`, `CustomEvent`, `WeakMap`, `queueMicrotask`, and `<template>`. No polyfills are bundled.
