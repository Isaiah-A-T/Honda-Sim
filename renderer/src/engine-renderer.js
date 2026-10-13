// 3D view of the K20Z3 for Honda Sim. Give it a container element and feed it the sim's state; it animates
// the crank, pistons, rods, cams (with VTEC and VTC), valves, throttle and turbo, flashes the plugs and combustion,
// and has view modes for the whole engine, the internals, the block, airflow, part stress and materials.
//
//   const view = createEngineRenderer(document.getElementById('engine3d'));
//   view.setState({ rpm: 6500, throttle: 1, boostKpa: 80, vtec: true, stress: { rods: 0.9 } });
//   view.setMode('airflow');
//
// gui.js adds the buttons and panels for switching modes. See README.md for every state field.

import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { RoomEnvironment } from 'three/addons/environments/RoomEnvironment.js';
import { buildEngine } from './parts.js';
import { FAMILIES, materialOf } from './materials.js';
import {
  K20, cycleAngle, throwAngle, pistonPinHeight, rodAngle, valveLift, advanceCrank, stressTint, strokeName,
} from './kinematics.js';

const SCALE = 0.01;                  // millimetres to scene units
const DEG = Math.PI / 180;
const TARGET = new THREE.Vector3(0, 1.3, 0);

// Camera directions from the engine's centre, and how far back to sit (scene units, ~1 = 100 mm).
export const VIEWS = {
  iso:   { label: 'Exhaust side', dir: [-0.45, 0.62, -0.64], dist: 11.5 },
  intake: { label: 'Intake side', dir: [0.4, 0.55, 0.73], dist: 11.5 },
  front: { label: 'Front', dir: [-1, 0.12, 0.001], dist: 10.5 },
  side:  { label: 'Side', dir: [0.001, 0.1, -1], dist: 10.5 },
  top:   { label: 'Top', dir: [0.001, 1, -0.08], dist: 11 },
  valvetrain: { label: 'Valvetrain', dir: [-0.5, 0.85, -0.3], dist: 6.5 },
};

// What each mode shows. castings: 'user' follows the xray option. ghost: systems drawn see-through.
export const MODES = {
  full:      { label: 'Full engine', show: ['casting', 'internals', 'intake', 'exhaust', 'turbo'], castings: 'user', colors: 'base' },
  internals: { label: 'Internals', show: ['internals'], colors: 'base' },
  block:     { label: 'Block', show: ['casting'], castings: 'solid', colors: 'base' },
  airflow:   { label: 'Airflow', show: ['casting', 'internals', 'intake', 'exhaust', 'turbo'], castings: 'faint',
               ghost: ['intake', 'exhaust', 'turbo'], colors: 'base', particles: true },
  stress:    { label: 'Stress', show: ['casting', 'internals', 'intake', 'exhaust', 'turbo'], castings: 'user', colors: 'stress' },
  materials: { label: 'Materials', show: ['casting', 'internals', 'intake', 'exhaust', 'turbo'], castings: 'user', colors: 'materials' },
};

export const DEFAULT_STATE = {
  rpm: 0,                // engine speed
  crankDeg: null,        // set to drive the crank angle yourself (0–720, 0 = cyl 1 TDC firing); null = follow rpm
  running: true,         // false: no combustion, crank coasts with rpm
  throttle: 0,           // 0–1 throttle plate opening
  mapKpa: 30,            // manifold pressure, scales the combustion glow
  boostKpa: 0,           // gauge boost, spins the turbo
  airGs: 0,              // airflow through the engine, g/s: airflow particle speed
  vtec: false,           // high cam engaged
  vtcDeg: 0,             // intake cam advance from VTC, crank degrees (0–50)
  ignDeg: 15,            // spark advance BTDC, when the plugs fire
  knock: false,          // knock right now: red combustion and a shake
  fuelCut: false,        // rev limiter, decel or overboost cut: no combustion
  lambda: 1,             // mixture: lean burns blue-white, rich burns orange
  turbo: false,          // show the turbo and charge pipe
  stress: {},            // { block, head, crank, rods, pistons, valves, cams, turbo, exhaust, intake, flywheel, clutch }
                         // 0–1, above 1 = past the limit. rods, pistons and valves also take a 4-number array (cyl 1–4).
  materials: {},         // { pistons: 'forged-2618', rods: '4340-h', ... } ids from materials.js
  partMaterials: null,   // or the host's own materials: { pistons: { label, family }, ... } (family from materials.js FAMILIES)
};

export function createEngineRenderer(container, options = {}) {
  const opts = { slowMo: 0.02, xray: true, background: null, autoRotate: false, mode: 'full', onPick: null, ...options };
  const state = structuredClone(DEFAULT_STATE);

  const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: opts.background == null });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  if (opts.background != null) renderer.setClearColor(opts.background);
  renderer.domElement.style.display = 'block';
  renderer.domElement.style.touchAction = 'none';
  if (getComputedStyle(container).position === 'static') container.style.position = 'relative';
  container.appendChild(renderer.domElement);

  const scene = new THREE.Scene();
  const pmrem = new THREE.PMREMGenerator(renderer);
  scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
  scene.add(new THREE.HemisphereLight(0xdfe8ff, 0x30281e, 0.6));
  const sun = new THREE.DirectionalLight(0xffffff, 1.6);
  sun.position.set(4, 8, 5);
  scene.add(sun);

  const camera = new THREE.PerspectiveCamera(38, 1, 0.05, 100);
  const controls = new OrbitControls(camera, renderer.domElement);
  controls.target.copy(TARGET);
  controls.enableDamping = true;
  controls.minDistance = 1.5;
  controls.maxDistance = 30;
  controls.autoRotateSpeed = 0.6;
  const viewPos = name => {
    const v = VIEWS[name];
    const fit = Math.max(1, 1.3 / Math.max(camera.aspect, 0.3));   // back off on tall, narrow screens
    return new THREE.Vector3(...v.dir).normalize().multiplyScalar(v.dist * fit).add(TARGET);
  };

  const eng = buildEngine();
  const engineRoot = new THREE.Group();
  engineRoot.scale.setScalar(SCALE);
  engineRoot.add(eng.root);
  scene.add(engineRoot);
  // Remember each material's own transparency so modes can change it and put it back.
  eng.root.traverse(o => {
    if (o.isMesh && !o.material.userData.base) {
      const m = o.material;
      m.userData.base = { transparent: m.transparent, opacity: m.opacity, depthWrite: m.depthWrite };
    }
  });

  const floor = new THREE.Mesh(new THREE.CircleGeometry(4.5, 64).rotateX(-Math.PI / 2),
    new THREE.MeshStandardMaterial({ color: 0x808890, transparent: true, opacity: 0.12, depthWrite: false }));
  floor.position.y = -1.9;
  scene.add(floor);

  // ---------- Airflow particles ----------
  const flows = eng.reg.flow.map(f => {
    const length = f.curve.getLength();
    const n = Math.max(6, Math.round(length / 16));
    const pos = new Float32Array(n * 3);
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.BufferAttribute(pos, 3));
    const mat = new THREE.PointsMaterial({ size: 0.11, transparent: true, opacity: 0.95, depthWrite: false,
      blending: THREE.AdditiveBlending, color: f.kind === 'intake' ? 0x6ec8ff : 0xff7a1a });
    const points = new THREE.Points(geo, mat);
    points.raycast = () => {};
    points.visible = false;
    points.renderOrder = 5;
    points.frustumCulled = false;
    eng.root.add(points);
    const u = Array.from({ length: n }, (_, i) => (i + Math.random() * 0.6) / n);
    return { ...f, length, points, pos, u, p: new THREE.Vector3() };
  });

  // ---------- Hover tooltip and click-to-pick ----------
  const tip = document.createElement('div');
  Object.assign(tip.style, {
    position: 'absolute', pointerEvents: 'none', padding: '4px 8px', borderRadius: '6px', font: '12px/1.3 system-ui, sans-serif',
    background: 'rgba(20,22,26,.88)', color: '#f2f4f7', whiteSpace: 'nowrap', display: 'none', zIndex: 3,
  });
  container.appendChild(tip);
  const ray = new THREE.Raycaster();
  const mouse = new THREE.Vector2();
  let hoverEvt = null, downAt = null;
  const onMove = e => { hoverEvt = e; };
  const onLeave = () => { hoverEvt = null; tip.style.display = 'none'; };
  const onDown = e => { downAt = [e.clientX, e.clientY]; };
  const onUp = e => {
    if (!downAt || Math.hypot(e.clientX - downAt[0], e.clientY - downAt[1]) > 5) return;   // a drag, not a click
    const hit = pick(e);
    const part = hit?.object.userData.part ?? null;
    selectedKey = part?.key ?? null;
    colorsDirty = true;
    opts.onPick?.(part);
  };
  renderer.domElement.addEventListener('pointermove', onMove);
  renderer.domElement.addEventListener('pointerleave', onLeave);
  renderer.domElement.addEventListener('pointerdown', onDown);
  renderer.domElement.addEventListener('pointerup', onUp);

  const resize = () => {
    const w = container.clientWidth || 640, h = container.clientHeight || 400;
    renderer.setSize(w, h, false);
    renderer.domElement.style.width = '100%';
    renderer.domElement.style.height = '100%';
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
  };
  const ro = new ResizeObserver(resize);
  ro.observe(container);
  resize();
  camera.position.copy(viewPos(opts.view ?? 'iso'));

  let crank = 0, turboAngle = 0, shake = 0, raf = 0, last = performance.now(), paused = false;
  let viewAnim = null, selectedKey = null, colorsDirty = true, modeDirty = true, lastTurbo = null;
  const tintColor = new THREE.Color();
  const listeners = new Set();

  function stressOf(key, index) {
    const s = state.stress?.[key];
    if (Array.isArray(s)) return s[index ?? 0] ?? 0;
    return typeof s === 'number' ? s : 0;
  }

  function partMaterial(key) {
    if (state.partMaterials) return state.partMaterials[key] ?? null;
    return materialOf(key, state.materials);
  }

  function applyMode() {
    const mode = MODES[opts.mode] ?? MODES.full;
    const castings = mode.castings === 'user' ? (opts.xray === true ? 'see' : opts.xray) : mode.castings;
    for (const child of eng.root.children) {
      const sys = child.userData.sys;
      if (!sys) continue;
      let visible = mode.show.includes(sys);
      if (sys === 'turbo') visible &&= !!state.turbo;
      if (child === eng.naIntake) visible &&= !state.turbo;
      if (child.userData.part?.label === 'Turbo up-pipe') visible &&= !!state.turbo;
      if (sys === 'casting') visible &&= castings !== 'hidden';
      child.visible = visible;
      const ghost = mode.ghost?.includes(sys);
      child.traverse(o => {
        if (!o.isMesh || !o.material.userData.base) return;
        const m = o.material, b = m.userData.base;
        if (sys === 'casting') {
          m.opacity = castings === 'solid' ? 0.92 : castings === 'faint' ? b.opacity * 0.45 : b.opacity;
          m.depthWrite = castings === 'solid';
        } else if (ghost) {
          m.transparent = true; m.opacity = 0.22; m.depthWrite = false;
        } else {
          m.transparent = b.transparent; m.opacity = b.opacity; m.depthWrite = b.depthWrite;
        }
        m.needsUpdate = true;
      });
    }
    for (const f of flows) f.points.visible = !!mode.particles && (!f.only || f.only === (state.turbo ? 'turbo' : 'na'));
    lastTurbo = !!state.turbo;
    modeDirty = false;
    colorsDirty = true;
  }

  function applyColors(now) {
    const colors = (MODES[opts.mode] ?? MODES.full).colors;
    const pulse = 0.18 + 0.12 * Math.sin(now / 160);
    for (const t of eng.reg.tintables) {
      const m = t.mesh.material;
      if (colors === 'stress') {
        const { rgb, amount } = stressTint(stressOf(t.key, t.index));
        tintColor.setRGB(...rgb);
        m.color.copy(t.base).lerp(tintColor, amount);
        m.emissive.copy(t.baseEmissive).lerp(tintColor, amount * amount * (t.key === 'exhaust' ? 0.8 : 0.3));
      } else if (colors === 'materials') {
        const mat = partMaterial(t.key);
        if (mat) m.color.setHex(FAMILIES[mat.family].color);
        else m.color.copy(t.base).multiplyScalar(0.55);
        if (t.key === selectedKey) m.emissive.setRGB(pulse, pulse, pulse);
        else m.emissive.setRGB(0, 0, 0);
      } else {
        m.color.copy(t.base);
        m.emissive.copy(t.baseEmissive);
      }
    }
    colorsDirty = colors === 'stress' || (colors === 'materials' && selectedKey != null);
  }

  function frame(now) {
    raf = requestAnimationFrame(frame);
    const dt = Math.min((now - last) / 1000, 0.1);
    last = now;
    if (modeDirty || lastTurbo !== !!state.turbo) applyMode();
    if (paused) { controls.update(); renderer.render(scene, camera); return; }

    const s = state;
    crank = typeof s.crankDeg === 'number' ? ((s.crankDeg % 720) + 720) % 720 : advanceCrank(crank, s.rpm, dt, opts.slowMo);
    const vtc = THREE.MathUtils.clamp(s.vtcDeg ?? 0, 0, K20.vtcMaxDeg);
    eng.crank.rotation.x = crank * DEG;
    eng.cams.intake.group.rotation.x = (crank + vtc) / 2 * DEG;
    eng.cams.exhaust.group.rotation.x = crank / 2 * DEG;

    const burning = s.running && !s.fuelCut && s.rpm > 300;
    const load = THREE.MathUtils.clamp((s.mapKpa ?? 30) / 100, 0.15, 2.2);
    const lean = THREE.MathUtils.clamp(((s.lambda ?? 1) - 0.85) / 0.35, 0, 1);   // 0 rich, 1 lean
    const lifts = {};
    for (let c = 1; c <= 4; c++) {
      const parts = eng.reg.cyl[c];
      const theta = crank + throwAngle(c);
      const pinY = pistonPinHeight(theta);
      parts.piston.position.y = pinY;
      parts.rod.position.y = pinY;
      parts.rod.rotation.x = -rodAngle(theta);
      lifts[c] = {};
      for (const side of ['intake', 'exhaust']) {
        const lift = valveLift(c, crank, side, s.vtec, vtc);
        lifts[c][side] = lift / K20.cams.high[side].lift;
        for (const v of parts.valves[side]) v.group.position.copy(v.seat).addScaledVector(v.axis, -lift);
      }

      const cyc = cycleAngle(c, crank);
      const sinceSpark = ((cyc + (s.ignDeg ?? 15)) % 720 + 720) % 720;     // degrees after the plug fired
      parts.plugGlow.material.opacity = burning && sinceSpark < 30 ? 1 - sinceSpark / 30 : 0;
      const burn = burning && sinceSpark < 110 ? Math.sin(Math.PI * Math.min(sinceSpark / 110, 1)) : 0;
      const crownY = pinY + 30, roof = K20.deckHeight + 10;
      parts.flame.position.y = (crownY + roof) / 2;
      parts.flame.scale.y = Math.max(roof - crownY, 1);
      parts.flame.material.opacity = burn * (0.35 + 0.35 * Math.min(load, 1.5));
      if (s.knock) parts.flame.material.color.setRGB(1, 0.15, 0.1);
      else parts.flame.material.color.setRGB(1, 0.45 + 0.4 * lean, 0.12 + 0.75 * lean);
    }

    for (const lobe of [...eng.cams.intake.vtecLobes, ...eng.cams.exhaust.vtecLobes]) {
      lobe.material.emissive.setHex(s.vtec ? 0x1f9d55 : 0x000000);
      lobe.material.emissiveIntensity = s.vtec ? 0.9 : 0;
    }

    eng.butterflyPivot.rotation.y = 0.12 + THREE.MathUtils.clamp(s.throttle, 0, 1) * 1.38;

    if (s.turbo) {
      const spin = (Math.max(s.boostKpa, 0) / 100 * 9 + s.rpm / 8000 * 2) * (s.running ? 1 : 0.2);
      turboAngle += spin * dt * Math.PI * 2;
      eng.turbineWheel.rotation.x = turboAngle;
      eng.compWheel.rotation.x = turboAngle;
    }

    if ((MODES[opts.mode] ?? MODES.full).particles) updateFlow(dt, lifts);
    if (colorsDirty) applyColors(now);

    shake = s.knock && burning ? Math.min(shake + dt * 8, 1) : Math.max(shake - dt * 4, 0);
    engineRoot.position.set((Math.random() - 0.5) * 0.02 * shake, (Math.random() - 0.5) * 0.02 * shake, 0);

    if (viewAnim) {
      viewAnim.t = Math.min(viewAnim.t + dt / 0.6, 1);
      const k = 1 - (1 - viewAnim.t) ** 3;
      camera.position.lerpVectors(viewAnim.from, viewAnim.to, k);
      if (viewAnim.t >= 1) viewAnim = null;
    }
    controls.autoRotate = !!opts.autoRotate;
    controls.update();

    if (hoverEvt) updateTooltip(hoverEvt);
    renderer.render(scene, camera);
    for (const fn of listeners) fn(api);
  }

  // Particles move along each path at a speed set by airflow; runner paths only move while their valve is open,
  // so you see each cylinder gulp and exhale in firing order. Exhaust glows hotter with exhaust stress.
  function updateFlow(dt, lifts) {
    const s = state;
    const base = s.running && s.rpm > 300 ? 220 + 900 * THREE.MathUtils.clamp((s.airGs ?? 0) / 150, 0, 2.2) : 0;
    const boost = THREE.MathUtils.clamp((s.boostKpa ?? 0) / 150, 0, 1);
    const heat = THREE.MathUtils.clamp(stressOf('exhaust'), 0, 1.2);
    for (const f of flows) {
      if (!f.points.visible) continue;
      const gate = f.cyl ? Math.min(lifts[f.cyl][f.kind] * 1.6, 1) : 1;
      const cut = f.kind === 'exhaust' && s.fuelCut ? 0.5 : 1;
      const du = base * gate * cut * dt / f.length;
      for (let i = 0; i < f.u.length; i++) {
        f.u[i] = (f.u[i] + du) % 1;
        f.curve.getPointAt(f.u[i], f.p);
        f.pos[i * 3] = f.p.x; f.pos[i * 3 + 1] = f.p.y; f.pos[i * 3 + 2] = f.p.z;
      }
      f.points.geometry.attributes.position.needsUpdate = true;
      if (f.kind === 'intake') f.points.material.color.setRGB(0.43 + 0.5 * boost, 0.78 + 0.2 * boost, 1);
      else f.points.material.color.setRGB(1, 0.42 + 0.45 * heat, 0.1 + 0.35 * Math.max(heat - 0.6, 0));
      f.points.material.opacity = f.cyl ? 0.35 + 0.6 * gate : 0.9;
    }
  }

  function pick(e) {
    const rect = renderer.domElement.getBoundingClientRect();
    mouse.set((e.clientX - rect.left) / rect.width * 2 - 1, -((e.clientY - rect.top) / rect.height) * 2 + 1);
    ray.setFromCamera(mouse, camera);
    const hits = ray.intersectObjects(eng.reg.pickables.filter(visibleDeep), false);
    return hits.find(h => !(h.object.material.transparent && h.object.material.opacity < 0.5)) || hits[0];
  }

  function updateTooltip(e) {
    const hit = pick(e);
    if (!hit) { tip.style.display = 'none'; return; }
    const p = hit.object.userData.part;
    let text = p.label;
    if (opts.mode === 'materials') {
      const m = p.key && partMaterial(p.key);
      if (m) text += ` · ${m.label}${state.partMaterials ? '' : ' · click to change'}`;
    } else if (p.key) {
      const sv = stressOf(p.key, p.index);
      if (sv > 0) text += ` · ${Math.round(sv * 100)}% of limit`;
    }
    const cyl = p.index != null ? p.index + 1 : null;
    if (cyl && ['pistons', 'rods', 'valves'].includes(p.key)) text += ` · ${strokeName(cycleAngle(cyl, crank))} stroke`;
    tip.textContent = text;
    tip.style.display = 'block';
    const rect = renderer.domElement.getBoundingClientRect();
    tip.style.left = `${Math.max(4, Math.min(e.clientX - rect.left + 14, rect.width - tip.offsetWidth - 4))}px`;
    tip.style.top = `${Math.max(e.clientY - rect.top - 30, 4)}px`;
  }

  function visibleDeep(o) {
    for (let x = o; x; x = x.parent) if (!x.visible) return false;
    return true;
  }

  raf = requestAnimationFrame(frame);

  const api = {
    // Merge in new sim state; call as often as you like.
    setState(next) {
      for (const [k, v] of Object.entries(next)) {
        if (k === 'stress' || k === 'materials') { state[k] = { ...v }; colorsDirty = true; }
        else if (k === 'partMaterials') { state[k] = v ? { ...v } : null; colorsDirty = true; }
        else if (k in state) state[k] = v;
      }
    },
    getState: () => ({ ...state, crankDeg: crank }),
    // slowMo: animation speed as a fraction of real crank speed (1 = real time, which strobes above ~1000 rpm).
    // xray: true for see-through castings, 'solid' or 'hidden'. autoRotate: slowly orbit. onPick(part): a part was clicked.
    setOptions(o) { Object.assign(opts, o); modeDirty = true; },
    getOptions: () => ({ ...opts }),
    setMode(name) {
      if (!MODES[name]) return;
      opts.mode = name;
      if (name !== 'materials') selectedKey = null;
      modeDirty = true;
    },
    get mode() { return opts.mode; },
    select(key) { selectedKey = key; colorsDirty = true; },
    setView(name) {
      if (!VIEWS[name]) return;
      controls.target.copy(TARGET);
      viewAnim = { from: camera.position.clone(), to: viewPos(name), t: 0 };
    },
    // Called after every rendered frame; returns a function that stops it.
    onFrame(fn) { listeners.add(fn); return () => listeners.delete(fn); },
    pause(p = true) { paused = p; },
    get crankDeg() { return crank; },
    get canvas() { return renderer.domElement; },
    get container() { return container; },
    dispose() {
      cancelAnimationFrame(raf);
      ro.disconnect();
      controls.dispose();
      for (const [ev, fn] of [['pointermove', onMove], ['pointerleave', onLeave], ['pointerdown', onDown], ['pointerup', onUp]])
        renderer.domElement.removeEventListener(ev, fn);
      scene.traverse(o => { o.geometry?.dispose(); o.material?.dispose?.(); });
      pmrem.dispose();
      renderer.dispose();
      renderer.domElement.remove();
      tip.remove();
    },
  };
  return api;
}
