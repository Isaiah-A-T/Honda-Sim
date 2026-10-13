# Cooling and oil

`web/src/thermal.js` (native: `shared/core/src/thermal.rs`) models the K20Z3's coolant and oil systems so the
engine model can use real coolant and oil temperatures instead of fixed ones.

## Coolant circuit

The belt-driven water pump at the timing-chain end pushes coolant through the block's water jacket, up
through the head gasket into the head (where ECT1 reads), and out of the water neck at the transmission end.
Below the thermostat's opening point it all returns through the bypass hose under the intake manifold to the
pump. As the thermostat (on the pump inlet) opens, more goes out the upper hose, down through the single-row
Denso radiator and back up the lower hose. The ECU runs the radiator and condenser fans off ECT2 in the
radiator's bottom tank. The factory water-to-oil cooler at the filter takes coolant from a port on the pump.

## Oil circuit

The chain-driven rotor pump in the balancer unit in the pan pushes oil through the water-to-oil cooler and
filter into the main gallery: to the main and rod bearings, the piston oil jets (above 200 kPa), the head
(cam journals, rockers, VTEC spool and pins) and the VTC valve and actuator, and back to the sump.

## Numbers

| Item | Value | Source |
| --- | --- | --- |
| Coolant | 6.8 L total incl. 0.4 L reserve, 50/50 Honda Type 2 | 2006 Si owner's manual |
| Thermostat | opens 78 °C (76–80), fully open 90 °C, 8 mm lift | K20A-family manual, Hybrid Racing |
| Radiator cap | 16 psi (110 kPa); boils near 129 °C | CSF Civic Si spec |
| Radiator | Denso 19010-RNB-A51, single row, about 375 × 678 × 16 mm core | Honda parts, CSF 3362 |
| Fans | two (radiator, condenser), on 93 °C / off 88 °C on ECT2 | Hondata; K20A-family fan switch 91–95 °C |
| Oil | 5W-30, 4.4 L with filter | 2006 Si owner's manual |
| Oil pressure (80 °C oil) | at least 70 kPa idle, 300 kPa at 3,000 rpm | K20A-family manual |
| Piston oil jets | open at 200 kPa | K20A manual |
| Oil cooler | water-to-oil at the filter, 15500-RBC-004 | Honda parts |

Everything with sources and conflicts is in `docs/cooling-oil-specs.json`. Heat-transfer coefficients,
thermal masses, pump flow and the VTEC minimum coolant temperature (60 °C) are estimates.

## What it changes in the engine model

- Port walls at coolant temperature heat the intake charge (breathing, knock); cold walls lose more heat.
- Oil viscosity sets friction and oil pressure, and so how fast the VTEC pins move.
- The ECU holds VTEC off below the minimum coolant temperature and raises idle when cold.
- Thin hot oil weakens the bearing film; the oil jets set how hot the pistons run.
- Boiling coolant loads the head gasket more the longer it lasts.

## How it behaves

Highway cruise settles near 88 °C coolant and 93 °C oil. A track session at 150 km/h runs about 101 °C
coolant and 117 °C oil with the fans off. Full load with no airflow (a dyno with no fan) boils the coolant
within a minute.

## Turbo lines (turbo builds)

A turbo build adds an oil feed from the filter housing's pressure port through a restrictor (-4 line, about
0.8 L/min at 300 kPa for a ball-bearing centre section) and a -10 gravity drain into the pan. If the turbo
is water-cooled (the GT35-class and Precision 6262 kits are), coolant runs from the block to the centre
section and back to the pump inlet; with the engine off it keeps moving by thermosiphon. Exhaust heats the
turbine housing, which soaks into the bearing housing; the oil and water carry that heat into the oil and
coolant. Switch off straight after hard driving and the oil stops while the housing is still hot, so the
bearing heat-soaks: about 165 °C water-cooled, about 255 °C oil-cooled only. Above 280 °C with no oil
moving, the sim counts time as oil coking. Thermal masses and heat paths are estimates.

## Rotating assembly

As on the K20: the main gallery feeds the five main bearings; oil holes cross-drilled through the crank carry
it from the mains to the four rod journals (which main feeds which journal isn't published; rod n from main n
is assumed); four squirters spray the piston undersides once oil pressure passes 200 kPa; oil flung off the
crank and rods splashes the bores and wrist pins and drains back to the pan. The rod journals need more
supply pressure as rpm rises (about 50 kPa + 0.03 kPa per rpm, an estimate); below that they starve. Each
bearing's film runs hotter than the sump from shear heating, more so when loaded or starved. Film strength
(viscosity at the film temperature, times supply) sets the rod bearing load and so its wear; the squirters
set how hot the pistons run. `renderState().cooling.rotating` and the native `Internals::lube` carry the
feed, film temperature and strength per bearing and the flow per squirter; `COMPONENTS` places main1–5,
rod1–4 and jet1–4 at a 98 mm bore pitch (approximate).
