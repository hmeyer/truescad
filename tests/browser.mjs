#!/usr/bin/env node
/**
 * Playwright smoke test.
 * Starts the esbuild dev server, opens the app in headless Chromium,
 * clicks Run, and checks for errors.
 *
 * Usage:  node tests/browser.mjs [--screenshot]
 * Prereqs: wasm-pack build --target web  (dist/truescad_bg.wasm must exist)
 */

import * as esbuild from "esbuild";
import { chromium } from "playwright";
import { cpSync, mkdirSync, existsSync, writeFileSync } from "fs";
import { resolve } from "path";

const ROOT = new URL("..", import.meta.url).pathname;
const SCREENSHOT = process.argv.includes("--screenshot");
const PORT = 8081; // use a different port from the dev server

// ── 1. Verify WASM build exists ───────────────────────────────────────────────

const wasmFile = resolve(ROOT, "pkg/truescad_bg.wasm");
if (!existsSync(wasmFile)) {
  console.error("ERROR: pkg/truescad_bg.wasm not found.");
  console.error("Run:  wasm-pack build --target web");
  process.exit(1);
}

// ── 2. Bundle and serve ───────────────────────────────────────────────────────

mkdirSync(resolve(ROOT, "dist"), { recursive: true });
cpSync(resolve(ROOT, "web/index.html"),       resolve(ROOT, "dist/index.html"));
cpSync(resolve(ROOT, "web/main.css"),         resolve(ROOT, "dist/main.css"));
cpSync(resolve(ROOT, "pkg/truescad.js"),      resolve(ROOT, "dist/truescad.js"));
cpSync(resolve(ROOT, "pkg/truescad_bg.wasm"), resolve(ROOT, "dist/truescad_bg.wasm"));
writeFileSync(resolve(ROOT, "dist/.nojekyll"), "");

const ctx = await esbuild.context({
  entryPoints: [resolve(ROOT, "web/main.js")],
  bundle: true,
  outfile: resolve(ROOT, "dist/main.js"),
  format: "esm",
  external: ["./truescad.js"],
});

await ctx.rebuild();
const { hosts } = await ctx.serve({ servedir: resolve(ROOT, "dist"), port: PORT });
const url = `http://${hosts[0]}:${PORT}`;
console.log(`Server: ${url}`);

// ── 3. Run Playwright test ────────────────────────────────────────────────────

const browser = await chromium.launch();
const page = await browser.newPage();

const consoleMessages = [];
page.on("console", msg => {
  const type = msg.type();
  const text = msg.text();
  consoleMessages.push({ type, text });
  if (type === "error") console.log(`  [console.error] ${text}`);
  if (type === "warning") console.log(`  [console.warn]  ${text}`);
});

page.on("pageerror", err => {
  console.error(`  [pageerror] ${err.message}`);
  consoleMessages.push({ type: "pageerror", text: err.message });
});

let passed = true;

try {
  // Load page
  console.log("\n── Load page ──");
  await page.goto(url, { waitUntil: "networkidle" });
  console.log("  OK");

  // Wait for WASM init. main.js calls init() async; we detect completion by
  // intercepting the first click-listener registration on btn-run via a short
  // script injected before the page script runs.
  await page.addInitScript(() => {
    window.__wasmReady = false;
    const orig = EventTarget.prototype.addEventListener;
    EventTarget.prototype.addEventListener = function(type, fn, ...rest) {
      if (type === 'click' && this?.id === 'btn-run') window.__wasmReady = true;
      return orig.call(this, type, fn, ...rest);
    };
  });

  // Reload so the init script runs before main.js
  await page.reload({ waitUntil: "networkidle" });
  await page.waitForFunction(() => window.__wasmReady === true, { timeout: 15000 });

  // Click Run and wait for log to update
  console.log("── Click Run ──");
  await page.click("#btn-run");
  await page.waitForFunction(
    () => document.getElementById("log").textContent !== "Ready. Press Run to evaluate the script.",
    { timeout: 15000 }
  );
  // Let rAF fire so the WebGL canvas renders at least one frame
  await page.waitForTimeout(500);

  // Read log output
  const logText = await page.$eval("#log", el => el.textContent);
  const logClass = await page.$eval("#log", el => el.className);
  console.log(`  log class : ${logClass || "(none)"}`);
  console.log(`  log text  : ${logText}`);

  if (logClass === "error") {
    console.error("  FAIL: script returned an error");
    passed = false;
  } else {
    console.log("  OK: script ran without error");
  }

  // Check that the WebGL canvas has rendered at least one non-background pixel.
  // Sample the center and four points at 25% offsets — robust to shapes with
  // a hole at the center (e.g. a sphere with a bore).
  console.log("── Check pixel ──");
  const pixels = await page.evaluate(() => {
    const canvas = document.getElementById("preview-canvas");
    const gl = canvas.getContext("webgl2");
    const w = canvas.width, h = canvas.height;
    const cx = w >> 1, cy = h >> 1;
    const pts = [
      [cx,            cy           ],
      [cx + (cx >> 1), cy           ],
      [cx - (cx >> 1), cy           ],
      [cx,             cy + (cy >> 1)],
      [cx,             cy - (cy >> 1)],
    ];
    return pts.map(([x, y]) => {
      const buf = new Uint8Array(4);
      gl.readPixels(x, y, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, buf);
      return Array.from(buf);
    });
  });
  const BACKGROUND = [31, 31, 31, 255]; // vec3(0.12) un-gamma-corrected
  const isBackground = p => p.every((v, i) => Math.abs(v - BACKGROUND[i]) <= 4);
  const anyLit = pixels.some(p => !isBackground(p));
  console.log(`  sampled pixels: ${pixels.map(p => `(${p.join(",")})`).join(" ")}`);
  if (!anyLit) {
    console.error("  FAIL: all sampled pixels are background color — nothing rendered");
    passed = false;
  } else {
    console.log("  OK: non-background pixel rendered");
  }

  // Check for JS errors in console
  const jsErrors = consoleMessages.filter(m => m.type === "error" || m.type === "pageerror");
  if (jsErrors.length > 0) {
    console.error(`  FAIL: ${jsErrors.length} console error(s)`);
    passed = false;
  }

  if (SCREENSHOT) {
    const shot = resolve(ROOT, "dist/screenshot.png");
    await page.screenshot({ path: shot });
    console.log(`\nScreenshot saved: ${shot}`);
  }

} finally {
  await browser.close();
  await ctx.dispose();
}

console.log(`\n${passed ? "PASS" : "FAIL"}`);
process.exit(passed ? 0 : 1);
