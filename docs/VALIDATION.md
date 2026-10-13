# Sim vs published dynos, paired with the tune that made them

`node tools/validate.mjs` runs each of these and prints the numbers. "Confirmed" means the source says what the tune
was; "guessed" means only the hardware and boost are known, so the sim runs a careful pump-gas tune
(timing 1° under the knock limit, λ 0.78 on boost) in place of the real one.

Public FlashPro datalogs (.fpdl files and CSV exports) are mostly posted on 8thcivic.com, the Hondata forum,
k20a.org and honda-tech.com, which this environment can't reach, so no log-to-log comparisons yet.

| Car and tune | Tune known? | Published | Sim | Match |
|---|---|---|---|---|
| Stock car, stock ECU | Confirmed (stock) | [DSPORT](https://dsportmag.com/the-tech/test-n-tune/2006-honda-civic-si-part-1/2/) Dynojet: 195.7 whp @ 8,100, 143.5 lb-ft @ 6,160 | 174 whp @ 7,900, 122 lb-ft @ 6,200 | Shape matches (peaks within 200 rpm). Level is 11% under: this dyno read above Honda's own crank rating, and the same car read 205 whp in [part 2](https://dsportmag.com/the-tech/test-n-tune/2006-honda-civic-si-part-2/2/), so it reads high. The sim follows Honda's rating. |
| Stock hardware, Hondata reflash with VTEC at 4,500 | Confirmed (Hondata describes it) | [Hondata](https://www.hondata.com/reflash-category/reflash-civicsi-2006): loses power from 4,500 to 5,600 | −17 lb-ft at 4,600, −13 at 5,000, −6 at 5,400, no change from 5,800 up | Matches |
| PRL turbo kit, Precision 6262, stock internals, 93 octane, 1200 cc, FlashPro | Guessed (timing and AFR not given) | [JD Tuning](https://jd-tuning.com/blogs/news/honda-civic-si-2008-turbo): 335 whp @ 5.6 psi, 357 whp @ 7 psi | 255 whp @ 5.6 psi, 275 whp @ 7 psi (287 and 309 on the DSPORT dyno's scale) | Sim is 14–24% low. 335 whp at 5.6 psi needs about 1.7× the stock airflow, more than 5.6 psi alone gives, so the quoted boost is probably not manifold pressure (or the dyno reads high). Treated as unreliable. |
| Kraftwerks supercharger, stock displacement, 12.8 psi, 93 octane, 1000 cc, FlashPro | Guessed (timing and AFR not given) | [DSPORT](https://dsportmag.com/the-cars/features/390-whp-honda-civic-si/): 390 whp and 258 lb-ft @ 8,400 | 359 whp and 224 lb-ft @ 8,400 before supercharger drive losses | About 8% low before drive losses, closer on a high-reading dyno. Reasonable for boosted K20Z3s. |

Not used: DSPORT part 2's FlashPro run (222.8 whp) used Drag Cartel cams, and the sim can't model different cams yet.

## What this says about the model

- Stock behaviour and where VTEC should switch: good.
- On boost the sim tends to read low rather than high, so it errs on the safe side for knock and power, but treat
  boosted numbers as ±15% until checked against a datalog from a known tune.
- The best next step is datalogs paired with the exact calibration that made them (your own car is ideal):
  export a few full-throttle pulls from FlashPro Manager as CSV along with the tune's tables.
