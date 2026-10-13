// The sim's state for add-on modules (the 3D renderer, gauges, loggers), as one plain object.
//
// A module should read everything through renderState(sim) rather than reaching into sim.eng or
// sim.internals, so the engine model can change without breaking it. The object is plain data
// (JSON-safe), built fresh each call; nothing in it writes back to the sim.
//
// A display module plugs in as:   const view = createX(container); every frame: view.update(renderState(sim), dt)
// See docs/MODULES.md.

import { PARTS, partMaterial, engineGeometry } from './internals.js';
import { bottomEnd } from './parts.js';
import { FLOW, valveLiftM, camCard } from './gasflow.js';
import { circuits, viscosityCst, boilingC } from './thermal.js';

export const FIRING_ORDER = [1, 3, 4, 2];
// Crank angle of each cylinder's firing TDC after cylinder 1's (cylinders 1–4).
export const FIRING_OFFSET_DEG = [0, 540, 180, 360];

export function renderState(sim) {
  const e = sim.eng, s = sim.internals, t = e.tune;
  const firing = sim.running && !e.revCut && !e.dfco && !e.boostCut;
  const geom = { ...FLOW, ...(t.flow || {}) }, G = engineGeometry(t), b = bottomEnd(t);
  return {
    running: sim.running,
    rpm: e.rpm,
    crankDeg: e.crankDeg ?? 0,                // 0–720, cylinder 1 firing TDC = 0
    firing,
    gear: sim.car.gear,                       // -1 reverse, 0 neutral, 1–6
    speedMs: sim.car.speedMs,
    throttlePct: e.tps,
    mapKpa: e.map,
    cams: {
      highCam: e.highCam,                     // ECU on the high-cam tables (VTEC solenoid open and oil pressure confirmed)
      vtecCommanded: !!e.vtecCmd,             // ECU wants VTEC (rpm, load and coolant conditions met)
      oilKpa: e.oilKpa ?? 0,
      vtecPins: [...(e.vtecPins ?? [0, 0, 0, 0])],   // lock pin travel per cylinder 1–4, 0 = low lobe, 1 = locked to the VTEC lobe
      vtecEngagedShare: e.camF ?? 0,          // share of cylinders breathing on the VTEC lobes
      vtcDeg: e.vtc ?? 0, vtecRpm: t.vtecRpm,
      card: camCard(geom),                    // lift, duration at 1 mm, centreline for each of the four lobes
    },
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
    geometry: { boreM: G.boreM, strokeM: G.strokeM, rodM: G.rodM, compression: G.compression, displacementL: b.displacementL,
      deckM: b.deckMm / 1000, deckClearanceM: b.deckClearanceMm / 1000, inValveM: geom.inValveM, exValveM: geom.exValveM },
    cooling: coolingState(e),
    bottomEnd: { stock: b.stock, crank: b.crank.name, pistons: b.pistonName, rods: b.rodName, block: b.crank.block ?? 'K20Z3', issues: b.issues },
  };
}

// Coolant and oil: temperatures, flows, the thermostat and fans, and the circuits as from/to segments
// between named components (see COMPONENTS in thermal.js for where each one sits on the car).
function coolingState(e) {
  const th = e.thermal, g = th.g, c = circuits(th, e.oilKpa ?? 0);
  return {
    coolantC: th.coolantC, radiatorInC: th.coolantC, radiatorOutC: th.radiatorOutC, pumpInletC: th.inletC,
    boilingC: boilingC(g), boiling: th.boiling, overheatS: th.overheatS,
    thermostatOpen: th.thermostat, thermostatOpenC: g.thermostatOpenC, thermostatFullC: g.thermostatFullC,
    fanOn: th.fanOn, fans: g.fans, fanOnC: g.fanOnC, fanOffC: g.fanOffC, airMs: th.airMs,
    pumpLpm: th.pumpLpm, radiatorLpm: th.radiatorLpm, bypassLpm: th.bypassLpm,
    radiatorKw: th.radiatorKw, heatToCoolantKw: th.heatToCoolantKw, heatToOilKw: th.heatToOilKw,
    oil: { tempC: th.oilC, pressureKpa: e.oilKpa ?? 0, viscosityCst: viscosityCst(th.oilC, g), grade: g.oilGrade,
      cooler: g.oilCooler, coolerKw: th.oilCoolerKw, toCoolantKw: th.oilToCoolantKw, sumpKw: th.sumpKw },
    coolantCircuit: c.coolant, oilCircuit: c.oil,
  };
}

// Valve lift in metres for cylinder 1–4 at crank angle crankDeg (cylinder 1 firing TDC = 0), from the
// same cam lobes and VTC phasing the flow model uses.
export function valveLift(state, cyl, intake, crankDeg = state.crankDeg) {
  const theta = crankDeg - FIRING_OFFSET_DEG[cyl - 1];
  return valveLiftM(theta, intake, state.cams.highCam, intake ? state.cams.vtcDeg : 0);
}
