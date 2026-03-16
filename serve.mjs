#!/usr/bin/env node
// Dev server: rebuilds JS/CSS on change and serves dist/ on localhost:8080.
// Run `wasm-pack build --target web` first (or when Rust code changes).

import * as esbuild from "esbuild";
import { cpSync, mkdirSync, writeFileSync } from "fs";

mkdirSync("dist", { recursive: true });

// Copy static assets that don't go through esbuild.
function copyStatics() {
  cpSync("web/index.html",       "dist/index.html");
  cpSync("web/main.css",         "dist/main.css");
  cpSync("pkg/truescad.js",      "dist/truescad.js");
  cpSync("pkg/truescad_bg.wasm", "dist/truescad_bg.wasm");
  writeFileSync("dist/.nojekyll", "");
}

copyStatics();

const ctx = await esbuild.context({
  entryPoints: ["web/main.js"],
  bundle: true,
  outfile: "dist/main.js",
  format: "esm",
  external: ["./truescad.js"],
});

await ctx.watch();

const { host, port } = await ctx.serve({ servedir: "dist", port: 8080 });
console.log(`Dev server running at http://${host}:${port}`);
console.log("JS/CSS changes rebuild automatically.");
console.log("Re-run `wasm-pack build --target web` after Rust changes, then refresh.");
