/** Tree-mode file list — DESIGN_SPEC.md §6.1 "Tree mode: folders collapsible with rolled-up
 * badges (`✎4 −1` on `web/`)". Builds a folder tree from flat paths and aggregates a per-status
 * count on every folder covering all files beneath it. */

import type { FileStatusCode } from "$lib/types";
import type { FileRow } from "./sections";

export interface TreeFileNode {
	kind: "file";
	row: FileRow;
}

export interface TreeFolderNode {
	kind: "folder";
	name: string;
	/** Full path from the repo root, e.g. `web/src`. */
	path: string;
	children: TreeNode[];
	/** Count of files below this folder per status code — the rolled-up badge. */
	counts: Partial<Record<FileStatusCode, number>>;
}

export type TreeNode = TreeFileNode | TreeFolderNode;

interface MutableFolder {
	kind: "folder";
	name: string;
	path: string;
	children: Map<string, MutableFolder | TreeFileNode>;
	counts: Partial<Record<FileStatusCode, number>>;
}

function bump(folder: MutableFolder, status: FileStatusCode) {
	folder.counts[status] = (folder.counts[status] ?? 0) + 1;
}

function finalize(folder: MutableFolder): TreeNode[] {
	const folders: TreeFolderNode[] = [];
	const files: TreeFileNode[] = [];
	for (const child of folder.children.values()) {
		if (child.kind === "folder") {
			folders.push({ ...child, children: finalize(child) });
		} else {
			files.push(child);
		}
	}
	folders.sort((a, b) => a.name.localeCompare(b.name));
	files.sort((a, b) => a.row.path.localeCompare(b.row.path));
	return [...folders, ...files];
}

/** Builds the folder tree for a section's rows (call separately for Unstaged/Staged). */
export function buildFileTree(rows: readonly FileRow[]): TreeNode[] {
	const root: MutableFolder = { kind: "folder", name: "", path: "", children: new Map() , counts: {} };

	for (const row of rows) {
		const parts = row.path.split("/");
		let node = root;
		let pathAcc = "";
		for (let i = 0; i < parts.length - 1; i += 1) {
			const name = parts[i];
			pathAcc = pathAcc ? `${pathAcc}/${name}` : name;
			let child = node.children.get(name);
			if (!child || child.kind !== "folder") {
				child = { kind: "folder", name, path: pathAcc, children: new Map(), counts: {} };
				node.children.set(name, child);
			}
			bump(child, row.status);
			node = child;
		}
		const leafName = parts[parts.length - 1];
		node.children.set(leafName, { kind: "file", row });
	}

	return finalize(root);
}
