# Engine internals model

`src/internals.js` turns what the engine is doing into loads on each part and tracks metal fatigue.

**How it works**
1. **Cylinder pressure.** Compression pressure (manifold pressure × volumetric efficiency × 11.0^1.3) times a
   combustion rise that goes up with spark advance and down with retard. Knock adds pressure spikes on top.
   A stock engine peaks around 55–75 bar at full throttle, and a 12–14 psi turbo around 110–130 bar.
2. **Forces.** Gas force on the 86 mm piston pushes the rod down on the firing stroke. The reciprocating mass
   (piston, pin, rings, rod small end ≈ 0.57 kg) pulls the rod and its bolts apart at exhaust top dead centre,
   growing with rpm².
3. **Piston temperature** rises with airflow and goes up further when lean or knocking. Aluminium loses strength
   fast above 250 °C.
4. **Load = demand ÷ capacity** for each part. Below 80% the metal is under its endurance limit. Above that,
   damage adds up every firing cycle (Miner's rule, S–N slope 9). At 150% the part breaks immediately.
5. When any part reaches 100% life used, the engine fails and stops until you press **Rebuild engine**.

| Part | Material (grade estimated) | What limits it |
|---|---|---|
| Pistons | Cast hypereutectic aluminium | Crown and ring land stress vs fatigue strength at crown temperature |
| Connecting rods | Powder-forged steel (stock rods are powdered metal) | Compression/buckling on the firing stroke |
| Rod bolts | 10.9 alloy steel, 2 × M8 | Fatigue from the inertia pull at high rpm |
| Rod bearings | Aluminium-tin bimetal | Peak load on the bearing area |
| Crankshaft | Forged micro-alloyed steel | Bending at the rod pin fillet |
| Head gasket | MLS with stock head bolts | Cylinder pressure that lifts the head |
| Valvetrain | Chrome-silicon springs | Valve float above ~9,000 rpm (safe to 8,600, Hondata's reflash limit) |

**Published inputs:** 86 × 86 mm and 11.0:1 ([Honda](https://hondanews.com/en-US/releases/release-e9dd7ff209cc71cc83947a004c34c1b2-2006-honda-civic-coupe-and-civic-si-specifications)),
139 mm rod length and 22 mm pin ([CP piston listing](https://www.realstreetperformance.com/cp-forged-pistons-acura-k20a-a2-a3-rsx-86mm-9-01.html); one rod maker lists 137 mm),
48 mm rod big end and powdered-metal stock rods ([875 Performance](https://875performance.com/bc6051-honda-acura-k20a2-z3-lightweight-connecting-rods-w-arp2000-fasteners-rated-to-400whp/)),
8,600 rpm reflash rev limit ([Hondata](https://www.hondata.com/reflash-category/reflash-civicsi-2006)).

**Estimated:** part masses and cross-sections, exact material grades, the pressure and temperature model and every limit.
They were set so a stock engine is safe at its redline, and so a stock bottom end turns marginal around 12–14 psi
on pump gas, with knock and heat-soaked pistons failing first. That matches what K20 owners commonly report, but
nobody has measured these parts for this sim. Treat the panel as "which part is closest to its limit and why",
not as a prediction of how many pulls an engine will last.
