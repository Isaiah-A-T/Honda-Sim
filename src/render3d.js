// 3D view of the engine: block, pistons, rods, crank, cams, valves and (on turbo builds) the turbo,
// moving with the sim and coloured by the stress the internals model reports.
//
// Geometry is simplified but to scale where the dimensions are known: 86 mm bore and stroke,
// 139 mm rods, 93 mm bore spacing, firing order 1-3-4-2. Units below are millimetres.

import * as THREE from 'three';
import { OrbitControls } from '../vendor/OrbitControls.js';
import { GEOMETRY } from './internals.js';

const BORE = GEOMETRY.boreM * 1000, R = GEOMETRY.strokeM * 500, ROD = GEOMETRY.rodM * 1000;
const PITCH = 93;                                 // bore spacing
const FIRING_OFFSET = [0, 540, 180, 360];         // cycle angle offsets for cylinders 1–4 (fires 1-3-4-2)
const THROW = [0, 180, 180, 0];                   // crank pin angles

const GREEN = new THREE.Color(0x2e9d5b), YELLOW = new THREE.Color(0xe0a400), RED = new THREE.Color(0xd32f2f);
export function stressColor(load, out = new THREE.Color()) {
  if (load <= 0.8) return out.copy(GREEN).lerp(YELLOW, Math.max(0, load) / 0.8);
  return out.copy(YELLOW).lerp(RED, Math.min(1, (load - 0.8) / 0.4));
}

const mat = (color, extra = {}) => new THREE.MeshStandardMaterial({ color, metalness: 0.6, roughness: 0.45, ...extra });
const STEEL = 0x9aa3ad, ALU = 0xc9ccd1;

// Piston top position (mm above the crank axis) for a crank angle in degrees.
export function pistonY(angleDeg) {
  const a = angleDeg * Math.PI / 180;
  return R * Math.cos(a) + Math.sqrt(ROD * ROD - (R * Math.sin(a)) ** 2);
}

// Valve lift 0–1 for a point in the 720° cycle (0 = firing TDC). Intake opens around TDC on the
// exhaust stroke; exhaust opens before bottom dead centre on the power stroke.
export function valveLift(cycleDeg, intake, highCam) {
  const c = ((cycleDeg % 720) + 720) % 720;
  const dur = highCam ? 260 : 230;
  const open = intake ? 700 : 500 - (highCam ? 15 : 0);
  let x = (c - open + 720) % 720;
  if (x > dur) return 0;
  return Math.sin(Math.PI * x / dur) ** 1.5;
}

export function createRenderer(container) {
  const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
  renderer.setPixelRatio(Math.min(2, window.devicePixelRatio || 1));
  container.append(renderer.domElement);
  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(40, 1, 1, 5000);
  camera.position.set(-430, 360, 520);
  const controls = new OrbitControls(camera, renderer.domElement);
  controls.target.set(0, 130, 0);
  controls.enableDamping = true;

  scene.add(new THREE.HemisphereLight(0xffffff, 0x445566, 1.4));
  const sun = new THREE.DirectionalLight(0xffffff, 1.6);
  sun.position.set(300, 600, 400);
  scene.add(sun);

  const parts = { pistons: [], rods: [], pins: [], valves: [], glows: [], bearings: [] };
  const deckY = R + ROD + 32;                      // piston pin height at TDC + compression height

  // Block: see-through so the moving parts show.
  const blockMat = new THREE.MeshStandardMaterial({ color: 0x5c6670, transparent: true, opacity: 0.12, depthWrite: false });
  const block = new THREE.Mesh(new THREE.BoxGeometry(PITCH * 4 + 40, deckY + 60, 190), blockMat);
  block.position.set(0, (deckY + 60) / 2 - 60, 0);
  scene.add(block);
  const blockEdges = new THREE.LineSegments(new THREE.EdgesGeometry(block.geometry), new THREE.LineBasicMaterial({ color: 0x8892a0 }));
  blockEdges.position.copy(block.position);
  scene.add(blockEdges);

  // Head and head gasket.
  const head = new THREE.Mesh(new THREE.BoxGeometry(PITCH * 4 + 40, 110, 210), new THREE.MeshStandardMaterial({ color: 0x8a939c, transparent: true, opacity: 0.1, depthWrite: false }));
  head.position.set(0, deckY + 61, 0);
  scene.add(head);
  const gasketMat = mat(0x666666, { metalness: 0.3 });
  const gasket = new THREE.Mesh(new THREE.BoxGeometry(PITCH * 4 + 40, 3, 200), gasketMat);
  gasket.position.set(0, deckY + 4, 0);
  scene.add(gasket);
  parts.gasket = gasket;

  // Crankshaft: main journals, throws (webs + counterweights) and rod pins.
  const crank = new THREE.Group();
  const crankMat = mat(STEEL);
  for (let i = 0; i <= 4; i++) {
    const main = new THREE.Mesh(new THREE.CylinderGeometry(25, 25, 22, 32), crankMat);
    main.rotation.z = Math.PI / 2;
    main.position.x = (i - 2) * PITCH;
    crank.add(main);
  }
  for (let i = 0; i < 4; i++) {
    const x = (i - 1.5) * PITCH, a = THROW[i] * Math.PI / 180;
    const throwGroup = new THREE.Group();
    throwGroup.rotation.x = a;
    throwGroup.position.x = x;
    const pin = new THREE.Mesh(new THREE.CylinderGeometry(24, 24, 22, 32), crankMat);
    pin.rotation.z = Math.PI / 2;
    pin.position.y = R;
    throwGroup.add(pin);
    parts.pins.push(pin);
    for (const side of [-1, 1]) {
      const web = new THREE.Mesh(new THREE.BoxGeometry(14, R + 50, 60), crankMat);
      web.position.set(side * 18, R / 2 - 12, 0);
      throwGroup.add(web);
      const cw = new THREE.Mesh(new THREE.CylinderGeometry(62, 62, 14, 32, 1, false, Math.PI * 0.7, Math.PI * 0.6), crankMat);
      cw.rotation.z = Math.PI / 2;
      cw.position.set(side * 18, -8, 0);
      throwGroup.add(cw);
    }
    crank.add(throwGroup);
  }
  scene.add(crank);
  parts.crank = crank;

  // Pistons and rods.
  for (let i = 0; i < 4; i++) {
    const x = (i - 1.5) * PITCH;
    const piston = new THREE.Group();
    const pMat = mat(ALU, { metalness: 0.4 });
    const crown = new THREE.Mesh(new THREE.CylinderGeometry(BORE / 2 - 0.5, BORE / 2 - 0.5, 30, 40), pMat);
    crown.position.y = 17;
    piston.add(crown);
    const skirt = new THREE.Mesh(new THREE.CylinderGeometry(BORE / 2 - 1.5, BORE / 2 - 1.5, 22, 40, 1, true), pMat);
    skirt.position.y = -8;
    piston.add(skirt);
    for (let k = 0; k < 3; k++) {
      const ring = new THREE.Mesh(new THREE.TorusGeometry(BORE / 2 - 0.6, 0.9, 6, 40), mat(0x333333));
      ring.rotation.x = Math.PI / 2;
      ring.position.y = 27 - k * 5;
      piston.add(ring);
    }
    const wrist = new THREE.Mesh(new THREE.CylinderGeometry(11, 11, 60, 16), mat(STEEL));
    wrist.rotation.z = Math.PI / 2;
    piston.add(wrist);
    piston.position.x = x;
    piston.userData.mat = pMat;
    scene.add(piston);
    parts.pistons.push(piston);

    const rMat = mat(STEEL);
    const rod = new THREE.Group();
    const beam = new THREE.Mesh(new THREE.BoxGeometry(14, ROD - 40, 20), rMat);
    beam.position.y = ROD / 2;
    rod.add(beam);
    const bigEnd = new THREE.Mesh(new THREE.TorusGeometry(31, 8, 10, 28), rMat);
    bigEnd.rotation.y = Math.PI / 2;
    rod.add(bigEnd);
    const smallEnd = new THREE.Mesh(new THREE.TorusGeometry(16, 5, 8, 20), rMat);
    smallEnd.rotation.y = Math.PI / 2;
    smallEnd.position.y = ROD;
    rod.add(smallEnd);
    const shell = new THREE.Mesh(new THREE.TorusGeometry(24.5, 2, 6, 28), mat(0xb08d57));
    shell.rotation.y = Math.PI / 2;
    rod.add(shell);
    rod.position.x = x;
    rod.userData.mat = rMat;
    rod.userData.shell = shell;
    scene.add(rod);
    parts.rods.push(rod);
    parts.bearings.push(shell);

    // Combustion glow above the piston on the power stroke.
    const glow = new THREE.Mesh(new THREE.CylinderGeometry(BORE / 2 - 2, BORE / 2 - 2, 6, 32), new THREE.MeshBasicMaterial({ color: 0xff7a1a, transparent: true, opacity: 0 }));
    glow.position.set(x, deckY, 0);
    scene.add(glow);
    parts.glows.push(glow);
  }

  // Cams and valves: intake side −z, exhaust side +z.
  const camY = deckY + 95;
  const cams = [];
  for (const side of [-1, 1]) {
    const cam = new THREE.Group();
    const shaft = new THREE.Mesh(new THREE.CylinderGeometry(13, 13, PITCH * 4 + 20, 20), mat(STEEL));
    shaft.rotation.z = Math.PI / 2;
    cam.add(shaft);
    for (let i = 0; i < 4; i++) {
      const lobeAngle = -(FIRING_OFFSET[i] / 2 + (side < 0 ? 0 : 90)) * Math.PI / 180;
      for (const dx of [-15, 15]) {
        const lobe = new THREE.Mesh(new THREE.CylinderGeometry(19, 19, 10, 3), mat(STEEL));
        lobe.rotation.z = Math.PI / 2;
        const holder = new THREE.Group();
        holder.rotation.x = lobeAngle;
        holder.position.x = (i - 1.5) * PITCH + dx;
        lobe.scale.set(1, 1, 1.3);
        holder.add(lobe);
        cam.add(holder);
      }
    }
    cam.position.set(0, camY, side * 48);
    scene.add(cam);
    cams.push(cam);
  }
  for (let i = 0; i < 4; i++) {
    for (const intake of [true, false]) {
      for (const dx of [-15, 15]) {
        const v = new THREE.Group();
        const vMat = mat(intake ? 0xbfc6cf : 0xa58a6a);
        const stem = new THREE.Mesh(new THREE.CylinderGeometry(2.8, 2.8, 95, 8), vMat);
        stem.position.y = 47;
        v.add(stem);
        const headDisc = new THREE.Mesh(new THREE.CylinderGeometry(intake ? 17 : 14.5, 9, 4, 24), vMat);
        v.add(headDisc);
        const spring = new THREE.Mesh(new THREE.CylinderGeometry(12, 12, 38, 12, 1, true), new THREE.MeshStandardMaterial({ color: 0x3f6fb5, wireframe: true }));
        spring.position.y = 70;
        v.add(spring);
        const side = intake ? -1 : 1;
        v.position.set((i - 1.5) * PITCH + dx, deckY + 6, side * 22);
        v.rotation.x = side * -0.22;
        v.userData = { cyl: i, intake, base: v.position.clone(), side, mat: vMat, spring };
        scene.add(v);
        parts.valves.push(v);
      }
    }
  }

  // Turbo: housings, and a compressor wheel that spins with boost.
  const turbo = new THREE.Group();
  const comp = new THREE.Mesh(new THREE.TorusGeometry(48, 26, 16, 40), mat(0xb7bcc2, { metalness: 0.7 }));
  turbo.add(comp);
  const hot = new THREE.Mesh(new THREE.TorusGeometry(44, 24, 16, 40), mat(0x5a4a40, { metalness: 0.5, roughness: 0.8 }));
  hot.position.z = 90;
  turbo.add(hot);
  const core = new THREE.Mesh(new THREE.CylinderGeometry(22, 22, 70, 20), mat(0x777f88));
  core.rotation.x = Math.PI / 2;
  core.position.z = 45;
  turbo.add(core);
  const wheel = new THREE.Group();
  for (let b = 0; b < 10; b++) {
    const blade = new THREE.Mesh(new THREE.BoxGeometry(2, 36, 18), mat(0xdfe3e8));
    blade.position.y = 18;
    const holder = new THREE.Group();
    holder.rotation.z = b / 10 * Math.PI * 2;
    blade.rotation.y = 0.5;
    holder.add(blade);
    wheel.add(holder);
  }
  wheel.position.z = -6;
  turbo.add(wheel);
  turbo.position.set(PITCH * 1.2, deckY - 70, 260);   // exhaust side, behind the block
  turbo.rotation.y = -Math.PI / 2;
  turbo.scale.setScalar(0.8);
  scene.add(turbo);

  const state = { crankDeg: 0, wheelSpin: 0, timeScale: 1 / 50, stress: true, failFlash: 0 };
  const tmp = new THREE.Color();

  function resize() {
    const w = container.clientWidth || 600, h = container.clientHeight || 380;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
  }
  new ResizeObserver(resize).observe(container);
  resize();

  // Advance the animation by dt seconds of wall time from the sim state.
  function update(sim, dt) {
    const e = sim.eng, s = sim.internals, load = s.load;
    state.crankDeg = (state.crankDeg + e.rpm / 60 * 360 * dt * state.timeScale) % 720;
    const theta = state.crankDeg;
    crank.rotation.x = theta * Math.PI / 180;
    for (const cam of cams) cam.rotation.x = theta / 2 * Math.PI / 180;

    for (let i = 0; i < 4; i++) {
      const phi = theta + THROW[i];
      const a = phi * Math.PI / 180;
      const pinY = R * Math.cos(a), pinZ = R * Math.sin(a);
      const py = pistonY(phi);
      parts.pistons[i].position.y = py;
      const rod = parts.rods[i];
      rod.position.set((i - 1.5) * PITCH, pinY, pinZ);
      rod.rotation.x = Math.atan2(-pinZ, py - pinY);         // lean toward the piston pin at (0, py)
      const cycle = ((theta - FIRING_OFFSET[i]) % 720 + 720) % 720;
      const burn = sim.running && !e.revCut && !e.dfco && !e.boostCut && cycle < 70 ? 1 - cycle / 70 : 0;
      const glow = parts.glows[i];
      glow.position.y = py + 34;
      glow.material.opacity = burn * Math.min(1, s.pmaxBar / 90) * 0.85;
      glow.material.color.set(s.knockSpike > 1 && burn ? 0xffffff : 0xff7a1a);
    }
    for (const v of parts.valves) {
      const { cyl, intake, base, side } = v.userData;
      const cycle = theta - FIRING_OFFSET[cyl];
      const lift = valveLift(cycle, intake, e.highCam) * (e.highCam ? 11 : 8);
      v.position.set(base.x, base.y - lift * Math.cos(0.22), base.z - side * lift * Math.sin(0.22));
    }

    // Stress colours (or plain metal), with the failed part flashing.
    state.failFlash += dt * 6;
    const flash = s.failed ? (Math.sin(state.failFlash) > 0 ? 1.4 : 0) : null;
    const paint = (m, l, base) => {
      if (flash !== null && l >= 1) m.color.copy(RED).multiplyScalar(flash || 0.2);
      else if (state.stress) m.color.copy(stressColor(l, tmp));
      else m.color.set(base);
    };
    const failedLoad = key => (s.failed && s.damage[key] >= 1 ? 1.5 : load[key]);
    for (const p of parts.pistons) paint(p.userData.mat, failedLoad('piston'), ALU);
    for (const r of parts.rods) { paint(r.userData.mat, failedLoad('rod'), STEEL); paint(r.userData.shell.material, failedLoad('bearing'), 0xb08d57); }
    paint(crankMat, failedLoad('crank'), STEEL);
    paint(gasketMat, failedLoad('gasket'), 0x666666);
    for (const v of parts.valves) paint(v.userData.spring.material, failedLoad('valvetrain'), 0x3f6fb5);

    const turboOn = !!e.tune.turbo?.enabled;
    turbo.visible = turboOn;
    if (turboOn) {
      const shaftRpm = 25000 + e.boost * 900;                  // rough: tens of thousands of rpm, rising with boost
      state.wheelSpin += shaftRpm / 60 * 2 * Math.PI * dt * state.timeScale;
      wheel.rotation.z = state.wheelSpin;
      hot.material.emissive = new THREE.Color(0xff3300).multiplyScalar(Math.min(1, e.boost / 120) * 0.35);
    }

    controls.update();
    renderer.render(scene, camera);
  }

  return { update, state, setStress: on => { state.stress = on; }, setTimeScale: k => { state.timeScale = k; }, setCutaway: on => { blockMat.opacity = on ? 0.12 : 0.6; } };
}
