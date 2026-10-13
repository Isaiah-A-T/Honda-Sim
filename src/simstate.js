// The sim's state for add-on modules (the 3D renderer, gauges, loggers), as one plain object.
//
// A module should read everything through renderState(sim) rather than reaching into sim.eng or
// sim.internals, so the engine model can change without breaking it. The object is plain data
// (JSON-safe), built fresh each call; nothing in it writes back to the sim.
//
// A display module plugs in as:   const view = createX(container); every frame: view.update(renderState(sim), dt)
// See docs/MODULES.md.

import { PARTS, GEOMETRY, partMaterial } from './internals.js';
import { FLOW, valveLiftM } from './gasflow.js';

export const FIRING_ORDER = [1, 3, 4, 2];
// Crank angle of each cylinder's firing TDC after cylinder 1's (cylinders 1–4).
export const FIRING_OFFSET_DEG = [0, 540, 180, 360];

export function renderState(sim) {
  const e = sim.eng, s = sim.internals, t = e.tune;
  const firing = sim.running && !e.revCut && !e.dfco && !e.boostCut;
  const geom = { ...FLOW, ...(t.flow || {}) };
  return {
    running: sim.running,
    rpm: e.rpm,
    crankDeg: sim.crankDeg ?? 0,              // 0–720, cylinder 1 firing TDC = 0
    firing,
    gear: sim.car.gear,                       // -1 reverse, 0 neutral, 1–6
    speedMs: sim.car.speedMs,
    throttlePct: e.tps,
    mapKpa: e.map,
    cams: { highCam: e.highCam, vtcDeg: e.vtc ?? 0, vtecRpm: t.vtecRpm },
    combustion: {
      ignDeg: e.ign, mbtDeg: e.mbt, knockLimitDeg: e.knockLimit, ca50Deg: Number.isFinite(e.ca50) ? e.ca50 : null,
      burnDeg: e.burnDeg ?? null, knocking: !!e.knocking, knockIndex: e.knockIndex ?? 0, lambda: e.lambda,
    },
    flow: {
      ve: e.ve, airGs: e.airGs, residual: e.residual ?? 0, pumpingKpa: e.pmepKpa ?? 0,
      exhaustKpa: e.exhKpa ?? 101.3, egtC: e.egtC ?? 0, peakCylinderBar: s.pmaxBar,
    },
    // 'na' (no turbo), 'turbo', or 'supercharger' (boost held fixed, no turbine)
    induction: !t.turbo?.enabled ? 'na' : t.turbo.fixed ? 'supercharger' : 'turbo',
    intercooler: !!t.turbo?.enabled && t.turbo.intercooler !== false,
    turbo: t.turbo?.enabled
      ? { chargeTempC: e.chargeOutC ?? e.iat, intercoolerDropKpa: e.icDropKpa ?? 0, boostKpa: e.boost, shaftRpm: (e.turboRadS ?? 0) * 30 / Math.PI, wastegateOpen: e.wgOpen ?? 0, turbineKw: e.turbineKw ?? 0, compressorKw: e.compressorKw ?? 0 }
      : null,
    temps: { pistonC: s.pistonC, iatC: e.iat, ectC: e.ect },
    // One entry per part: load (demand ÷ capacity; 0.8 = endurance limit, 1.5 = breaks), life used 0–1, material.
    parts: Object.fromEntries(PARTS.map(p => {
      const m = partMaterial(t, p);
      return [p.key, { name: p.name, load: s.load[p.key], damage: s.damage[p.key], failed: s.failed === p.name, material: { name: m.name, category: m.category, densityKgM3: m.densityKgM3 ?? null } }];
    })),
    failed: s.failed,
    forces: { rodDownN: s.rodCompN, rodUpN: s.rodTensionN, sleeveMPa: s.sleeveMPa ?? 0 },
    geometry: { boreM: GEOMETRY.boreM, strokeM: GEOMETRY.strokeM, rodM: GEOMETRY.rodM, inValveM: geom.inValveM, exValveM: geom.exValveM },
  };
}

// Valve lift in metres for cylinder 1–4 at crank angle crankDeg (cylinder 1 firing TDC = 0), from the
// same cam lobes and VTC phasing the flow model uses.
export function valveLift(state, cyl, intake, crankDeg = state.crankDeg) {
  const theta = crankDeg - FIRING_OFFSET_DEG[cyl - 1];
  return valveLiftM(theta, intake, state.cams.highCam, intake ? state.cams.vtcDeg : 0);
}
