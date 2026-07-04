import { describe, expect, it } from "vitest";
import { buildFileTree, type TreeFolderNode } from "./tree";
import type { FileRow } from "./sections";

function row(path: string, status: FileRow["status"] = "modified"): FileRow {
	return { path, origPath: null, status, partial: false };
}

describe("buildFileTree", () => {
	it("nests files under their folders", () => {
		const tree = buildFileTree([row("web/src/app.ts"), row("README.md")]);
		expect(tree).toHaveLength(2);
		const folder = tree[0] as TreeFolderNode;
		expect(folder.kind).toBe("folder");
		expect(folder.name).toBe("web");
	});

	it("rolls up per-status counts on ancestor folders", () => {
		const tree = buildFileTree([
			row("web/a.ts", "modified"),
			row("web/b.ts", "modified"),
			row("web/c.ts", "deleted"),
		]);
		const web = tree[0] as TreeFolderNode;
		expect(web.counts.modified).toBe(2);
		expect(web.counts.deleted).toBe(1);
	});

	it("aggregates counts through multiple folder levels", () => {
		const tree = buildFileTree([row("web/src/deep/file.ts", "added")]);
		const web = tree[0] as TreeFolderNode;
		const src = web.children[0] as TreeFolderNode;
		const deep = src.children[0] as TreeFolderNode;
		expect(web.counts.added).toBe(1);
		expect(src.counts.added).toBe(1);
		expect(deep.counts.added).toBe(1);
	});

	it("sorts folders before files, both alphabetically", () => {
		const tree = buildFileTree([row("b.txt"), row("a.txt"), row("zdir/x.ts")]);
		expect(tree.map((n) => (n.kind === "folder" ? n.name : n.row.path))).toEqual([
			"zdir",
			"a.txt",
			"b.txt",
		]);
	});
});
