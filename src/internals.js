// Engine internals: cylinder pressure, the loads it puts on each part, and metal fatigue.
//
// Every firing stroke loads the piston, rod, rod bearing, head gasket and crank; every revolution the
// reciprocating mass tries to pull the rod apart at top dead centre. Each part has a material with a
// strength that drops with temperature. A part's "load" is demand ÷ capacity: below 0.8 the metal is
// under its endurance limit and lives forever; above it, fatigue damage builds up (Miner's rule with a
// Basquin-style S–N curve) until the part fails; above 1.5 it fails at once (overload).
//
// Published: bore and stroke 86 × 86 mm, 11.0:1 (Honda); 139 mm rod length and 22 mm pin (aftermarket
// parts listed as stock dimensions); 48 mm rod big end; stock rods are powdered-metal forgings.
// Everything else (masses, section sizes, material grades, the pressure and temperature model, and
// the limits) is an engineering estimate, tuned so a stock engine is safe at its redline and a stock
// bottom end gets marginal around 12–14 psi on pump gas, which is where K20 owners report trouble.

import { clamp, lookup1 } from './tables.js';

// Typical properties for each material grade (room temperature unless a temperature curve is given).
// strengthMPa is the fatigue strength used for the part's capacity; derate scales it with temperature.
export const MATERIALS = {
  pistonAl: {
    name: 'Cast hypereutectic aluminium (A390-type)', densityKgM3: 2730, modulusGPa: 81, yieldMPa: 240, utsMPa: 280,
    strengthMPa: 110, note: 'fatigue strength at 10⁸ cycles',
    derate: [[20, 1], [150, 0.9], [200, 0.78], [250, 0.64], [300, 0.46], [350, 0.32]],
  },
  rodPM: {
    name: 'Powder-forged steel (Fe-Cu-C, PF-11C60-type)', densityKgM3: 7800, modulusGPa: 200, yieldMPa: 600, utsMPa: 900,
    strengthMPa: 600, note: 'yield; capacity is beam area × yield × 0.75 for buckling',
  },
  rodBolt: {
    name: 'Alloy steel bolt, property class 10.9', densityKgM3: 7850, modulusGPa: 205, yieldMPa: 940, utsMPa: 1040,
    strengthMPa: 70, note: 'fatigue strength of a rolled thread (stress amplitude)',
  },
  bearing: {
    name: 'Aluminium-tin bimetal bearing shell', densityKgM3: 2900, modulusGPa: 70, yieldMPa: 110, utsMPa: 160,
    strengthMPa: 85, note: 'peak load on the projected bearing area',
  },
  crank: {
    name: 'Forged micro-alloyed steel (38MnVS-type)', densityKgM3: 7850, modulusGPa: 210, yieldMPa: 580, utsMPa: 900,
    strengthMPa: 420, note: 'bending fatigue strength at the pin fillet',
  },
  gasket: {
    name: 'Multi-layer steel head gasket, stock head bolts', densityKgM3: 7850, modulusGPa: 200, yieldMPa: 1000, utsMPa: 1200,
    strengthMPa: 160, note: 'cylinder pressure (bar) the clamp load holds before the gasket lifts',
  },
  valveSpring: {
    name: 'Chrome-silicon valve spring wire', densityKgM3: 7850, modulusGPa: 206, yieldMPa: 1600, utsMPa: 2000,
    strengthMPa: 9000, note: 'rpm where the high-cam springs stop controlling the valves (float); safe to 8,600',
  },
};

export const GEOMETRY = {
  boreM: 0.086, strokeM: 0.086, rodM: 0.139, compression: 11.0,     // published
  pinM: 0.022, bigEndM: 0.048,                                       // published (aftermarket parts listed as stock size)
  pistonKg: 0.290, pinKg: 0.085, ringsKg: 0.035, rodSmallEndKg: 0.160, // estimated
  rodBeamAreaM2: 140e-6,                                             // estimated I-beam section
  rodBoltStressAreaM2: 2 * 39e-6,                                    // two M8 × 0.75 bolts, estimated
  bearingWidthM: 0.017,                                              // estimated
  pistonStressPerBar: 0.55,                                          // MPa of crown/ring-land stress per bar, estimated
};

const G = GEOMETRY;
const PISTON_AREA = Math.PI / 4 * G.boreM ** 2;
const RECIP_KG = G.pistonKg + G.pinKg + G.ringsKg + G.rodSmallEndKg;
const LAMBDA = G.strokeM / 2 / G.rodM;

export const PARTS = [
  { key: 'piston', name: 'Pistons', material: 'pistonAl' },
  { key: 'rod', name: 'Connecting rods', material: 'rodPM' },
  { key: 'rodBolt', name: 'Rod bolts', material: 'rodBolt' },
  { key: 'bearing', name: 'Rod bearings', material: 'bearing' },
  { key: 'crank', name: 'Crankshaft', material: 'crank' },
  { key: 'gasket', name: 'Head gasket', material: 'gasket' },
  { key: 'valvetrain', name: 'Valvetrain', material: 'valveSpring' },
];

export function createInternals() {
  return {
    pmaxBar: 0, pistonC: 120, rodCompN: 0, rodTensionN: 0, knockSpike: 1,
    load: Object.fromEntries(PARTS.map(p => [p.key, 0])),
    damage: Object.fromEntries(PARTS.map(p => [p.key, 0])),   // 0–1, 1 = failed
    failed: null,
  };
}

// Cycles to failure at a given load ratio (demand ÷ capacity). Infinite below the endurance limit.
export function cyclesToFailure(load) {
  if (load < 0.8) return Infinity;
  if (load >= 1.5) return 1;
  return 1e7 * (0.8 / load) ** 9;
}

// Peak cylinder pressure (bar): compression pressure × combustion rise, shifted by spark timing.
export function peakPressureBar(e) {
  const charge = (e.map / 101.3) * (e.ve ?? 0.9) * (298 / (273 + e.iat));
  const compressionBar = 1.013 * charge * G.compression ** 1.3;
  if (e.revCut || e.dfco || e.boostCut || e.torqueNm === undefined) return compressionBar;
  const spark = clamp(1 + 0.025 * (e.ign - e.mbt), 0.35, 1.3);       // advance raises it, retard lowers it
  const mixture = e.lambda < 1 ? 1 - (1 - e.lambda) * 0.3 : 1 / e.lambda;
  return compressionBar * 3.8 * spark * mixture;
}

// Update loads and fatigue for dt seconds of running. e is the engine state from engine.js.
export function stepInternals(s, e, dt) {
  if (s.failed) return;
  const rpm = Math.max(e.rpm, 0), w = rpm * Math.PI / 30;

  // Knock: pressure oscillations on top of the peak, getting worse the further past the knock limit.
  const over = Math.max(0, e.ign - e.knockLimit);
  s.knockSpike = e.knocking ? 1 + Math.min(0.15 * (1 + over), 1) : 1;
  s.pmaxBar = peakPressureBar(e) * s.knockSpike;

  // Piston crown temperature: heat from combustion, more when lean or knocking.
  const cut = e.revCut || e.dfco || e.boostCut;
  const tTarget = cut ? 150 : 150 + 0.64 * (e.airGs ?? 0) + (!cut && e.lambda > 0.85 && e.map > 90 ? 150 * Math.min(e.lambda - 0.85, 0.5) : 0) + (e.knocking ? 40 * (s.knockSpike - 1) / 0.15 : 0);
  s.pistonC += (tTarget - s.pistonC) * Math.min(1, dt * 0.25);

  // Forces on the rod: gas load down on the firing stroke (inertia helps a little), inertia up at exhaust TDC.
  const inertiaN = RECIP_KG * (G.strokeM / 2) * w * w * (1 + LAMBDA);
  s.rodCompN = Math.max(0, s.pmaxBar * 1e5 * PISTON_AREA - 0.85 * inertiaN);
  s.rodTensionN = inertiaN;

  const M = MATERIALS;
  const pistonCapacity = M.pistonAl.strengthMPa * lookup1(M.pistonAl.derate.map(d => d[0]), M.pistonAl.derate.map(d => d[1]), s.pistonC);
  s.load.piston = (G.pistonStressPerBar * s.pmaxBar) / pistonCapacity;
  s.load.rod = s.rodCompN / (G.rodBeamAreaM2 * M.rodPM.yieldMPa * 1e6 * 0.75);
  s.load.rodBolt = (0.25 * s.rodTensionN / G.rodBoltStressAreaM2 / 2) / (M.rodBolt.strengthMPa * 1e6);
  s.load.bearing = Math.max(s.rodCompN, s.rodTensionN) / (G.bigEndM * G.bearingWidthM) / (M.bearing.strengthMPa * 1e6);
  s.load.crank = 0.0035 * (s.rodCompN + 0.5 * s.rodTensionN) / M.crank.strengthMPa;   // MPa per N at the pin fillet, estimated
  s.load.gasket = s.pmaxBar / M.gasket.strengthMPa;
  // Valvetrain: endurance up to Hondata's 8,600 rpm reflash limit, valve float (load 1.0) at the float rpm.
  const floatRpm = M.valveSpring.strengthMPa * (e.highCam ? 1 : 1.08), safeRpm = floatRpm - 400;
  s.load.valvetrain = rpm <= safeRpm ? 0.8 * (rpm / safeRpm) ** 2 : 0.8 + 0.2 * (rpm - safeRpm) / 400;

  // Fatigue: each cylinder fires every other revolution.
  const cycles = rpm / 120 * dt;
  for (const p of PARTS) {
    const n = cyclesToFailure(s.load[p.key]);
    if (n !== Infinity) s.damage[p.key] = Math.min(1, s.damage[p.key] + cycles / n);
    if (s.damage[p.key] >= 1 && !s.failed) s.failed = p.name;
  }
}

export function partStatus(load, damage) {
  if (damage >= 1) return 'failed';
  if (load >= 1 || damage > 0.5) return 'danger';
  if (load >= 0.8 || damage > 0) return 'warn';
  return 'ok';
}
