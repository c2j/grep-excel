// Tauri API functions
import { invoke } from "@tauri-apps/api/tauri";

export async function greet(name: string): Promise<string> {
  return await invoke("greet", { name });
}
