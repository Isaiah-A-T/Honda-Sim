# Builds src/materials.js from docs/materials-sources.json (published values, with sources) plus the
# estimates the model needs where no published value was found. Estimates are listed per material.
#   python3 tools/build-materials.py
import json, pathlib
root = pathlib.Path(__file__).resolve().parents[2]
src = json.loads((root / 'docs/materials-sources.json').read_text())

NAMES = {
  'pistonAl': ('Cast hypereutectic aluminium A390 (stock-type cast piston)', 'piston'),
  'piston4032': ('Forged aluminium 4032-T6 (low-expansion forged piston)', 'piston'),
  'piston2618': ('Forged aluminium 2618-T61 (high-strength forged piston)', 'piston'),
  'rodPM': ('Powder-forged steel Fe-Cu-C (stock rods)', 'rod'),
  'rod4340': ('AISI 4340 forged steel, quenched and tempered', 'rod'),
  'rod300M': ('300M steel', 'rod'),
  'rodTitanium': ('Titanium Ti-6Al-4V', 'rod'),
  'rodBolt': ('Alloy steel bolt, property class 10.9 (stock)', 'rodBolt'),
  'boltARP2000': ('ARP2000', 'rodBolt'),
  'boltARPL19': ('ARP L19', 'rodBolt'),
  'bearing': ('Aluminium-tin bimetal shell (stock)', 'bearing'),
  'bearingTriMetal': ('Copper-lead tri-metal race shell', 'bearing'),
  'crank': ('38MnVS6 micro-alloyed forged steel (stock-type)', 'crank'),
  'crank4340': ('4340 billet steel', 'crank'),
  'crankEN40B': ('EN40B nitriding steel', 'crank'),
  'gasket': ('Multi-layer steel, AISI 301 spring temper (stock)', 'gasket'),
  'gasketCopper': ('Solid copper C11000, annealed', 'gasket'),
  'valveSpring': ('Chrome-silicon spring wire SAE 9254', 'valveSpring'),
  'blockAl': ('Die-cast aluminium 383 / ADC12 (assumed stock grade)', 'block'),
  'sleeve': ('Grey cast iron, ASTM A48 class 30 (stock-type)', 'sleeve'),
  'sleeveDuctile': ('Ductile iron (aftermarket sleeves)', 'sleeve'),
  'headBolt': ('Alloy steel bolt, property class 10.9 (stock)', 'headBolt'),
  'headStudARP2000': ('ARP2000 head studs', 'headBolt'),
}

# Values the model needs that the sources didn't give. (value, why)
EST = {
  'pistonAl': {'strengthVsTempC': None},
  'piston4032': {'strengthVsTempC': ([[25, 1.0], [150, 0.88], [205, 0.75], [260, 0.6], [315, 0.45], [370, 0.33]], 'not found; a 12% Si forging falls off like A390 but a little faster')},
  'piston2618': {'strengthVsTempC': ([[20, 1.0], [150, 0.92], [204, 0.8], [260, 0.66], [315, 0.52], [370, 0.4]], 'only 204 °C point found; extended along the usual 2618 curve')},
  'rodPM': {'densityKgM3': (7780, 'steel; machinability blanks 7.74–7.80 g/cm³'), 'modulusGPa': (200, 'typical steel'), 'utsMPa': (900, 'not published for this grade; about 1.25 × yield'),
            'fatigueMPa': (340, 'not published; about 0.38 × tensile, typical for powder-forged steel'), 'thermalExpansionUmMK': (12, 'typical steel')},
  'rod4340': {}, 'rodTitanium': {},
  'rod300M': {'fatigueMPa': (770, 'not found; about 0.4 × tensile'), 'thermalExpansionUmMK': (12, 'typical steel')},
  'rodBolt': {}, 'headBolt': {},
  'boltARP2000': {'yieldMPa': (1330, 'ARP gives clamp/tensile only; about 0.88 × tensile')},
  'headStudARP2000': {'yieldMPa': (1330, 'ARP gives clamp/tensile only; about 0.88 × tensile')},
  'boltARPL19': {'yieldMPa': (1610, 'ARP gives tensile only; about 0.9 × tensile'), 'densityKgM3': (7800, 'steel'), 'modulusGPa': (200, 'steel'), 'thermalExpansionUmMK': (12, 'steel')},
  'bearing': {'densityKgM3': (2900, 'Al-Sn range 2.6–2.9 g/cm³'), 'modulusGPa': (70, 'Al-Sn range 69–79 GPa')},
  'bearingTriMetal': {'densityKgM3': (8900, 'Cu-Pb range 7.5–9.3 g/cm³'), 'modulusGPa': (120, 'Cu-Pb range 100–150 GPa')},
  'crank': {'fatigueMPa': (400, 'not published; about 0.5 × tensile at a rolled pin fillet')},
  'crank4340': {},
  'crankEN40B': {'fatigueMPa': (550, 'not found; nitrided surface, about 0.65 × tensile'), 'densityKgM3': (7850, 'steel'), 'modulusGPa': (210, 'steel'), 'thermalExpansionUmMK': (12, 'steel')},
  'sleeveDuctile': {'fatigueMPa': (270, 'not found; about 0.45 × tensile, typical for ductile iron')},
  'valveSpring': {'fatigueMPa': (650, 'not found; typical shot-peened Cr-Si valve spring torsional endurance')},
  'blockAl': {'strengthVsTempC': ([[20, 1.0], [100, 0.95], [150, 0.86], [200, 0.72], [250, 0.55]], 'not found for 383; typical Al-Si-Cu die-cast curve')},
}

# What sets each part's capacity in the model, and load ratings that aren't material constants.
MODEL = {
  'bearing': {'loadRatingMPa': (85, 'peak load on the projected area; bearing makers rate Al-Sn below tri-metal, value is an estimate')},
  'bearingTriMetal': {'loadRatingMPa': (110, 'peak load on the projected area; estimate')},
  'gasket': {'sealFactor': (0.73, 'share of head-bolt clamp the gasket needs to keep sealing (MLS); estimate')},
  'gasketCopper': {'sealFactor': (0.80, 'estimate; copper is usually run with O-ringed bores')},
  'rodBolt': {'threadFatigueMPa': (70, 'rolled thread, rolled before heat treatment; stress amplitude, estimate')},
  'headBolt': {'threadFatigueMPa': (70, 'as above')},
  'boltARP2000': {'threadFatigueMPa': (130, 'threads rolled after heat treatment; ARP says this gives several times the fatigue life; estimate')},
  'headStudARP2000': {'threadFatigueMPa': (130, 'as above')},
  'boltARPL19': {'threadFatigueMPa': (150, 'estimate, as above')},
}

FIELDS = ['densityKgM3', 'modulusGPa', 'poisson', 'yieldMPa', 'utsMPa', 'fatigueMPa', 'elongationPct', 'hardness',
          'thermalExpansionUmMK', 'conductivityWmK', 'specificHeatJkgK', 'meltingOrSolidusC', 'maxServiceTempC', 'strengthVsTempC']
out = {}
for key, (name, cat) in NAMES.items():
  s = src.get(key, {})
  m = {'name': name, 'category': cat}
  est = {}
  for f in FIELDS:
    if s.get(f) is not None: m[f] = s[f]
  for f, v in {**EST.get(key, {}), **MODEL.get(key, {})}.items():
    if v is None: continue
    if f in m and f not in MODEL.get(key, {}): continue   # published value wins
    m[f] = v[0]; est[f] = v[1]
  if s.get('fatigueCondition'): m['fatigueNote'] = s['fatigueCondition']
  m['estimated'] = est
  m['sources'] = s.get('source') or []
  out[key] = m

js = ['// Material library: published properties with their sources, and the estimates the model needs where',
      '// no published value was found (listed in each entry\'s `estimated`, with the reason).',
      '// Generated by tools/build-materials.py from docs/materials-sources.json; edit those, not this file.',
      '', 'export const MATERIALS = ' + json.dumps(out, indent=2, ensure_ascii=False) + ';', '']
(root / 'web/src/materials.js').write_text('\n'.join(js))
print(len(out), 'materials')
