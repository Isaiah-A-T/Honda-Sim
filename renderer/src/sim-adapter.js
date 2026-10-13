// Turns Honda Sim's state into the renderer's state, including how hard each part is working.
//
//   import { stateFromSim } from './renderer/src/sim-adapter.js';
//   view.setState(stateFromSim(sim, { materials }));   // once per frame, after step(sim, ...)
//
// materials is the { part: materialId } map from the materials view (materials.js). Until the sim models
// materials itself, a stronger part simply divides that part's stress by its rough relative strength.
//
// When the sim runs its internals model (sim.internals.load: piston, rod, rodBolt, bearing, crank, gasket,
// valvetrain), those loads replace the estimates for the parts they cover.
//
// The stress numbers are rough indicators, 0–1 with 1 at the limit, not a failure model. They lean on the
// same limits the sim already uses: the tune's torque warning (stock rods), clutch capacity, rev limit,
// overboost cut and the knock model.

import { materialOf } from './materials.js';

const clamp = (x, a, b) => Math.min(b, Math.max(a, x));

export function stateFromSim(sim, { materials = {} } = {}) {
  const e = sim.eng, t = e.tune, running = !!sim.running;
  const tq = running ? Math.max(e.torqueNm, 0) : 0;
  const rodLimit = t.torqueLimitNm ?? 350;                 // stock K20 rod figure the sim's turbo preset uses
  const revRef = Math.max(t.revLimit ?? 8000, 6000) + 600; // valve float and rod inertia grow with rpm²
  const spin = (e.rpm / revRef) ** 2;
  const turboOn = !!t.turbo?.enabled;
  const knockHit = e.knocking ? 0.35 : 0;
  const retardHeat = Math.max(0, (e.mbt ?? e.ign) - e.ign) / 20;   // late spark sends heat out the exhaust
  const lean = Math.max(0, e.lambda - 0.9);
  const firing = running && !e.revCut && !e.dfco && !e.boostCut;

  const stress = {
    rods: Math.max(tq / rodLimit, spin * 0.8),
    pistons: tq / rodLimit * 0.8 + knockHit + e.knockRetard / 8 * 0.25,
    crank: Math.max(tq / rodLimit * 0.75, spin * 0.65),
    valves: spin,
    cams: spin * 0.6,
    head: knockHit + clamp((e.iat - 25) / 60, 0, 0.5) + (firing ? e.map / 400 : 0),
    block: tq / rodLimit * 0.45,
    exhaust: firing ? clamp(e.rpm / 8000 * e.map / 100 * 0.55 + (retardHeat * 0.4 + lean * 1.5) * clamp(e.map / 100, 0, 1), 0, 1.3) : 0,
    intake: turboOn && t.overboostKpa ? clamp((e.map - 101) / (t.overboostKpa - 101), 0, 1.3) : 0,
    turbo: turboOn ? Math.max(0, e.boost) / (t.turbo.maxBoostKpa || 150) : 0,
    clutch: tq / (t.clutchNm || 320),
    flywheel: spin * 0.5,
  };
  for (const k of Object.keys(stress)) stress[k] /= materialOf(k, materials)?.strength ?? 1;

  const L = sim.internals?.load;
  if (L) {
    stress.pistons = L.piston;
    stress.rods = Math.max(L.rod, L.rodBolt, L.bearing);
    stress.crank = L.crank;
    stress.head = Math.max(L.gasket, stress.head);
    stress.valves = L.valvetrain;
    stress.cams = L.valvetrain * 0.75;
  }
  const D = sim.internals?.damage;
  if (D) {                                     // a broken part shows past its limit
    const broken = { pistons: ['piston'], rods: ['rod', 'rodBolt', 'bearing'], crank: ['crank'], head: ['gasket'], valves: ['valvetrain'] };
    for (const [k, keys] of Object.entries(broken)) if (keys.some(x => D[x] >= 1)) stress[k] = Math.max(stress[k], 1.3);
  }

  return {
    rpm: e.rpm,
    running,
    throttle: clamp(e.tps / 100, 0, 1),
    mapKpa: e.map,
    boostKpa: Math.max(0, e.boost),
    vtec: !!e.highCam,
    ignDeg: e.ign,
    knock: !!e.knocking,
    fuelCut: !!(e.revCut || e.dfco || e.boostCut),
    lambda: e.lambda > 5 ? 1 : e.lambda,
    turbo: turboOn,
    airGs: e.airGs ?? 0,
    vtcDeg: e.vtcDeg ?? e.vtc ?? 0,      // the sim has no VTC model yet; 0 until it does
    stress,
    materials,
  };
}
