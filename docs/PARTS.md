# Bottom end: pistons, rods and stroker kits

The **Bottom end** panel under Engine internals swaps the crank, rods and pistons. Every aftermarket part
is a real, purchasable part from `docs/parts-catalog.json`, with the manufacturer's or a retailer's page as the
source. Regenerate `src/parts-catalog.js` with `node tools/build-parts.mjs` after editing the JSON.

What changes when you swap parts:

| Effect | How |
|---|---|
| Bore, stroke, rod length | From the piston, the crank or kit, and the rod |
| Displacement | 4 × π/4 × bore² × stroke |
| Compression | Each piston's advertised ratio fixes its clearance volume for the engine it was made for. In another combination the ratio is recomputed from the new swept volume and any change in deck clearance |
| Breathing and power | The gas exchange model runs with the new bore, stroke, rod and compression, so a bigger engine pulls more air and makes more torque, and higher compression raises knock on pump gas |
| Loads | Piston and rod weights from the listings set the reciprocating mass; a longer stroke raises rod tension at the same rpm; forged parts bring their alloy (2618, 4032, 4340) |
| Sleeves | Boring stock sleeves thins them. Pistons the maker says need sleeves fit ductile iron sleeves automatically |

Fit rules: the piston can stick out of the deck by at most 0.3 mm; rods must match the crank's rod length and
journal and the piston's pin. Parts that don't fit are greyed out with the reason. Stroker kits that ship
with their own pistons or rods (Toda, Brian Crower 92 mm) lock those choices.

The K24/K20 option is the usual hybrid: a K24A block (231.5 mm deck) with its 99 mm crank and 152 mm rods under
the K20Z3 head.

Left out: the Brian Crower 102 mm K24 kit (bore and compression not published).

Estimated, and shown as such in the panel: weights a listing doesn't give, the Toda kits' rod length, the BC
7048 pistons' bore and compression, and the share of rod mass that reciprocates (32%).
