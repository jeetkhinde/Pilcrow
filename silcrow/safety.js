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

function safeSetHTML(el, raw) {
  if (el.setHTML) {
    el.setHTML(raw);
    return;
  }

  const doc = new DOMParser().parseFromString(raw, "text/html");

  for (const script of doc.querySelectorAll("script")) script.remove();
  for (const node of doc.querySelectorAll("*")) {
    for (const attr of [...node.attributes]) {
      if (attr.name.toLowerCase().startsWith("on")) {
        node.removeAttribute(attr.name);
      }
    }
  }

  el.innerHTML = doc.body.innerHTML;
}
