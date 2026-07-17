import { readdir, readFile } from "node:fs/promises";
import { join, relative } from "node:path";

const root = new URL("../src", import.meta.url).pathname;
const files = [];

async function walk(dir) {
	for (const entry of await readdir(dir, { withFileTypes: true })) {
		const path = join(dir, entry.name);
		if (entry.isDirectory()) await walk(path);
		else if (entry.name.endsWith(".svelte")) files.push(path);
	}
}

await walk(root);
const failures = [];
for (const file of files) {
	const source = await readFile(file, "utf8");
	const buttons = source.matchAll(/<button\b([^>]*)>([\s\S]*?)<\/button>/g);
	for (const match of buttons) {
		const [, attributes, body] = match;
		const hasProgrammaticName = /\baria-(?:label|labelledby)\s*=/.test(attributes);
		const hasIconComponent = /<(?:svg|[A-Z][A-Za-z0-9_]*)\b/.test(body);
		const hasDynamicText = /\{\s*[A-Za-z_$][^}]*\}/.test(body);
		const text = body
			.replace(/<!--[\s\S]*?-->/g, "")
			.replace(/<[^>]+>/g, " ")
			.replace(/\{[#/:@][^}]+\}/g, " ")
			.replace(/\{[^}]+\}/g, " ")
			.replace(/\s+/g, " ")
			.trim();
		const glyphOnly = text.length > 0 && !/[\p{L}\p{N}]/u.test(text);
		if ((hasIconComponent && text.length === 0 && !hasDynamicText) || (glyphOnly && !hasDynamicText)) {
			if (!hasProgrammaticName) {
				const line = source.slice(0, match.index).split("\n").length;
				failures.push(`${relative(new URL("..", import.meta.url).pathname, file)}:${line}`);
			}
		}
	}
}

if (failures.length > 0) {
	console.error(`Icon buttons without aria-label/aria-labelledby:\n${failures.join("\n")}`);
	process.exit(1);
}
console.log(`Icon-button accessible-name audit passed (${files.length} Svelte files).`);
