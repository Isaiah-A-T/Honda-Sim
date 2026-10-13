// Cooling and lubrication: coolant and oil temperatures, flows and pressures.
//
// Coolant: the water pump (driven with the engine) pushes coolant through the block's water jacket, up
// through the head gasket into the head, and out of the head toward the thermostat. Below the thermostat's
// opening temperature it all returns through the bypass straight to the pump, so the engine warms up fast;
// as the thermostat opens, more goes out the upper hose, through the radiator and back down the lower hose.
// The radiator gives its heat to air pushed through it by road speed and the cooling fans, which the ECU
// switches on and off on coolant temperature. Above the coolant's boiling point at the cap pressure the
// coolant flashes to steam at the hottest walls and stops cooling them: that's an overheat.
//
// Oil: the crank-driven pump pushes oil through the filter into the main gallery, to the main and rod
// bearings, up to the head (cam journals, rocker shafts, VTEC spool and pins, VTC actuator) and the chain
// tensioner, and it drains back to the sump. It picks up heat from friction and from the pistons and passes
// most of it to the block (and so the coolant), some to the air around the sump, and to an oil cooler if
// one is fitted. Oil gets thinner as it heats up (viscosity from the Vogel equation), which lowers friction
// and oil pressure; a relief valve caps the pressure when cold oil would push it too high.
//
// Effects on the engine (engine.js, gasflow.js, internals.js): the head and port walls heat the intake air
// (knock, breathing) and take heat from combustion; oil viscosity sets friction and oil pressure (which
// strokes the VTEC pins); the ECU holds VTEC off below its coolant threshold; bearing film strength falls
// with thin hot oil; the pistons run as hot as their oil cooling allows; boiling coolant overheats the head.
//
// Lumped model: one node for the engine (metal plus the coolant in it), one for the radiator, one for the oil.
// Sizes, temperatures and pressures come from docs/cooling-oil-specs.json where published; the heat
// split, heat transfer coefficients and thermal masses are engineering estimates (listed in COOLING).

import { clamp } from './tables.js';

export const COOLING = {
  // Coolant and thermostat
  coolantL: 5.3, engineCoolantShare: 0.55,       // estimate split between engine passages and radiator/hoses
  thermostatOpenC: 78, thermostatFullC: 90,       // to be replaced from docs/cooling-oil-specs.json
  pumpLpmPerKrpm: 20,                              // estimate: ~120 L/min at 6,000 rpm, typical for a 2.0 L
  capKpa: 108,                                     // radiator cap (gauge)
  // Radiator and fans
  radiatorFaceM2: 0.24, radiatorUaWkAt5ms: 1500,  // estimate for a single-row aluminium core
  grilleShare: 0.30,                               // share of road speed that reaches the core face (estimate)
  fanFaceMs: 3.0, fans: 2,                         // face velocity the fans make at a standstill (estimate)
  fanOnC: 98, fanOffC: 93,                         // to be replaced from docs/cooling-oil-specs.json
  // Thermal masses, J/K (estimates: ~45 kg of aluminium and iron/steel in the engine, 4.2 L of oil)
  engineMetalJk: 45000, radiatorMetalJk: 4500, oilJk: 9000,
  // Heat paths
  wallToCoolant: 0.85,                             // share of combustion wall heat into the coolant; the rest into the oil via the pistons
  frictionToOil: 0.5,                              // share of friction heat into the oil
  oilToEngineUaWk: 200, oilToEngineUaPerKrpm: 100, // oil sprayed and pumped through the hot block (estimate)
  sumpUaWk: 15, sumpUaPerMs: 1.5,                  // oil pan to air
  oilCooler: 'none',                               // 'none' | 'water' (oil-to-coolant at the filter) | 'air' (aftermarket)
  oilCoolerUaWk: 250,
  // Oil
  oilL: 4.2, oilGrade: '0W-20',
  // Kinematic viscosity at 40 and 100 °C (cSt), typical for the grade; Vogel fit through them.
  oilVisc40: 44, oilVisc100: 8.5,
  oilReliefKpa: 600,
  oilHotIdleKpa: 80, oilKpaPerRpm: 0.065,          // hot-oil pressure line (as before; matches service-manual minimums)
  ectVtecMinC: 60,                                 // ECU won't engage VTEC on a cold engine (estimate until confirmed)
};

const CP_COOLANT = 3500, RHO_COOLANT = 1.04;      // 50/50 ethylene glycol, J/kg·K and kg/L
const RHO_AIR = 1.18, CP_AIR = 1005;

// Vogel: ν = A·exp(B / (T + C)), with C fixed at 95 and A, B fitted through the two published points.
export function viscosityCst(oilC, g = COOLING) {
  const C = 95, t1 = 40, t2 = 100;
  const B = Math.log(g.oilVisc40 / g.oilVisc100) / (1 / (t1 + C) - 1 / (t2 + C));
  const A = g.oilVisc100 / Math.exp(B / (t2 + C));
  return A * Math.exp(B / (clamp(oilC, -30, 180) + C));
}
// Viscosity relative to hot (100 °C) oil, which is what the friction and oil pressure figures were set for.
export const viscRatio = (oilC, g = COOLING) => viscosityCst(oilC, g) / g.oilVisc100;

// Coolant boiling point at the cap pressure (°C): 50/50 glycol boils at about 107 °C at sea level and
// about 0.2 °C higher per kPa of cap pressure.
export const boilingC = (g = COOLING) => 107 + 0.2 * g.capKpa;

export function createThermal(ambientC = 25, warm = true, overrides = {}) {
  const g = { ...COOLING, ...overrides };
  return {
    g,
    coolantC: warm ? 90 : ambientC,        // engine outlet (what the ECT sensor reads)
    radiatorC: warm ? 70 : ambientC,
    oilC: warm ? 100 : ambientC,
    thermostat: warm ? clamp((90 - g.thermostatOpenC) / (g.thermostatFullC - g.thermostatOpenC), 0, 1) : 0,
    fanOn: false,
    pumpLpm: 0, radiatorLpm: 0, bypassLpm: 0,
    airMs: 0, radiatorKw: 0, heatToCoolantKw: 0, heatToOilKw: 0, oilToCoolantKw: 0, sumpKw: 0, oilCoolerKw: 0,
    radiatorOutC: warm ? 70 : ambientC, inletC: warm ? 84 : ambientC,
    boiling: false, overheatS: 0,
  };
}

// Step dt seconds. fuelGs: fuel burned (g/s); frictionW: mechanical friction power; heatLoss: share of
// fuel heat lost to the walls (from the gas model); speedMs: road speed for ram air; ambientC.
export function stepThermal(th, { rpm, fuelGs, frictionW, heatLoss, speedMs, mapKpa = 100, ambientC = 25 }, dt) {
  const g = th.g;
  const fuelW = fuelGs / 1000 * 44e6;
  // Wall heat: three quarters of the gas model's heat loss (the rest is crevices and unburned fuel), a larger
  // share at light load, where the charge spends longer per unit of work against the walls (Heywood ch. 12).
  const light = 1 + 0.8 * clamp(1 - mapKpa / 100, 0, 1);
  const wallW = fuelW * heatLoss * 0.75 * light;
  const toCoolant = wallW * g.wallToCoolant + frictionW * (1 - g.frictionToOil);
  const toOil = wallW * (1 - g.wallToCoolant) + frictionW * g.frictionToOil;

  th.pumpLpm = g.pumpLpmPerKrpm * Math.max(rpm, 0) / 1000;
  th.radiatorLpm = th.pumpLpm * th.thermostat;
  th.bypassLpm = th.pumpLpm - th.radiatorLpm;
  // The thermostat sits on the pump inlet, so it feels the mix of radiator return and bypass, which is
  // cooler than the head outlet by the heat the engine adds on the way through. Wax element, a few seconds' lag.
  const mix = th.pumpLpm > 0 ? (th.radiatorOutC * th.radiatorLpm + th.coolantC * th.bypassLpm) / th.pumpLpm : th.coolantC;
  const cPump = th.pumpLpm / 60 * RHO_COOLANT * CP_COOLANT;
  th.inletC = cPump > 0 ? Math.min(mix, th.coolantC - th.heatToCoolantKw * 1000 / cPump) : th.coolantC;
  const want = clamp((th.inletC - g.thermostatOpenC) / (g.thermostatFullC - g.thermostatOpenC), 0, 1);
  th.thermostat += (want - th.thermostat) * Math.min(1, dt / 4);

  // Fans: on above fanOnC, off below fanOffC.
  if (th.coolantC >= g.fanOnC) th.fanOn = true;
  else if (th.coolantC <= g.fanOffC) th.fanOn = false;
  const ram = g.grilleShare * Math.abs(speedMs), fan = th.fanOn ? g.fanFaceMs : 0;
  th.airMs = Math.sqrt(ram * ram + fan * fan) + 0.3;   // a little natural convection

  // Radiator: effectiveness-NTU, crossflow with both fluids unmixed (approximate formula).
  const mCool = th.radiatorLpm / 60 * RHO_COOLANT, cCool = mCool * CP_COOLANT;
  const cAir = RHO_AIR * g.radiatorFaceM2 * th.airMs * CP_AIR;
  let qRad = 0;
  if (cCool > 0) {
    const cMin = Math.min(cCool, cAir), cMax = Math.max(cCool, cAir), cr = cMin / cMax;
    const ua = g.radiatorUaWkAt5ms * (th.airMs / 5) ** 0.6;
    const ntu = ua / cMin;
    const eff = 1 - Math.exp((ntu ** 0.22 / cr) * (Math.exp(-cr * ntu ** 0.78) - 1));
    qRad = eff * cMin * (th.coolantC - ambientC);
  }
  th.radiatorOutC = cCool > 0 ? th.coolantC - qRad / cCool : th.radiatorC;
  // Coolant leaving the radiator mixes with the bypass flow back at the pump.
  const engineJk = g.engineMetalJk + g.coolantL * g.engineCoolantShare * RHO_COOLANT * CP_COOLANT;
  const radJk = g.radiatorMetalJk + g.coolantL * (1 - g.engineCoolantShare) * RHO_COOLANT * CP_COOLANT;
  const exchange = cCool * (th.coolantC - th.radiatorC);           // hot coolant in, cooled coolant back

  // Oil: heat from friction and pistons; to the block, the sump's air and any cooler.
  const uaOil = g.oilToEngineUaWk + g.oilToEngineUaPerKrpm * Math.max(rpm, 0) / 1000;
  const oilToEngine = uaOil * (th.oilC - th.coolantC);
  const sump = (g.sumpUaWk + g.sumpUaPerMs * Math.abs(speedMs)) * (th.oilC - ambientC);
  const cooler = g.oilCooler === 'water' ? g.oilCoolerUaWk * (th.oilC - th.coolantC)
    : g.oilCooler === 'air' ? g.oilCoolerUaWk * (th.airMs / 5) ** 0.6 * (th.oilC - ambientC) : 0;
  const coolerToCoolant = g.oilCooler === 'water' ? cooler : 0;

  // Boiling: past the boiling point the hottest walls blanket in steam and stop passing heat well.
  const boil = boilingC(g);
  th.boiling = th.coolantC > boil;
  th.overheatS = th.boiling ? th.overheatS + dt : Math.max(0, th.overheatS - dt);

  th.coolantC += (toCoolant + oilToEngine + coolerToCoolant - exchange) / engineJk * dt;
  th.radiatorC += (exchange - qRad) / radJk * dt;
  th.oilC += (toOil - oilToEngine - sump - cooler) / g.oilJk * dt;

  th.heatToCoolantKw = toCoolant / 1000; th.heatToOilKw = toOil / 1000; th.radiatorKw = qRad / 1000;
  th.oilToCoolantKw = (oilToEngine + coolerToCoolant) / 1000; th.sumpKw = sump / 1000; th.oilCoolerKw = cooler / 1000;
  return th;
}

// Oil pressure (kPa): flow through the bearings and galleries rises with pump speed and with viscosity,
// until the relief valve opens. Same as before for hot oil.
export function oilPressureKpa(rpm, oilC, g = COOLING) {
  const hot = g.oilHotIdleKpa + rpm * g.oilKpaPerRpm;
  return clamp(hot * viscRatio(oilC, g) ** 0.5, 0, g.oilReliefKpa);
}

// Circuits for renderers: each segment's name, the parts it joins, and what flows in it right now.
export function circuits(th) {
  const g = th.g, oilLpm = Math.min(60, 4 + Math.max(0, th.pumpLpm) * 0.25);
  const cool = [
    ['pump', 'blockJacket', 'Water pump to block water jacket', th.pumpLpm, th.radiatorOutC * (th.radiatorLpm / Math.max(th.pumpLpm, 1e-6)) + th.coolantC * (th.bypassLpm / Math.max(th.pumpLpm, 1e-6))],
    ['blockJacket', 'headJacket', 'Block to head through the head gasket', th.pumpLpm, th.coolantC - 3],
    ['headJacket', 'waterOutlet', 'Head to water outlet', th.pumpLpm, th.coolantC],
    ['waterOutlet', 'radiator', 'Upper radiator hose', th.radiatorLpm, th.coolantC],
    ['radiator', 'thermostat', 'Lower radiator hose', th.radiatorLpm, th.radiatorOutC],
    ['waterOutlet', 'thermostat', 'Bypass', th.bypassLpm, th.coolantC],
    ['thermostat', 'pump', 'Thermostat to pump inlet', th.pumpLpm, null],
    ['waterOutlet', 'heaterCore', 'Heater hoses (feed and return)', 0, th.coolantC],
  ].map(([from, to, name, lpm, tempC]) => ({ from, to, name, flowLpm: lpm, tempC }));
  const oil = [
    ['sump', 'pump', 'Pickup', oilLpm],
    ['pump', 'filter', 'Pump to filter', oilLpm],
    ['filter', 'mainGallery', 'Filter to main gallery', oilLpm],
    ['mainGallery', 'mainBearings', 'Main bearings, then through the crank to the rod bearings', oilLpm * 0.5],
    ['mainGallery', 'head', 'Up to the head: cam journals, rocker shafts, VTEC spool and pins', oilLpm * 0.3],
    ['mainGallery', 'vtc', 'VTC actuator and chain tensioner', oilLpm * 0.1],
    ['head', 'sump', 'Drain back', oilLpm * 0.4],
  ].map(([from, to, name, lpm]) => ({ from, to, name, flowLpm: lpm, tempC: th.oilC }));
  if (g.oilCooler !== 'none') oil.splice(2, 0, { from: 'filter', to: 'oilCooler', name: g.oilCooler === 'water' ? 'Oil-to-coolant cooler at the filter' : 'Oil cooler lines', flowLpm: oilLpm, tempC: th.oilC });
  return { coolant: cool, oil };
}
