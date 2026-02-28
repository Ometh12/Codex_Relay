import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import fs from "node:fs";
import path from "node:path";
import type { Plugin } from "vite";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

function cleanupDistArtifacts(): Plugin {
  return {
    name: "codexrelay-cleanup-dist-artifacts",
    apply: "build",
    closeBundle() {
      // Keep release bundles tidy across platforms. (macOS often drops `.DS_Store`,
      // and Windows can add `Thumbs.db`.)
      const distDir = path.resolve(process.cwd(), "dist");
      if (!fs.existsSync(distDir)) return;

      const junk = new Set([".DS_Store", "Thumbs.db"]);
      const stack: string[] = [distDir];
      while (stack.length) {
        const dir = stack.pop();
        if (!dir) continue;
        let entries: fs.Dirent[];
        try {
          entries = fs.readdirSync(dir, { withFileTypes: true });
        } catch {
          continue;
        }
        for (const ent of entries) {
          const p = path.join(dir, ent.name);
          if (ent.isDirectory()) {
            stack.push(p);
            continue;
          }
          if (ent.isFile() && junk.has(ent.name)) {
            try {
              fs.unlinkSync(p);
            } catch {
              // ignore best-effort cleanup
            }
          }
        }
      }
    },
  };
}

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  const isTauri = mode === "tauri";
  const tauriPort = 5273;
  const webPort = 5273;

  return {
    plugins: [react(), cleanupDistArtifacts()],
    // Avoid absolute asset URLs in desktop builds so opening `dist/index.html` via file://
    // works and bundlers/protocols are less sensitive to path base assumptions.
    base: isTauri ? "./" : "/",

    // 1) prevent Vite from obscuring rust errors
    clearScreen: false,

    // 2) Use a fixed port so `pnpm tauri dev`, `pnpm dev`, and headless previews are predictable.
    server: {
      port: isTauri ? tauriPort : webPort,
      strictPort: isTauri,
      host: isTauri ? host || false : false,
      hmr:
        isTauri && host
          ? {
              protocol: "ws",
              host,
              port: tauriPort + 1,
            }
          : undefined,
      watch: {
        // 3) tell Vite to ignore watching `src-tauri`
        ignored: ["**/src-tauri/**"],
      },
    },
  };
});
