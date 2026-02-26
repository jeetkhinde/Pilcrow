// silcrow/live.js
// ════════════════════════════════════════════════════════════
// Live — SSE connections & real-time updates
// ════════════════════════════════════════════════════════════

const liveConnections = new Map(); // route → { es, root, backoff, paused }
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

function openLive(root, url) {
  const element = typeof root === "string" ? document.querySelector(root) : root;
  if (!element) {
    warn("Live root not found: " + root);
    return;
  }

  const fullUrl = normalizeSSEEndpoint(url);
  if (!fullUrl) return;

  // Close existing connection for this route
  const existing = liveConnections.get(fullUrl);
  if (existing && existing.es) {
    existing.es.close();
  }

  const state = {
    es: null,
    element,
    backoff: 1000,
    paused: false,
    reconnectTimer: null,
  };
  liveConnections.set(fullUrl, state);

  connectSSE(fullUrl, state);
}

function connectSSE(url, state) {
  if (state.paused) return;

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
      applyLivePatchPayload(payload, state.element);
    } catch (err) {
      warn("Failed to parse SSE patch event: " + err.message);
    }
  });

  // Named event: html
  es.addEventListener("html", function (e) {
    try {
      const payload = JSON.parse(e.data);
      const target = resolveLiveTarget(payload && payload.target, state.element);
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
  const url = resolveSSEUrl(root);
  if (!url) return;

  const state = liveConnections.get(url);
  if (!state) return;

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

function reconnectLive(root) {
  const url = resolveSSEUrl(root);
  if (!url) return;

  const state = liveConnections.get(url);
  if (!state) return;

  state.paused = false;
  state.backoff = 1000; // Reset backoff
  if (state.reconnectTimer) {
    clearTimeout(state.reconnectTimer);
    state.reconnectTimer = null;
  }
  connectSSE(url, state);
}

function resolveSSEUrl(root) {
  // If root is a URL string, treat it as a route key.
  if (isLikelyLiveUrl(root)) {
    const route = normalizeSSEEndpoint(root);
    if (route) return route;
  }
  // If root is an element or selector, find its connection by element match
  const element =
    typeof root === "string" ? document.querySelector(root) : root;
  if (!element) return null;

  for (const [url, state] of liveConnections) {
    if (state.element === element) return url;
  }
  return null;
}

function destroyAllLive() {
  for (const [url, state] of liveConnections) {
    state.paused = true;
    if (state.reconnectTimer) {
      clearTimeout(state.reconnectTimer);
    }
    if (state.es) {
      state.es.close();
    }
  }
  liveConnections.clear();
}

// ── Process silcrow-sse header from navigator responses ────
function processSSEHeader(response) {
  const ssePath = normalizeSSEEndpoint(response.headers.get("silcrow-sse"));
  if (ssePath) {
    document.dispatchEvent(
      new CustomEvent("silcrow:sse", {
        bubbles: true,
        detail: {path: ssePath},
      })
    );
  }
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
