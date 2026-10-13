// Bottom end: which crank, rods and pistons are in the engine, and the bore, stroke, rod length,
// compression ratio and displacement they add up to.
//
// Every aftermarket part comes from docs/parts-catalog.json: real, purchasable parts with the
// manufacturer's or a major retailer's page as the source. Where a listing leaves a number out, the
// value used here is derived from the geometry or copied from stock, and listed in `estimated`.
//
// How the parts fit together:
//   deck height = stroke/2 + rod length + piston compression height + deck clearance
// A piston that would stick out of the bore by more than 0.3 mm hits the head, so it can't be fitted.
// One that sits down in the bore leaves extra clearance volume and loses compression. Rods must match
// the crank's rod journal (big-end housing bore) and the piston's wrist pin.
//
// Compression ratio: each piston's advertised ratio fixes its clearance volume for the engine it was
// made for (its stroke, rod and deck). In another combination the clearance volume changes only by the
// difference in deck clearance, so the ratio is recomputed from the new swept volume.

import { CATALOG } from './parts-catalog.js';
export { CATALOG };

const K20_DECK = 212, K24_DECK = 231.5;     // mm, published (catalog stock notes)
const STOCK_PISTON_KG = 0.290, STOCK_ROD_RECIP_KG = 0.160;   // estimates, as in internals.js
const ROD_RECIP_SHARE = 0.32;               // share of a rod's mass counted as reciprocating, estimate
const MAX_PROTRUSION_MM = 0.3;

const cc = (boreMm, strokeMm) => Math.PI / 4 * boreMm ** 2 * strokeMm / 1000;
const byId = (list, id) => list.find(p => p.id === id);
const pistonMaterial = alloy => (/4032/.test(alloy) ? 'piston4032' : /2618/.test(alloy) ? 'piston2618' : null);
const rodMaterial = m => (/43[34]0|chromoly/i.test(m || '') ? 'rod4340' : null);

export const STOCK_PISTON = {
  id: 'stock', brand: 'Honda', partNumber: 'OEM', name: 'Stock K20Z3 pistons (86 mm, 11.0:1)',
  bore_mm: 86, compressionRatio: 11.0, compressionHeight_mm: 30, pinDia_mm: 22, pistonWeight_g: null,
  forStroke_mm: 86, forRodLength_mm: 139, forDeck_mm: K20_DECK, requiresSleeves: false, material: null,
  sources: CATALOG.stock.sources,
};
export const STOCK_ROD = {
  id: 'stock', brand: 'Honda', partNumber: 'OEM', name: 'Stock rods (OEM length for the crank)',
  length_mm: null, bigEnd_mm: null, pinDia_mm: 22, weight_g: null, material: 'powdered-metal forging', sources: CATALOG.stock.sources,
};

// Cranks and stroker kits. A kit that ships with its own pistons or rods locks those choices.
const toda = (id, bore, i, extra = {}) => {
  const c = byId(CATALOG.cranks, id);
  const pick = v => (Array.isArray(v) ? v[i] : v);
  return {
    id: `${id}-${bore}`, crankId: id, name: `Toda ${c.stroke_mm} mm kit, ${bore} mm bore`, brand: 'Toda', partNumber: c.partNumber,
    stroke_mm: c.stroke_mm, deck_mm: K20_DECK, housing_mm: null, material: null, sources: c.sources, notes: c.notes,
    kitPiston: { name: 'Toda kit pistons', bore_mm: bore, compressionRatio: pick(c.compressionRatio), clearance_mm: 0, alloy: 'forged' },
    kitRod: { name: 'Toda kit rods (I-section chromoly)', material: 'chromoly' },
    estimated: { rodLength: 'Toda does not list rod length; 139 mm assumed (only affects piston speed and side load)',
      pistonWeight: 'not listed; stock weight scaled by bore area', rodWeight: 'not listed; stock rod mass used', ...extra },
  };
};

export const CRANKS = [
  { id: 'stock', name: 'Stock K20Z3 crank (86 mm stroke)', brand: 'Honda', partNumber: 'OEM', stroke_mm: 86, deck_mm: K20_DECK,
    rodLength_mm: 139, housing_mm: 51, material: null, sources: CATALOG.stock.sources },
  (() => {
    const c = byId(CATALOG.cranks, 'bc-k20-92-stroker'), r = byId(CATALOG.rods, 'bc-k20-stroker-140');
    return {
      id: c.id, crankId: c.id, name: 'Brian Crower 92 mm stroker kit (BC5048 + BC6040 rods + BC7048 pistons)', brand: c.brand, partNumber: c.partNumber,
      stroke_mm: c.stroke_mm, rodLength_mm: c.rodLength_mm, deck_mm: K20_DECK, housing_mm: r.bigEnd_mm, material: 'crank4340',
      rodLocked: r.id, sources: c.sources, notes: c.notes,
      kitPiston: { name: 'BC7048 custom pistons', bore_mm: 86, compressionRatio: null, clearance_mm: 0, pin_mm: r.pinDia_mm, alloy: '2618' },
      estimated: {
        bore: 'BC7048 bore not published; stock 86 mm assumed',
        compressionRatio: 'not published; stock combustion chamber volume assumed, so the longer stroke raises it',
        pistonAlloy: 'BC7048 alloy not published; 2618 forging assumed', pistonWeight: 'not listed; stock weight used',
      },
    };
  })(),
  toda('toda-k20-90.7-hc', 86.0, 0), toda('toda-k20-90.7-hc', 86.5, 1), toda('toda-k20-90.7-hc', 87.0, 2),
  toda('toda-k20-90.7-lc', 86.5, 0),
  toda('toda-k20-84-short', 87.0, 0, { compressionRatio: 'Toda quotes this ratio with its GK head; with the stock head it would be lower' }),
  toda('toda-k20-84-short', 87.5, 1, { compressionRatio: 'Toda quotes this ratio with its GK head; with the stock head it would be lower' }),
  (() => {
    const c = byId(CATALOG.cranks, 'honda-k24-99-hybrid');
    return {
      id: c.id, crankId: c.id, name: 'K24 block + 99 mm crank + 152 mm rods ("K24/K20" with the K20Z3 head)', brand: c.brand, partNumber: c.partNumber,
      stroke_mm: c.stroke_mm, rodLength_mm: c.rodLength_mm, deck_mm: K24_DECK, housing_mm: 51, material: null, block: 'K24A',
      sources: c.sources, notes: c.notes,
      estimated: { rods: 'stock K24 rods (152 mm) used; no aftermarket 152 mm rods in the catalog yet' },
    };
  })(),
];

export const PISTONS = [STOCK_PISTON, ...CATALOG.pistons.map(p => ({
  ...p, name: `${p.brand} ${p.partNumber} (${p.bore_mm} mm, ${p.compressionRatio}:1)`,
  forDeck_mm: p.forStroke_mm === 99 ? K24_DECK : K20_DECK, material: pistonMaterial(p.alloy),
}))];
export const RODS = [STOCK_ROD, ...CATALOG.rods.map(r => ({
  ...r, name: `${r.brand} ${r.partNumber} (${r.length_mm} mm, ${r.weight_g ?? '?'} g)`,
  bigEnd_mm: r.bigEnd_mm ?? 51, pinDia_mm: r.pinDia_mm ?? 22, materialId: rodMaterial(r.material),
}))];

// Can this piston go in with this crank? Returns null if it fits, or the reason it doesn't.
export function pistonProblem(crank, piston) {
  if (crank.kitPiston) return piston.id === 'stock' ? null : 'this kit comes with its own pistons';
  const rodMm = crank.rodLength_mm;
  const clearance = crank.deck_mm - (crank.stroke_mm / 2 + rodMm + piston.compressionHeight_mm);
  if (clearance < -MAX_PROTRUSION_MM) return `sticks out ${(-clearance).toFixed(1)} mm and hits the head`;
  if (clearance > 3) return `sits ${clearance.toFixed(1)} mm down the bore`;
  return null;
}
export function rodProblem(crank, rod, piston) {
  if (crank.kitRod) return rod.id === 'stock' ? null : 'this kit comes with its own rods';
  if (crank.rodLocked) return rod.id === crank.rodLocked || rod.id === 'stock' ? null : 'this kit uses the BC6040 rods';
  if (rod.id === 'stock') return null;
  if (rod.length_mm !== crank.rodLength_mm) return `${rod.length_mm} mm long; this crank needs ${crank.rodLength_mm} mm`;
  if (rod.bigEnd_mm !== crank.housing_mm) return `big end ${rod.bigEnd_mm} mm doesn't fit this crank's ${crank.housing_mm} mm journal`;
  if (piston && piston.pinDia_mm && rod.pinDia_mm !== piston.pinDia_mm) return `${rod.pinDia_mm} mm pin; pistons use ${piston.pinDia_mm} mm`;
  return null;
}

// Everything the engine model needs from the parts list. `tune.parts = { crank, piston, rod }` (ids).
const cache = new WeakMap();
export function bottomEnd(tune) {
  const sel = tune?.parts || {};
  if (cache.has(sel)) return cache.get(sel);
  const b = computeBottomEnd(sel);
  if (tune?.parts) cache.set(sel, b);
  return b;
}
function computeBottomEnd(sel) {
  const issues = [], estimated = {};
  const crank = byId(CRANKS, sel.crank) || CRANKS[0];
  if (sel.crank && crank.id !== sel.crank) issues.push(`Unknown crank "${sel.crank}"; using stock.`);
  Object.assign(estimated, crank.estimated || {});

  let piston = byId(PISTONS, sel.piston) || STOCK_PISTON;
  if (crank.kitPiston) piston = STOCK_PISTON;
  else {
    const why = pistonProblem(crank, piston);
    if (why) {
      issues.push(`${piston.name} can't be used: ${why}.`);
      piston = PISTONS.find(p => !pistonProblem(crank, p));
    }
  }
  let rod = byId(RODS, crank.rodLocked || sel.rod) || STOCK_ROD;
  const rodWhy = rodProblem(crank, rod, crank.kitPiston ? null : piston);
  if (rodWhy) { issues.push(`${rod.name} can't be used: ${rodWhy}.`); rod = crank.rodLocked ? byId(RODS, crank.rodLocked) : STOCK_ROD; }

  const strokeMm = crank.stroke_mm;
  const rodMm = rod.length_mm ?? crank.rodLength_mm ?? 139;
  let boreMm, compression, pistonKg, chMm, pistonMat, pinMm, requiresSleeves = false;
  if (crank.kitPiston) {
    const k = crank.kitPiston;
    boreMm = k.bore_mm;
    chMm = crank.deck_mm - strokeMm / 2 - rodMm - k.clearance_mm;
    // No published ratio: keep the stock clearance volume (stock head, gasket and dome).
    compression = k.compressionRatio ?? 1 + cc(boreMm, strokeMm) / (cc(86, 86) / (11 - 1));
    pistonKg = STOCK_PISTON_KG * (boreMm / 86) ** 2;
    pistonMat = pistonMaterial(k.alloy); pinMm = k.pin_mm ?? 22;
  } else {
    boreMm = piston.bore_mm; chMm = piston.compressionHeight_mm; pinMm = piston.pinDia_mm ?? 22;
    const vcDesign = cc(boreMm, piston.forStroke_mm) / (piston.compressionRatio - 1);
    const designClear = piston.forDeck_mm - (piston.forStroke_mm / 2 + piston.forRodLength_mm + chMm);
    const clear = crank.deck_mm - (strokeMm / 2 + rodMm + chMm);
    const vc = vcDesign + cc(boreMm, clear - designClear);
    compression = 1 + cc(boreMm, strokeMm) / vc;
    pistonKg = piston.pistonWeight_g ? piston.pistonWeight_g / 1000 : STOCK_PISTON_KG * (boreMm / 86) ** 2;
    if (piston.id !== 'stock' && !piston.pistonWeight_g) estimated.pistonWeight = 'not listed; stock weight scaled by bore area';
    pistonMat = piston.material; requiresSleeves = !!piston.requiresSleeves;
  }
  const deckClearanceMm = crank.deck_mm - (strokeMm / 2 + rodMm + chMm);
  const rodRecipKg = rod.weight_g ? rod.weight_g / 1000 * ROD_RECIP_SHARE : STOCK_ROD_RECIP_KG;
  const housingMm = rod.id === 'stock' ? (crank.housing_mm ?? 51) : rod.bigEnd_mm;
  const isStock = crank.id === 'stock' && piston.id === 'stock' && rod.id === 'stock';
  if (requiresSleeves) issues.push(`${boreMm} mm needs the block re-sleeved. Picking these pistons fits ductile iron sleeves; set stock sleeves under Materials to see them bored ${((boreMm - 86) / 2).toFixed(2)} mm thinner per side.`);

  return {
    stock: isStock, crank, piston, rod, issues, estimated,
    boreMm, strokeMm, rodMm, deckMm: crank.deck_mm, compressionHeightMm: chMm, deckClearanceMm,
    compression, displacementL: isStock ? 1.998 : 4 * cc(boreMm, strokeMm) / 1000,
    pistonKg, rodRecipKg, pinMm, bigEndBearingMm: housingMm - 3, requiresSleeves,
    pistonName: crank.kitPiston ? crank.kitPiston.name : piston.name,
    rodName: crank.kitRod ? crank.kitRod.name : rod.name,
    materials: {
      piston: pistonMat || null,
      rod: crank.kitRod ? rodMaterial(crank.kitRod.material) : rod.materialId || null,
      crank: crank.material || null,
    },
  };
}

// Geometry overrides for the gas exchange model (nothing for the stock engine, so its calibration holds).
export function flowGeometry(tune) {
  const b = bottomEnd(tune);
  if (b.stock) return {};
  return { boreM: b.boreMm / 1000, strokeM: b.strokeMm / 1000, rodM: b.rodMm / 1000, compression: b.compression };
}
