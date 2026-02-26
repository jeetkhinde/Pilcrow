(function(){"use strict";
// ./public/silcrow/debug.js
// ════════════════════════════════════════════════════════════
// Debug — shared diagnostics
// ════════════════════════════════════════════════════════════

const DEBUG = document.body.hasAttribute("s-debug");

function warn(msg) {
  if (DEBUG) console.warn("[silcrow]", msg);
}

function throwErr(msg) {
  if (DEBUG) throw new Error("[silcrow] " + msg);
}

// ./public/silcrow/patcher.js
// ════════════════════════════════════════════════════════════
// Patcher — reactive data binding & DOM patching
// ════════════════════════════════════════════════════════════

const instanceCache = new WeakMap();
const validatedTemplates = new WeakSet();
const localBindingsCache = new WeakMap();

const PATH_RE = /^\.?[A-Za-z0-9_-]+(\.[A-Za-z0-9_-]+)*$/;
function isValidPath(p) {return PATH_RE.test(p);}

function parseBind(el) {
  const raw = el.getAttribute("s-bind");
  if (!raw) return null;
  const idx = raw.indexOf(":");
  const path = idx === -1 ? raw : raw.substring(0, idx);
  const prop = idx === -1 ? null : raw.substring(idx + 1);
  return {path, prop};
}

function isOnHandler(prop) {
  return prop && prop.toLowerCase().startsWith("on");
}

const knownProps = {
  value: "string",
  checked: "boolean",
  disabled: "boolean",
  selected: "boolean",
  src: "string",
  href: "string",
  selectedIndex: "number",
};

function setValue(el, prop, value) {
  if (isOnHandler(prop)) {
    throwErr("Binding to event handler attribute rejected: " + prop);
    return;
  }

  if (prop === null) {
    el.textContent = value == null ? "" : String(value);
    return;
  }

  if (value == null) {
    if (prop in knownProps) {
      const t = knownProps[prop];
      if (t === "boolean") el[prop] = false;
      else if (t === "number") el[prop] = 0;
      else el[prop] = "";
    } else {
      el.removeAttribute(prop);
    }
    return;
  }

  if (prop in knownProps) {
    el[prop] = value;
  } else {
    el.setAttribute(prop, String(value));
  }
}

function scanBindableNodes(root) {
  const result = [];
  if (root.hasAttribute && root.hasAttribute("s-bind")) result.push(root);
  const descendants = root.querySelectorAll("[s-bind]");
  for (const el of descendants) {
    if (el.closest("template")) continue;
    result.push(el);
  }
  return result;
}

function registerBinding(el, scalarMap) {
  const parsed = parseBind(el);
  if (!parsed) return;
  const {path, prop} = parsed;
  if (!path || path.startsWith(".")) return;
  if (isOnHandler(prop)) {
    throwErr("Binding to event handler attribute rejected: " + prop);
    return;
  }
  if (!isValidPath(path)) {
    warn("Invalid path: " + path);
    return;
  }

  if (!scalarMap.has(path)) scalarMap.set(path, []);
  scalarMap.get(path).push({el, prop});
}

function registerSubtreeBindings(node, scalarMap) {
  const nodes = scanBindableNodes(node);
  for (const el of nodes) {
    const parsed = parseBind(el);
    if (!parsed) continue;
    if (parsed.path.startsWith('.')) continue;
    registerBinding(el, scalarMap);
  }
}

function validateTemplate(tpl) {
  const content = tpl.content;
  if (content.querySelectorAll("script").length) {
    throwErr("Script not allowed in template");
  }
  for (const el of content.querySelectorAll("*")) {
    for (const attr of el.attributes) {
      if (attr.name.toLowerCase().startsWith("on")) {
        throwErr("Event handler attribute not allowed in template");
      }
    }
    if (el.hasAttribute("s-list")) {
      throwErr("Nested s-list not allowed");
    }
  }
}

function cloneTemplate(tpl, scalarMap) {
  if (!validatedTemplates.has(tpl)) {
    validateTemplate(tpl);
    validatedTemplates.add(tpl);
  }
  const frag = tpl.content.cloneNode(true);
  const elements = [];
  for (const n of frag.children) {
    if (n.nodeType === 1) elements.push(n);
  }
  if (elements.length !== 1) {
    throwErr("Template must contain exactly one element child");
    return document.createElement("div");
  }
  const node = elements[0];

  const localBindings = new Map();

  if (node.hasAttribute("s-bind")) {
    const parsed = parseBind(node);
    if (parsed?.path.startsWith('.')) {
      const field = parsed.path.substring(1);
      if (!localBindings.has(field)) {
        localBindings.set(field, []);
      }
      localBindings.get(field).push({el: node, prop: parsed.prop});
    }
  }

  for (const el of node.querySelectorAll("[s-bind]")) {
    const parsed = parseBind(el);
    if (parsed?.path.startsWith('.')) {
      const field = parsed.path.substring(1);
      if (!localBindings.has(field)) {
        localBindings.set(field, []);
      }
      localBindings.get(field).push({el, prop: parsed.prop});
    }
  }

  localBindingsCache.set(node, localBindings);
  registerSubtreeBindings(node, scalarMap);
  return node;
}

function makeTemplateResolver(container, scalarMap) {
  const templateId = container.getAttribute("s-template");

  return function resolve(item) {
    let tpl = null;

    if (item && item.key != null) {
      const keyStr = String(item.key);
      const hashIdx = keyStr.indexOf("#");
      if (hashIdx !== -1) {
        const tplName = keyStr.substring(0, hashIdx);
        tpl = document.getElementById(tplName);
      }
    }

    if (!tpl && templateId) tpl = document.getElementById(templateId);
    if (!tpl) tpl = container.querySelector(":scope > template");

    if (!tpl) {
      throwErr("No resolvable template for collection");
      return document.createElement("div");
    }

    return cloneTemplate(tpl, scalarMap);
  };
}

function isValidCollectionArray(items) {
  for (let i = 0; i < items.length; i++) {
    const item = items[i];
    if (item == null || typeof item !== "object" || Array.isArray(item)) return false;
    if (!("key" in item)) return false;
  }
  return true;
}

function reconcile(container, items, resolveTemplate) {
  if (!isValidCollectionArray(items)) {
    warn("Collection array contains invalid items, discarding");
    return;
  }

  const existing = new Map();
  for (const child of container.children) {
    if (child.dataset && child.hasAttribute("s-key")) {
      existing.set(child.dataset.key, child);
    }
  }

  const validItems = [];
  for (const item of items) {
    if (item.key == null) {
      warn("Collection item missing key, skipping");
      continue;
    }
    validItems.push(item);
  }

  const seen = new Set();
  for (const item of validItems) {
    const k = String(item.key);
    if (seen.has(k)) {
      warn("Duplicate key: " + k);
      return;
    }
    seen.add(k);
  }

  const nextKeys = new Set();
  let prevNode = null;

  for (const item of validItems) {
    const key = String(item.key);
    nextKeys.add(key);

    let node = existing.get(key);

    if (!node) {
      node = resolveTemplate(item);
      node.dataset.key = key;
      node.setAttribute("s-key", "");
    }

    patchItem(node, item);

    if (prevNode) {
      if (prevNode.nextElementSibling !== node) {
        prevNode.after(node);
      }
    } else {
      if (container.firstElementChild !== node) {
        container.prepend(node);
      }
    }

    prevNode = node;
  }

  for (const [key, node] of existing) {
    if (!nextKeys.has(key)) {
      node.remove();
    }
  }
}

function patchItem(node, item) {
  const bindings = localBindingsCache.get(node);
  if (!bindings) return;

  for (const field in item) {
    if (field === "key") continue;
    const targets = bindings.get(field);
    if (!targets) continue;
    for (const {el, prop} of targets) {
      setValue(el, prop, item[field]);
    }
  }
}

function resolvePath(obj, path) {
  const parts = path.split('.');
  let current = obj;
  for (const part of parts) {
    if (current == null) return undefined;
    if (part === '__proto__' || part === 'constructor' || part === 'prototype') {
      return undefined;
    }
    current = current[part];
  }
  return current;
}

function buildMaps(root) {
  const scalarMap = new Map();
  const collectionMap = new Map();

  const bindings = root.querySelectorAll("[s-bind]");
  for (const el of bindings) {
    if (el.closest("template")) continue;
    registerBinding(el, scalarMap);
  }

  if (root.hasAttribute && root.hasAttribute("s-bind") && !root.closest("template")) {
    registerBinding(root, scalarMap);
  }

  const lists = root.querySelectorAll("[s-list]");
  for (const container of lists) {
    const listName = container.getAttribute("s-list");
    if (!isValidPath(listName)) {
      throwErr("Invalid collection name: " + listName);
      continue;
    }

    collectionMap.set(listName, {
      container,
      resolveTemplate: makeTemplateResolver(container, scalarMap),
    });
  }

  return {scalarMap, collectionMap};
}

function applyPatch(data, scalarMap, collectionMap) {
  for (const [path, bindings] of scalarMap.entries()) {
    const value = resolvePath(data, path);
    if (value !== undefined) {
      for (const {el, prop} of bindings) {
        setValue(el, prop, value);
      }
    }
  }

  for (const [path, {container, resolveTemplate}] of collectionMap.entries()) {
    const value = resolvePath(data, path);
    if (Array.isArray(value)) {
      reconcile(container, value, resolveTemplate);
    } else if (value !== undefined && DEBUG) {
      warn("Collection value is not an array: " + path);
    }
  }
}

function resolveRoot(root) {
  if (typeof root === "string") {
    const el = document.querySelector(root);
    if (!el) {
      throwErr("Root element not found: " + root);
      return document.createElement("div");
    }
    return el;
  }
  if (root instanceof Element) return root;
  throwErr("Invalid root: must be selector string or Element");
  return document.createElement("div");
}

function patch(data, root, options = {}) {
  const element = resolveRoot(root);

  let instance = instanceCache.get(element);

  if (!instance || options.invalidate) {
    instance = buildMaps(element);
    instanceCache.set(element, instance);
  }

  applyPatch(data, instance.scalarMap, instance.collectionMap);

  if (!options.silent) {
    element.dispatchEvent(new CustomEvent('silcrow:patched', {
      bubbles: true,
      detail: {paths: Array.from(instance.scalarMap.keys())}
    }));
  }
}

function invalidate(root) {
  const element = resolveRoot(root);
  instanceCache.delete(element);

  const templates = element.querySelectorAll('template');
  for (const tpl of templates) {
    validatedTemplates.delete(tpl);
  }
}

function stream(root) {
  const element = resolveRoot(root);
  let pending = null;
  let scheduled = false;

  return function update(data) {
    pending = data;
    if (scheduled) return;

    scheduled = true;
    queueMicrotask(() => {
      scheduled = false;
      patch(pending, element);
    });
  };
}

// silcrow/safety.js
// ════════════════════════════════════════════════════════════
// Safety — HTML extraction & sanitization
// ════════════════════════════════════════════════════════════

function extractHTML(html, targetSelector, isFullPage) {
  const trimmed = html.trimStart();
  if (trimmed.startsWith("<!") || trimmed.startsWith("<html")) {
    const parser = new DOMParser();
    const doc = parser.parseFromString(html, "text/html");

    if (isFullPage) {
      const title = doc.querySelector("title");
      if (title) document.title = title.textContent;
    }

    if (targetSelector) {
      const match = doc.querySelector(targetSelector);
      if (match) return match.innerHTML;
    }

    return doc.body.innerHTML;
  }
  return html;
}

const FORBIDDEN_HTML_TAGS = new Set([
  "base",
  "embed",
  "frame",
  "iframe",
  "link",
  "meta",
  "object",
  "script",
  "style",
]);

const URL_ATTRS = new Set([
  "action",
  "background",
  "cite",
  "formaction",
  "href",
  "poster",
  "src",
  "xlink:href",
]);

const SAFE_PROTOCOLS = new Set(["http:", "https:", "mailto:", "tel:"]);
const SAFE_DATA_IMAGE_RE =
  /^data:image\/(?:avif|bmp|gif|jpe?g|png|webp);base64,[a-z0-9+/]+=*$/i;

function hasSafeProtocol(raw, allowDataImage) {
  const value = String(raw || "").trim();
  if (!value) return true;

  const compact = value.replace(/[\u0000-\u0020\u007F]+/g, "");
  if (/^(?:javascript|vbscript|file):/i.test(compact)) return false;

  if (/^data:/i.test(compact)) {
    return allowDataImage && SAFE_DATA_IMAGE_RE.test(compact);
  }

  try {
    const parsed = new URL(value, location.origin);
    return SAFE_PROTOCOLS.has(parsed.protocol);
  } catch (e) {
    return false;
  }
}

function hasSafeSrcSet(raw) {
  const parts = String(raw || "").split(",");
  for (const part of parts) {
    const candidate = part.trim();
    if (!candidate) continue;
    const idx = candidate.search(/\s/);
    const url = idx === -1 ? candidate : candidate.slice(0, idx);
    if (!hasSafeProtocol(url, false)) {
      return false;
    }
  }
  return true;
}

function hardenBlankTargets(node) {
  if (node.tagName !== "A") return;
  if (String(node.getAttribute("target") || "").toLowerCase() !== "_blank") return;

  const relTokens = new Set(
    String(node.getAttribute("rel") || "")
      .toLowerCase()
      .split(/\s+/)
      .filter(Boolean)
  );
  relTokens.add("noopener");
  relTokens.add("noreferrer");
  node.setAttribute("rel", Array.from(relTokens).join(" "));
}

function sanitizeTree(root) {
  for (const tag of FORBIDDEN_HTML_TAGS) {
    for (const node of root.querySelectorAll(tag)) {
      node.remove();
    }
  }

  for (const node of root.querySelectorAll("*")) {
    if (node.namespaceURI !== "http://www.w3.org/1999/xhtml") {
      node.remove();
      continue;
    }

    for (const attr of [...node.attributes]) {
      const name = attr.name.toLowerCase();
      const value = attr.value;

      if (name.startsWith("on") || name === "style" || name === "srcdoc") {
        node.removeAttribute(attr.name);
        continue;
      }

      if (name === "srcset" && !hasSafeSrcSet(value)) {
        node.removeAttribute(attr.name);
        continue;
      }

      if (URL_ATTRS.has(name)) {
        const allowDataImage = name === "src" && node.tagName === "IMG";
        if (!hasSafeProtocol(value, allowDataImage)) {
          node.removeAttribute(attr.name);
        }
      }
    }

    hardenBlankTargets(node);
  }

  for (const tpl of root.querySelectorAll("template")) {
    sanitizeTree(tpl.content);
  }
}

function safeSetHTML(el, raw) {
  const markup = raw == null ? "" : String(raw);

  if (el.setHTML) {
    el.setHTML(markup);
    return;
  }

  const doc = new DOMParser().parseFromString(markup, "text/html");
  sanitizeTree(doc.body);

  el.innerHTML = doc.body.innerHTML;
}

// silcrow/toasts.js
// ════════════════════════════════════════════════════════════
// Toasts — notification processing
// ════════════════════════════════════════════════════════════

let toastHandler = null;

function processToasts(isJSON, content = null) {
  if (!toastHandler) return;

  if (isJSON && content && content._toasts) {
    content._toasts.forEach(t => toastHandler(t.message, t.level));
    delete content._toasts;

    if (content.data !== undefined && Object.keys(content).length === 1) {
      Object.assign(content, content.data);
      delete content.data;
    }
  } else if (!isJSON) {
    const match = document.cookie.match(new RegExp('(^|;\\s*)silcrow_toasts=([^;]+)'));
    if (match) {
      try {
        const rawJSON = decodeURIComponent(match[2]);
        const toasts = JSON.parse(rawJSON);
        toasts.forEach(t => toastHandler(t.message, t.level));
      } catch (e) {
        console.error("Failed to parse toasts", e);
      }
      document.cookie = "silcrow_toasts=; Max-Age=0; path=/";
    }
  }
}

function setToastHandler(handler) {
  toastHandler = handler;
  processToasts(false);
}

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
function processSideEffectHeaders(sideEffects, liveRoot) {
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
        detail: {path: ssePath, root: liveRoot || null},
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

// silcrow/optimistic.js
// ════════════════════════════════════════════════════════════
// Optimistic — snapshot & revert for instant UI feedback
// ════════════════════════════════════════════════════════════

const snapshots = new WeakMap();

function optimisticPatch(root, data) {
  const element = typeof root === "string" ? document.querySelector(root) : root;
  if (!element) {
    warn("Optimistic root not found: " + root);
    return;
  }

  // Snapshot current DOM state
  snapshots.set(element, element.innerHTML);

  // Apply the optimistic data
  patch(data, element);

  document.dispatchEvent(
    new CustomEvent("silcrow:optimistic", {
      bubbles: true,
      detail: {root: element, data},
    })
  );
}

function revertOptimistic(root) {
  const element = typeof root === "string" ? document.querySelector(root) : root;
  if (!element) {
    warn("Revert root not found: " + root);
    return;
  }

  const saved = snapshots.get(element);
  if (saved === undefined) {
    warn("No snapshot to revert for element");
    return;
  }

  element.innerHTML = saved;
  snapshots.delete(element);
  invalidate(element);

  document.dispatchEvent(
    new CustomEvent("silcrow:revert", {
      bubbles: true,
      detail: {root: element},
    })
  );
}
// silcrow/index.js

// ════════════════════════════════════════════════════════════
// API — public surface & lifecycle
// ════════════════════════════════════════════════════════════

function init() {
  document.addEventListener("click", onClick);
  document.addEventListener("submit", onSubmit);
  window.addEventListener("popstate", onPopState);
  document.addEventListener("mouseenter", onMouseEnter, true);

  if (!history.state?.silcrow) {
    history.replaceState(
      {silcrow: true, url: location.href},
      "",
      location.href
    );
  }

  // Auto-scan for s-live elements
  initLiveElements();
}

function destroy() {
  document.removeEventListener("click", onClick);
  document.removeEventListener("submit", onSubmit);
  window.removeEventListener("popstate", onPopState);
  document.removeEventListener("mouseenter", onMouseEnter, true);
  responseCache.clear();
  preloadInflight.clear();
  destroyAllLive();
}

window.Silcrow = {
  // Runtime
  patch,
  invalidate,
  stream,
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

// Backward compatibility
window.SilcrowNavigate = window.Silcrow;

// Auto-init navigation when DOM is ready
if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", init);
} else {
  init();
}
})();