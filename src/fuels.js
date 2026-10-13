// Fuels, and the fuel system that delivers them (rail pressure, injectors, pump).
//
// Properties from docs/boost-fuel-data.json (sources there): gasoline LHV 43.4 MJ/kg and heat of
// vaporization 150 Btu/lb (349 kJ/kg), ethanol 26.7 MJ/kg and 396 Btu/lb (921 kJ/kg), stoichiometric
// AFR 14.1 for E10 and 9.7 for E85, specific gravities as listed. Blends (E10, E85) take the mass-weighted
// LHV and heat of vaporization of their gasoline and ethanol (10% and 85% ethanol by volume). Octane is the
// pump number (AKI), which is what the knock model uses: E85's is the spec minimum (RON 104, MON 88);
// what E85 gains beyond that comes from its charge cooling, which the gas model works out separately.

const GAS = { lhv: 43.4, hv: 349, sg: 0.745 }, ETH = { lhv: 26.7, hv: 921, sg: 0.794 };
function blend(ethVol) {
  const m = ethVol * ETH.sg / (ethVol * ETH.sg + (1 - ethVol) * GAS.sg);   // ethanol mass fraction
  return { lhvMJ: m * ETH.lhv + (1 - m) * GAS.lhv, hvKJ: m * ETH.hv + (1 - m) * GAS.hv, density: ethVol * ETH.sg + (1 - ethVol) * GAS.sg };
}
const e10 = blend(0.10), e85 = blend(0.85);

export const FUELS = [
  { id: 'pump87', name: '87 pump gas (E10)', octane: 87, stoich: 14.1, ...e10 },
  { id: 'pump91', name: '91 pump gas (E10)', octane: 91, stoich: 14.1, ...e10 },
  { id: 'pump93', name: '93 pump gas (E10)', octane: 93, stoich: 14.1, ...e10 },
  { id: 'e85', name: 'E85', octane: 96, stoich: 9.7, ...e85 },
  // Sunoco 260 GT: AKI 100, stoich 14.1, 17,900 Btu/lb, SG 0.734; 3.7% oxygen, so E10-like cooling (estimate).
  { id: 'sunoco260', name: 'Sunoco 260 GT (100 octane)', octane: 100, stoich: 14.1, lhvMJ: 41.6, hvKJ: e10.hvKJ, density: 0.734 },
  // VP C16: MON 116, AKI over 118 (118 used), stoich 14.7, SG 0.735, no oxygen; LHV and cooling as gasoline (estimate).
  { id: 'c16', name: 'VP C16 race fuel (118 octane)', octane: 118, stoich: 14.7, lhvMJ: GAS.lhv, hvKJ: GAS.hv, density: 0.735 },
];
// The gas model's calibration fuel: the stock engine was fitted on 91 pump gas.
export const REF_FUEL = FUELS[1];

// The fitted fuel. Tunes from before the fuel list carry only an octane; they get pump gas with it.
export function fuelOf(tune) {
  const f = FUELS.find(x => x.id === tune?.fuel);
  if (f) return f;
  const oct = tune?.octane ?? 91;
  return { ...REF_FUEL, id: 'pump', name: `${oct} pump gas (E10)`, octane: oct };
}

// Fuel system. Every engine gets a 50 psi rail (Fenta's spec; K-series systems are returnless with the
// regulator in the tank, so the rail doesn't rise with boost). Injector flow is rated at 3 bar (43.5 psi,
// how Injector Dynamics and most makers rate them) and follows the square root of the pressure across
// the injector, rail minus manifold. The pump delivers its rated flow at the rail pressure; asked for more,
// the rail sags and the engine gets only what the pump moves.
export const RAIL_KPA = 50 * 6.895;          // gauge
export const INJECTOR_RATED_KPA = 300;
export const STOCK_PUMP_LPH = 160;            // OEM flow not published; sized so the stock returnless system tops out near 450 hp (Evans Tuning)
export const PUMP_LPH_MIN = 20, PUMP_LPH_MAX = 1000, PUMP_LPH_STEP = 10;
export const pumpLphOf = tune => Math.round(Math.min(PUMP_LPH_MAX, Math.max(PUMP_LPH_MIN, tune?.fuelPumpLph ?? STOCK_PUMP_LPH)) / PUMP_LPH_STEP) * PUMP_LPH_STEP;

// Injector flow (g/ms) at a manifold pressure, for injectors rated `cc` cc/min at 3 bar.
export function injectorGPerMs(cc, fuel, mapKpa) {
  const dp = Math.max(RAIL_KPA + 101.3 - mapKpa, 0);
  return cc * fuel.density / 60000 * Math.sqrt(dp / INJECTOR_RATED_KPA);
}
// What the pump can deliver (g/s).
export const pumpGs = (lph, fuel) => lph * fuel.density / 3.6;

// Charge cooling from the fuel evaporating, relative to the calibration fuel at the same lambda (the
// calibration already holds pump gas's own cooling). Kelvin, before any share is applied:
// fuel/air ratio × heat of vaporization ÷ cp of air.
export function extraCoolingK(fuel, lambda) {
  const lam = Math.max(lambda, 0.5);
  const q = f => f.hvKJ * 1000 / (f.stoich * lam);
  return (q(fuel) - q(REF_FUEL)) / 1005;
}
