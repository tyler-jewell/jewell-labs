import * as esbuild from "esbuild";
import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const outfile = join(root, "dist/ui/index.js");
mkdirSync(dirname(outfile), { recursive: true });

await esbuild.build({
  entryPoints: [join(root, "src/ui/index.tsx")],
  bundle: true,
  outfile,
  format: "esm",
  platform: "browser",
  jsx: "automatic",
  external: ["react", "react-dom", "react/jsx-runtime"],
  logLevel: "info",
});

console.log("ui bundle →", outfile);
