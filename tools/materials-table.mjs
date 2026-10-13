// Prints the material library as a Markdown table for docs/INTERNALS.md.
import { MATERIALS } from '../src/materials.js';
const f = (m, k, d = 0) => (m[k] == null ? '–' : `${typeof m[k] === 'number' ? m[k].toFixed(d) : m[k]}${m.estimated?.[k] ? '*' : ''}`);
console.log('| Part | Alloy | Density kg/m³ | Modulus GPa | Yield MPa | Tensile MPa | Fatigue MPa | Elong. % | Expansion µm/m·K | Conductivity W/m·K | Sources |');
console.log('|---|---|---|---|---|---|---|---|---|---|---|');
for (const m of Object.values(MATERIALS)) {
  const src = m.sources.map((u, i) => `[${i + 1}](${u})`).join(' ');
  console.log(`| ${m.category} | ${m.name} | ${f(m, 'densityKgM3')} | ${f(m, 'modulusGPa')} | ${f(m, 'yieldMPa')} | ${f(m, 'utsMPa')} | ${f(m, 'fatigueMPa')} | ${f(m, 'elongationPct', 1)} | ${f(m, 'thermalExpansionUmMK', 1)} | ${f(m, 'conductivityWmK')} | ${src} |`);
}
