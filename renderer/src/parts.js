// Basic 3D models of the K20Z3's moving and fixed parts. Everything is built in millimetres from the
// numbers in kinematics.js; the renderer scales the whole engine down to scene units.
//
// Each part that can show stress is registered with a stress key (and a cylinder index where it has one),
// so the renderer can tint it, and it carries a label for the hover tooltip.

import * as THREE from 'three';
import { K20 } from './kinematics.js';

const DECK = K20.deckHeight;
const ROOF = DECK + 10;              // top of the combustion chamber
const CAM_Y = DECK + 128;
const CAM_Z = 38;
const VALVE_SEAT_Z = 20;
const VALVE_TIP_Y = CAM_Y - 19;      // where the stem meets the cam base circle
export const cylX = cyl => (cyl - 2.5) * K20.boreSpacing;

export function buildEngine() {
  const root = new THREE.Group();
  root.name = 'K20Z3';
  const reg = { tintables: [], pickables: [], cyl: {}, flow: [] };
  // Paths the airflow view runs particles along, in engine millimetres. kind: intake or exhaust;
  // cyl: which cylinder's valves gate it; only: show just for 'turbo' or 'na' builds.
  const flow = (kind, pts, extra = {}) => reg.flow.push({ kind, curve: new THREE.CatmullRomCurve3(pts.map(p => new THREE.Vector3(...p))), ...extra });

  const mat = (color, o = {}) => new THREE.MeshStandardMaterial({ color, metalness: 0.55, roughness: 0.45, ...o });
  const glass = (color, opacity) => new THREE.MeshStandardMaterial({
    color, metalness: 0.1, roughness: 0.6, transparent: true, opacity, depthWrite: false, side: THREE.DoubleSide,
  });
  const add = (parent, mesh, key, label, index) => {
    mesh.userData.part = { key, label, index };
    parent.add(mesh);
    reg.pickables.push(mesh);
    if (key) {
      const m = mesh.material;
      reg.tintables.push({ key, index, mesh, base: m.color.clone(), baseEmissive: m.emissive.clone() });
    }
    return mesh;
  };
  const cylAlongX = (r, len, seg = 32) => new THREE.CylinderGeometry(r, r, len, seg).rotateZ(Math.PI / 2);
  const tube = (pts, r) => new THREE.TubeGeometry(new THREE.CatmullRomCurve3(pts.map(p => new THREE.Vector3(...p))), 48, r, 16);
  const shell = (mesh, color = 0x5a6b7d) => {           // outline so translucent castings still read
    const lines = new THREE.LineSegments(new THREE.EdgesGeometry(mesh.geometry, 25),
      new THREE.LineBasicMaterial({ color, transparent: true, opacity: 0.5 }));
    lines.raycast = () => {};
    mesh.add(lines);
    return mesh;
  };
  const len = 4 * K20.boreSpacing + 40;

  // ---------- Block, bores, oil pan ----------
  const castings = [];
  const block = shell(add(root, new THREE.Mesh(new THREE.BoxGeometry(len, DECK + 60, 190), glass(0x8fa3b8, 0.12)),
    'block', 'Engine block'));
  block.position.y = (DECK - 60) / 2;
  castings.push(block);
  for (let c = 1; c <= 4; c++) {
    const liner = new THREE.Mesh(new THREE.CylinderGeometry(K20.bore / 2 + 2, K20.bore / 2 + 2, 130, 40, 1, true),
      glass(0xa9b8c8, 0.18));
    liner.position.set(cylX(c), DECK - 65, 0);
    liner.raycast = () => {};
    castings.push(liner);
    root.add(liner);
  }
  const pan = shell(add(root, new THREE.Mesh(new THREE.BoxGeometry(len - 30, 90, 170), glass(0x6f7782, 0.12)),
    null, 'Oil pan'));
  pan.position.y = -60 - 45;
  castings.push(pan);

  // ---------- Head and cam cover ----------
  const head = shell(add(root, new THREE.Mesh(new THREE.BoxGeometry(len, 120, 160), glass(0x9fb0c2, 0.1)),
    'head', 'Cylinder head'));
  head.position.y = DECK + 60;
  castings.push(head);
  const cover = shell(add(root, new THREE.Mesh(new THREE.BoxGeometry(len - 20, 45, 150), glass(0xb3262b, 0.14)),
    null, 'Cam cover'), 0x8a2a2a);
  cover.position.y = DECK + 120 + 22;
  castings.push(cover);

  // ---------- Crankshaft (rotates about X) ----------
  const crank = new THREE.Group();
  crank.name = 'crankshaft';
  root.add(crank);
  const steel = () => mat(0x9aa0a6, { metalness: 0.8, roughness: 0.3 });
  const r = K20.stroke / 2;
  for (let i = 0; i < 5; i++) {                       // main journals between and outside the cylinders
    const j = add(crank, new THREE.Mesh(cylAlongX(27, 22), steel()), 'crank', 'Crankshaft main journal');
    j.position.x = (i - 2) * K20.boreSpacing;
  }
  for (let c = 1; c <= 4; c++) {
    const up = c === 1 || c === 4 ? 1 : -1;
    const pin = add(crank, new THREE.Mesh(cylAlongX(24, 26), steel()), 'crank', `Crank pin ${c}`);
    pin.position.set(cylX(c), up * r, 0);
    for (const side of [-1, 1]) {                     // web with counterweight opposite the pin
      const web = new THREE.Group();
      web.position.x = cylX(c) + side * 24;
      const arm = add(web, new THREE.Mesh(new THREE.BoxGeometry(14, r + 30, 48), steel()), 'crank', 'Crank web');
      arm.position.y = up * r / 2;
      const cwShape = new THREE.Shape();
      cwShape.absarc(0, 0, 70, Math.PI * 1.18, Math.PI * 1.82, false);
      cwShape.lineTo(0, 0);
      const cw = add(web, new THREE.Mesh(new THREE.ExtrudeGeometry(cwShape, { depth: 14, bevelEnabled: false })
        .translate(0, 0, -7).rotateY(Math.PI / 2), steel()), 'crank', 'Counterweight');
      if (up < 0) cw.rotation.x = Math.PI;
      crank.add(web);
    }
  }
  const snout = add(crank, new THREE.Mesh(cylAlongX(18, 70), steel()), 'crank', 'Crank snout');
  snout.position.x = cylX(1) - 85;
  const pulley = add(crank, new THREE.Mesh(cylAlongX(70, 22, 48), mat(0x2c3138)), null, 'Crank pulley');
  pulley.position.x = cylX(1) - 120;
  const mark = new THREE.Mesh(new THREE.BoxGeometry(24, 40, 8), mat(0xf4f4f4, { emissive: 0x666666 }));
  mark.position.set(cylX(1) - 120, 48, 0);         // timing mark so rotation is easy to see
  crank.add(mark);
  const crankSprocket = add(crank, new THREE.Mesh(cylAlongX(22, 14, 18), steel()), null, 'Crank timing sprocket');
  crankSprocket.position.x = cylX(1) - 72;

  const flywheel = add(crank, new THREE.Mesh(cylAlongX(140, 26, 64), mat(0x6f757c, { metalness: 0.85 })),
    'flywheel', 'Flywheel');
  flywheel.position.x = cylX(4) + 110;
  const fwMark = new THREE.Mesh(new THREE.BoxGeometry(28, 90, 14), mat(0xffc400, { emissive: 0x553300 }));
  fwMark.position.set(cylX(4) + 96, 80, 0);
  crank.add(fwMark);
  const clutch = add(crank, new THREE.Mesh(cylAlongX(118, 30, 48), mat(0x4a4f57)), 'clutch', 'Clutch pressure plate');
  clutch.position.x = cylX(4) + 138;

  // ---------- Pistons, wrist pins, rods ----------
  for (let c = 1; c <= 4; c++) {
    const piston = new THREE.Group();
    piston.position.x = cylX(c);
    const body = add(piston, new THREE.Mesh(new THREE.CylinderGeometry(K20.bore / 2 - 0.5, K20.bore / 2 - 0.5, 48, 40),
      mat(0xd5d8dc, { metalness: 0.4, roughness: 0.35 })), 'pistons', `Piston ${c}`, c - 1);
    body.position.y = 6;
    for (const ry of [24, 18, 12]) {                  // ring lands
      const ring = new THREE.Mesh(new THREE.CylinderGeometry(K20.bore / 2, K20.bore / 2, 1.6, 40), mat(0x30343a));
      ring.position.y = ry;
      ring.raycast = () => {};
      piston.add(ring);
    }
    add(piston, new THREE.Mesh(cylAlongX(10, 60, 16), mat(0x8c939b, { metalness: 0.9 })), 'pistons', `Wrist pin ${c}`, c - 1);
    root.add(piston);

    const rod = new THREE.Group();
    rod.position.x = cylX(c);
    const rodMat = mat(0xb8bcc2, { metalness: 0.75, roughness: 0.3 });
    const beam = add(rod, new THREE.Mesh(new THREE.BoxGeometry(20, K20.rodLength - 30, 16), rodMat), 'rods', `Connecting rod ${c}`, c - 1);
    beam.position.y = -K20.rodLength / 2;
    const small = add(rod, new THREE.Mesh(cylAlongX(15, 22, 20), rodMat.clone()), 'rods', `Rod small end ${c}`, c - 1);
    const big = add(rod, new THREE.Mesh(cylAlongX(32, 24, 28), rodMat.clone()), 'rods', `Rod big end ${c}`, c - 1);
    big.position.y = -K20.rodLength;
    small.position.y = 0;
    root.add(rod);

    // Combustion glow between the crown and the chamber roof.
    const flame = new THREE.Mesh(new THREE.CylinderGeometry(K20.bore / 2 - 4, K20.bore / 2 - 4, 1, 32),
      new THREE.MeshBasicMaterial({ color: 0xff8a1e, transparent: true, opacity: 0, blending: THREE.AdditiveBlending, depthWrite: false }));
    flame.position.x = cylX(c);
    flame.raycast = () => {};
    root.add(flame);

    reg.cyl[c] = { piston, rod, flame, valves: { intake: [], exhaust: [] }, plugGlow: null };
  }

  // ---------- Camshafts, lobes, valves, spark plugs ----------
  const cams = {};
  const lobeGeo = (base, liftMm, width) => {
    const s = new THREE.Shape();
    const n = 48;
    for (let i = 0; i <= n; i++) {                    // base circle with a smooth nose pointing +Y
      const a = i / n * Math.PI * 2;
      const nose = Math.max(0, Math.cos(a)) ** 3 * liftMm;
      const rr = base + nose;
      const p = [rr * Math.sin(a), rr * Math.cos(a)];
      i ? s.lineTo(...p) : s.moveTo(...p);
    }
    return new THREE.ExtrudeGeometry(s, { depth: width, bevelEnabled: false }).translate(0, 0, -width / 2).rotateY(Math.PI / 2);
  };
  for (const side of ['intake', 'exhaust']) {
    const z = side === 'intake' ? CAM_Z : -CAM_Z;
    const cam = new THREE.Group();
    cam.position.set(0, CAM_Y, z);
    root.add(cam);
    const camMat = mat(0x7d8590, { metalness: 0.8, roughness: 0.35 });
    add(cam, new THREE.Mesh(cylAlongX(12, len - 10, 20), camMat), 'cams', `${side === 'intake' ? 'Intake' : 'Exhaust'} camshaft`);
    const sprocket = add(cam, new THREE.Mesh(cylAlongX(34, 14, 36), mat(0x5e656e)), null, `${side === 'intake' ? 'Intake' : 'Exhaust'} cam sprocket`);
    sprocket.position.x = cylX(1) - 72;
    const sMark = new THREE.Mesh(new THREE.BoxGeometry(16, 14, 6), mat(0xf4f4f4, { emissive: 0x555555 }));
    sMark.position.set(cylX(1) - 72, 28, 0);
    cam.add(sMark);
    cams[side] = { group: cam, vtecLobes: [] };
    const lowLift = K20.cams.low[side].lift, highLift = K20.cams.high[side].lift;
    for (let c = 1; c <= 4; c++) {
      const slot = K20.firingOrder.indexOf(c);
      const centre = K20.cams.low[side].centre + slot * 180;   // crank degrees at peak lift
      const alpha = Math.PI - (centre / 2) * Math.PI / 180;    // nose points down (-Y) at peak lift
      for (const dx of [-17, 17]) {
        const lobe = add(cam, new THREE.Mesh(lobeGeo(17, lowLift, 12), camMat.clone()), 'cams', `Cyl ${c} ${side} lobe (low cam)`);
        lobe.position.x = cylX(c) + dx;
        lobe.rotation.x = alpha;
      }
      const hi = add(cam, new THREE.Mesh(lobeGeo(17, highLift, 10), mat(0x8a929c, { metalness: 0.8 })), 'cams', `Cyl ${c} ${side} VTEC lobe (high cam)`);
      hi.position.x = cylX(c);
      hi.rotation.x = alpha;
      cams[side].vtecLobes.push(hi);

      for (const dx of [-17, 17]) {
        const seat = new THREE.Vector3(cylX(c) + dx, ROOF, side === 'intake' ? VALVE_SEAT_Z : -VALVE_SEAT_Z);
        const tip = new THREE.Vector3(cylX(c) + dx, VALVE_TIP_Y, z);
        const axis = tip.clone().sub(seat);
        const stemLen = axis.length();
        axis.normalize();
        const valve = new THREE.Group();
        valve.position.copy(seat);
        valve.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), axis);
        const vMat = mat(side === 'intake' ? 0xc8b88a : 0x9a8a7a, { metalness: 0.7 });
        const headR = side === 'intake' ? 17.5 : 15;
        add(valve, new THREE.Mesh(new THREE.CylinderGeometry(headR, headR * 0.55, 4, 28), vMat), 'valves', `Cyl ${c} ${side} valve`, c - 1);
        const stem = add(valve, new THREE.Mesh(new THREE.CylinderGeometry(2.8, 2.8, stemLen, 10), vMat.clone()), 'valves', `Cyl ${c} ${side} valve`, c - 1);
        stem.position.y = stemLen / 2;
        const spring = new THREE.Mesh(new THREE.TorusGeometry(9, 1.6, 6, 18).rotateX(Math.PI / 2),
          mat(0x50565e, { metalness: 0.6 }));
        spring.position.y = stemLen - 30;
        spring.raycast = () => {};
        valve.add(spring);
        root.add(valve);
        reg.cyl[c].valves[side].push({ group: valve, seat, axis });
      }
    }
  }
  // Timing chain: a loop around the crank sprocket and both cam sprockets.
  const chainX = cylX(1) - 72;
  const chainPts = [[-25, 0], [-12, -22], [12, -22], [25, 0], [CAM_Z + 36, CAM_Y - 10], [CAM_Z + 22, CAM_Y + 28],
    [0, CAM_Y + 30], [-CAM_Z - 22, CAM_Y + 28], [-CAM_Z - 36, CAM_Y - 10]].map(([zz, y]) => new THREE.Vector3(chainX, y, zz));
  const chain = new THREE.Mesh(new THREE.TubeGeometry(new THREE.CatmullRomCurve3(chainPts, true), 120, 3, 6, true),
    mat(0x3b4048, { metalness: 0.9 }));
  chain.userData.part = { label: 'Timing chain' };
  reg.pickables.push(chain);
  root.add(chain);

  for (let c = 1; c <= 4; c++) {
    const plug = new THREE.Group();
    plug.position.set(cylX(c), ROOF + 6, 0);
    const body = new THREE.Mesh(new THREE.CylinderGeometry(6, 6, 90, 12), mat(0xeeeeee, { metalness: 0.1, roughness: 0.4 }));
    body.position.y = 50;
    body.userData.part = { label: `Spark plug ${c}` };
    reg.pickables.push(body);
    plug.add(body);
    const coil = new THREE.Mesh(new THREE.BoxGeometry(26, 40, 26), mat(0x1e2226));
    coil.position.y = 120;
    coil.userData.part = { label: `Ignition coil ${c}` };
    reg.pickables.push(coil);
    plug.add(coil);
    const glow = new THREE.Mesh(new THREE.SphereGeometry(7, 16, 12),
      new THREE.MeshBasicMaterial({ color: 0x9fd4ff, transparent: true, opacity: 0, blending: THREE.AdditiveBlending, depthWrite: false }));
    glow.raycast = () => {};
    plug.add(glow);
    root.add(plug);
    reg.cyl[c].plugGlow = glow;
  }

  // ---------- Intake: runners, plenum, throttle body, injectors ----------
  const intakeMat = mat(0x3a3f46, { metalness: 0.4, roughness: 0.6 });
  for (let c = 1; c <= 4; c++) {
    const x = cylX(c);
    add(root, new THREE.Mesh(tube([[x, DECK + 70, 80], [x, DECK + 110, 150], [x, DECK + 70, 220], [x, DECK - 10, 235]], 19), intakeMat.clone()),
      'intake', `Intake runner ${c}`);
    flow('intake', [[x, DECK - 30, 250], [x, DECK - 10, 235], [x, DECK + 70, 220], [x, DECK + 110, 150], [x, DECK + 70, 80],
      [x, ROOF + 22, VALVE_SEAT_Z + 6], [x, ROOF - 10, 0]], { cyl: c });
    const inj = new THREE.Mesh(new THREE.CylinderGeometry(6, 4, 55, 10).rotateX(-0.6), mat(0x24282e));
    inj.position.set(x + 20, DECK + 100, 105);
    inj.userData.part = { label: `Fuel injector ${c}` };
    reg.pickables.push(inj);
    root.add(inj);
  }
  flow('intake', [[-len / 2 - 70, DECK - 30, 250], [-len / 2 + 30, DECK - 30, 250], [len / 2 - 30, DECK - 30, 250]]);
  const plenum = add(root, new THREE.Mesh(cylAlongX(55, len - 20, 32), intakeMat.clone()), 'intake', 'Intake plenum');
  plenum.position.set(0, DECK - 30, 250);
  const tb = new THREE.Group();
  tb.position.set(-len / 2 - 40, DECK - 30, 250);
  add(tb, new THREE.Mesh(new THREE.CylinderGeometry(36, 36, 60, 32, 1, true).rotateZ(Math.PI / 2),
    mat(0x9aa3ad, { side: THREE.DoubleSide })), null, 'Throttle body');
  const butterfly = new THREE.Mesh(new THREE.CylinderGeometry(33, 33, 2, 32), mat(0xc7ccd2));
  butterfly.rotation.z = Math.PI / 2;               // disc faces down the bore
  const butterflyPivot = new THREE.Group();
  butterflyPivot.add(butterfly);
  butterfly.userData.part = { label: 'Throttle plate' };
  reg.pickables.push(butterfly);
  tb.add(butterflyPivot);
  root.add(tb);

  // ---------- Exhaust manifold and collector ----------
  for (let c = 1; c <= 4; c++) {
    const x = cylX(c);
    add(root, new THREE.Mesh(tube([[x, DECK + 70, -80], [x, DECK + 60, -150], [x * 0.6, DECK - 10, -210], [x * 0.15, DECK - 80, -235]], 18),
      mat(0x4f4945, { metalness: 0.45, roughness: 0.8 })), 'exhaust', `Exhaust runner ${c}`);
    flow('exhaust', [[x, ROOF - 10, 0], [x, ROOF + 22, -VALVE_SEAT_Z - 6], [x, DECK + 70, -80], [x, DECK + 60, -150], [x * 0.6, DECK - 10, -210],
      [x * 0.15, DECK - 80, -235], [0, DECK - 140, -240]], { cyl: c });
  }
  const collector = add(root, new THREE.Mesh(new THREE.CylinderGeometry(40, 32, 70, 24), mat(0x4f4945, { metalness: 0.45, roughness: 0.8 })),
    'exhaust', 'Exhaust collector');
  collector.position.set(0, DECK - 110, -240);

  // ---------- Turbo (shown for turbo builds) ----------
  const turbo = new THREE.Group();
  turbo.position.set(0, DECK - 180, -260);
  const tMat = mat(0x8a8f96, { metalness: 0.6, roughness: 0.5 });
  const turbHousing = add(turbo, new THREE.Mesh(new THREE.TorusGeometry(55, 30, 16, 40).rotateY(Math.PI / 2), tMat.clone()),
    'turbo', 'Turbine housing');
  turbHousing.position.x = -50;
  const compHousing = add(turbo, new THREE.Mesh(new THREE.TorusGeometry(65, 32, 16, 40).rotateY(Math.PI / 2), mat(0xb3b8be, { metalness: 0.5 })),
    'turbo', 'Compressor housing');
  compHousing.position.x = 70;
  add(turbo, new THREE.Mesh(cylAlongX(38, 90, 24), mat(0x52575e)), 'turbo', 'Centre section (bearings)');
  const wheel = (radius, blades, color) => {
    const g = new THREE.Group();
    g.add(new THREE.Mesh(cylAlongX(12, 30, 12), mat(color)));
    for (let i = 0; i < blades; i++) {
      const b = new THREE.Mesh(new THREE.BoxGeometry(26, radius, 2.5), mat(color, { metalness: 0.85 }));
      b.position.y = radius / 2;
      const holder = new THREE.Group();
      holder.rotation.x = i / blades * Math.PI * 2;
      b.rotation.y = 0.5;
      holder.add(b);
      g.add(holder);
    }
    g.traverse(o => { if (o.isMesh) o.raycast = () => {}; });
    return g;
  };
  const turbineWheel = wheel(36, 11, 0x8d7f70);
  turbineWheel.position.x = -50;
  const compWheel = wheel(44, 12, 0xd8dde2);
  compWheel.position.x = 70;
  turbo.add(turbineWheel, compWheel);
  const downpipe = add(turbo, new THREE.Mesh(tube([[-50, 0, -60], [-60, -60, -80], [-60, -160, -60]], 30), tMat.clone()), 'exhaust', 'Downpipe');
  const chargePipe = add(turbo, new THREE.Mesh(tube([[110, 0, 0], [180, 60, 80], [-len / 2 - 120, 150, 510], [-len / 2 - 70, 150, 510]], 26),
    mat(0x2f6fb5, { metalness: 0.5 })), 'intake', 'Charge pipe');
  void downpipe; void chargePipe;
  root.add(turbo);

  // Collector to turbine inlet.
  const up = add(root, new THREE.Mesh(tube([[0, DECK - 140, -240], [-30, DECK - 160, -250], [-50, DECK - 180, -262]], 28),
    mat(0x4f4945, { metalness: 0.45, roughness: 0.8 })), 'exhaust', 'Turbo up-pipe');
  turbo.userData.extras = [up];

  // NA intake pipe in place of the charge pipe when the turbo is hidden.
  const naIntake = add(root, new THREE.Mesh(tube([[-len / 2 - 70, DECK - 30, 250], [-len / 2 - 160, DECK - 30, 260], [-len / 2 - 220, DECK - 80, 150]], 36),
    mat(0x2b2f35, { roughness: 0.8 })), 'intake', 'Intake pipe');

  const T = [turbo.position.x, turbo.position.y, turbo.position.z];
  const at = pts => pts.map(([x, y, zz]) => [x + T[0], y + T[1], zz + T[2]]);
  flow('exhaust', [[0, DECK - 140, -240], [-30, DECK - 160, -250], [-50, DECK - 180, -262], ...at([[-50, 0, -60], [-60, -60, -80], [-60, -160, -60]])], { only: 'turbo' });
  flow('intake', at([[70, 0, 0], [110, 0, 0], [180, 60, 80], [-len / 2 - 120, 150, 510], [-len / 2 - 70, 150, 510]]), { only: 'turbo' });
  flow('intake', [[-len / 2 - 220, DECK - 80, 150], [-len / 2 - 160, DECK - 30, 260], [-len / 2 - 70, DECK - 30, 250]], { only: 'na' });
  flow('exhaust', [[0, DECK - 140, -240], [0, DECK - 260, -240]], { only: 'na' });

  // Which system each top-level piece belongs to, so view modes can show or hide whole systems.
  for (const child of root.children) {
    const part = child.userData.part;
    child.userData.sys = castings.includes(child) ? 'casting'
      : child === turbo ? 'turbo'
      : child === tb || child === naIntake || part?.key === 'intake' || /injector/i.test(part?.label ?? '') ? 'intake'
      : part?.key === 'exhaust' ? 'exhaust'
      : 'internals';
  }

  return { root, reg, crank, cams, butterflyPivot, turbo, turbineWheel, compWheel, naIntake, castings, chain };
}
