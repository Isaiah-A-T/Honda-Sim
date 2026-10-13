// Sends the sim's state to the native 3D viewer (viewer/, honda-sim-viewer.exe) over a localhost WebSocket.
// The viewer listens on ws://127.0.0.1:8765; start it before or after the sim, this keeps retrying.
// The JSON fields are listed in viewer/README.md.

import { PARTS, MATERIALS } from './internals.js';

export const VIEWER_URL = 'ws://127.0.0.1:8765';
const RETRY_MS = 2000;
const SEND_HZ = 30;

const clamp = (x, a, b) => Math.min(b, Math.max(a, x));

// Viewer part → sim internals part(s). The first key decides the material shown.
const PART_MAP = {
  pistons: ['piston'],
  rods: ['rod', 'rodBolt', 'bearing'],
  crank: ['crank'],
  valves: ['valvetrain'],
  cams: ['valvetrain'],
};

function familyOf(name) {
  const n = name.toLowerCase();
  if (n.includes('alumin')) return 'aluminum';
  if (n.includes('titan')) return 'titanium';
  if (n.includes('chrom') && n.includes('moly')) return 'chromoly';
  if (n.includes('stainless')) return 'stainless';
  if (n.includes('inconel') || n.includes('nickel')) return 'nickel';
  if (n.includes('iron')) return 'iron';
  return 'steel';
}

function partMaterials() {
  const out = {};
  for (const [key, simKeys] of Object.entries(PART_MAP)) {
    const p = PARTS.find(x => x.key === simKeys[0]);
    const m = p && MATERIALS[p.material];
    if (!m) continue;
    out[key] = { part: p.name, label: m.name, family: familyOf(m.name), detail: `yield ${m.yieldMPa} MPa, tensile ${m.utsMPa} MPa` };
  }
  return out;
}

// Stress per viewer part, 0–1 with 1 at the limit. Internals loads where the sim has them, rough estimates elsewhere.
function stressOf(sim) {
  const e = sim.eng, t = e.tune, running = !!sim.running;
  const tq = running ? Math.max(e.torqueNm, 0) : 0;
  const rodLimit = t.torqueLimitNm ?? 350;
  const spin = (e.rpm / (Math.max(t.revLimit ?? 8000, 6000) + 600)) ** 2;
  const turboOn = !!t.turbo?.enabled;
  const firing = running && !e.revCut && !e.dfco && !e.boostCut;
  const knockHit = e.knocking ? 0.35 : 0;
  const L = sim.internals.load, D = sim.internals.damage;
  const worst = keys => Math.max(...keys.map(k => (D[k] >= 1 ? 1.3 : L[k])));

  return {
    pistons: worst(['piston']),
    rods: worst(['rod', 'rodBolt', 'bearing']),
    crank: worst(['crank']),
    valves: worst(['valvetrain']),
    cams: worst(['valvetrain']) * 0.75,
    head: Math.max(worst(['gasket']), knockHit + clamp((e.iat - 25) / 60, 0, 0.5)),
    block: tq / rodLimit * 0.45,
    exhaust: firing ? clamp(e.rpm / 8000 * e.map / 100 * 0.55 + Math.max(0, e.lambda - 0.9) * 1.5 * clamp(e.map / 100, 0, 1), 0, 1.3) : 0,
    intake: turboOn && t.overboostKpa ? clamp((e.map - 101) / (t.overboostKpa - 101), 0, 1.3) : 0,
    turbo: turboOn ? Math.max(0, e.boost) / (t.turbo.maxBoostKpa || 150) : 0,
    clutch: tq / (t.clutchNm || 320),
    flywheel: spin * 0.5,
  };
}

export function stateForViewer(sim) {
  const e = sim.eng, s = sim.internals;
  return {
    rpm: Math.max(0, e.rpm),
    running: !!sim.running,
    throttle: clamp(e.tps / 100, 0, 1),
    mapKpa: e.map,
    boostKpa: Math.max(0, e.boost),
    airGs: e.airGs ?? 0,
    vtec: !!e.highCam,
    vtcDeg: e.vtcDeg ?? 0,                 // the sim has no VTC model yet
    ignDeg: e.ign,
    knock: !!e.knocking,
    fuelCut: !!(e.revCut || e.dfco || e.boostCut),
    lambda: e.lambda > 5 ? 1 : e.lambda,
    turbo: !!e.tune.turbo?.enabled,
    stress: stressOf(sim),
    partMaterials: partMaterials(),
    readouts: [
      ['Gear', sim.car.gear === 0 ? 'N' : sim.car.gear < 0 ? 'R' : String(sim.car.gear)],
      ['Torque', `${(sim.running ? e.torqueNm : 0).toFixed(0)} Nm`],
      ['Peak pressure', `${s.pmaxBar.toFixed(0)} bar`],
      ['Piston crown', `${s.pistonC.toFixed(0)} °C`],
      ...(s.failed ? [['FAILED', s.failed]] : []),
    ],
  };
}

// Call send(sim) every frame; it throttles itself. onStatus(connected) fires when the link changes.
export function createViewerLink(onStatus = () => {}) {
  let ws = null, connected = false, lastSend = 0;
  const set = c => { if (c !== connected) { connected = c; onStatus(c); } };
  function connect() {
    if (typeof WebSocket === 'undefined') return;
    try { ws = new WebSocket(VIEWER_URL); } catch { setTimeout(connect, RETRY_MS); return; }
    ws.onopen = () => set(true);
    ws.onclose = () => { set(false); ws = null; setTimeout(connect, RETRY_MS); };
    ws.onerror = () => {};                 // onclose follows and retries
  }
  connect();
  return {
    get connected() { return connected; },
    send(sim) {
      const now = performance.now();
      if (!connected || ws?.readyState !== 1 || now - lastSend < 1000 / SEND_HZ) return;
      lastSend = now;
      ws.send(JSON.stringify(stateForViewer(sim)));
    },
  };
}
