# Fuel, injectors and boost

How the fuel you pick, the injectors and pump you fit, and the boost you run change what the engine
makes (src/fuels.js, src/engine.js, src/gasflow.js; ported to native/sim/src/fuels.rs). The data and
sources are in docs/boost-fuel-data.json.

## What already worked

The gas exchange model takes manifold pressure and charge temperature directly, so boost, the
intercooler and the turbo's compressor efficiency change the air trapped each cycle, and the fuel
burned with it. Injectors cap fuel: when the commanded pulse is longer than the cycle the engine
runs lean. Octane only matters where the timing is knock-limited, which is how a real engine behaves.

One thing was broken: the turbo setup kept the NA tune's maps, which stop at 100 kPa, so every
boosted cell read the 100 kPa column. Fuel stayed at the NA target and timing stayed at NA advance,
the knock model pulled the full 8° and octane made no difference. The turbo setup now adds boost
columns (120 to 250 kPa) with richer targets and timing pulled 0.15° per kPa.

## Fuel system

- **Rail:** 50 psi on every engine. K-series systems are returnless (the regulator is in the
  tank), so the rail doesn't rise with boost.
- **Injectors:** rated at 3 bar. Flow follows the square root of the pressure across the
  injector (rail plus atmosphere minus manifold), so injectors flow less as boost rises. In lambda
  or AFR mode the ECU lengthens the pulse to compensate; in injector-ms mode it doesn't.
- **Pump:** 20 to 1000 l/h in 10 l/h steps. The stock pump's flow isn't published; 160 l/h
  puts the stock returnless system's limit near the 450 hp tuners quote. Asked for more than it
  moves, the rail sags with the square of the shortfall and the engine gets only what the pump
  delivers, so it runs lean.

## Fuels

| Fuel | Octane (AKI) | Stoich | LHV MJ/kg | Heat of vaporization kJ/kg |
|---|---|---|---|---|
| 87 / 91 / 93 pump gas (E10) | 87 / 91 / 93 | 14.1 | 41.6 | 410 |
| E85 | 96 | 9.7 | 29.1 | 840 |
| Sunoco 260 GT | 100 | 14.1 | 41.6 | 410 |
| VP C16 | 118 | 14.7 | 43.4 | 349 |

E10 and E85 are mass-weighted blends of gasoline and ethanol. Two physical effects come from the
fuel, both measured against 91 pump gas (the fuel the stock engine was calibrated on):

- **Energy per kg of air:** LHV ÷ stoich. E85 releases about 1.5% more heat per kg of air at
  the same lambda.
- **Charge cooling:** the extra heat the fuel takes to evaporate, fuel/air ratio × heat of
  vaporization ÷ cp of air. A quarter of it cools the charge in the port (denser charge) and a
  quarter lowers the end-gas temperature (less knock); the rest goes into the walls. E85 at
  λ 0.8 cools about 33 K more than pump gas.

The stock engine on 91 makes exactly what it did before.

## Against dynos

| Build | Measured | Model |
|---|---|---|
| Stock K20Z3, Dynojet (DSPORT, two pulls) | 196–205 whp | 197 hp (Honda rates it 197 hp at the crank) |
| K20Z3, Kraftwerks supercharger at 12.8 psi, 93 (DSPORT) | 390 whp | 359 hp |
| K24A4, Precision 6266 at 15 psi, 93 (Hondata forum) | 340 whp | 332 hp |

The supercharged K20Z3 at 12.8 psi on a 255 l/h pump makes 357 hp on 91, 359 on 93 and 382 on E85
(+7%). On the stock pump E85 asks for 107% of its flow, the rail sags to 44 psi and the engine runs
lean (λ 0.85 for a 0.80 target). On 93 with 440 cc injectors it maxes them out, runs at λ 1.01 and makes
335 hp.

## Limits

- Exhaust temperature falls when the mixture goes lean in the model. Real engines usually run
  hotter just lean of best power.
- Flex-fuel blends (E30, E50) aren't in the list.
