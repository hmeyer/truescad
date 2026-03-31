import { EditorView, basicSetup } from "codemirror";
import { StreamLanguage } from "@codemirror/language";
import { lua } from "@codemirror/legacy-modes/mode/lua";
import { oneDark } from "@codemirror/theme-one-dark";
import * as THREE from "three";
import { STLLoader } from "three/addons/loaders/STLLoader.js";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import init, { run_script, get_shader_source, get_world_transform, get_object_width,
               rotate, pan, tessellate } from "./truescad.js";

const INITIAL_SCRIPT =
`-- Left: hollow cube (sphere carved out of a box)
cube   = Box(1, 1, 1, 0.3)
sphere = Sphere(0.5)
hollow = Difference({cube, sphere}, 0.3)
hollow = hollow:translate(-1.5, 0, 0)

-- Right: 3-axis cross — two plain cylinder arms + one 90°-twisted box arm along Z
local cyl      = Cylinder({l=1.5, r=0.25, s=0.02})
local arm      = Box(0.35, 0.35, 1.5, 0.02)  -- same length as cylinder
local twist_arm = Twist(arm, 6.0)              -- height=6 → 90° total twist (±45° from centre)
cross  = Union({twist_arm, cyl:rotate(tau/4,0,0), cyl:rotate(0,tau/4,0)}, 0.01)
cross  = cross:translate(1.5, 0, 0)

result = Union({hollow, cross}, 0.0)
result = result:scale(8, 8, 8)
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

  // ── WebGL2 preview canvas ────────────────────────────────────────────────

  const previewCanvas = document.getElementById("preview-canvas");
  const gl = previewCanvas.getContext("webgl2", { preserveDrawingBuffer: true });

  if (!gl) {
    document.getElementById("log").textContent =
      "WebGL2 not available in this browser.";
    document.getElementById("log").className = "error";
  }

  // Full-screen quad (two triangles covering clip space)
  const quadVerts = new Float32Array([-1, -1,  1, -1, -1, 1,  -1, 1,  1, -1,  1, 1]);
  const quadBuf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, quadBuf);
  gl.bufferData(gl.ARRAY_BUFFER, quadVerts, gl.STATIC_DRAW);

  const VERT_SRC = `#version 300 es
in vec2 aPos;
void main() { gl_Position = vec4(aPos, 0.0, 1.0); }`;

  let glProgram = null;
  let uResolution, uTransform, uCameraZ;
  let rafId = null;

  function compileProgram(fragSrc) {
    function makeShader(type, src) {
      const s = gl.createShader(type);
      gl.shaderSource(s, src);
      gl.compileShader(s);
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS))
        throw new Error("Shader compile error:\n" + gl.getShaderInfoLog(s));
      return s;
    }
    const prog = gl.createProgram();
    gl.attachShader(prog, makeShader(gl.VERTEX_SHADER, VERT_SRC));
    gl.attachShader(prog, makeShader(gl.FRAGMENT_SHADER, fragSrc));
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS))
      throw new Error("Program link error:\n" + gl.getProgramInfoLog(prog));
    return prog;
  }

  function startRenderLoop() {
    if (rafId) cancelAnimationFrame(rafId);
    function frame() {
      const w = previewCanvas.clientWidth;
      const h = previewCanvas.clientHeight;
      if (previewCanvas.width !== w || previewCanvas.height !== h) {
        previewCanvas.width  = w;
        previewCanvas.height = h;
        gl.viewport(0, 0, w, h);
      }
      gl.useProgram(glProgram);

      const aPos = gl.getAttribLocation(glProgram, "aPos");
      gl.bindBuffer(gl.ARRAY_BUFFER, quadBuf);
      gl.enableVertexAttribArray(aPos);
      gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);

      gl.uniform2f(uResolution, previewCanvas.width, previewCanvas.height);
      gl.uniformMatrix4fv(uTransform, false, get_world_transform());
      // Camera Z: place camera at 2.5× the object half-width
      const cameraZ = get_object_width() * 2.5;
      gl.uniform1f(uCameraZ, cameraZ);

      gl.drawArrays(gl.TRIANGLES, 0, 6);
      rafId = requestAnimationFrame(frame);
    }
    rafId = requestAnimationFrame(frame);
  }

  function onNewObject() {
    const src = get_shader_source();
    if (!src || !gl) return;

    if (glProgram) gl.deleteProgram(glProgram);
    try {
      // WebGL2 fragment shaders need a version header and out variable
      const fragSrc = `#version 300 es\nprecision highp float;\nout vec4 fragColor;\n` +
        src.replace("gl_FragColor", "fragColor");
      glProgram = compileProgram(fragSrc);
    } catch (e) {
      console.error(e);
      setLog("Shader compile error: " + e.message, true);
      return;
    }
    gl.useProgram(glProgram);
    uResolution = gl.getUniformLocation(glProgram, "iResolution");
    uTransform  = gl.getUniformLocation(glProgram, "iWorldTransform");
    uCameraZ    = gl.getUniformLocation(glProgram, "iCameraZ");

    startRenderLoop();
  }

  // ── Three.js mesh canvas ─────────────────────────────────────────────────

  const meshCanvas = document.getElementById("mesh-canvas");
  let three = null;

  function initThree() {
    if (three) return;
    const w = meshCanvas.clientWidth  || 400;
    const h = meshCanvas.clientHeight || 400;
    meshCanvas.width  = w;
    meshCanvas.height = h;
    const renderer = new THREE.WebGLRenderer({ canvas: meshCanvas, antialias: true });
    renderer.setSize(w, h, false);
    renderer.setPixelRatio(window.devicePixelRatio);

    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x1a1a1a);

    const camera = new THREE.PerspectiveCamera(45, w / h, 0.001, 10000);
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
    const result = run_script(editor.state.doc.toString());
    if (result.error) {
      setLog(result.error, true);
    } else {
      setLog(result.output);
      onNewObject();
    }
  });

  // ── Tab toggle ───────────────────────────────────────────────────────────

  document.getElementById("tab-preview").addEventListener("click", () => {
    previewCanvas.hidden = false;
    meshCanvas.hidden = true;
    document.getElementById("tab-preview").classList.add("active");
    document.getElementById("tab-mesh").classList.remove("active");
    if (rafId === null && glProgram) startRenderLoop();
  });

  document.getElementById("tab-mesh").addEventListener("click", () => {
    previewCanvas.hidden = true;
    meshCanvas.hidden = false;
    document.getElementById("tab-preview").classList.remove("active");
    document.getElementById("tab-mesh").classList.add("active");
    // Pause rAF loop while mesh tab is visible
    if (rafId) { cancelAnimationFrame(rafId); rafId = null; }
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
    // rAF loop picks up the new transform on the next frame automatically
  });

  window.addEventListener("mouseup", () => { drag = null; });
  previewCanvas.addEventListener("contextmenu", (e) => e.preventDefault());
}

main().catch(console.error);
