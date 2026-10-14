// Bottom end: which block, crank, rods and pistons are in the engine, and the bore, stroke, rod length,
// compression ratio and displacement they add up to. The block (src/engines.js) brings its own stock
// crank, rods and pistons; the parts here replace them.
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
// difference in deck clearance, so the ratio is recomputed from the new swept volume. K20 and K24 heads
// both have 50.5 cc chambers (Wiseco, CP), so changing the head leaves the ratio alone.

import { CATALOG } from './parts-catalog.js';
import { engineSetup, headGeometry } from './engines.js';
export { CATALOG };

const K20_DECK = 212;                       // mm, published (catalog stock notes)
const STOCK_PISTON_KG = 0.290, STOCK_ROD_RECIP_KG = 0.160;   // K20Z3, estimates as in internals.js
const ROD_RECIP_SHARE = 0.32;               // share of a rod's mass counted as reciprocating, estimate
const MAX_PROTRUSION_MM = 0.3;
const MAX_OVERBORE_MM = 1.0;                // bigger than this over the block's bore needs new sleeves (catalog: 87 mm fits a K20, 87.5 needs sleeves)
const LEGACY_K24 = 'honda-k24-99-hybrid';   // the K24 block used to be picked as a crank

const cc = (boreMm, strokeMm) => Math.PI / 4 * boreMm ** 2 * strokeMm / 1000;
const byId = (list, id) => list.find(p => p.id === id);
const pistonMaterial = alloy => (/4032/.test(alloy) ? 'piston4032' : /2618/.test(alloy) ? 'piston2618' : null);
const rodMaterial = m => (/43[34]0|chromoly/i.test(m || '') ? 'rod4340' : null);
const K20Z3_BLOCK = engineSetup({}).block;

// The block's own pistons, rods and crank.
export function stockPiston(block = K20Z3_BLOCK) {
  return {
    id: 'stock', brand: 'Honda', partNumber: 'OEM', name: `Stock ${block.id} pistons (${block.boreMm} mm, ${block.compression.toFixed(1)}:1)`,
    bore_mm: block.boreMm, compressionRatio: block.compression, compressionHeight_mm: block.compressionHeightMm, pinDia_mm: block.pinMm, pistonWeight_g: null,
    forStroke_mm: block.strokeMm, forRodLength_mm: block.rodMm, forDeck_mm: block.deckMm, material: null, sources: block.sources,
  };
}
export function stockCrank(block = K20Z3_BLOCK) {
  return { id: 'stock', name: `Stock ${block.id} crank (${block.strokeMm} mm stroke)`, brand: 'Honda', partNumber: 'OEM', family: block.family,
    stroke_mm: block.strokeMm, deck_mm: block.deckMm, rodLength_mm: block.rodMm, housing_mm: block.bigEndMm, material: null, sources: block.sources };
}
export const STOCK_PISTON = stockPiston();
export const STOCK_ROD = {
  id: 'stock', brand: 'Honda', partNumber: 'OEM', name: 'Stock rods (OEM length for the crank)',
  length_mm: null, bigEnd_mm: null, pinDia_mm: 22, weight_g: null, material: 'powdered-metal forging', sources: CATALOG.stock.sources,
};

// Cranks and stroker kits (all made for the K20 block). A kit that ships with its own pistons or rods locks those choices.
const toda = (id, bore, i, extra = {}) => {
  const c = byId(CATALOG.cranks, id);
  const pick = v => (Array.isArray(v) ? v[i] : v);
  return {
    id: `${id}-${bore}`, crankId: id, name: `Toda ${c.stroke_mm} mm kit, ${bore} mm bore`, brand: 'Toda', partNumber: c.partNumber, family: 'K20',
    stroke_mm: c.stroke_mm, deck_mm: K20_DECK, housing_mm: null, material: null, sources: c.sources, notes: c.notes,
    kitPiston: { name: 'Toda kit pistons', bore_mm: bore, compressionRatio: pick(c.compressionRatio), clearance_mm: 0, alloy: 'forged' },
    kitRod: { name: 'Toda kit rods (I-section chromoly)', material: 'chromoly' },
    estimated: { rodLength: 'Toda does not list rod length; 139 mm assumed (only affects piston speed and side load)',
      pistonWeight: 'not listed; stock weight scaled by bore area', rodWeight: 'not listed; stock rod mass used', ...extra },
  };
};

const AFTERMARKET_CRANKS = [
  (() => {
    const c = byId(CATALOG.cranks, 'bc-k20-92-stroker'), r = byId(CATALOG.rods, 'bc-k20-stroker-140');
    return {
      id: c.id, crankId: c.id, name: 'Brian Crower 92 mm stroker kit (BC5048 + BC6040 rods + BC7048 pistons)', brand: c.brand, partNumber: c.partNumber, family: 'K20',
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
];
export const CRANKS = [stockCrank(), ...AFTERMARKET_CRANKS];
// The crank list for a block: its own crank first.
export const cranksFor = block => [stockCrank(block), ...AFTERMARKET_CRANKS];

export const PISTONS = [STOCK_PISTON, ...CATALOG.pistons.map(p => ({
  ...p, name: `${p.brand} ${p.partNumber} (${p.bore_mm} mm, ${p.compressionRatio}:1)`,
  forDeck_mm: p.forStroke_mm === 99 ? 231.5 : K20_DECK, material: pistonMaterial(p.alloy),
}))];
export const pistonsFor = block => [stockPiston(block), ...PISTONS.slice(1)];
export const RODS = [STOCK_ROD, ...CATALOG.rods.map(r => ({
  ...r, name: `${r.brand} ${r.partNumber} (${r.length_mm} mm, ${r.weight_g ?? '?'} g)`,
  bigEnd_mm: r.bigEnd_mm ?? 51, pinDia_mm: r.pinDia_mm ?? 22, materialId: rodMaterial(r.material),
}))];

const A3_NOTE = 'aftermarket K20 bottom ends don\'t list the K20A3, whose rods and bearings differ';
// Can this crank go in this block? Returns null if it fits, or the reason it doesn't.
export function crankProblem(block, crank) {
  if (crank.id === 'stock') return null;
  if (!block.aftermarketBottomEnd) return A3_NOTE;
  if (crank.family !== block.family) return `made for the ${crank.family} block (${crank.deck_mm} mm deck); the ${block.id} deck is ${block.deckMm} mm`;
  return null;
}
// Can this piston go in with this crank? Returns null if it fits, or the reason it doesn't.
export function pistonProblem(crank, piston, block = K20Z3_BLOCK) {
  if (crank.kitPiston) return piston.id === 'stock' ? null : 'this kit comes with its own pistons';
  if (piston.id === 'stock') return null;
  if (!block.aftermarketBottomEnd) return A3_NOTE;
  if (piston.bore_mm < block.boreMm) return `${piston.bore_mm} mm is smaller than the ${block.id}'s ${block.boreMm} mm bore`;
  const rodMm = crank.rodLength_mm;
  const clearance = crank.deck_mm - (crank.stroke_mm / 2 + rodMm + piston.compressionHeight_mm);
  if (clearance < -MAX_PROTRUSION_MM) return `sticks out ${(-clearance).toFixed(1)} mm and hits the head`;
  if (clearance > 3) return `sits ${clearance.toFixed(1)} mm down the bore`;
  return null;
}
export function rodProblem(crank, rod, piston, block = K20Z3_BLOCK) {
  if (crank.kitRod) return rod.id === 'stock' ? null : 'this kit comes with its own rods';
  if (crank.rodLocked) return rod.id === crank.rodLocked || rod.id === 'stock' ? null : 'this kit uses the BC6040 rods';
  if (rod.id === 'stock') return null;
  if (!block.aftermarketBottomEnd) return A3_NOTE;
  if (rod.length_mm !== crank.rodLength_mm) return `${rod.length_mm} mm long; this crank needs ${crank.rodLength_mm} mm`;
  if (rod.bigEnd_mm !== crank.housing_mm) return `big end ${rod.bigEnd_mm} mm doesn't fit this crank's ${crank.housing_mm} mm journal`;
  if (piston && piston.pinDia_mm && rod.pinDia_mm !== piston.pinDia_mm) return `${rod.pinDia_mm} mm pin; pistons use ${piston.pinDia_mm} mm`;
  return null;
}

// Everything the engine model needs from the engine and parts lists.
// `tune.engine = { block, head, oilPump }` and `tune.parts = { crank, piston, rod }` (ids).
const cache = new WeakMap();
export function bottomEnd(tune) {
  const sel = tune?.parts || {}, eng = tune?.engine;
  const hit = tune?.parts && cache.get(sel);
  if (hit && hit.eng === eng) return hit.b;
  const b = computeBottomEnd(tune || {});
  if (tune?.parts) cache.set(sel, { eng, b });
  return b;
}
function computeBottomEnd(tune) {
  const sel = tune.parts || {}, issues = [], estimated = {};
  const setup = engineSetup(tune);
  const block = setup.block;
  let head = setup.head, headFit = setup.headFit;
  issues.push(...setup.issues);
  Object.assign(estimated, block.estimated);

  const crankId = sel.crank === LEGACY_K24 ? 'stock' : sel.crank;
  let crank = crankId && crankId !== 'stock' ? byId(AFTERMARKET_CRANKS, crankId) : stockCrank(block);
  if (!crank) { issues.push(`Unknown crank "${crankId}"; using stock.`); crank = stockCrank(block); }
  const crankWhy = crankProblem(block, crank);
  if (crankWhy) { issues.push(`${crank.name} can't be used: ${crankWhy}.`); crank = stockCrank(block); }
  Object.assign(estimated, crank.estimated || {});

  const STOCK = stockPiston(block);
  let piston = (sel.piston && sel.piston !== 'stock' && byId(PISTONS, sel.piston)) || STOCK;
  if (crank.kitPiston) piston = STOCK;
  else {
    const why = pistonProblem(crank, piston, block);
    if (why) { issues.push(`${piston.name} can't be used: ${why}.`); piston = STOCK; }
  }
  let rod = byId(RODS, crank.rodLocked || sel.rod) || STOCK_ROD;
  const rodWhy = rodProblem(crank, rod, crank.kitPiston ? null : piston, block);
  if (rodWhy) { issues.push(`${rod.name} can't be used: ${rodWhy}.`); rod = crank.rodLocked ? byId(RODS, crank.rodLocked) : STOCK_ROD; }

  // Stock K24 pistons under a K20 head (Hybrid Racing's K20/K24 build guide): A4 pistons risk hitting
  // the valves, A8 pistons hit them.
  const k20OnK24 = head.family === 'K20' && block.family === 'K24';
  if (k20OnK24 && piston.id === 'stock' && block.pistonsVsK20Head === 'contact') {
    issues.push(`Stock ${block.id} pistons hit the ${head.name}'s valves; fit aftermarket pistons. Using the ${block.id}'s own head.`);
    head = engineSetup({ engine: { block: block.id } }).head; headFit = { works: 'yes', needs: [], note: 'This block\'s own head.', sources: [] };
  } else if (k20OnK24 && piston.id === 'stock' && block.pistonsVsK20Head === 'risk') {
    issues.push(`Stock ${block.id} pistons can touch the ${head.name}'s valves; the build guides use K24A1/A2 or aftermarket pistons.`);
  }

  const strokeMm = crank.stroke_mm;
  const rodMm = rod.length_mm ?? crank.rodLength_mm ?? 139;
  const blockVc = cc(block.boreMm, block.strokeMm) / (block.compression - 1);   // the block's stock clearance volume
  let boreMm, compression, pistonKg, chMm, pistonMat, pinMm;
  if (crank.kitPiston) {
    const k = crank.kitPiston;
    boreMm = k.bore_mm;
    chMm = crank.deck_mm - strokeMm / 2 - rodMm - k.clearance_mm;
    // No published ratio: keep the stock clearance volume (stock head, gasket and dome).
    compression = k.compressionRatio ?? 1 + cc(boreMm, strokeMm) / blockVc;
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
    pistonMat = piston.material;
  }
  const requiresSleeves = boreMm > block.boreMm + MAX_OVERBORE_MM + 1e-9;
  const deckClearanceMm = crank.deck_mm - (strokeMm / 2 + rodMm + chMm);
  const rodRecipKg = rod.weight_g ? rod.weight_g / 1000 * ROD_RECIP_SHARE : STOCK_ROD_RECIP_KG;
  const housingMm = rod.id === 'stock' ? (crank.housing_mm ?? 51) : rod.bigEnd_mm;
  const partsStock = crank.id === 'stock' && piston.id === 'stock' && rod.id === 'stock';
  // Geometry the gas model was calibrated on: the K20Z3 block and head with their own parts.
  const geomStock = partsStock && block.id === 'K20Z3' && head.id === 'K20Z3';
  if (requiresSleeves) issues.push(`${boreMm} mm needs the block re-sleeved. Picking these pistons fits ductile iron sleeves; set stock sleeves under Materials to see them bored ${((boreMm - block.boreMm) / 2).toFixed(2)} mm thinner per side.`);

  return {
    stock: geomStock && setup.pump.id === 'rbc', geomStock, crank, piston, rod, issues, estimated,
    block, head, pump: setup.pump, headFit, pumpFit: setup.pumpFit,
    boreMm, strokeMm, rodMm, deckMm: crank.deck_mm, compressionHeightMm: chMm, deckClearanceMm,
    compression, displacementL: geomStock ? 1.998 : 4 * cc(boreMm, strokeMm) / 1000,
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

// Geometry overrides for the gas exchange model (nothing for the stock K20Z3, so its calibration holds).
export function flowGeometry(tune) {
  const b = bottomEnd(tune);
  if (b.geomStock) return {};
  return { boreM: b.boreMm / 1000, strokeM: b.strokeMm / 1000, rodM: b.rodMm / 1000, compression: b.compression, ...headGeometry(b.head) };
}
