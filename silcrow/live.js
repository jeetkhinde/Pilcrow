// silcrow/live.js
// ════════════════════════════════════════════════════════════
// Live — SSE connections & real-time updates
// ════════════════════════════════════════════════════════════

const liveConnections = new Map(); // element -> state
const liveConnectionsByUrl = new Map(); // url -> Set<state>
const MAX_BACKOFF = 30000;
const LIVE_HTTP_PROTOCOLS = new Set(["http:", "https:"]);

function isLikelyLiveUrl(value) {
  return (
    typeof value === "string" &&
    (value.startsWith("/") ||
      value.startsWith("http://") ||
      value.startsWith("https://"))
  );
}

function normalizeSSEEndpoint(rawUrl) {
  if (typeof rawUrl !== "string") return null;
  const value = rawUrl.trim();
  if (!value) return null;

  let parsed;
  try {
    parsed = new URL(value, location.origin);
  } catch (e) {
    warn("Invalid SSE URL: " + value);
    return null;
  }

  if (!LIVE_HTTP_PROTOCOLS.has(parsed.protocol)) {
    warn("Rejected non-http(s) SSE URL: " + parsed.href);
    return null;
  }
  if (parsed.origin !== location.origin) {
    warn("Rejected cross-origin SSE URL: " + parsed.href);
    return null;
  }

  return parsed.href;
}

function resolveLiveTarget(selector, fallback) {
  if (typeof selector !== "string" || !selector) return fallback;
  return document.querySelector(selector) || null;
}

function applyLivePatchPayload(payload, fallbackTarget) {
  if (
    payload &&
    typeof payload === "object" &&
    Object.prototype.hasOwnProperty.call(payload, "data")
  ) {
    const target = resolveLiveTarget(payload.target, fallbackTarget);
    if (target) {
      patch(payload.data, target);
    }
    return;
  }

  patch(payload, fallbackTarget);
}

function registerLiveState(state) {
  liveConnections.set(state.element, state);

  let byUrl = liveConnectionsByUrl.get(state.url);
  if (!byUrl) {
    byUrl = new Set();
    liveConnectionsByUrl.set(state.url, byUrl);
  }
  byUrl.add(state);
}

function unregisterLiveState(state) {
  if (liveConnections.get(state.element) === state) {
    liveConnections.delete(state.element);
  }

  const byUrl = liveConnectionsByUrl.get(state.url);
  if (!byUrl) return;

  byUrl.delete(state);
  if (byUrl.size === 0) {
    liveConnectionsByUrl.delete(state.url);
  }
}

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
}

function resolveLiveStates(root) {
  if (typeof root === "string") {
    // Route key: disconnect/reconnect all connections for the URL
    if (
      root.startsWith("/") ||
      root.startsWith("http://") ||
      root.startsWith("https://")
    ) {
      const fullUrl = new URL(root, location.origin).href;
      const states = liveConnectionsByUrl.get(fullUrl);
      return states ? Array.from(states) : [];
    }

    const element = document.querySelector(root);
    if (!element) return [];
    const state = liveConnections.get(element);
    return state ? [state] : [];
  }

  if (!root) return [];
  const state = liveConnections.get(root);
  return state ? [state] : [];
}

function onSSEEvent(e) {
  const path = e?.detail?.path;
  if (!path || typeof path !== "string") return;

  const root = e?.detail?.target || document.body;
  openLive(root, path);
}

function openLive(root, url) {
  const element = typeof root === "string" ? document.querySelector(root) : root;
  if (!element) {
    warn("Live root not found: " + root);
    return;
  }

  const fullUrl = normalizeSSEEndpoint(url);
  if (!fullUrl) return;

  // Replace existing connection for this root element
  const existing = liveConnections.get(element);
  if (existing) {
    pauseLiveState(existing);
    unregisterLiveState(existing);
  }

  const state = {
    es: null,
    url: fullUrl,
    element,
    backoff: 1000,
    paused: false,
    reconnectTimer: null,
  };
  registerLiveState(state);

  connectSSE(fullUrl, state);
}

function connectSSE(url, state) {
  if (state.paused) return;
  if (liveConnections.get(state.element) !== state) return;

  const es = new EventSource(url);
  state.es = es;

  es.onopen = function () {
    state.backoff = 1000; // Reset backoff on successful connect
    document.dispatchEvent(
      new CustomEvent("silcrow:live:connect", {
        bubbles: true,
        detail: {root: state.element, url},
      })
    );
  };

  // Default message event → patch
  es.onmessage = function (e) {
    try {
      const payload = JSON.parse(e.data);
      applyLivePatchPayload(payload, state.element);
    } catch (err) {
      warn("Failed to parse SSE message: " + err.message);
    }
  };

  // Named event: patch
  es.addEventListener("patch", function (e) {
    try {
      const payload = JSON.parse(e.data);
      let target = state.element;
      let data = payload;

      // Supports both:
      // 1) {"target":"#el","data":{...}} (Pilcrow SilcrowEvent::patch)
      // 2) {...} or [...] (direct root patch payload)
      if (
        payload &&
        typeof payload === "object" &&
        !Array.isArray(payload) &&
        Object.prototype.hasOwnProperty.call(payload, "data")
      ) {
        data = payload.data;
        if (payload.target) {
          const selected = document.querySelector(payload.target);
          if (selected) target = selected;
        }
      }

      if (target && data !== undefined) {
        patch(data, target);
      }
    } catch (err) {
      warn("Failed to parse SSE patch event: " + err.message);
    }
  });

  // Named event: html
  es.addEventListener("html", function (e) {
    try {
      const payload = JSON.parse(e.data);
      const target = payload.target
        ? document.querySelector(payload.target)
        : state.element;
      if (
        target &&
        payload &&
        typeof payload === "object" &&
        Object.prototype.hasOwnProperty.call(payload, "html")
      ) {
        safeSetHTML(target, payload.html == null ? "" : String(payload.html));
      }
    } catch (err) {
      warn("Failed to parse SSE html event: " + err.message);
    }
  });

  // Named event: invalidate
  es.addEventListener("invalidate", function () {
    invalidate(state.element);
  });

  // Named event: navigate
  es.addEventListener("navigate", function (e) {
    if (e.data) {
      navigate(e.data.trim(), {trigger: "sse"});
    }
  });

  es.onerror = function () {
    es.close();
    state.es = null;

    if (state.paused) return;
    if (liveConnections.get(state.element) !== state) return;

    const reconnectIn = state.backoff;

    document.dispatchEvent(
      new CustomEvent("silcrow:live:disconnect", {
        bubbles: true,
        detail: {root: state.element, url, reconnectIn},
      })
    );

    state.reconnectTimer = setTimeout(function () {
      state.reconnectTimer = null;
      connectSSE(url, state);
    }, reconnectIn);

    // Exponential backoff
    state.backoff = Math.min(state.backoff * 2, MAX_BACKOFF);
  };
}

function disconnectLive(root) {
  const states = resolveLiveStates(root);
  if (!states.length) return;

  for (const state of states) {
    pauseLiveState(state);
  }
}

function reconnectLive(root) {
  const states = resolveLiveStates(root);
  if (!states.length) return;

  for (const state of states) {
    state.paused = false;
    state.backoff = 1000; // Reset backoff
    if (state.reconnectTimer) {
      clearTimeout(state.reconnectTimer);
      state.reconnectTimer = null;
    }
    connectSSE(state.url, state);
  }
}

function destroyAllLive() {
  for (const state of liveConnections.values()) {
    pauseLiveState(state);
  }
  liveConnections.clear();
  liveConnectionsByUrl.clear();
}

// ── Auto-scan for s-live elements on init ──────────────────
function initLiveElements() {
  const elements = document.querySelectorAll("[s-live]");
  for (const el of elements) {
    const url = el.getAttribute("s-live");
    if (url) {
      openLive(el, url);
    }
  }
}
