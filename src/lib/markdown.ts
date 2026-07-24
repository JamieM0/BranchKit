import { marked } from "marked";

/**
 * Full Markdown parsing is provided by `marked`, the one added frontend dependency. AI output is
 * untrusted, so its HTML is then reduced to this deliberately small safe allow-list before it is
 * inserted with Svelte's `{@html}`. Links are restricted to ordinary web/mail targets.
 */
const allowedTags = new Set([
  "a",
  "blockquote",
  "br",
  "code",
  "del",
  "details",
  "div",
  "em",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "hr",
  "img",
  "kbd",
  "li",
  "ol",
  "p",
  "pre",
  "s",
  "span",
  "strong",
  "summary",
  "table",
  "tbody",
  "td",
  "th",
  "thead",
  "tr",
  "ul",
  "input",
]);
const globalAttrs = new Set(["class", "title"]);
const tagAttrs: Record<string, Set<string>> = {
  a: new Set(["href"]),
  img: new Set(["src", "alt", "width", "height"]),
  ol: new Set(["start"]),
  input: new Set(["checked", "disabled", "type"]),
};

function safeUrl(value: string, image = false): boolean {
  try {
    const url = new URL(value, window.location.href);
    return image
      ? ["http:", "https:"].includes(url.protocol)
      : ["http:", "https:", "mailto:"].includes(url.protocol);
  } catch {
    return false;
  }
}

function sanitize(html: string): string {
  const template = document.createElement("template");
  template.innerHTML = html;
  for (const element of [...template.content.querySelectorAll("*")]) {
    if (!allowedTags.has(element.tagName.toLowerCase())) {
      element.replaceWith(document.createTextNode(element.textContent ?? ""));
      continue;
    }
    for (const attribute of [...element.attributes]) {
      const tag = element.tagName.toLowerCase();
      const name = attribute.name.toLowerCase();
      const allowed = globalAttrs.has(name) || tagAttrs[tag]?.has(name);
      if (!allowed || name.startsWith("on")) {
        element.removeAttribute(attribute.name);
        continue;
      }
      if (name === "href" && !safeUrl(attribute.value))
        element.removeAttribute(attribute.name);
      if (name === "src" && !safeUrl(attribute.value, true))
        element.removeAttribute(attribute.name);
    }
  }
  return template.innerHTML;
}

export function renderMarkdown(markdown: string): string {
  if (typeof document === "undefined") return "";
  return sanitize(
    marked.parse(markdown, { async: false, gfm: true, breaks: true }) as string,
  );
}
