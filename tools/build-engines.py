#!/usr/bin/env python3
"""Writes docs/engines.json, the sim's K-series heads, blocks and oil pumps, from docs/k-series-specs.json.

Published numbers come from the specs file (its sources are copied onto each entry). Anything the
specs file has as null gets a value here only if the sim needs one, and that value is listed in
the entry's `estimated` with the reason. Cam lobes are estimates for every head (Honda publishes
none; see the K20Z3 lobes in src/gasflow.js), shaped from what is known about each head."""
import json, pathlib
root = pathlib.Path(__file__).resolve().parent.parent
spec = json.loads((root / 'docs/k-series-specs.json').read_text())
E_ = spec['engines']

def srcs(o):
    s = []
    def walk(o):
        if isinstance(o, dict):
            for k, v in o.items():
                if k == 'sources' and isinstance(v, list):
                    for u in v:
                        if u not in s: s.append(u)
                else: walk(v)
        elif isinstance(o, list):
            for v in o: walk(v)
    walk(o); return s

def rating(hp, hp_rpm, lbft, tq_rpm, redline, limit=None):
    return {'hp': hp, 'hpRpm': hp_rpm, 'torqueLbft': lbft, 'torqueRpm': tq_rpm, 'redlineRpm': redline, 'revLimitRpm': limit}

def block(code, key, name, family, series, bore, stroke, rod, deck, cr, balance, pump, head, r, est=None, **kw):
    e = E_[key]
    b = {'id': code, 'name': name, 'family': family, 'series': series, 'vehicles': (e.get('vehicles') or {}).get('value', []) if isinstance(e.get('vehicles'), dict) else e.get('vehicles', []),
         'boreMm': bore, 'strokeMm': stroke, 'rodMm': rod, 'deckMm': deck, 'compression': cr,
         'compressionHeightMm': 30, 'pinMm': 22, 'bigEndMm': 51, 'balanceShafts': balance,
         'oilPump': pump, 'head': head, 'rating': r, 'aftermarketBottomEnd': True, 'pistonsVsK20Head': None,
         'estimated': est or {}, 'sources': srcs({k: e.get(k) for k in ('block', 'internals', 'ratings')})}
    b.update(kw)
    return b

K20, K24 = 212, 231.5
blocks = [
    block('K20Z3', 'K20Z3', 'K20Z3 (2006-11 Civic Si)', 'K20', 'Z', 86, 86, 139, K20, 11.0, True, 'rbc', 'K20Z3',
          rating(197, 7800, 139, 6200, 8000, 8300)),
    block('K20A2', 'K20A2', 'K20A2 (2002-04 RSX Type S)', 'K20', 'A', 86, 86, 139, K20, 11.0, False, 'prb', 'K20A2',
          rating(200, 7400, 142, 6000, 7900, 8250)),
    block('K20Z1', 'K20Z1', 'K20Z1 (2005-06 RSX Type S)', 'K20', 'Z', 86, 86, 139, K20, 11.0, False, 'prb', 'K20Z1',
          rating(201, 7800, 140, 7000, 8100, 8300)),
    block('K20A', 'K20A_JDM_TypeR', 'K20A Type R (JDM DC5 / EP3)', 'K20', 'A', 86, 86, 139, K20, 11.5, False, 'prb', 'K20A-R',
          rating(217, 8000, 152, 7000, 8400, 8400),
          {'compressionHeight': 'not found; 30 mm as on the other K20 blocks',
           'balanceShafts': 'sources disagree; the FD2 pump is sold as a balance-shaft delete, so none assumed',
           'oilPump': 'DC5/EP3 pump part number not found; the RSX Type S pump assumed'}),
    block('K20A-FD2', 'K20A_JDM_TypeR', 'K20A Type R (JDM FD2)', 'K20', 'A', 86, 86, 139, K20, 11.7, False, 'rrc', 'K20A-R',
          rating(222, 8000, 159, 6100, 8400, 8400),
          {'compressionHeight': 'not found; 30 mm as on the other K20 blocks'}),
    block('K20Z4', 'K20Z4', 'K20Z4 (2007-10 Civic Type R FN2)', 'K20', 'Z', 86, 86, 139, K20, 11.0, True, 'rbc', 'K20Z4',
          rating(198, 7800, 142, 5600, 8000, 8200),
          {'bore': 'bore, stroke and rod not found for the FN2; same as the K20Z3', 'oilPump': 'part number not found; K20Z3 balancer pump assumed'}),
    block('K20A3', 'K20A3', 'K20A3 (RSX base / 2002-05 Civic Si)', 'K20', 'A3', 86, 86, 139, K20, 9.8, False, 'prb', 'K20A3',
          rating(160, 6500, 141, 4000, 6800, 7000),
          {'rod': 'K20A3 rod length not found; 139 mm assumed', 'compressionHeight': 'not found; 30 mm assumed'},
          aftermarketBottomEnd=False),
    block('K20Z2', 'K20Z2', 'K20Z2 (CSX base / Euro Accord)', 'K20', 'Z', 86, 86, 139, K20, 9.8, True, 'rbc', 'K20Z2',
          rating(153, 6000, 139, 4500, 6800),
          {'bore': 'Wikipedia only; 86 x 86 like the other K20s', 'oilPump': 'not found; K20Z3 balancer pump assumed',
           'balanceShafts': 'not found; assumed like the K20Z3'}),
    block('K24A2', 'K24A2', 'K24A2 (2004-08 TSX)', 'K24', 'A', 87, 99, 152, K24, 10.5, True, 'raa', 'K24A2',
          rating(205, 7000, 164, 4500, 7100, 7200)),
    block('K24A1', 'K24A1', 'K24A1 (2002-06 CR-V)', 'K24', 'A', 87, 99, 152, K24, 9.6, True, 'raa', 'K24A1',
          rating(160, 6000, 162, 3600, 6500, 7000),
          {'oilPump': 'CR-V pump page would not load; the K24A balancer pump assumed', 'balanceShafts': 'not stated; assumed like the K24A2'}),
    block('K24A4', 'K24A4', 'K24A4 (2003-05 Accord / Element)', 'K24', 'A', 87, 99, 152, K24, 9.7, True, 'raa', 'K24A4',
          rating(160, 5500, 161, 4500, 6500), pistonsVsK20Head='risk'),
    block('K24A8', 'K24A8', 'K24A8 (2006-07 Accord / Element)', 'K24', 'A', 87, 99, 152, K24, 9.7, True, 'raa', 'K24A4',
          rating(166, 5800, 160, 4000, 6500), pistonsVsK20Head='contact'),
    block('K24Z3', 'K24Z3', 'K24Z3 (2008-12 Accord)', 'K24', 'Z', 87, 99, 152, K24, 10.5, True, 'r40', 'K24Z',
          rating(190, 7000, 162, 4400, 7100),
          {'rod': 'not found for the Z3; 152 mm as on the other K24s', 'balanceShafts': 'stated for the K24Z7; assumed for the Z3'}),
    block('K24Z7', 'K24Z7', 'K24Z7 (2012-15 Civic Si)', 'K24', 'Z', 87, 99, 152, K24, 11.0, True, 'r40', 'K24Z',
          rating(205, 7000, 174, 4400, 7000, 7500)),
]
for b in blocks:
    b['estimated'].setdefault('deck', 'deck heights (212 / 231.5 mm) are Wikipedia figures, consistent with stroke/2 + rod + 30 mm compression height')

# Cam lobes: lift (m) and duration (crank degrees, seat to seat), as FLOW.cams in src/gasflow.js.
Z3 = {'low': {'inLift': 0.0095, 'inDur': 228, 'exLift': 0.0090, 'exDur': 222},
      'high': {'inLift': 0.0117, 'inDur': 262, 'exLift': 0.0110, 'exDur': 256}}
def cams(low=None, high=None):
    c = json.loads(json.dumps(Z3))
    c['low'].update(low or {}); c['high'].update(high or {})
    return c
# Economy i-VTEC: below the switch point one intake valve barely opens (swirl), the exhaust has one lobe.
ECON = cams({'inLift': 0.0080, 'inDur': 216, 'inValves': 1.15},
            {'inLift': 0.0095, 'inDur': 228, 'exLift': 0.0090, 'exDur': 222})
# Intake-only VTEC (K24Z): normal low cam on both intake valves, one exhaust lobe.
INTAKE_ONLY = cams(None, {'exLift': 0.0090, 'exDur': 222})
CAM_EST = 'cam lobes not published; K20Z3 estimates used'

def head(code, key, name, family, vtec, vtec_rpm, c, cam_note, inv=35, exv=30, est=None, **kw):
    e = E_[key]
    h = {'id': code, 'name': name, 'family': family, 'vtec': vtec, 'vtecRpm': vtec_rpm, 'inValveMm': inv, 'exValveMm': exv,
         'partNumber': (e.get('head', {}).get('headPartNumber') or {}).get('value'), 'cams': c,
         'estimated': {'cams': cam_note, **(est or {})}, 'sources': srcs(e.get('head', {}))}
    h.update(kw)
    return h

heads = [
    head('K20Z3', 'K20Z3', 'K20Z3 head (RBC)', 'K20', 'dohc', 5800, cams(), CAM_EST),
    head('K20A2', 'K20A2', 'K20A2 head (PRB)', 'K20', 'dohc', 5800, cams(), CAM_EST + ' (only low-confidence K20A2 cam figures were found)'),
    head('K20Z1', 'K20Z1', 'K20Z1 head', 'K20', 'dohc', 5800, cams(), CAM_EST),
    head('K20A-R', 'K20A_JDM_TypeR', 'K20A Type R head (PRC / RRC)', 'K20', 'dohc', 6000,
         cams(None, {'inLift': 0.0119, 'exLift': 0.0120}),
         'K20Z3 estimates, with the high lobes raised by the ratio of the K20A to K20A2 figures on motorreviewer.com (low confidence: +2% intake, +9% exhaust)'),
    head('K20Z4', 'K20Z4', 'K20Z4 head (FN2)', 'K20', 'dohc', 5400, cams(), CAM_EST, est={'valves': 'not found; K20Z3 sizes'}),
    head('K20A3', 'K20A3', 'K20A3 head (economy i-VTEC)', 'K20', 'economy', 2200, ECON,
         'not published; the low intake lobe is smaller and one intake valve only cracks open (the swirl mode Honda describes), one exhaust lobe'),
    head('K20Z2', 'K20Z2', 'K20Z2 head (economy i-VTEC)', 'K20', 'economy', 2200, ECON,
         'not published; same economy layout as the K20A3', est={'vtecRpm': 'not found; K20A3 value', 'valves': 'not found; K20A3 sizes'}),
    head('K24A2', 'K24A2', 'K24A2 head (RBB, 2006-08)', 'K24', 'dohc', 6000,
         cams(None, {'inLift': 0.0126, 'inDur': 274}),
         'not published; K20Z3 estimates plus the 2006 change Honda describes for the intake high lobe (+0.9 mm lift, +12 degrees)', inv=36),
    head('K24A1', 'K24A1', 'K24A1 head (CR-V)', 'K24', 'economy', 2750, ECON,
         'not published; Honda does not say which i-VTEC the CR-V has, economy layout assumed'),
    head('K24A4', 'K24A4', 'K24A4 / K24A8 head (RAA)', 'K24', 'economy', 3500, ECON,
         'not published; economy layout assumed', est={'valves': 'not found; 35 / 30 mm assumed'}),
    head('K24Z', 'K24Z7', 'K24Z3 / K24Z7 head (R40)', 'K24', 'intake', 5000, INTAKE_ONLY,
         'not published; Honda says VTEC on the intake valves only, so one exhaust lobe', est={'valves': 'not found; 35 / 30 mm assumed'}),
]

P = spec['oilPumps']
def pump(pid, key, name, part, balance, fits, flow=1.0, max_rpm=8500, est=None, src_extra=()):
    s = srcs(P[key]) if key else []
    for u in src_extra:
        if u not in s: s.append(u)
    return {'id': pid, 'name': name, 'partNumber': part, 'balanceShafts': balance, 'flowScale': flow, 'maxRpm': max_rpm,
            'fits': fits, 'estimated': est or {}, 'sources': s}

HR = 'https://www.hybrid-racing.com/products/hybrid-racing-k20a2-oil-pump-installation-kit'
TYPE_S_KIT = ['K20A2 windage tray 11221-PNC-000', 'oil pump chain 13441-PCX-004', 'chain guide 13460-PNC-004', 'tensioner 13450-RAA-A02']
K24A_CUT = ['pump housing notched to clear the K24 block'] + TYPE_S_KIT
K24Z_KIT = ['PRL or KPower swap kit: oil filter moved back to the K24A position, bracket, girdle trimmed'] + TYPE_S_KIT
FLOW_EST = 'no published flow or relief figures; same delivery as the stock pump assumed'
CAV_EST = 'no published limit for this pump; 8,500 rpm assumed like the RSX Type S pump'
pumps = [
    pump('rbc', 'K20Z3_Si', 'K20Z3 balancer pump (15100-RBC-016)', '15100-RBC-016', True,
         [{'blocks': ['K20Z3', 'K20Z4', 'K20Z2'], 'needs': []}], est={'flow': FLOW_EST, 'maxRpm': CAV_EST}),
    pump('prb', 'K20A2_RSX_TypeS_PRB', 'RSX Type S pump (15100-PRB-A01)', '15100-PRB-A01', False,
         [{'blocks': ['K20A2', 'K20Z1', 'K20A3', 'K20A', 'K20A-FD2'], 'needs': []},
          {'blocks': ['K20Z3', 'K20Z4', 'K20Z2'], 'needs': TYPE_S_KIT},
          {'blocks': ['K24A2', 'K24A1', 'K24A4', 'K24A8'], 'needs': K24A_CUT},
          {'blocks': ['K24Z3', 'K24Z7'], 'needs': K24Z_KIT}],
         est={'flow': FLOW_EST}, src_extra=[HR]),
    pump('pne', 'K20A3_CivicSi_EP3', '2002-05 Civic Si pump (15100-PNE-013)', '15100-PNE-013', True,
         [{'blocks': ['K20A3'], 'needs': []}],
         est={'flow': FLOW_EST, 'maxRpm': CAV_EST, 'balanceShafts': 'not stated; it comes with a holder set like the balancer pumps, so balance shafts assumed'}),
    pump('raa', 'K24A_balancer', 'K24A balancer pump (15100-RAA-A02)', '15100-RAA-A02', True,
         [{'blocks': ['K24A2', 'K24A1', 'K24A4', 'K24A8'], 'needs': []}], est={'flow': FLOW_EST, 'maxRpm': CAV_EST}),
    pump('r40', 'K24Z_balancer_R40', 'K24Z balancer pump (15100-R40-A02)', '15100-R40-A02', True,
         [{'blocks': ['K24Z3', 'K24Z7'], 'needs': []}], est={'flow': FLOW_EST, 'maxRpm': CAV_EST}),
    pump('rrc', 'FD2_TypeR_RRC_kit', 'FD2 Type R pump, balance-shaft delete kit (15100-RRC-003)', '15100-RRC-003', False,
         [{'blocks': ['K20A-FD2', 'K20A', 'K20A2', 'K20Z1'], 'needs': []},
          {'blocks': ['K20Z4', 'K20Z3', 'K20Z2'], 'needs': ['kit parts: chain 13441-PCX-004, guide 13460-PNC-004, windage tray 11221-PNC-000, tensioner 13450-RRC-004']},
          {'blocks': ['K24A2', 'K24A1', 'K24A4', 'K24A8'], 'needs': ['kit parts as above', 'K20 oil pan', '1 L more oil']}],
         est={'flow': FLOW_EST, 'maxRpm': CAV_EST}),
    pump('4p-ported', '4Piston_ported_TypeS', '4 Piston ported Type S pump', '4P-KPUMP', False,
         [{'blocks': ['K20A2', 'K20Z1', 'K20A3', 'K20A', 'K20A-FD2'], 'needs': []},
          {'blocks': ['K20Z3', 'K20Z4', 'K20Z2'], 'needs': TYPE_S_KIT},
          {'blocks': ['K24A2', 'K24A1', 'K24A4', 'K24A8'], 'needs': ['notched for the K24 as sold'] + TYPE_S_KIT},
          {'blocks': ['K24Z3', 'K24Z7'], 'needs': K24Z_KIT}],
         flow=1.25, max_rpm=12000,
         est={'flow': '4 Piston claims 25% more flow than stock; no pressure figures published', 'maxRpm': '4 Piston says tested to 12,000 rpm'}),
    pump('dc-s2k', 'DragCartel_S2K_F_series_pump', 'Drag Cartel S2000 (F-series) pump conversion', 'Drag Cartel', False,
         [{'blocks': [b['id'] for b in blocks if b['family'] == 'K20'] + ['K24A2', 'K24A1', 'K24A4', 'K24A8'],
           'needs': ['windage tray 11221-PNC-000', 'tensioner 13450-RAA-A02', 'chain guide 13460-PNC-004', 'chain 13441-PNA-004', 'Moroso baffled or steel oil pan']}],
         max_rpm=9000, est={'flow': FLOW_EST, 'maxRpm': 'Drag Cartel says it runs above 9,000 rpm without cavitation; 9,000 used'}),
]

def g(o, *ks):
    for k in ks: o = o[k]
    return o
C = spec['compatibility']
HRG = 'https://www.hybrid-racing.com/blogs/hybrid-racing/k20-k24-hybrid-engine-build-guide'
K20_DOHC = ['K20Z3', 'K20A2', 'K20Z1', 'K20A-R', 'K20Z4']
K24_SWAP = C[0]['needs'][:4]
compat = [
    {'heads': K20_DOHC, 'blocks': ['K24A2', 'K24A1', 'K24A4', 'K24A8'], 'works': 'mods', 'needs': K24_SWAP,
     'note': 'Both heads have 50.5 cc chambers, so compression stays at the block\'s ratio.', 'sources': C[0]['sources']},
    {'heads': K20_DOHC, 'blocks': ['K24Z3', 'K24Z7'], 'works': 'mods',
     'needs': ['front (timing) cover modified', 'MotoIQ\'s K20Z3-on-K24Z7 build also used aftermarket pistons'],
     'note': 'Hybrid Racing\'s guide does not support Z-series blocks; one documented build.', 'sources': C[2]['sources'] + [HRG]},
    {'heads': ['K20A2'], 'blocks': ['K20A3'], 'works': 'yes', 'needs': [], 'note': 'Direct fit; compression stays 9.8:1 with the A3 pistons.', 'sources': C[3]['sources']},
]
_meta = {'generatedBy': 'tools/build-engines.py from docs/k-series-specs.json',
         'rules': ['A head fits a block of its own family (K20 on K20, K24 on K24). Only the K20A2-on-K20A3 swap is documented; the rest are inferred from the shared design.',
                   'A K20 DOHC VTEC head fits a K24 block with the parts listed in `compatibility`.',
                   'Combinations no fetched page documents (a K24 head on a K20 block, an economy K20 head on a K24 block) are not allowed.',
                   'Stock K24A4 pistons risk hitting a K20 head\'s valves; K24A8 pistons hit them. Aftermarket pistons are needed.',
                   'Aftermarket K20 cranks, rods and stroker kits do not list the K20A3, whose rods and bearings differ, and are not allowed in it.'],
         'blockedRuleSources': [HRG]}
out = {'_meta': _meta, 'blocks': blocks, 'heads': heads, 'oilPumps': pumps, 'compatibility': compat}
(root / 'docs/engines.json').write_text(json.dumps(out, indent=1) + '\n')
print(len(blocks), 'blocks', len(heads), 'heads', len(pumps), 'pumps')
