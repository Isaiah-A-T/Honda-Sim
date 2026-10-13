# Air and exhaust flow model

`src/gasflow.js` follows one cylinder through a whole four-stroke cycle, one crank degree at a time. The engine
runs a cycle every 10 ms of sim time with the current rpm, manifold pressure, cam, cam angle, spark and mixture, so
everything below reacts to the tune as you drive.

## What it models

| Piece | How |
|---|---|
| Cylinder volume | Slider-crank geometry: 86.0 bore, 85.9 stroke, 139 mm rod, 11.0:1 |
| Valve lift | Intake and exhaust lobes for the low and high cam (lift, duration, lobe centre), sine-shaped profiles |
| Cam phasing (VTC) | Moves the intake lobe earlier by the cam angle. Uses your **Cam angle** tables if you paste them, otherwise a stock-like schedule |
| Valve flow | Compressible flow through the valve curtain (π × valve diameter × lift, capped at the port throat), both ways, so overlap blow-through, backflow into the intake at low rpm and exhaust reversion all happen on their own |
| Intake runner | A column of air with mass between the plenum and the port: it keeps ramming air in after the piston slows, which gives the rpm-dependent tuning a real runner has |
| Exhaust | Port, a primary pipe with the same inertia treatment, a collector shared by four cylinders, then the rest of the exhaust as one orifice (or the turbine plus the open part of the wastegate on turbo builds) |
| Combustion | Wiebe heat release starting at the spark. Burn duration grows with leftover exhaust in the cylinder (part throttle, big overlap), lean or very rich mixtures, and rpm |
| Knock | Livengood–Wu integral over the unburned end gas with the Douaud–Eyzat autoignition delay for the fuel's octane. Knock happens when the end gas autoignites before the flame burns 90% of the charge, so it depends on timing, boost, intake temperature, octane, rpm (time available), residual gas and mixture (rich mixtures cool the charge) |
| Turbo (engine.js) | Turbine power from exhaust flow × temperature × the pressure drop across it; compressor power to make the boost its wheel speed gives; shaft inertia; wastegate actuator opening past the spring plus duty |

## What comes out of it

Per cycle: trapped fresh air (volumetric efficiency), air that blew straight through, residual exhaust, pumping work,
indicated work (from the pressure trace), peak pressure and where it happens, the crank angle where half the charge
has burned (CA50), the knock integral, exhaust temperature, exhaust manifold pressure and flow.

The engine uses these directly:
- **Torque** = indicated work − pumping work − mechanical friction. Spark timing works only through the burn: too
  early and the pressure rises against the piston coming up, too late and the burn finishes while the piston runs
  away, wasting heat into the exhaust (hotter EGT).
- **Knock** comes from the knock integral, not a formula. The dash shows it as "knock" (1.00 is the edge).
- **Peak cylinder pressure** for the internals model comes from the pressure trace.
- **Turbo** spool and back pressure come from the exhaust side.

## Published vs estimated

Published: bore, stroke, rod length, compression ratio; 35 mm intake and 30 mm exhaust valves, two each, 5.5 mm
stems; VTEC on both cams and VTC on the intake cam; the RBC intake manifold; stock VTEC point 5,800 rpm.

Estimated (Honda doesn't publish them): cam lift, duration and lobe centres; the stock VTC schedule (shaped after the
cam angles Hondata describes); port, runner, manifold and exhaust volumes and lengths; valve flow coefficient; the
exhaust system's effective diameter (52 mm gives about 20 kPa of back pressure at the power peak); burn duration;
wall heat loss; turbine and wastegate flow areas and turbo efficiencies. The knock correlation is from the literature
and scaled once (`knockScale`) so the stock engine on 91 octane is knock limited at low rpm and close to best timing
up top, which is how K20s behave.

A lumped model like this overstates wave effects in long pipes, so runner and primary lengths are "effective" lengths
(about half the physical pipe). After that, `tools/calibrate.mjs` fits a per-rpm correction to the airflow so the
stock engine lands on Honda's rating. The fitted corrections are 0.98–1.06 on the high cam from 5,800 rpm up and 0.79–0.89 on the low cam from 3,000 to
5,800 rpm (1.0 would mean the flow model alone is exact). Below 2,000 rpm they drop to 0.7, where the flow model is
too optimistic.

## Things you can change

In the tune settings: throttle body diameter, intake runner effective length, exhaust effective diameter, and for
turbos the turbine and wastegate areas and compressor wheel size. Advanced: `tune.flow` in a saved tune JSON can
override any field of `FLOW` in `src/gasflow.js` (cam lobes, valve sizes, volumes).
