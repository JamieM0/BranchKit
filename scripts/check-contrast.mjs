import fs from "node:fs";

const css = fs.readFileSync(new URL("../src/lib/tokens.css", import.meta.url), "utf8");

function declarations(block) {
  return Object.fromEntries(
    [...block.matchAll(/--([a-z0-9-]+):\s*(#[0-9a-f]{6})\s*;/gi)].map((match) => [
      match[1],
      match[2].toLowerCase(),
    ]),
  );
}

function block(pattern, label) {
  const match = css.match(pattern);
  if (!match) throw new Error(`Could not find ${label} token block`);
  return declarations(match[1]);
}

const shared = block(/:root\s*\{([\s\S]*?)\n\}/, "shared");
const themes = Object.fromEntries(
  ["dark", "light"].map((theme) => [
    theme,
    {
      ...shared,
      ...block(
        new RegExp(`:root\\[data-theme=["']${theme}["']\\]\\s*\\{([\\s\\S]*?)\\n\\}`),
        `${theme} theme`,
      ),
    },
  ]),
);

function luminance(hex) {
  const channels = hex
    .slice(1)
    .match(/../g)
    .map((value) => Number.parseInt(value, 16) / 255)
    .map((value) =>
      value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4,
    );
  return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2];
}

function ratio(a, b) {
  const [lighter, darker] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (lighter + 0.05) / (darker + 0.05);
}

const surfaces = ["bg", "surface", "raised", "overlay"];
const foregrounds = [
  "text",
  "text-muted",
  "text-faint",
  "accent",
  "info",
  "warn",
  "danger",
  "ahead",
  "behind",
  "status-added",
  "status-modified",
  "status-deleted",
  "status-renamed",
  "status-conflicted",
  "status-untracked",
];

const failures = [];
for (const [theme, tokens] of Object.entries(themes)) {
  for (const foreground of foregrounds) {
    for (const surface of surfaces) {
      const measured = ratio(tokens[foreground], tokens[surface]);
      const result = `${theme.padEnd(5)} ${foreground.padEnd(18)} on ${surface.padEnd(7)} ${measured.toFixed(2)}:1`;
      console.log(result);
      if (measured < 4.5) failures.push(result);
    }
  }
  for (const fill of ["accent", "danger"]) {
    const measured = ratio(tokens.bg, tokens[fill]);
    const result = `${theme.padEnd(5)} bg text            on ${fill.padEnd(7)} ${measured.toFixed(2)}:1`;
    console.log(result);
    if (measured < 4.5) failures.push(result);
  }
}

if (failures.length > 0) {
  console.error(`\n${failures.length} WCAG AA contrast check(s) failed.`);
  process.exit(1);
}

console.log("\nAll semantic text/status token pairs meet WCAG AA (4.5:1). ");
