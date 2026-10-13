// Engine internals: cylinder pressure, the loads it puts on each part, and metal fatigue.
//
// Every firing stroke loads the piston, rod, rod bearing, head gasket and crank; every revolution the
// reciprocating mass tries to pull the rod apart at top dead centre. Each part has a material with a
// strength that drops with temperature. A part's "load" is demand ÷ capacity: below 0.8 the metal is
// under its endurance limit and lives forever; above it, fatigue damage builds up (Miner's rule with a
// Basquin-style S–N curve) until the part fails; above 1.5 it fails at once (overload).
//
// Published: bore and stroke 86 × 86 mm, 11.0:1 (Honda); 139 mm rod length and 22 mm pin (aftermarket
// parts listed as stock dimensions); 48 mm rod big end; stock rods are powdered-metal forgings;
// aluminium open-deck block with cast-in iron sleeves. Material properties come from published data
// (src/materials.js lists the source of each, and marks the few values that had to be estimated).
// Peak cylinder pressure comes from the gas exchange model's pressure trace.
// Part sizes and masses, which grade Honda used, and the load paths are engineering estimates, tuned so a stock engine is safe at its redline and a stock
// bottom end gets marginal around 12–14 psi on pump gas, which is where K20 owners report trouble.

import { clamp, lookup1 } from './tables.js';
import { MATERIALS } from './materials.js';
import { bottomEnd } from './parts.js';
import { rotatingOil } from './thermal.js';
export { MATERIALS };

export const GEOMETRY = {
  boreM: 0.086, strokeM: 0.086, rodM: 0.139, compression: 11.0,     // published
  pinM: 0.022, bigEndM: 0.048,                                       // published (aftermarket parts listed as stock size)
  pistonKg: 0.290, pinKg: 0.085, ringsKg: 0.035, rodSmallEndKg: 0.160, // estimated, for the stock materials
  rodBeamAreaM2: 116e-6,                                             // estimated I-beam section
  rodBoltStressAreaM2: 2 * 39e-6,                                    // two M8 × 0.75 bolts, estimated
  bearingWidthM: 0.017,                                              // estimated
  pistonStressPerBar: 0.55,                                          // MPa of crown/ring-land stress per bar, estimated
  // Block. Published: aluminium block, cast-in iron sleeves, open deck (hence aftermarket "block guards").
  sleeveWallM: 0.004, jacketWallM: 0.005,                            // estimated iron sleeve and aluminium wall around it
  openDeckFactor: 1.25,                                              // estimated extra sleeve stress at the unsupported top of an open deck
  mainWebAreaM2: 1.0e-3, mainWebKt: 2.5,                             // estimated web section and stress concentration at the bolt bosses
  headBolts: 10, headBoltAreaM2: 72.3e-6,                            // 10 × M11 × 1.5 head bolts (tensile stress area from ISO 898)
  headJointFactor: 0.2,                                              // share of the firing load the head bolts feel (stiff joint), estimated
};

// Which material each part is made from. Change them with tune.materials = { piston: 'piston2618', ... }.
export const PARTS = [
  { key: 'piston', name: 'Pistons', category: 'piston', default: 'pistonAl' },
  { key: 'rod', name: 'Connecting rods', category: 'rod', default: 'rodPM' },
  { key: 'rodBolt', name: 'Rod bolts', category: 'rodBolt', default: 'rodBolt' },
  { key: 'bearing', name: 'Rod bearings', category: 'bearing', default: 'bearing' },
  { key: 'crank', name: 'Crankshaft', category: 'crank', default: 'crank' },
  { key: 'gasket', name: 'Head gasket', category: 'gasket', default: 'gasket' },
  { key: 'headBolt', name: 'Head bolts', category: 'headBolt', default: 'headBolt' },
  { key: 'sleeve', name: 'Cylinder sleeves', category: 'sleeve', default: 'sleeve' },
  { key: 'block', name: 'Block (main webs)', category: 'block', default: 'blockAl' },
  { key: 'valvetrain', name: 'Valvetrain', category: 'valveSpring', default: 'valveSpring' },
];

export const materialsFor = category => Object.entries(MATERIALS).filter(([, m]) => m.category === category).map(([id]) => id);
// The alloy picked for a part wins; otherwise the alloy of the aftermarket part fitted (src/parts.js);
// otherwise stock.
export function partMaterial(tune, part) {
  const id = tune?.materials?.[part.key];
  if (MATERIALS[id]?.category === part.category) return MATERIALS[id];
  const fitted = tune?.parts ? bottomEnd(tune).materials[part.key] : null;
  return MATERIALS[fitted]?.category === part.category ? MATERIALS[fitted] : MATERIALS[part.default];
}

// Geometry with the fitted crank, rods and pistons. Stock sleeves get thinner as the bore grows; a
// re-sleeved block (any aftermarket sleeve material) is back to full wall thickness.
export function engineGeometry(tune) {
  const b = bottomEnd(tune);
  if (b.geomStock) return GEOMETRY;
  const resleeved = (tune?.materials?.sleeve ?? 'sleeve') !== 'sleeve';
  return {
    ...GEOMETRY, boreM: b.boreMm / 1000, strokeM: b.strokeMm / 1000, rodM: b.rodMm / 1000, compression: b.compression,
    pinM: b.pinMm / 1000, bigEndM: b.bigEndBearingMm / 1000, pistonKg: b.pistonKg, rodSmallEndKg: b.rodRecipKg,
    sleeveWallM: resleeved ? GEOMETRY.sleeveWallM : Math.max(0.0015, GEOMETRY.sleeveWallM - (b.boreMm - b.block.boreMm) / 2000),
  };
}
const derate = (m, tempC) => (m.strengthVsTempC ? lookup1(m.strengthVsTempC.map(d => d[0]), m.strengthVsTempC.map(d => d[1]), tempC) : 1);
// Goodman: a steady (mean) stress uses up some of the alternating stress the metal can take forever.
const goodman = (ampMPa, meanMPa, m, k = 1) => ampMPa / Math.max(1, m.fatigueMPa * k * (1 - clamp(meanMPa / (m.utsMPa * k), 0, 0.95)));


export function createInternals() {
  return {
    pmaxBar: 0, pistonC: 120, rodCompN: 0, rodTensionN: 0, knockSpike: 1, sleeveMPa: 0, gasketLiftBar: 160,
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
  const compressionBar = 1.013 * charge * engineGeometry(e.tune).compression ** 1.3;
  if (e.revCut || e.dfco || e.boostCut || e.torqueNm === undefined) return compressionBar;
  const spark = clamp(1 + 0.025 * (e.ign - e.mbt), 0.35, 1.3);       // advance raises it, retard lowers it
  const mixture = e.lambda < 1 ? 1 - (1 - e.lambda) * 0.3 : 1 / e.lambda;
  return compressionBar * 3.8 * spark * mixture;
}

// Update loads and fatigue for dt seconds of running. e is the engine state from engine.js; the parts'
// materials come from e.tune.materials.
export function stepInternals(s, e, dt) {
  if (s.failed) return;
  const rpm = Math.max(e.rpm, 0), w = rpm * Math.PI / 30;
  const M = Object.fromEntries(PARTS.map(p => [p.key, partMaterial(e.tune, p)]));
  const G = engineGeometry(e.tune);
  const PISTON_AREA = Math.PI / 4 * G.boreM ** 2, LAMBDA = G.strokeM / 2 / G.rodM;

  // Peak pressure from the cycle model, plus knock: pressure oscillations on top of the peak, getting
  // worse the further past the knock limit.
  const over = Math.max(0, e.ign - e.knockLimit);
  s.knockSpike = e.knocking ? 1 + Math.min(0.15 * (1 + over), 1) : 1;
  const cut = e.revCut || e.dfco || e.boostCut;
  s.pmaxBar = (cut || !Number.isFinite(e.cylPmaxBar) ? peakPressureBar(e) : e.cylPmaxBar) * s.knockSpike;

  // Piston crown temperature: heat from combustion, more when lean or knocking. A piston alloy that
  // conducts heat better runs cooler (relative to A390's published 130 W/m·K).
  const kRel = (M.piston.conductivityWmK ?? 130) / 130;
  const heat = 0.64 * (e.airGs ?? 0) + (e.lambda > 0.85 && e.map > 90 ? 150 * Math.min(e.lambda - 0.85, 0.5) : 0) + (e.knocking ? 40 * (s.knockSpike - 1) / 0.15 : 0);
  // The oil under the crown sets its floor; the oil jets (check balls open at 200 kPa) cool it much better.
  const jets = clamp(((e.oilKpa ?? 300) - (e.thermal?.g.oilJetKpa ?? 200)) / 50, 0, 1);
  const base = (e.oilC ?? 100) + 80 - 30 * jets;
  const tTarget = cut ? base : base + heat / kRel ** 0.5;
  s.pistonC += (tTarget - s.pistonC) * Math.min(1, dt * 0.25);

  // Forces on the rod: gas load down on the firing stroke (inertia helps a little), inertia up at exhaust
  // TDC. Reciprocating mass scales with the piston and rod materials' densities.
  const recipKg = G.pistonKg * (M.piston.densityKgM3 / 2700) + G.pinKg + G.ringsKg + G.rodSmallEndKg * (M.rod.densityKgM3 / 7780);
  const inertiaN = recipKg * (G.strokeM / 2) * w * w * (1 + LAMBDA);
  s.rodCompN = Math.max(0, s.pmaxBar * 1e5 * PISTON_AREA - 0.85 * inertiaN);
  s.rodTensionN = inertiaN;

  // Pistons: crown and ring-land stress against fatigue strength at crown temperature. Brittle cast
  // hypereutectic pistons take knock worse than ductile forgings (elongation from the material data).
  const brittle = (M.piston.elongationPct ?? 1) < 2 ? 1.6 : (M.piston.elongationPct ?? 1) < 5 ? 1 : 0.6;
  const pistonBar = s.pmaxBar / s.knockSpike * (1 + (s.knockSpike - 1) * brittle);
  s.load.piston = G.pistonStressPerBar * pistonBar / (M.piston.fatigueMPa * derate(M.piston, s.pistonC));
  // Rods: compression against yield (with margin for buckling), tension against fatigue.
  s.load.rod = Math.max(s.rodCompN / (G.rodBeamAreaM2 * M.rod.yieldMPa * 1e6 * 0.75), s.rodTensionN / (G.rodBeamAreaM2 * 1e6) / M.rod.fatigueMPa);
  // Rod bolts: stress amplitude in the thread from the inertia load (the joint takes most of it).
  s.load.rodBolt = (0.25 * s.rodTensionN / G.rodBoltStressAreaM2 / 2 / 1e6) / M.rodBolt.threadFatigueMPa;
  // Rod bearing oil film: oil fed through the crank from the mains, thinned by its own shear heating and by
  // a starved supply when oil pressure can't keep up with rpm (thermal.js rotatingOil).
  const lube = rotatingOil(rpm, e.oilKpa ?? 300, e.oilC ?? 100, e.thermal?.g, clamp(s.load.bearing ?? 0.5, 0, 1.5));
  s.lube = lube;
  const film = lube.rods[0].film;
  s.load.bearing = Math.max(s.rodCompN, s.rodTensionN) / (G.bigEndM * G.bearingWidthM) / 1e6 / (M.bearing.loadRatingMPa * film);
  s.load.crank = 0.0035 * (s.rodCompN + 0.5 * s.rodTensionN) / M.crank.fatigueMPa;   // MPa per N at the pin fillet, estimated

  // Head joint: bolts torqued to 75% of yield clamp the head down; the gasket lifts when cylinder
  // pressure overcomes the share of clamp it needs to seal.
  const clampN = G.headBolts / 4 * 0.75 * M.headBolt.yieldMPa * 1e6 * G.headBoltAreaM2;
  s.gasketLiftBar = M.gasket.sealFactor * clampN / PISTON_AREA / 1e5;
  // Boiling coolant lets the head run hot and grow, which unloads the gasket; it gets worse the longer it lasts.
  const overheat = clamp((e.thermal?.overheatS ?? 0) / 20, 0, 1);
  s.load.gasket = s.pmaxBar / s.gasketLiftBar * (1 + 0.6 * overheat);
  const boltAmp = G.headJointFactor * s.pmaxBar * 1e5 * PISTON_AREA / (G.headBolts / 4) / G.headBoltAreaM2 / 2 / 1e6;
  s.load.headBolt = boltAmp / M.headBolt.threadFatigueMPa;

  // Sleeves: cylinder pressure pushes the bore out (hoop stress P·r/t). The iron sleeve and the aluminium
  // around it share the load by stiffness (modulus × wall), and the open deck leaves the top unsupported.
  // When hot, the aluminium grows more than the iron (thermal expansion) and pulls the sleeve outward,
  // which adds a steady stress (half of it assumed relaxed by creep in the aluminium).
  const blockC = (e.ect ?? 90) + 10, sleeveC = (e.ect ?? 90) + 30;
  const Es = M.sleeve.modulusGPa * 1e3, Ea = M.block.modulusGPa * 1e3;   // MPa
  const share = Es * G.sleeveWallM / (Es * G.sleeveWallM + Ea * G.jacketWallM);
  const hoop = s.pmaxBar * 0.1 * (G.boreM / 2) / G.sleeveWallM * share * G.openDeckFactor;
  const thermal = 0.5 * (M.block.thermalExpansionUmMK - M.sleeve.thermalExpansionUmMK) * 1e-6 * (sleeveC - 20) * Es * Ea * G.jacketWallM / (Es * G.sleeveWallM + Ea * G.jacketWallM);
  s.sleeveMPa = hoop + thermal;
  s.load.sleeve = goodman(hoop / 2, hoop / 2 + thermal, M.sleeve);
  // Block: main bearing webs carry the firing load (two webs per crank throw) into the bolt bosses.
  const web = G.mainWebKt * 0.6 * s.rodCompN / G.mainWebAreaM2 / 1e6;
  s.load.block = goodman(web / 2, web / 2, M.block, derate(M.block, blockC));

  // Valvetrain: endurance up to Hondata's 8,600 rpm reflash limit, valve float (load 1.0) at 9,000 rpm.
  const floatRpm = 9000 * (e.highCam ? 1 : 1.08), safeRpm = floatRpm - 400;
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
