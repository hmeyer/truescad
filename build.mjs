#!/usr/bin/env node
// Bundles the web frontend into dist/.
// Run after `wasm-pack build --target web`.

import * as esbuild from "esbuild";
import { cpSync, mkdirSync, writeFileSync } from "fs";

const release = process.argv.includes("--release");

mkdirSync("dist", { recursive: true });

await esbuild.build({
  entryPoints: ["web/main.js"],
  bundle: true,
  outfile: "dist/main.js",
  format: "esm",
  // truescad.js is the wasm-pack glue — keep it as a separate file so that
  // its import.meta.url correctly locates truescad_bg.wasm at runtime.
  external: ["./truescad.js"],
  minify: release,
});

cpSync("web/index.html", "dist/index.html");
cpSync("web/main.css",   "dist/main.css");
cpSync("pkg/truescad.js",       "dist/truescad.js");
cpSync("pkg/truescad_bg.wasm",  "dist/truescad_bg.wasm");

// Prevents GitHub Pages from running Jekyll on the output.
writeFileSync("dist/.nojekyll", "");

console.log(`Built → dist/  (${release ? "release" : "dev"})`);
