# How the model is calibrated

The goal is a sim that behaves like a stock 2006–2011 Civic Si before any tune is loaded, so that changes
you make to the tables show up the way they would on the car. Everything below is from public sources.

## Sources

| What | Value used | Source |
|---|---|---|
| Rated power and torque | 197 hp @ 7,800, 139 lb-ft @ 6,200 (SAE J1349) | [Honda 2006 Civic Si specifications](https://hondanews.com/en-US/releases/release-e9dd7ff209cc71cc83947a004c34c1b2-2006-honda-civic-coupe-and-civic-si-specifications) |
| Gear ratios, final drive | 3.267 / 2.130 / 1.517 / 1.147 / 0.921 / 0.659, R 3.583, final 4.765 | same |
| Curb weight, weight split, tires | 2,877 lb, 61/39 front/rear, 215/45R17 | same |
| Redline, compression, bore × stroke | 8,000 rpm, 11.0:1, 86 × 86 mm | same |
| Where the high cam overtakes the low cam | about 5,600 rpm | [Hondata reflash page](https://www.hondata.com/reflash-category/reflash-civicsi-2006): moving VTEC from 5,800 to 4,500 "will lose power between 4500rpm and 5600 rpm" on a stock car |
| Stock wheel dyno | 195.7 whp @ 8,100, 143.5 lb-ft @ 6,160 (Dynojet) | [DSPORT Test & Tune, 2006 Civic Si](https://dsportmag.com/the-tech/test-n-tune/2006-honda-civic-si-part-1/2/) |
| Road test | 0–60 7.2 s, ¼ mile 15.6 s @ 93 mph | [MotorWeek 2006 Civic Si](https://www.motorweek.org/reviews/road_tests/2006_honda_civic) |
| Road test | 0–100 km/h 6.9 s, ¼ mile 14.9 s @ 94.4 mph | [FastestLaps Civic Si](https://fastestlaps.com/models/honda-civic-si) (source magazine not named) |

The DSPORT wheel numbers are higher than the crank rating, which happens with Dynojet readings and correction
factors; they are used only for where the peaks fall (torque near 6,200, power near 8,000), not for the absolute level.

## What was tuned

- **Breathing** comes from the gas exchange model ([GASFLOW.md](GASFLOW.md)): cams, cam phasing, valves, runners
  and exhaust. `node tools/calibrate.mjs` then fits a per-rpm, per-cam correction on top so the stock engine hits the
  crank torque targets in `src/k20z3.js` (`TORQUE_TARGETS`). The anchors are the Honda rating, the peak torque rpm,
  and the 5,600 rpm cam crossover. The low cam shape between anchors is an estimate.
- **Timing and knock** come from the same model (burn phasing and an end-gas autoignition model). Its knock scale was
  set once so the stock engine on 91 octane is knock limited at low rpm and near best timing above about 6,000 rpm.
- **Launch and traction**: front tires limit acceleration (about 0.5 g in 1st), with weight moving off the front as the car accelerates.
- **Car data**: Honda spec ratios and weight, plus an 80 kg driver.

## How well it matches

| Check | Real | Sim |
|---|---|---|
| Peak power | 197 hp @ 7,800 | 197 hp @ 7,800–8,000 |
| Peak torque | 139 lb-ft @ 6,200 | 138–139 lb-ft @ 6,200–6,800 |
| VTEC at 4,500 on a stock engine | loses power 4,500–5,600 | loses power 4,500–5,600 |
| 0–60 mph (1 ft rollout) | 6.5–7.2 s | 6.8 s |
| ¼ mile | 14.9–15.6 s @ 93–94.4 mph | 15.2 s @ 94.3 mph |

These checks run as tests (`npm test`), so a change that breaks them gets caught.

## What isn't calibrated yet

- **Ignition and knock.** Best-torque timing and the knock limit come from the combustion model, which uses textbook
  burn rates and a published autoignition correlation, not stock K20Z3 datalogs. Public FlashPro datalogs are mostly on forums (8thcivic.com, the Hondata forum, k20a.org) that this
  environment can't reach. Datalogs from your own car would be the best way to fix this.
- **Part throttle and cruise.** Manifold pressure versus throttle is a generic curve.
- **Turbo.** Spool comes from exhaust energy, but turbine and wastegate flow areas and efficiencies are estimates, and
  how much boost a given wastegate duty makes depends on the actual solenoid. Match the WG duty gain and turbine area
  to a real boost log.
- **Cam profiles and VTC.** VTC is modelled and follows your cam angle tables, but the stock lobe lift, duration and
  centres aren't published, so they are estimates. Cam card numbers for aftermarket cams can go in `tune.flow.cams`.
