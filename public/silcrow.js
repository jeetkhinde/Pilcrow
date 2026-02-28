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

// silcrow/url-safety.js
// ════════════════════════════════════════════════════════════
// URL Safety — shared protocol & URL validation primitives
// ════════════════════════════════════════════════════════════

const URL_SAFE_PROTOCOLS = new Set(["http:", "https:", "mailto:", "tel:"]);

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
    return URL_SAFE_PROTOCOLS.has(parsed.protocol);
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

function isUnsafeBoundUrl(el, prop, value) {
  const name = String(prop || "").toLowerCase();
  if (!name) return false;

  if (name === "srcset") {
    return !hasSafeSrcSet(value);
  }

  if (!URL_ATTRS.has(name)) return false;

  const allowDataImage = name === "src" && el.tagName === "IMG";
  return !hasSafeProtocol(value, allowDataImage);
}

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

  if (isUnsafeBoundUrl(el, prop, value)) {
    warn("Rejected unsafe URL in binding: " + prop);
    if (prop in knownProps) {
      el[prop] = "";
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
    const ssePath = normalizeSSEEndpoint(sideEffects.sse);
    if (!ssePath) return;
    document.dispatchEvent(
      new CustomEvent("silcrow:sse", {
        bubbles: true,
        detail: {path: ssePath, target: primaryTarget || null},
      })
    );
  }

  if (sideEffects.ws) {
    const target = primaryTarget || document.body;
    openWsLive(target, sideEffects.ws);
  }
}

// ── Fetch Request Construction ─────────────────────────────
function buildFetchOptions(method, body, wantsHTML, signal) {
  const opts = {
    method,
    headers: {
      "silcrow-target": "true",
      "Accept": wantsHTML ? "text/html" : "application/json",
    },
    signal,
  };

  if (body) {
    if (body instanceof FormData) {
      opts.body = body;
    } else {
      opts.headers["Content-Type"] = "application/json";
      opts.body = JSON.stringify(body);
    }
  }

  return opts;
}

// ── Response Header Processing ─────────────────────────────
function processResponseHeaders(response, fullUrl) {
  const result = {
    redirected: response.redirected,
    finalUrl: response.url || fullUrl,
    pushUrl: null,
    retargetSelector: null,
    sideEffects: {
      patch: response.headers.get("silcrow-patch"),
      invalidate: response.headers.get("silcrow-invalidate"),
      navigate: response.headers.get("silcrow-navigate"),
      sse: response.headers.get("silcrow-sse"),
      ws: response.headers.get("silcrow-ws"),
    },
  };

  // Fire trigger events
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

  // Retarget
  result.retargetSelector = response.headers.get("silcrow-retarget");

  // Push URL override
  result.pushUrl = response.headers.get("silcrow-push");
  if (result.pushUrl) {
    result.finalUrl = new URL(result.pushUrl, location.origin).href;
    result.redirected = true;
  }

  return result;
}

// ── Swap Content Preparation ───────────────────────────────
function prepareSwapContent(text, contentType, targetSelector) {
  const isJSON = contentType.includes("application/json");
  let swapContent;

  if (isJSON) {
    swapContent = JSON.parse(text);
    processToasts(true, swapContent);
  } else {
    const isFullPage = !targetSelector;
    swapContent = extractHTML(text, targetSelector, isFullPage);
    processToasts(false);
  }

  return {swapContent, isJSON};
}

// ── Post-Swap Finalization ─────────────────────────────────
function finalizeNavigation(ctx) {
  const {pushUrl, redirected, finalUrl, fullUrl, shouldPushHistory,
         trigger, targetSelector, targetEl, sideEffects} = ctx;

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
  let targetEl = target || document.body;
  const targetSelector = sourceEl?.getAttribute("s-target") || null;
  const shouldPushHistory = !skipHistory && !targetSelector && method === "GET";

  const event = new CustomEvent("silcrow:navigate", {
    bubbles: true,
    cancelable: true,
    detail: {url: fullUrl, method, trigger, target: targetEl},
  });
  if (!document.dispatchEvent(event)) return;

  // Abort previous in-flight GET to the same target
  const prevAbort = abortMap.get(targetEl);
  if (prevAbort && prevAbort.method === "GET") {
    prevAbort.controller.abort();
  }
  const controller = new AbortController();
  abortMap.set(targetEl, {controller, method});

  const timeout = getTimeout(sourceEl);
  let timedOut = false;
  const timeoutId = setTimeout(() => { timedOut = true; controller.abort(); }, timeout);

  showLoading(targetEl);

  try {
    let cached = method === "GET" ? cacheGet(fullUrl) : null;

    let text, contentType, redirected = false, finalUrl = fullUrl, pushUrl = null;
    let sideEffects = null;

    const wantsHTML = sourceEl?.hasAttribute("s-html");
    if (cached) {
      // Side-effect headers are intentionally not cached — they are
      // one-shot triggers that should only fire on the original response.
      text = cached.text;
      contentType = cached.contentType;
    } else {
      const fetchOpts = buildFetchOptions(method, body, wantsHTML, controller.signal);
      const response = await fetch(fullUrl, fetchOpts);

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const headerResult = processResponseHeaders(response, fullUrl);
      redirected = headerResult.redirected;
      finalUrl = headerResult.finalUrl;
      pushUrl = headerResult.pushUrl;
      sideEffects = headerResult.sideEffects;

      // Apply retarget
      if (headerResult.retargetSelector) {
        const newTarget = document.querySelector(headerResult.retargetSelector);
        if (newTarget) targetEl = newTarget;
      }

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

    // Route handler middleware
    if (routeHandler) {
      const handled = await routeHandler({
        url: fullUrl, finalUrl, redirected, method,
        trigger, response: text, contentType, target: targetEl,
      });
      if (handled === false) {
        hideLoading(targetEl);
        return;
      }
    }

    // Save scroll position before pushing
    if (shouldPushHistory && trigger !== "popstate") {
      const current = history.state || {};
      history.replaceState(
        {...current, scrollY: window.scrollY},
        "",
        location.href
      );
    }

    // Prepare and execute swap
    const {swapContent, isJSON} = prepareSwapContent(text, contentType, targetSelector);

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
      detail: {url: finalUrl, target: targetEl, content: swapContent, isJSON, proceed},
    });

    if (!document.dispatchEvent(beforeSwap)) return;
    if (!swapExecuted) proceed();

    // Finalize: side-effects, history, scroll, load event
    finalizeNavigation({
      pushUrl, redirected, finalUrl, fullUrl,
      shouldPushHistory, trigger, targetSelector, targetEl,
      sideEffects,
    });

  } catch (err) {
    if (err.name === "AbortError") {
      if (timedOut) {
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
    !Array.isArray(payload) &&
    Object.prototype.hasOwnProperty.call(payload, "target")
  ) {
    if (!Object.prototype.hasOwnProperty.call(payload, "data")) {
      warn("SSE patch envelope missing data field");
      return;
    }

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

function resolveLiveStates(root) {
  if (typeof root === "string") {
    // Route key: disconnect/reconnect all connections for the URL
    if (
      root.startsWith("/") ||
      root.startsWith("http://") ||
      root.startsWith("https://")
    ) {
      const fullUrl = new URL(root, location.origin).href;
      // Try HTTP-scheme first (SSE connections)
      let states = liveConnectionsByUrl.get(fullUrl);
      if (!states || states.size === 0) {
        // Fall back to WS-scheme (WebSocket connections)
        const wsUrl = fullUrl.replace(/^http(s?)/, "ws$1");
        states = liveConnectionsByUrl.get(wsUrl);
      }
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
    socket: null,
    url: fullUrl,
    element,
    backoff: 1000,
    paused: false,
    reconnectTimer: null,
    protocol: "sse",
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
        Object.prototype.hasOwnProperty.call(payload, "target")
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
  // Recommendation: Park this. It's a future-proofing concern, not a current bug. When you add SilcrowEvent::invalidate(target) to Rust, fix the JS listener at the same time to parse e.data for a target selector.
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

// ── Auto-scan for s-live elements on init ──────────────────
function initLiveElements() {
  const elements = document.querySelectorAll("[s-live]");
  for (const el of elements) {
    const raw = el.getAttribute("s-live");
    if (!raw) continue;

    if (raw.startsWith("ws:")) {
      openWsLive(el, raw.slice(3));
    } else {
      openLive(el, raw);
    }
  }
}

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

  const expectedOrigin = location.origin.replace(/^http(s?)/, "ws$1");
  if (parsed.origin !== expectedOrigin) {
    warn("Rejected cross-origin WebSocket URL: " + parsed.href);
    return null;
  }

  return parsed.href;
}

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

function dispatchWsMessage(hub, rawData) {
  try {
    const msg = JSON.parse(rawData);
    const type = msg && msg.type;

    let targets;
    if (msg.target) {
      const el = document.querySelector(msg.target);
      targets = el ? [el] : [];
    } else {
      targets = hub.subscribers;
    }

    if (type === "patch") {
      if (msg.data !== undefined) {
        for (const el of targets) {
          patch(msg.data, el);
        }
      }
    } else if (type === "html") {
      for (const el of targets) {
        safeSetHTML(el, msg.markup == null ? "" : String(msg.markup));
      }
    } else if (type === "invalidate") {
      for (const el of targets) {
        invalidate(el);
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
    function cleanupLiveNode(node) {
      const state = liveConnections.get(node);
      if (!state) return;

      if (state.protocol === "ws") {
        unsubscribeWs(node);
      } else {
        pauseLiveState(state);
        unregisterLiveState(state);
      }
    }

    for (const mutation of mutations) {
      for (const removed of mutation.removedNodes) {
        if (removed.nodeType !== 1) continue;

        cleanupLiveNode(removed);

        if (removed.querySelectorAll) {
          for (const child of removed.querySelectorAll("[s-live]")) {
            cleanupLiveNode(child);
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
      skipHistory: options.skipHistory || false,
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

})();