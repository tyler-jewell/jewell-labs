import type { KnipConfig } from "knip";

/**
 * Dead-code / unused dependency analysis.
 * Entry points match Paperclip package contract (worker, manifest, UI).
 * Public library exports from index are intentionally not required to be
 * used inside this package (consumed by hosts / tests via re-export).
 */
const config: KnipConfig = {
  entry: [
    "src/worker.ts",
    "src/manifest.ts",
    "src/ui/index.tsx",
    "scripts/*.{js,mjs}",
  ],
  project: ["src/**/*.{ts,tsx}", "scripts/**/*.{js,mjs}"],
  // Peer resolved by Paperclip host; UI imports react only.
  ignoreDependencies: ["react-dom"],
  // Suppress noise; peers/aliases are intentional.
  rules: {
    unlisted: "error",
    unresolved: "error",
  },
  vitest: {
    config: ["vitest.config.ts"],
    entry: ["tests/**/*.{ts,tsx}"],
  },
};

export default config;
