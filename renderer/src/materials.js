// Material choices per part, for the materials view. Colours group materials by family so the view reads at a
// glance. `strength` is a rough relative figure (stock = 1) the demo uses to scale stress; it is a placeholder
// until the sim models materials itself, not engineering data.

export const FAMILIES = {
  aluminum:  { label: 'Aluminium', color: 0x4f97e3 },
  steel:     { label: 'Steel', color: 0x8d97a4 },
  chromoly:  { label: 'Chromoly 4340', color: 0xe0843f },
  titanium:  { label: 'Titanium', color: 0xa67ad6 },
  nickel:    { label: 'Inconel', color: 0xd4b13a },
  iron:      { label: 'Iron', color: 0x7a6656 },
  stainless: { label: 'Stainless', color: 0x3fb59a },
  friction:  { label: 'Friction material', color: 0xc75b7a },
};

export const MATERIALS = {
  pistons: { label: 'Pistons', options: [
    { id: 'stock-cast', label: 'Stock cast', family: 'aluminum', strength: 1 },
    { id: 'forged-4032', label: 'Forged 4032', family: 'aluminum', strength: 1.35 },
    { id: 'forged-2618', label: 'Forged 2618', family: 'aluminum', strength: 1.5 },
  ] },
  rods: { label: 'Connecting rods', options: [
    { id: 'stock', label: 'Stock steel I-beam', family: 'steel', strength: 1 },
    { id: '4340-h', label: '4340 H-beam', family: 'chromoly', strength: 1.7 },
    { id: '4340-i', label: '4340 I-beam', family: 'chromoly', strength: 1.8 },
    { id: 'ti', label: 'Titanium', family: 'titanium', strength: 1.6 },
  ] },
  crank: { label: 'Crankshaft', options: [
    { id: 'stock', label: 'Stock forged steel', family: 'steel', strength: 1 },
    { id: 'billet', label: 'Billet 4340', family: 'chromoly', strength: 1.4 },
  ] },
  valves: { label: 'Valves', options: [
    { id: 'stock', label: 'Stock steel', family: 'steel', strength: 1 },
    { id: 'ss', label: 'Stainless', family: 'stainless', strength: 1.1 },
    { id: 'inconel', label: 'Inconel', family: 'nickel', strength: 1.3 },
    { id: 'ti', label: 'Titanium (lighter, less float)', family: 'titanium', strength: 1.25 },
  ] },
  cams: { label: 'Camshafts', options: [
    { id: 'stock', label: 'Stock chilled iron', family: 'iron', strength: 1 },
    { id: 'billet', label: 'Billet steel', family: 'steel', strength: 1.2 },
  ] },
  block: { label: 'Block / sleeves', options: [
    { id: 'stock', label: 'Stock aluminium, cast-in iron liners', family: 'aluminum', strength: 1 },
    { id: 'sleeved', label: 'Ductile iron sleeves', family: 'iron', strength: 1.4 },
    { id: 'closed', label: 'Closed-deck sleeves', family: 'iron', strength: 1.6 },
  ] },
  head: { label: 'Cylinder head', options: [
    { id: 'stock', label: 'Stock aluminium', family: 'aluminum', strength: 1 },
  ] },
  exhaust: { label: 'Exhaust manifold', options: [
    { id: 'mild', label: 'Mild steel', family: 'steel', strength: 1 },
    { id: '304', label: '304 stainless', family: 'stainless', strength: 1.2 },
    { id: '321', label: '321 stainless', family: 'stainless', strength: 1.3 },
    { id: 'inconel', label: 'Inconel', family: 'nickel', strength: 1.6 },
  ] },
  turbo: { label: 'Turbo housings', options: [
    { id: 'cast-iron', label: 'Cast iron', family: 'iron', strength: 1 },
    { id: 'ni-resist', label: 'Ni-resist', family: 'nickel', strength: 1.2 },
  ] },
  flywheel: { label: 'Flywheel', options: [
    { id: 'stock', label: 'Stock steel', family: 'steel', strength: 1 },
    { id: 'chromoly', label: 'Lightweight chromoly', family: 'chromoly', strength: 1.2 },
    { id: 'alu', label: 'Aluminium', family: 'aluminum', strength: 0.9 },
  ] },
  clutch: { label: 'Clutch', options: [
    { id: 'stock', label: 'Stock organic', family: 'friction', strength: 1 },
    { id: 'stage2', label: 'Stage 2 organic', family: 'friction', strength: 1.3 },
    { id: 'stage3', label: 'Stage 3 ceramic puck', family: 'friction', strength: 1.6 },
  ] },
};

export const defaultMaterials = () => Object.fromEntries(Object.entries(MATERIALS).map(([k, v]) => [k, v.options[0].id]));

export function materialOf(key, choices = {}) {
  const part = MATERIALS[key];
  if (!part) return null;
  return part.options.find(o => o.id === choices[key]) ?? part.options[0];
}
