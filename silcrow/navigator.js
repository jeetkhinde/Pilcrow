// silcrow/navigator.js
// ════════════════════════════════════════════════════════════
// Navigator — client-side routing, history, caching
// ════════════════════════════════════════════════════════════

const HTTP_METHODS = ["DELETE", "PUT", "POST", "PATCH", "GET"];
const DEFAULT_TIMEOUT = 30000;

const CACHE_TTL = 5 * 60 * 1000;
const MAX_CACHE = 50;
const abortMap = new WeakMap();
let routeHandler = null;
let errorHandler = null;
const responseCache = new Map();
const preloadInflight = new Map();

// ── HTTP Method Detection ──────────────────────────────────
function getMethod(el) {
  if (el.tagName === "FORM") {
    return (el.getAttribute("method") || "POST").toUpperCase();
  }
  for (const method of HTTP_METHODS) {
    if (el.hasAttribute(method) || el.hasAttribute(method.toLowerCase())) {
      return method;
    }
  }
  return "GET";
}

// ── URL Resolution ─────────────────────────────────────────
function resolveUrl(el) {
  const raw = el.getAttribute("s-action");
  if (!raw) return null;
  try {
    return new URL(raw, location.origin).href;
  } catch (e) {
    return null;
  }
}

// ── Target Resolution ──────────────────────────────────────
function getTarget(el) {
  const sel = el.getAttribute("s-target");
  if (sel) {
    const target = document.querySelector(sel);
    if (target) return target;
  }
  return el;
}

// ── Timeout Resolution ─────────────────────────────────────
function getTimeout(el) {
  const val = el?.getAttribute("s-timeout");
  return val ? parseInt(val, 10) : DEFAULT_TIMEOUT;
}

// ── Loading State ──────────────────────────────────────────
function showLoading(el) {
  el.classList.add("silcrow-loading");
  el.setAttribute("aria-busy", "true");
}

function hideLoading(el) {
  el.classList.remove("silcrow-loading");
  el.removeAttribute("aria-busy");
}

// ── Cache Management ───────────────────────────────────────
function cacheSet(url, entry) {
  responseCache.set(url, entry);
  if (responseCache.size > MAX_CACHE) {
    const oldest = responseCache.keys().next().value;
    responseCache.delete(oldest);
  }
}

function cacheGet(url) {
  const cached = responseCache.get(url);
  if (!cached) return null;
  if (Date.now() - cached.ts > CACHE_TTL) {
    responseCache.delete(url);
    return null;
  }
  return cached;
}

function bustCacheOnMutation() {
  responseCache.clear();
}

// ── Side-Effect Header Processing ──────────────────────────
function processSideEffectHeaders(sideEffects, primaryTarget) {
  if (!sideEffects) return;

  // Order: patch → invalidate → navigate → sse
  if (sideEffects.patch) {
    try {
      const payload = JSON.parse(sideEffects.patch);
      if (
        payload &&
        typeof payload === "object" &&
        payload.target &&
        Object.prototype.hasOwnProperty.call(payload, "data")
      ) {
        const el = document.querySelector(payload.target);
        if (el) patch(payload.data, el);
      }
    } catch (e) {
      warn("Failed to process silcrow-patch header: " + e.message);
    }
  }

  if (sideEffects.invalidate) {
    const el = document.querySelector(sideEffects.invalidate);
    if (el) invalidate(el);
  }

  if (sideEffects.navigate) {
    navigate(sideEffects.navigate, {trigger: "header"});
  }

  if (sideEffects.sse) {
    const ssePath =
      typeof normalizeSSEEndpoint === "function"
        ? normalizeSSEEndpoint(sideEffects.sse)
        : sideEffects.sse;
    if (!ssePath) return;

    document.dispatchEvent(
      new CustomEvent("silcrow:sse", {
        bubbles: true,
        detail: {path: sideEffects.sse, target: primaryTarget || null},
      })
    );
  }
}

// ── Core Navigate ──────────────────────────────────────────
async function navigate(url, options = {}) {
  const {
    method = "GET",
    body = null,
    target = null,
    trigger = "click",
    skipHistory = false,
    sourceEl = null,
  } = options;

  const fullUrl = new URL(url, location.origin).href;
  const targetEl = target || document.body;
  const targetSelector = sourceEl?.getAttribute("s-target") || null;

  const shouldPushHistory = !skipHistory && !targetSelector && method === "GET";

  const event = new CustomEvent("silcrow:navigate", {
    bubbles: true,
    cancelable: true,
    detail: {url: fullUrl, method, trigger, target: targetEl},
  });
  if (!document.dispatchEvent(event)) return;

  const prevAbort = abortMap.get(targetEl);
  if (prevAbort && prevAbort.method === "GET") {
    prevAbort.controller.abort();
  }
  const controller = new AbortController();
  abortMap.set(targetEl, {controller, method});

  const timeout = getTimeout(sourceEl);
  const timeoutId = setTimeout(() => controller.abort(), timeout);

  showLoading(targetEl);

  try {
    let cached = method === "GET" ? cacheGet(fullUrl) : null;

    let text, contentType, redirected = false, finalUrl = fullUrl, pushUrl = null;
    let sideEffects = null;

    const wantsHTML = sourceEl?.hasAttribute("s-html");
    if (cached) {
      text = cached.text;
      contentType = cached.contentType;
    } else {
      const fetchOptions = {
        method,
        headers: {
          "silcrow-target": "true",
          "Accept": wantsHTML ? "text/html" : "application/json",
        },
        signal: controller.signal,
      };

      if (body) {
        if (body instanceof FormData) {
          fetchOptions.body = body;
        } else {
          fetchOptions.headers["Content-Type"] = "application/json";
          fetchOptions.body = JSON.stringify(body);
        }
      }

      const response = await fetch(fullUrl, fetchOptions);

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      redirected = response.redirected;

      const triggerHeader = response.headers.get("silcrow-trigger");
      if (triggerHeader) {
        try {
          const triggers = JSON.parse(triggerHeader);
          Object.entries(triggers).forEach(([evt, detail]) => {
            document.dispatchEvent(new CustomEvent(evt, {bubbles: true, detail}));
          });
        } catch (e) {
          document.dispatchEvent(new CustomEvent(triggerHeader, {bubbles: true}));
        }
      }

      const retarget = response.headers.get("silcrow-retarget");
      if (retarget) {
        const newTarget = document.querySelector(retarget);
        if (newTarget) options.target = newTarget;
      }

      pushUrl = response.headers.get("silcrow-push-url");
      if (pushUrl) {
        finalUrl = new URL(pushUrl, location.origin).href;
        redirected = true;
      }

      // Capture side-effect headers for post-swap processing
      sideEffects = {
        patch: response.headers.get("silcrow-patch"),
        invalidate: response.headers.get("silcrow-invalidate"),
        navigate: response.headers.get("silcrow-navigate"),
        sse: response.headers.get("silcrow-sse"),
      };

      finalUrl = response.url || fullUrl;

      text = await response.text();
      contentType = response.headers.get("Content-Type") || "";

      const cacheControl = response.headers.get("silcrow-cache");
      if (method === "GET" && !redirected && cacheControl !== "no-cache") {
        cacheSet(fullUrl, {text, contentType, ts: Date.now()});
      }

      if (method !== "GET") {
        bustCacheOnMutation();
      }
    }

    if (routeHandler) {
      const handled = await routeHandler({
        url: fullUrl,
        finalUrl,
        redirected,
        method,
        trigger,
        response: text,
        contentType,
        target: targetEl,
      });
      if (handled === false) {
        hideLoading(targetEl);
        return;
      }
    }

    if (shouldPushHistory && trigger !== "popstate") {
      const current = history.state || {};
      history.replaceState(
        {...current, scrollY: window.scrollY},
        "",
        location.href
      );
    }

    let swapContent;
    const isJSON = contentType.includes("application/json");

    if (isJSON) {
      swapContent = JSON.parse(text);
      processToasts(true, swapContent);
    } else {
      const isFullPage = !targetSelector;
      swapContent = extractHTML(text, targetSelector, isFullPage);
      processToasts(false);
    }

    let swapExecuted = false;
    const proceed = () => {
      if (swapExecuted) return;
      swapExecuted = true;
      if (isJSON) {
        patch(swapContent, targetEl);
      } else {
        safeSetHTML(targetEl, swapContent);
      }
    };

    const beforeSwap = new CustomEvent("silcrow:before-swap", {
      bubbles: true,
      cancelable: true,
      detail: {
        url: finalUrl,
        target: targetEl,
        content: swapContent,
        isJSON,
        proceed,
      },
    });

    if (!document.dispatchEvent(beforeSwap)) return;

    if (!swapExecuted) proceed();

    // Process side-effect headers after the main swap
    processSideEffectHeaders(sideEffects, targetEl);

    const finalHistoryUrl = pushUrl || (redirected ? finalUrl : fullUrl);
    if (shouldPushHistory && trigger !== "popstate") {
      history.pushState(
        {silcrow: true, url: finalHistoryUrl, targetSelector},
        "",
        finalHistoryUrl
      );
    }

    if (trigger === "popstate") {
      const saved = (history.state || {}).scrollY;
      window.scrollTo(0, saved || 0);
    } else if (shouldPushHistory) {
      window.scrollTo(0, 0);
    }

    document.dispatchEvent(
      new CustomEvent("silcrow:load", {
        bubbles: true,
        detail: {url: finalUrl, target: targetEl, redirected},
      })
    );
  } catch (err) {
    if (err.name === "AbortError") {
      if (controller.signal.aborted) {
        const timeoutErr = new Error(
          `[silcrow] Request timed out after ${timeout}ms`
        );
        timeoutErr.name = "TimeoutError";
        document.dispatchEvent(
          new CustomEvent("silcrow:error", {
            bubbles: true,
            detail: {error: timeoutErr, url: fullUrl},
          })
        );
        if (errorHandler) {
          errorHandler(timeoutErr, {url: fullUrl, method, trigger, target: targetEl});
        }
      }
      return;
    }

    if (errorHandler) {
      errorHandler(err, {url: fullUrl, method, trigger, target: targetEl});
    } else {
      console.error("[silcrow]", err);
    }

    document.dispatchEvent(
      new CustomEvent("silcrow:error", {
        bubbles: true,
        detail: {error: err, url: fullUrl},
      })
    );
  } finally {
    clearTimeout(timeoutId);
    hideLoading(targetEl);
    abortMap.delete(targetEl);
  }
}

// ── Click Handler (opt-in: only [s-action]) ────────────────
async function onClick(e) {
  if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
  if (e.button !== 0) return;

  const el = e.target.closest("[s-action]");
  if (!el || el.tagName === "FORM") return;

  e.preventDefault();

  const fullUrl = resolveUrl(el);
  if (!fullUrl) return;

  const inflight = preloadInflight.get(fullUrl);
  if (inflight) await inflight;

  navigate(fullUrl, {
    method: getMethod(el),
    target: getTarget(el),
    skipHistory: el.hasAttribute("s-skip-history"),
    sourceEl: el,
    trigger: "click",
  });
}

// ── Form Handler (opt-in: only form[s-action]) ─────────────
function onSubmit(e) {
  const form = e.target.closest("form[s-action]");
  if (!form) return;

  e.preventDefault();

  const method = getMethod(form);
  const formData = new FormData(form);

  if (method === "GET") {
    const actionUrl = new URL(form.getAttribute("s-action"), location.origin);
    for (const [k, v] of formData) {
      actionUrl.searchParams.append(k, v);
    }
    navigate(actionUrl.href, {
      method: "GET",
      target: getTarget(form),
      sourceEl: form,
      trigger: "submit",
    });
  } else {
    navigate(form.getAttribute("s-action"), {
      method,
      body: formData,
      target: getTarget(form),
      sourceEl: form,
      trigger: "submit",
    });
  }
}

// ── Popstate Handler ───────────────────────────────────────
function onPopState(e) {
  if (!e.state) return;

  const url = location.href;
  const state = e.state;

  const targetSelector = state.targetSelector;
  const target = targetSelector
    ? document.querySelector(targetSelector)
    : document.body;

  navigate(url, {
    method: "GET",
    target: target || document.body,
    trigger: "popstate",
    skipHistory: true,
  });
}

// ── Preload Handler ────────────────────────────────────────
function onMouseEnter(e) {
  const el = e.target.closest("[s-preload]");
  if (!el) return;

  const fullUrl = resolveUrl(el);
  if (!fullUrl || responseCache.has(fullUrl) || preloadInflight.has(fullUrl)) return;
  const controller = new AbortController();
  const wantsHTML = el.hasAttribute("s-html");
  const promise = fetch(fullUrl, {
    headers: {"silcrow-target": "true", "Accept": wantsHTML ? "text/html" : "application/json"},
    signal: controller.signal,
  })
    .then((r) => {
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      const contentType = r.headers.get("Content-Type") || "";
      const cacheControl = r.headers.get("silcrow-cache");
      return r.text().then((text) => ({text, contentType, cacheControl}));
    })
    .then(({text, contentType, cacheControl}) => {
      if (cacheControl !== "no-cache") {
        cacheSet(fullUrl, {text, contentType, ts: Date.now()});
      }
    })
    .catch(() => {})
    .finally(() => preloadInflight.delete(fullUrl));

  preloadInflight.set(fullUrl, promise);
}
