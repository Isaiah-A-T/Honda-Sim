// Which K-series block, head and oil pump the engine is built from, and whether they go together.
//
// `tune.engine = { block, head, oilPump }` (ids from docs/engines.json; anything missing is the
// K20Z3 the car came with). The block sets bore, stroke, rod length, deck height and the stock
// pistons' compression ratio; the head sets valve sizes, cam lobes and the kind of VTEC; the oil pump
// sets oil delivery and whether the balance shafts are driven. docs/engines.json is built from
// docs/k-series-specs.json by tools/build-engines.py; every entry carries its sources and lists the
// values that had to be estimated.
//
// Fit rules (docs/engines.json _meta.rules):
//   - a head fits a block of its own family (K20 head on K20 block, K24 on K24);
//   - a K20 DOHC VTEC head fits a K24 block with the parts the build guides list;
//   - combinations no source documents are refused, and the block's own head is used instead;
//   - an oil pump fits the blocks its maker or Honda lists, some only with extra parts.

import { ENGINES } from './engines-data.js';
export { ENGINES };

export const BLOCKS = ENGINES.blocks;
export const HEADS = ENGINES.heads;
export const OIL_PUMPS = ENGINES.oilPumps;
export const STOCK_ENGINE = { block: 'K20Z3', head: 'K20Z3', oilPump: 'rbc' };

const byId = (list, id) => list.find(p => p.id === id);

// Can this head go on this block? { works: 'yes' | 'mods' | 'no', needs: [...], note, sources }
export function headFit(head, block) {
  const rule = ENGINES.compatibility.find(r => r.heads.includes(head.id) && r.blocks.includes(block.id));
  if (rule) return { works: rule.works, needs: rule.needs, note: rule.note, sources: rule.sources };
  if (head.family === block.family) {
    const own = head.id === block.head;
    return { works: 'yes', needs: [], note: own ? 'This block\'s own head.' : `Same ${head.family} family, so it bolts on (inferred; not separately documented).`, sources: [] };
  }
  return { works: 'no', needs: [], note: `No documented build puts a ${head.family} ${head.vtec === 'economy' ? 'economy ' : ''}head on a ${block.family} block.`, sources: [] };
}

// Can this oil pump go in this block? { works: 'yes' | 'mods' | 'no', needs: [...] }
export function pumpFit(pump, block) {
  const rule = pump.fits.find(f => f.blocks.includes(block.id));
  if (!rule) return { works: 'no', needs: [], note: `Not listed for the ${block.id}.` };
  return { works: rule.needs.length ? 'mods' : 'yes', needs: rule.needs, note: '' };
}

// The block, head and pump that are actually fitted, after the fit checks.
export function engineSetup(tune) {
  const sel = tune?.engine || {}, issues = [];
  // Saved tunes from before blocks were selectable picked a K24 through the crank list.
  const legacyK24 = !sel.block && tune?.parts?.crank === 'honda-k24-99-hybrid';
  let block = byId(BLOCKS, legacyK24 ? 'K24A2' : sel.block ?? 'K20Z3');
  if (!block) { issues.push(`Unknown block "${sel.block}"; using the K20Z3.`); block = byId(BLOCKS, 'K20Z3'); }
  let head = byId(HEADS, sel.head ?? (legacyK24 ? 'K20Z3' : block.head));
  if (!head) { issues.push(`Unknown head "${sel.head}"; using the ${block.id}'s own.`); head = byId(HEADS, block.head); }
  let fit = headFit(head, block);
  if (fit.works === 'no') {
    issues.push(`The ${head.name} can't go on the ${block.id}: ${fit.note} Using the ${block.id}'s own head.`);
    head = byId(HEADS, block.head); fit = headFit(head, block);
  }
  let pump = byId(OIL_PUMPS, sel.oilPump ?? block.oilPump);
  if (!pump) { issues.push(`Unknown oil pump "${sel.oilPump}"; using the ${block.id}'s own.`); pump = byId(OIL_PUMPS, block.oilPump); }
  let pFit = pumpFit(pump, block);
  if (pFit.works === 'no') {
    issues.push(`The ${pump.name} can't go in the ${block.id}: ${pFit.note} Using the ${block.id}'s own pump.`);
    pump = byId(OIL_PUMPS, block.oilPump); pFit = pumpFit(pump, block);
  }
  const stock = block.id === STOCK_ENGINE.block && head.id === STOCK_ENGINE.head && pump.id === STOCK_ENGINE.oilPump;
  return { block, head, pump, headFit: fit, pumpFit: pFit, issues, stock };
}

// The head's valves and cam lobes in the gas exchange model's terms.
export function headGeometry(head) {
  return { inValveM: head.inValveMm / 1000, exValveM: head.exValveMm / 1000, cams: head.cams };
}

// Balance shafts: two shafts in the oil pump housing spinning at twice crank speed. Taking them out
// (a pump without them) frees about 5-10 bhp by vendors' claims (a4h-tech's FD2 delete kit; Hybrid
// Racing quotes 10-15 whp, no dyno data from either). Midpoint 7.5 hp at 7,800 rpm, with the drag
// growing in proportion to speed (oil shear in the shaft bearings). The stock K20Z3 has them, and the
// friction model is calibrated with them in, so only removing them changes anything.
export const BALANCER_KW_AT_7800 = 5.6;
export function balancerNm(rpm) {
  return BALANCER_KW_AT_7800 * 1000 / (7800 * Math.PI / 30) * Math.max(rpm, 0) / 7800;
}
