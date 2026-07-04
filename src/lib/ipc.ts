import { invoke } from "@tauri-apps/api/core";

/** The ONLY place invoke() is called — ARCHITECTURE.md §1. */

export async function greet(name: string): Promise<string> {
  return invoke("greet", { name });
}
