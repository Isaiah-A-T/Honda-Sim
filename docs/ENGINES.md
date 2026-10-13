# Other K-series engines: blocks, heads and oil pumps

The **Engine** section at the top of the Parts tab picks the block, the head and the oil pump separately, so you
can build any K-series combination. Saved as `tune.engine = { block, head, oilPump }`; anything left out is the
K20Z3 the car came with.

Data: `docs/k-series-specs.json` is the research file. Every number in it has its source pages, and anything not
found is null with a note. `web/tools/build-engines.py` turns it into `docs/engines.json`, the catalog the sim uses.
Any value the sim needs that wasn't published is filled in there and listed under `estimated`, and the panel
shows it as an estimate. Rebuild with `python3 web/tools/build-engines.py && node web/tools/build-parts.mjs`.

## What each part brings

| Part | Sets |
|---|---|
| Block | Bore, stroke, rod length, deck height and the stock pistons' compression ratio, which makes its stock crank, rods and pistons. Aftermarket parts then fit against that block |
| Head | Valve sizes, cam lobes and the VTEC type: both cams, intake cam only, or economy (on the low cam one intake valve barely opens). Its stock VTEC point and the block's rev limit are one click away |
| Oil pump | Oil delivery (pressure at every rpm), the rpm where it stops keeping up, and whether it drives the balance shafts |

Blocks: K20Z3, K20A2, K20Z1, K20A Type R (DC5/EP3 and FD2), K20Z4, K20A3, K20Z2, K24A2, K24A1, K24A4, K24A8,
K24Z3 and K24Z7. Heads: one per engine, with the K24A4/A8 sharing the RAA head and the K24Z3/Z7 the R40 head.

Oil pumps:

| Pump | Balance shafts | Notes |
|---|---|---|
| K20Z3 (15100-RBC-016) | yes | stock |
| RSX Type S (15100-PRB-A01) | no | direct fit on K20A2/Z1/A3 and the JDM K20A; on the K20Z3 it needs the K20A2 windage tray, chain, guide and tensioner; on a K24A the housing is notched too; on a K24Z it needs a filter relocation kit |
| 2002-05 Civic Si (15100-PNE-013) | assumed yes | K20A3 |
| K24A balancer (15100-RAA-A02) | yes | K24A1/A2/A4/A8 |
| K24Z balancer (15100-R40-A02) | yes | K24Z3/Z7 |
| FD2 Type R delete kit (15100-RRC-003) | no | K24 needs the K20 pan and 1 L more oil |
| 4 Piston ported Type S | no | 25% more flow (maker's claim), tested to 12,000 rpm |
| Drag Cartel S2000 conversion | no | runs above 9,000 rpm (maker's claim); needs a baffled pan |

No pump maker or Honda publishes flow or relief pressures, so every pump uses the K20Z3's pressure curve except
the 4 Piston pump (25% more flow, so 25% more pressure until the relief valve opens). Pumps hold their pressure
to 8,500 rpm (Hybrid Racing says the Type S pump cavitates past that) unless the maker says more. Dropping the
balance shafts frees about 7.5 hp at 7,800 rpm, the midpoint of the vendors' 5-10 bhp claims, with the drag
growing in proportion to rpm.

## Fit rules

- A head fits a block of its own family (K20 on K20, K24 on K24). Only the K20A2 head on a K20A3 block is
  documented; the rest are inferred from the shared design and say so.
- A K20 DOHC VTEC head (K20Z3, K20A2, K20Z1, K20A Type R, K20Z4) fits a K24A block with the K24 timing chain
  and chain cover, a K24 head gasket and the K24 crank sensor (Hybrid Racing's K20/K24 build guide). Both heads
  have 50.5 cc chambers (Wiseco, CP), so compression stays at the block's ratio.
- On a K24Z block it needs the front cover modified (MotoIQ's one documented build). Hybrid Racing doesn't
  support Z blocks.
- Stock K24A4 pistons risk touching a K20 head's valves, which the panel warns about. Stock K24A8 pistons hit
  them, so that combination falls back to the block's own head until you fit aftermarket pistons.
- Nothing documents a K24 head on a K20 block or an economy K20 head on a K24, so those are greyed out.
- Aftermarket K20 cranks, stroker kits and rods are made for the K20's 212 mm deck, so they don't go in a K24.
  None of them list the K20A3, whose rods and bearings differ.
- Pistons smaller than the block's bore don't fit. More than 1 mm over the bore needs new sleeves.

## How close it lands

The gas model and its per-rpm corrections were calibrated on the K20Z3, and Honda publishes no cam figures for any
K-series head, so other engines are a rough guide. With each engine's own head, VTEC point and pump, peak power
lands within about 10% of Honda's ratings: K20A2 205 hp against 200 rated, K24A2 201 against 205, K20A3 168 against
160. Peak torque comes in higher in the rpm range than rated on the K24s and economy engines, because the
corrections carry the K20Z3's high-rpm intake tuning.
