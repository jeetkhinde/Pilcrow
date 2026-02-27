// silcrow/index.js

// ════════════════════════════════════════════════════════════
// API — public surface & lifecycle
// ════════════════════════════════════════════════════════════
let liveObserver = null;
function init() {
  document.addEventListener("click", onClick);
  document.addEventListener("submit", onSubmit);
  window.addEventListener("popstate", onPopState);
  document.addEventListener("mouseenter", onMouseEnter, true);
  document.addEventListener("silcrow:sse", onSSEEvent);

  if (!history.state?.silcrow) {
    history.replaceState(
      {silcrow: true, url: location.href},
      "",
      location.href
    );
  }

  // Auto-scan for s-live elements
  initLiveElements();
// Observe DOM for removed live elements
  liveObserver = new MutationObserver(function (mutations) {
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

}

function destroy() {
  document.removeEventListener("click", onClick);
  document.removeEventListener("submit", onSubmit);
  window.removeEventListener("popstate", onPopState);
  document.removeEventListener("mouseenter", onMouseEnter, true);
  document.removeEventListener("silcrow:sse", onSSEEvent);
  if (liveObserver) {
    liveObserver.disconnect();
    liveObserver = null;
  }
  responseCache.clear();
  preloadInflight.clear();
  destroyAllLive();
}

window.Silcrow = {
  // Runtime
  patch,
  invalidate,
  stream,
  send: sendWs,
  onToast(handler) {
    setToastHandler(handler);
    return this;
  },

  // Navigation
  go(path, options = {}) {
    return navigate(path, {
      method: options.method || (options.body ? "POST" : "GET"),
      body: options.body || null,
      target: options.target
        ? document.querySelector(options.target)
        : null,
      trigger: "api",
    });
  },

  onRoute(handler) {
    routeHandler = handler;
    return this;
  },

  onError(handler) {
    errorHandler = handler;
    return this;
  },

  cache: {
    clear(path) {
      if (path) {
        const url = new URL(path, location.origin).href;
        responseCache.delete(url);
      } else {
        responseCache.clear();
      }
    },
    has(path) {
      const url = new URL(path, location.origin).href;
      return !!cacheGet(url);
    },
  },

  // Live (SSE)
  live(root, url) {
    openLive(root, url);
  },

  disconnect(root) {
    disconnectLive(root);
  },

  reconnect(root) {
    reconnectLive(root);
  },

  // Optimistic
  optimistic(root, data) {
    optimisticPatch(root, data);
  },

  revert(root) {
    revertOptimistic(root);
  },

  destroy,
};


// Auto-init navigation when DOM is ready
if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", init);
} else {
  init();
}
