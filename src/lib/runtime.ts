export function isTauriRuntime(): boolean {
  if (typeof window === "undefined") return false;
  const w = window as unknown as Record<string, unknown>;
  // Heuristic for Tauri v2 runtime.
  return Boolean(w.__TAURI_INTERNALS__ || w.__TAURI__ || w.__TAURI_IPC__);
}

