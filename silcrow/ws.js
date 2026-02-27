// silcrow/ws.js
// ════════════════════════════════════════════════════════════
// WebSocket — bidirectional live connections
// ════════════════════════════════════════════════════════════

function normalizeWsEndpoint(rawUrl) {
  if (typeof rawUrl !== "string") return null;
  const value = rawUrl.trim();
  if (!value) return null;

  let parsed;
  try {
    parsed = new URL(value, location.origin);
  } catch (e) {
    warn("Invalid WS URL: " + value);
    return null;
  }

  // Convert http(s) to ws(s) for WebSocket
  if (parsed.protocol === "https:") {
    parsed.protocol = "wss:";
  } else if (parsed.protocol === "http:") {
    parsed.protocol = "ws:";
  }

  if (parsed.protocol !== "ws:" && parsed.protocol !== "wss:") {
    warn("Rejected non-ws(s) WebSocket URL: " + parsed.href);
    return null;
  }

  return parsed.href;
}

function openWsLive(root, url) {
  const element = typeof root === "string" ? document.querySelector(root) : root;
  if (!element) {
    warn("WS live root not found: " + root);
    return;
  }

  const fullUrl = normalizeWsEndpoint(url);
  if (!fullUrl) return;

  // Replace existing connection for this root element
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

function connectWS(url, state) {
  if (state.paused) return;
  if (liveConnections.get(state.element) !== state) return;

  const socket = new WebSocket(url);
  state.socket = socket;

  socket.onopen = function () {
    state.backoff = 1000;
    document.dispatchEvent(
      new CustomEvent("silcrow:live:connect", {
        bubbles: true,
        detail: {root: state.element, url, protocol: "ws"},
      })
    );
  };

  socket.onmessage = function (e) {
    try {
      const msg = JSON.parse(e.data);
      const type = msg && msg.type;

      if (type === "patch") {
        const target = msg.target
          ? document.querySelector(msg.target) || state.element
          : state.element;
        if (msg.data !== undefined) {
          patch(msg.data, target);
        }
      } else if (type === "html") {
        const target = msg.target
          ? document.querySelector(msg.target) || state.element
          : state.element;
        safeSetHTML(target, msg.markup == null ? "" : String(msg.markup));
      } else if (type === "invalidate") {
        const target = msg.target
          ? document.querySelector(msg.target) || state.element
          : state.element;
        invalidate(target);
      } else if (type === "navigate") {
        if (msg.path) {
          navigate(msg.path.trim(), {trigger: "ws"});
        }
      } else if (type === "custom") {
        document.dispatchEvent(
          new CustomEvent("silcrow:ws:" + (msg.event || "message"), {
            bubbles: true,
            detail: {root: state.element, data: msg.data},
          })
        );
      } else {
        warn("Unknown WS event type: " + type);
      }
    } catch (err) {
      warn("Failed to parse WS message: " + err.message);
    }
  };

  socket.onclose = function () {
    state.socket = null;

    if (state.paused) return;
    if (liveConnections.get(state.element) !== state) return;

    const reconnectIn = state.backoff;

    document.dispatchEvent(
      new CustomEvent("silcrow:live:disconnect", {
        bubbles: true,
        detail: {root: state.element, url, reconnectIn, protocol: "ws"},
      })
    );

    state.reconnectTimer = setTimeout(function () {
      state.reconnectTimer = null;
      connectWS(url, state);
    }, reconnectIn);

    state.backoff = Math.min(state.backoff * 2, MAX_BACKOFF);
  };

  socket.onerror = function () {
    // onerror is always followed by onclose, so reconnect logic lives there
  };
}

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