// Runs tunes that match public dyno results and prints sim vs published numbers.
//   node tools/validate.mjs
// Sim numbers are crank figures turned into wheel figures with the car's driveline efficiency.
// Sources and whether each tune is confirmed or guessed are in docs/VALIDATION.md.
import { placeholderTune, turboSetup, mbt, knockLimit, CAR } from '../src/k20z3.js';
import { createEngine, stepEngine, hp } from '../src/engine.js';

const NM_LBFT = 0.737562, eff = CAR.drivelineEff;

function wot(tune, rpm) {
  const e = createEngine(tune);
  for (let i = 0; i < 4000; i++) { e.rpm = rpm; stepEngine(e, 1, 0.001); }
  return e;
}
function sweep(tune, from = 2000, to = 8000, stepRpm = 100) {
  const pts = [];
  for (let r = from; r <= to; r += stepRpm) { const e = wot(tune, r); pts.push({ rpm: r, whp: hp(e.torqueNm, r) * eff, wtq: e.torqueNm * NM_LBFT * eff, e }); }
  const pHp = pts.reduce((a, b) => (b.whp > a.whp ? b : a)), pTq = pts.reduce((a, b) => (b.wtq > a.wtq ? b : a));
  return { pts, pHp, pTq };
}
// A careful boosted pump-gas tune: best timing up to 1° under the knock limit, rich on boost.
const grid = (fn) => {
  const rows = [1000, 2000, 3000, 4000, 5000, 6000, 7000, 8000, 9000], cols = [20, 40, 60, 80, 100, 130, 160, 190, 220, 250];
  return { rows, cols, data: rows.map(r => cols.map(m => fn(r, m))) };
};
const boostedTune = (octane, injCc) => {
  const ign = grid((r, m) => Math.min(mbt(r, m), knockLimit(r, m, 40, octane) - 1));
  const lam = grid((r, m) => (m < 80 ? 1 : m < 110 ? 0.88 : 0.78));
  return { ...turboSetup(), octane, injectorCc: injCc, fuelUnits: 'lambda', fuelLow: lam, fuelHigh: lam, ignLow: ign, ignHigh: ign, revLimit: 9000, overboostKpa: 400, torqueLimitNm: Infinity };
};
// Fixed boost from a given rpm up: wastegate spring = target boost, turbo big enough to hold it.
const fixedBoost = (tune, psi, fullAtRpm) => ({ ...tune, turbo: { enabled: true, maxBoostKpa: psi / 0.145, springKpa: psi / 0.145, spoolRpm: fullAtRpm / 1.4 } });

const fmt = (x, d = 0) => x.toFixed(d);
const rows = [];

// 1. Stock car, stock ECU vs DSPORT baseline (Dynojet).
const stock = sweep(placeholderTune(), 2000, 8000);
rows.push(['Stock ECU, stock car', 'DSPORT baseline: 195.7 whp @ 8,100, 143.5 lb-ft @ 6,160 (Dynojet)',
  `${fmt(stock.pHp.whp)} whp @ ${stock.pHp.rpm}, ${fmt(stock.pTq.wtq)} lb-ft @ ${stock.pTq.rpm}`]);

// 2. Hondata reflash (VTEC 5,800 → 4,500) on stock hardware: loses power 4,500–5,600.
const refl = { ...placeholderTune(), vtecRpm: 4500, revLimit: 8600 };
const diffs = [4600, 5000, 5400, 5800, 6200].map(r => `${r}: ${fmt((wot(refl, r).torqueNm - wot(placeholderTune(), r).torqueNm) * NM_LBFT, 1)}`);
rows.push(['Hondata reflash VTEC 4,500 on stock hardware', 'Hondata: loses power 4,500–5,600 rpm', `torque change, lb-ft at the crank: ${diffs.join(', ')}`]);

// 3. PRL kit with Precision 6262, stock internals, 93 octane, 1200 cc: 335 whp @ 5.6 psi, 357 whp @ 7 psi.
for (const [psi, pub] of [[5.6, 335], [7, 357]]) {
  const r = sweep(fixedBoost(boostedTune(93, 1200), psi, 4500), 3000, 8000);
  rows.push([`Precision 6262 @ ${psi} psi, 93 octane`, `JD Tuning: ${pub} whp`, `${fmt(r.pHp.whp)} whp @ ${r.pHp.rpm}, ${fmt(r.pTq.wtq)} lb-ft @ ${r.pTq.rpm}`]);
}

// 4. Kraftwerks supercharger, 12.8 psi at 8,400, 93 octane, 1000 cc: 390 whp and 258 lb-ft @ 8,400.
const sc = fixedBoost(boostedTune(93, 1000), 12.8, 4000);
const sc84 = wot(sc, 8400);
rows.push(['Kraftwerks supercharger @ 12.8 psi, 93 octane', 'DSPORT: 390 whp and 258 lb-ft @ 8,400',
  `${fmt(hp(sc84.torqueNm, 8400) * eff)} whp and ${fmt(sc84.torqueNm * NM_LBFT * eff)} lb-ft @ 8,400 (before supercharger drive losses)`]);

for (const [what, pub, sim] of rows) console.log(`${what}\n  published: ${pub}\n  sim:       ${sim}\n`);
