/** Extension → highlight.js language mapping — ARCHITECTURE.md §6.2. */

const EXTENSION_MAP: Record<string, string> = {
	ts: "typescript",
	tsx: "typescript",
	js: "javascript",
	jsx: "javascript",
	mjs: "javascript",
	cjs: "javascript",
	svelte: "xml",
	rs: "rust",
	py: "python",
	rb: "ruby",
	go: "go",
	java: "java",
	kt: "kotlin",
	swift: "swift",
	c: "c",
	h: "c",
	cpp: "cpp",
	cc: "cpp",
	hpp: "cpp",
	cs: "csharp",
	php: "php",
	sh: "bash",
	bash: "bash",
	zsh: "bash",
	sql: "sql",
	json: "json",
	yaml: "yaml",
	yml: "yaml",
	toml: "ini",
	md: "markdown",
	html: "xml",
	xml: "xml",
	css: "css",
	scss: "scss",
	less: "less",
	dart: "dart",
};

export function languageForPath(path: string): string | undefined {
	const ext = path.split(".").pop()?.toLowerCase();
	return ext ? EXTENSION_MAP[ext] : undefined;
}
