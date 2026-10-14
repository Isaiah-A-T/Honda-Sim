# Engine internals model

`web/src/internals.js` turns what the engine is doing into loads on each part and tracks metal fatigue. Every part has a
material you can change in the **Materials** section of the internals panel (stock cast pistons or forged 4032/2618,
powdered-metal or 4340/300M/titanium rods, ARP bolts and studs, tri-metal bearings, billet or EN40B cranks, ductile iron
sleeves, copper head gaskets). The choice is saved with the tune (`tune.materials`).

**How it works**
1. **Cylinder pressure** comes from the gas exchange model's pressure trace ([GASFLOW.md](GASFLOW.md)), so it follows
   timing, boost, cam and mixture. Knock adds pressure spikes on top. A stock engine at full throttle peaks around 55 bar at
   3,000 rpm (knock-limited timing) and 90–100 bar near redline (best timing).
2. **Forces.** Gas force on the 86 mm piston pushes the rod down on the firing stroke. The reciprocating mass
   (piston, pin, rings, rod small end) pulls the rod and its bolts apart at exhaust top dead centre, growing with rpm².
   It scales with the piston and rod alloys' densities, so titanium rods pull less on the bolts.
3. **Piston temperature** rises with airflow and goes up further when lean or knocking. Alloys that conduct heat
   better run cooler. Strength drops with temperature along each alloy's published curve.
4. **Load = demand ÷ capacity** for each part. Below 80% the metal is under its endurance limit. Above that,
   damage adds up every firing cycle (Miner's rule, S–N slope 9). At 150% the part breaks immediately.
   Parts with a steady stress on top of the cycling one (sleeves, block) use a Goodman correction: the mean stress
   uses up part of the alloy's fatigue strength, in proportion to its tensile strength.
5. When any part reaches 100% life used, the engine fails and stops until you press **Rebuild engine**.

| Part | What limits it | Material properties used |
|---|---|---|
| Pistons | Crown and ring-land stress vs fatigue strength at crown temperature; brittle cast alloys take knock harder | fatigue, strength vs temperature, elongation, density, conductivity |
| Connecting rods | Compression (buckling margin) on the firing stroke and inertia tension at TDC | yield, fatigue, density |
| Rod bolts | Thread fatigue from the inertia pull at high rpm | thread fatigue (rolled-after-heat-treat ARP threads rate higher) |
| Rod bearings | Peak load on the projected bearing area | load rating |
| Crankshaft | Bending at the rod pin fillet | fatigue |
| Head gasket | Cylinder pressure that lifts the head off the gasket | head bolt yield (clamp at 75% of yield) × gasket seal factor |
| Head bolts | Fatigue from the share of firing load the bolts feel | yield, thread fatigue |
| Cylinder sleeves | Hoop stress P·r/t shared with the aluminium around it by stiffness, worse at the unsupported top of the open deck, plus a steady stress from the aluminium expanding more than the iron when hot | modulus (both), fatigue and tensile (Goodman), thermal expansion (both) |
| Block (main webs) | Firing load through the main bearing webs into the bolt bosses | fatigue and tensile (Goodman), strength vs temperature |
| Valvetrain | Valve float above ~9,000 rpm (safe to 8,600, Hondata's reflash limit) | – |

## Material library

Every value below comes from the linked page, except those marked * which had to be estimated (the app shows the
reason when you hover "est."). Load ratings for bearings, thread fatigue for bolts and gasket seal factors are model
inputs, not material constants, and are estimates. The full data with notes is in
[materials-sources.json](materials-sources.json); `python3 web/tools/build-materials.py` rebuilds `web/src/materials.js` from it.

| Part | Alloy | Density kg/m³ | Modulus GPa | Yield MPa | Tensile MPa | Fatigue MPa | Elong. % | Expansion µm/m·K | Conductivity W/m·K | Sources |
|---|---|---|---|---|---|---|---|---|---|---|
| piston | Cast hypereutectic aluminium A390 (stock-type cast piston) | 2700 | 75 | 290 | 290 | 100 | 0.9 | 20.0 | 130 | [1](https://makeitfrom.com/material-properties/A390.0-T6-Cast-Aluminum) [2](https://corpora.tika.apache.org/base/docs/govdocs1/719/719097.pdf) |
| piston | Forged aluminium 4032-T6 (low-expansion forged piston) | 2600 | 73 | 320 | 390 | 110 | 6.7 | 19.0 | 140 | [1](https://makeitfrom.com/material-properties/4032-4032-T6-AlSi12.5MgCuNi-Aluminum) |
| piston | Forged aluminium 2618-T61 (high-strength forged piston) | 2900 | 71 | 350 | 420 | 110 | 5.8 | 22.0 | 160 | [1](https://makeitfrom.com/material-properties/2618-2618-T61-3.1924-Aluminum) [2](https://apps.dtic.mil/sti/html/tr/AD0801499/index.html) |
| rod | Powder-forged steel Fe-Cu-C (stock rods) | 7780* | 200* | 723 | 900* | 340* | – | 12.0* | – | [1](https://www.hoganas.com/globalassets/downloads/technical-papers/pm/newmaterialsforpowderforgedconnectingrods.pdf) [2](https://www.metal-powder.tech/?p=2128) [3](https://www.hoganas.com/globalassets/downloads/technical-papers/pm/materialsforhighlyloadedpartssuchasconnectingrods.pdf) |
| rod | AISI 4340 forged steel, quenched and tempered | 7800 | 190 | 1150 | 1210 | 740 | 14.0 | 13.0 | 44 | [1](https://makeitfrom.com/material-properties/Quenched-and-Tempered-4340-Ni-Cr-Mo-Steel) |
| rod | 300M steel | 7870 | 205 | 1586 | 1931 | 770* | – | 12.0* | – | [1](https://alloysintl.com/inventory/alloy-steel/alloy-steel-300m/) |
| rod | Titanium Ti-6Al-4V | 4400 | 110 | 910 | 1000 | 530 | 8.6 | 8.9 | 7 | [1](https://www.makeitfrom.com/material-properties/Grade-5-Ti-6Al-4V-3.7165-R56400-Titanium) |
| rodBolt | Alloy steel bolt, property class 10.9 (stock) | 7800 | 190 | 940 | 1040 | 650 | 9.0 | 13.0 | 43 | [1](https://www.trfastenings.com/knowledge-base/metric-screws/screws-and-bolts-mechanical-properties) [2](https://www.makeitfrom.com/material-properties/Quenched-and-Tempered-4140-Cr-Mo-Steel) |
| rodBolt | ARP2000 | 7800 | 190 | 1330* | 1517 | – | – | 13.0 | 39 | [1](https://arp-bolts.com/tech/materials) [2](https://www.makeitfrom.com/material-properties/SAE-AISI-8740-G87400-Ni-Cr-Mo-Steel) |
| rodBolt | ARP L19 | 7800* | 200* | 1610* | 1793 | – | – | 12.0* | – | [1](https://arp-bolts.com/tech/materials) |
| bearing | Aluminium-tin bimetal shell (stock) | 2900* | 70* | – | 200 | – | – | 18.0 | 47 | [1](https://www.viiplus.com/product/jf-20-high-tin-aluminum-bushing/) [2](https://www.totalmateria.com/en-us/material/3211817) |
| bearing | Copper-lead tri-metal race shell | 8900* | 120* | – | – | – | – | 19.0 | 60 | [1](https://www.bronzegleitlager.com/sale-13377330-bimetal-bushing-steel-backed-cupb24sn4-alloy.html) [2](https://www.totalmateria.com/en-us/material/3256135) |
| crank | 38MnVS6 micro-alloyed forged steel (stock-type) | 7670 | 210 | 520 | 800 | 400* | 12.0 | 11.1 | 43 | [1](https://steelnavigator.ovako.com/steel-grades/38mnvs6/) [2](https://swisssteel-group.com/content-media/documents/Data-Sheets/Engineering-Steel/1.5231_de.pdf) [3](https://www.saarstahl.com/app/uploads/2024/03/20160318122935-1303_38MnVS6.pdf) |
| crank | 4340 billet steel | 7800 | 190 | 1150 | 1210 | 740 | 14.0 | 13.0 | 44 | [1](https://makeitfrom.com/material-properties/Quenched-and-Tempered-4340-Ni-Cr-Mo-Steel) |
| crank | EN40B nitriding steel | 7850* | 210* | 680 | 850 | 550* | 13.0 | 12.0* | – | [1](https://westyorkssteel.com/alloy-steel/nitriding-steel/en40b) |
| gasket | Multi-layer steel, AISI 301 spring temper (stock) | 7800 | 200 | 1080 | 1460 | 540 | 7.4 | 17.0 | 16 | [1](https://makeitfrom.com/material-properties/Full-Hard-301-Stainless-Steel) |
| gasket | Solid copper C11000, annealed | 9000 | 120 | 69 | 220 | 75 | 50.0 | 17.0 | 390 | [1](https://www.makeitfrom.com/material-properties/Annealed-OS050-C11000-Copper) |
| valveSpring | Chrome-silicon spring wire SAE 9254 | 7700 | 206 | 1674 | 1860 | 650* | – | 13.0 | 46 | [1](https://makeitfrom.com/material-properties/SAE-AISI-9254-G92540-Silicon-Chromium-Steel) [2](https://www.wire-pengg.com/images/data/downloads/en/datenblaetter/pengg_austria/844_e.pdf) [3](https://www.gibbswire.com/pdf/chrome-silicon-comm-wire.pdf) |
| block | Die-cast aluminium 383 / ADC12 (assumed stock grade) | 2800 | 73 | 150 | 280 | 150 | 3.5 | 21.0 | 96 | [1](https://www.makeitfrom.com/material-properties/383.0-383.0-F-SC102A-A03830-Cast-Aluminum) |
| sleeve | Grey cast iron, ASTM A48 class 30 (stock-type) | 7500 | 180 | 130 | 240 | 97 | 9.6 | 11.0 | 46 | [1](https://www.makeitfrom.com/material-properties/ASTM-Grade-30-or-200-ISO-200-EN-JL-1030-F12101-Grey-Cast-Iron) |
| sleeve | Ductile iron (aftermarket sleeves) | 7500 | 170 | 410 | 600 | 270* | 8.2 | 11.0 | 36 | [1](https://www.makeitfrom.com/material-properties/ASTM-Grade-80-55-06-SAE-D5506-Ductile-Cast-Iron) [2](https://www.myalro.com/gradeinfo/Metals_Gridpt.aspx?gp=0139) [3](https://dragcartel.com/products/k-series-sleeve-kit) |
| headBolt | Alloy steel bolt, property class 10.9 (stock) | 7800 | 190 | 940 | 1040 | 650 | 9.0 | 13.0 | 43 | [1](https://www.trfastenings.com/knowledge-base/metric-screws/screws-and-bolts-mechanical-properties) [2](https://www.makeitfrom.com/material-properties/Quenched-and-Tempered-4140-Cr-Mo-Steel) |
| headBolt | ARP2000 head studs | 7800 | 190 | 1330* | 1517 | – | – | 13.0 | 39 | [1](https://arp-bolts.com/tech/materials) [2](https://www.makeitfrom.com/material-properties/SAE-AISI-8740-G87400-Ni-Cr-Mo-Steel) |

Honda doesn't publish which grades it uses. The stock picks are the usual ones for these parts: a hypereutectic
cast piston, powder-forged rods ([Höganäs](https://www.hoganas.com/globalassets/downloads/technical-papers/pm/newmaterialsforpowderforgedconnectingrods.pdf)),
a micro-alloyed forged crank (38MnVS6), an MLS gasket, and die-cast 383/ADC12 aluminium for the block with grey iron sleeves.

**Published inputs:** 86 × 86 mm and 11.0:1 ([Honda](https://hondanews.com/en-US/releases/release-e9dd7ff209cc71cc83947a004c34c1b2-2006-honda-civic-coupe-and-civic-si-specifications)),
139 mm rod length and 22 mm pin ([CP piston listing](https://www.realstreetperformance.com/cp-forged-pistons-acura-k20a-a2-a3-rsx-86mm-9-01.html); one rod maker lists 137 mm),
48 mm rod big end and powdered-metal stock rods ([875 Performance](https://875performance.com/bc6051-honda-acura-k20a2-z3-lightweight-connecting-rods-w-arp2000-fasteners-rated-to-400whp/)),
aluminium block with cast iron sleeves and a 212 mm deck ([Wikipedia: Honda K engine](https://en.wikipedia.org/wiki/Honda_K_engine)),
open deck (the reason for aftermarket [block guards](https://www.hybrid-racing.com/products/supertech-block-guard-k-series)),
8,600 rpm reflash rev limit ([Hondata](https://www.hondata.com/reflash-category/reflash-civicsi-2006)).

**Estimated:** part masses and cross-sections, sleeve and block wall thickness, the open-deck factor, head bolt joint
stiffness, the piston temperature model and the load paths. They were set so a stock engine is safe at its redline
and the sleeves and pistons are what give up first when a stock block sees heavy boost with too much timing, which
matches what K20 owners report. Treat the panel as "which part is closest to its limit and why", not as a prediction
of how many pulls an engine will last.
