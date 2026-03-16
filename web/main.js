import { EditorView, basicSetup } from "codemirror";
import { StreamLanguage } from "@codemirror/language";
import { lua } from "@codemirror/legacy-modes/mode/lua";
import { oneDark } from "@codemirror/theme-one-dark";
import * as THREE from "three";
import { STLLoader } from "three/addons/loaders/STLLoader.js";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import init, { eval as wasmEval, render, rotate, pan, tessellate } from "./truescad.js";

const INITIAL_SCRIPT =
`cube = Box(1, 1, 1, 0.3)
sphere = Sphere(0.5)
result = Difference({cube, sphere}, 0.3)
result = result:scale(15, 15, 15)
build(result)
`;

async function main() {
  await init();

  // ── CodeMirror 6 editor ──────────────────────────────────────────────────

  const editor = new EditorView({
    doc: INITIAL_SCRIPT,
    extensions: [basicSetup, StreamLanguage.define(lua), oneDark],
    parent: document.getElementById("editor-pane"),
  });

  // ── Ray-march preview canvas ─────────────────────────────────────────────

  const previewCanvas = document.getElementById("preview-canvas");
  const previewCtx = previewCanvas.getContext("2d");

  function syncSize(canvas) {
    const r = canvas.getBoundingClientRect();
    canvas.width  = Math.round(r.width);
    canvas.height = Math.round(r.height);
  }

  function doRender() {
    syncSize(previewCanvas);
    const w = previewCanvas.width;
    const h = previewCanvas.height;
    if (w === 0 || h === 0) return;
    const pixels = render(w, h);
    previewCtx.putImageData(new ImageData(pixels, w, h), 0, 0);
  }

  // ── Three.js mesh canvas ─────────────────────────────────────────────────

  const meshCanvas = document.getElementById("mesh-canvas");
  let three = null; // lazily initialised

  function initThree() {
    if (three) return;
    syncSize(meshCanvas);
    const renderer = new THREE.WebGLRenderer({ canvas: meshCanvas, antialias: true });
    renderer.setSize(meshCanvas.width, meshCanvas.height, false);
    renderer.setPixelRatio(window.devicePixelRatio);

    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x1a1a1a);

    const camera = new THREE.PerspectiveCamera(45, meshCanvas.width / meshCanvas.height, 0.001, 10000);
    camera.position.set(0, 0, 5);

    scene.add(new THREE.AmbientLight(0xffffff, 0.5));
    const dir = new THREE.DirectionalLight(0xffffff, 1.0);
    dir.position.set(1, 2, 3);
    scene.add(dir);

    const controls = new OrbitControls(camera, meshCanvas);
    controls.addEventListener("change", () => renderer.render(scene, camera));

    three = { renderer, scene, camera, controls, mesh: null };
  }

  // ── Log helper ───────────────────────────────────────────────────────────

  const log = document.getElementById("log");

  function setLog(text, isError = false) {
    log.textContent = text || "(no output)";
    log.className = isError ? "error" : "";
  }

  // ── Run ──────────────────────────────────────────────────────────────────

  document.getElementById("btn-run").addEventListener("click", () => {
    const result = wasmEval(editor.state.doc.toString());
    if (result.error) {
      setLog(result.error, true);
    } else {
      setLog(result.output);
      doRender();
    }
  });

  // ── Tab toggle ───────────────────────────────────────────────────────────

  document.getElementById("tab-preview").addEventListener("click", () => {
    previewCanvas.hidden = false;
    meshCanvas.hidden = true;
    document.getElementById("tab-preview").classList.add("active");
    document.getElementById("tab-mesh").classList.remove("active");
  });

  document.getElementById("tab-mesh").addEventListener("click", () => {
    previewCanvas.hidden = true;
    meshCanvas.hidden = false;
    document.getElementById("tab-preview").classList.remove("active");
    document.getElementById("tab-mesh").classList.add("active");
    if (three) three.renderer.render(three.scene, three.camera);
  });

  // ── Mesh button ──────────────────────────────────────────────────────────

  document.getElementById("btn-mesh").addEventListener("click", () => {
    const stlBytes = tessellate();
    if (!stlBytes) { setLog("No object — run the script first.", true); return; }

    initThree();

    if (three.mesh) three.scene.remove(three.mesh);
    const geometry = new STLLoader().parse(stlBytes.buffer);
    geometry.computeVertexNormals();
    three.mesh = new THREE.Mesh(
      geometry,
      new THREE.MeshPhongMaterial({ color: 0xffcc00, side: THREE.DoubleSide })
    );
    three.scene.add(three.mesh);

    // Auto-fit camera to the mesh.
    const box    = new THREE.Box3().setFromObject(three.mesh);
    const center = box.getCenter(new THREE.Vector3());
    const size   = box.getSize(new THREE.Vector3()).length();
    three.controls.target.copy(center);
    three.camera.position.copy(center).add(new THREE.Vector3(0, 0, size * 1.5));
    three.camera.near = size / 100;
    three.camera.far  = size * 100;
    three.camera.updateProjectionMatrix();
    three.controls.update();
    three.renderer.render(three.scene, three.camera);

    document.getElementById("tab-mesh").click();
    setLog("Tessellation complete.");
  });

  // ── Export STL ───────────────────────────────────────────────────────────

  document.getElementById("btn-export").addEventListener("click", () => {
    const stlBytes = tessellate();
    if (!stlBytes) { setLog("No object — run the script first.", true); return; }

    const url = URL.createObjectURL(new Blob([stlBytes], { type: "application/octet-stream" }));
    const a = Object.assign(document.createElement("a"), { href: url, download: "model.stl" });
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  });

  // ── Drag to rotate (left) / pan (right) on preview canvas ────────────────

  let drag = null;

  previewCanvas.addEventListener("mousedown", (e) => {
    drag = { x: e.clientX, y: e.clientY, button: e.button };
    e.preventDefault();
  });

  window.addEventListener("mousemove", (e) => {
    if (!drag) return;
    const dx = (e.clientX - drag.x) * 0.01;
    const dy = (e.clientY - drag.y) * 0.01;
    drag.x = e.clientX;
    drag.y = e.clientY;
    if (drag.button === 0) rotate(dx, dy);
    else pan(dx, dy);
    doRender();
  });

  window.addEventListener("mouseup", () => { drag = null; });
  previewCanvas.addEventListener("contextmenu", (e) => e.preventDefault());
}

main().catch(console.error);
