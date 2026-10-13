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
  // 6.8 L total less the 0.4 L reserve tank (2006 Si owner's manual); engine/radiator split is an estimate.
  coolantL: 6.4, engineCoolantShare: 0.55,
  thermostatOpenC: 78, thermostatFullC: 90,       // K-series factory 78 °C (76–80 °C, K20A manual), fully open 90 °C, 8 mm lift
  pumpLpmPerKrpm: 20,                              // estimate: ~120 L/min at 6,000 rpm, typical for a 2.0 L
  capKpa: 110,                                     // radiator cap 16 psi (19045-RAA-003; CSF's Civic Si spec)
  // Radiator and fans
  radiatorFaceM2: 0.254, radiatorUaWkAt5ms: 1500, // Denso 19010-RNB-A51: single-row aluminium core about 375 × 678 × 16 mm; UA is an estimate
  grilleShare: 0.30,                               // share of road speed that reaches the core face (estimate)
  fanFaceMs: 3.0, fans: 2,                         // radiator fan (driver side) and condenser fan; face velocity is an estimate
  // The ECU runs the fans off ECT2, the sensor in the radiator's bottom tank (Hondata). Switch points from
  // the K20A-family manual's fan switch: on at 91–95 °C, off 3–8 °C lower.
  fanOnC: 93, fanOffC: 88,
  // Thermal masses, J/K (estimates: ~45 kg of aluminium and iron/steel in the engine, 4.2 L of oil)
  engineMetalJk: 45000, radiatorMetalJk: 4500, oilJk: 9000,
  // Heat paths
  wallToCoolant: 0.85,                             // share of combustion wall heat into the coolant; the rest into the oil via the pistons
  frictionToOil: 0.5,                              // share of friction heat into the oil
  oilToEngineUaWk: 200, oilToEngineUaPerKrpm: 100, // oil sprayed and pumped through the hot block (estimate)
  sumpUaWk: 15, sumpUaPerMs: 1.5,                  // oil pan to air
  oilCooler: 'water',                              // Si: water-to-oil cooler at the filter, fed from the water pump (15500-RBC-004); 'none' | 'water' | 'air'
  oilCoolerUaWk: 250,
  // Oil
  oilL: 4.4, oilGrade: '5W-30',                   // 4.4 L with filter; 5W-30 per the 2006 Si owner's manual
  // Kinematic viscosity at 40 and 100 °C (cSt), typical for the grade; Vogel fit through them.
  oilVisc40: 62, oilVisc100: 10.5,
  oilReliefKpa: 600,                               // relief opening not published; estimate
  // Hot (100 °C) oil pressure line. Manual minimums with 80 °C oil: 70 kPa at idle, 300 kPa at 3,000 rpm.
  oilHotIdleKpa: 80, oilKpaPerRpm: 0.09,
  oilJetKpa: 200,                                  // piston oil jets' check balls open at 200 kPa (K20A manual)
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
    radiatorOutC: warm ? 70 : ambientC, ect2C: warm ? 70 : ambientC, inletC: warm ? 84 : ambientC,
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

  // Fans: on when the radiator's outlet (ECT2) passes fanOnC, off below fanOffC.
  // ECT2 sits in the bottom tank, which mixes the outlet coolant over several seconds.
  th.ect2C += (th.radiatorOutC - th.ect2C) * Math.min(1, dt / 15);
  if (th.ect2C >= g.fanOnC) th.fanOn = true;
  else if (th.ect2C <= g.fanOffC) th.fanOn = false;
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
export function circuits(th, oilKpa = 0) {
  const g = th.g, oilLpm = Math.min(60, 4 + Math.max(0, th.pumpLpm) * 0.25);
  const cool = [
    ['pump', 'blockJacket', 'Water pump to block water jacket', th.pumpLpm, th.radiatorOutC * (th.radiatorLpm / Math.max(th.pumpLpm, 1e-6)) + th.coolantC * (th.bypassLpm / Math.max(th.pumpLpm, 1e-6))],
    ['blockJacket', 'headJacket', 'Block to head through the head gasket', th.pumpLpm, th.coolantC - 3],
    ['headJacket', 'waterOutlet', 'Head to water outlet', th.pumpLpm, th.coolantC],
    ['waterOutlet', 'radiator', 'Upper radiator hose', th.radiatorLpm, th.coolantC],
    ['radiator', 'thermostat', 'Lower radiator hose', th.radiatorLpm, th.radiatorOutC],
    ['waterOutlet', 'bypass', 'Bypass (warm-up loop back to the pump)', th.bypassLpm, th.coolantC],
    ['bypass', 'thermostat', 'Bypass into the thermostat case', th.bypassLpm, th.coolantC],
    ['thermostat', 'pump', 'Thermostat to pump inlet', th.pumpLpm, null],
    ['waterOutlet', 'heaterCore', 'Heater feed hose', 0, th.coolantC],
    ['heaterCore', 'pump', 'Heater return hose', 0, th.coolantC],
    ['radiator', 'reserveTank', 'Overflow to the reserve tank', 0, th.radiatorC],
  ].map(([from, to, name, lpm, tempC]) => ({ from, to, name, flowLpm: lpm, tempC }));
  const oil = [
    ['sump', 'oilPump', 'Pickup', oilLpm],
    ['oilPump', 'filter', 'Pump to filter', oilLpm],
    ['filter', 'mainGallery', 'Filter to main gallery', oilLpm],
    ['mainGallery', 'mainBearings', 'Main bearings, then through the crank to the rod bearings', oilLpm * 0.5],
    ['mainGallery', 'pistonJets', 'Piston oil jets (open above 200 kPa)', oilKpa >= g.oilJetKpa ? oilLpm * 0.1 : 0],
    ['mainGallery', 'head', 'Up to the head: cam journals, rocker shafts, VTEC spool and pins', oilLpm * 0.3],
    ['mainGallery', 'vtc', 'VTC actuator and chain tensioner', oilLpm * 0.1],
    ['head', 'sump', 'Drain back', oilLpm * 0.4],
  ].map(([from, to, name, lpm]) => ({ from, to, name, flowLpm: lpm, tempC: th.oilC }));
  if (g.oilCooler !== 'none') {
    // Oil passes through the cooler on its way to the filter; the water cooler takes coolant from a port on
    // the water pump and returns it to the pump's inlet side.
    oil.splice(1, 1, { from: 'oilPump', to: 'oilCooler', name: g.oilCooler === 'water' ? 'Pump to the water-to-oil cooler' : 'Oil cooler feed line', flowLpm: oilLpm, tempC: th.oilC },
      { from: 'oilCooler', to: 'filter', name: 'Cooler to filter', flowLpm: oilLpm, tempC: th.oilC });
    if (g.oilCooler === 'water') cool.push(
      { from: 'pump', to: 'oilCooler', name: 'Oil cooler in hose (19422-RRB-A01)', flowLpm: th.pumpLpm * 0.05, tempC: th.inletC },
      { from: 'oilCooler', to: 'thermostat', name: 'Oil cooler outlet hose (19423-RRB-A01)', flowLpm: th.pumpLpm * 0.05, tempC: th.inletC + th.oilCoolerKw * 1000 / Math.max(th.pumpLpm * 0.05 / 60 * RHO_COOLANT * CP_COOLANT, 1) });
  }
  return { coolant: cool, oil };
}

// Where each component of the two circuits sits, for drawing. Engine frame in metres: origin on the crank
// axis midway along the block, +x along the crank toward the timing-chain end (cylinder 1, passenger side
// in the Civic), +y up the bores, +z toward the front of the car (the Si's intake side, facing the radiator;
// its exhaust faces the firewall). size is a rough bounding box. Part numbers and the sides things are on
// come from docs/cooling-oil-specs.json; exact positions are approximate placements for drawing, not measured.
export const COMPONENTS = {
  // Coolant
  pump:        { circuit: 'coolant', name: 'Water pump (19200-RBC-013), belt driven, timing-chain end', pos: [0.22, 0.10, 0.12], size: [0.06, 0.14, 0.14] },
  thermostat:  { circuit: 'coolant', name: 'Thermostat (19301-RAF-004) in its case at the pump inlet', pos: [0.22, 0.02, 0.18], size: [0.07, 0.07, 0.07] },
  blockJacket: { circuit: 'coolant', name: 'Block water jacket around the four iron liners', pos: [0, 0.17, 0], size: [0.40, 0.10, 0.14] },
  headJacket:  { circuit: 'coolant', name: 'Head water jacket (ECT1 sensor here)', pos: [0, 0.25, 0], size: [0.42, 0.06, 0.18] },
  waterOutlet: { circuit: 'coolant', name: 'Water neck / EGR passage (18712-RRA-A00) on the head, transmission end', pos: [-0.24, 0.27, 0.06], size: [0.06, 0.06, 0.08] },
  bypass:      { circuit: 'coolant', name: 'Bypass hose (19504-RAC-000) under the intake manifold', pos: [0, 0.12, 0.16], size: [0.44, 0.03, 0.03] },
  radiator:    { circuit: 'coolant', name: 'Radiator (Denso 19010-RNB-A51), single-row down-flow, plastic tanks, 16 psi cap', pos: [0, 0.10, 0.62], size: [0.68, 0.375, 0.016] },
  radiatorFan: { circuit: 'coolant', name: 'Radiator fan (19020-RRA-A01), driver side', pos: [-0.17, 0.10, 0.58], size: [0.37, 0.37, 0.10] },
  condenserFan:{ circuit: 'coolant', name: 'Condenser fan (38611-RRA-A01), passenger side', pos: [0.17, 0.10, 0.58], size: [0.37, 0.37, 0.09] },
  reserveTank: { circuit: 'coolant', name: 'Reserve tank, 0.4 L', pos: [0.32, 0.28, 0.52], size: [0.10, 0.12, 0.08] },
  heaterCore:  { circuit: 'coolant', name: 'Heater core (behind the firewall)', pos: [0, 0.30, -0.80], size: [0.20, 0.15, 0.05] },
  // Oil
  sump:        { circuit: 'oil', name: 'Cast aluminium oil pan, 4.4 L with filter', pos: [0, -0.13, 0], size: [0.42, 0.12, 0.26] },
  oilPump:     { circuit: 'oil', name: 'Rotor oil pump, chain driven, in the balancer unit in the pan', pos: [0, -0.07, 0], size: [0.20, 0.06, 0.12] },
  filter:      { circuit: 'oil', name: 'Oil filter', pos: [-0.05, 0.02, -0.16], size: [0.08, 0.09, 0.08] },
  oilCooler:   { circuit: 'oil', name: 'Water-to-oil cooler (15500-RBC-004), sandwiched under the filter', pos: [-0.05, 0.02, -0.12], size: [0.09, 0.04, 0.09] },
  mainGallery: { circuit: 'oil', name: 'Main oil gallery along the block', pos: [0, 0.03, -0.08], size: [0.40, 0.012, 0.012] },
  mainBearings:{ circuit: 'oil', name: 'Main and rod bearings', pos: [0, 0, 0], size: [0.40, 0.06, 0.06] },
  pistonJets:  { circuit: 'oil', name: 'Piston oil jets (15280-PRB-A00), one per cylinder', pos: [0, 0.06, -0.05], size: [0.36, 0.02, 0.02] },
  head:        { circuit: 'oil', name: 'Cam journals, rocker shafts, VTEC spool valve and pressure switch', pos: [0, 0.32, 0], size: [0.42, 0.06, 0.18] },
  vtc:         { circuit: 'oil', name: 'VTC oil control valve (15830-RBB-003) and intake cam actuator', pos: [0.22, 0.32, 0.06], size: [0.05, 0.10, 0.10] },
};
