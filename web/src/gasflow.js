// Gas exchange: one cylinder followed degree by degree through a whole four-stroke cycle.
//
// This is a "filling and emptying" model, the kind engine programs use before going to full 1D wave
// codes. Each crank degree it works out:
//   - cylinder volume from the slider-crank geometry,
//   - how far each valve is open from the cam profile (intake cam phased by VTC),
//   - compressible flow through the valve curtains in either direction (so overlap, backflow into the
//     intake at low rpm and blow-through on boost all happen on their own),
//   - the intake runner as a column of air with mass: it accelerates toward the port when the valve
//     opens and keeps ramming air in after the piston has stopped pulling (inertia/Helmholtz tuning),
//   - the exhaust primary the same way, into a collector that empties through the exhaust system
//     (or a turbine and wastegate),
//   - combustion as a Wiebe heat-release curve that starts at the spark.
// Out of one cycle it gives what an engine dyno can't see directly: trapped fresh air (volumetric
// efficiency), residual exhaust left in the cylinder, pumping work, peak pressure, exhaust temperature
// and the pressure and energy in the exhaust manifold that a turbine can use.
//
// Published: bore 86.0, stroke 85.9, 11.0:1 (Honda); 35 mm intake and 30 mm exhaust valves, 2 each,
// 5.5 mm stems; VTEC on both cams, VTC on the intake cam. Honda doesn't publish K20Z3 cam lift,
// duration, lobe centres, port or runner sizes, so those are estimates in the normal range for a K20
// head and RBC manifold, and the stock VTC schedule is shaped after the cam angles Hondata describes.
// tools/calibrate.mjs then fits a small per-rpm correction so the stock engine lands on Honda's rating.

import { clamp, lookup1, lookup2 } from './tables.js';

export const FLOW = {
  boreM: 0.086, strokeM: 0.0859, rodM: 0.139, compression: 11.0,      // published
  inValveM: 0.035, exValveM: 0.030, valves: 2, stemM: 0.0055,          // published
  // Cam lobes (estimates). Duration is seat to seat in crank degrees; lift at the valve.
  cams: {
    low: { inLift: 0.0095, inDur: 228, exLift: 0.0090, exDur: 222 },
    high: { inLift: 0.0117, inDur: 262, exLift: 0.0110, exDur: 256 },
  },
  intakeCentre0: 130,     // intake lobe centre, degrees after overlap TDC, with VTC at 0 (fully retarded)
  exhaustCentre: 105,     // exhaust lobe centre, degrees before overlap TDC
  valveCd: 0.72,           // effective flow coefficient on the valve curtain (estimate for a good 4-valve head)
  throttleDiaM: 0.062,    // drive-by-wire throttle body bore (estimate)
  // Intake: RBC manifold runner and port (estimates). runnerM is the effective length of the air column
  // the model treats as one slug; a lumped slug overstates wave effects, so it is about half the
  // physical runner.
  runnerM: 0.15, runnerDiaM: 0.044, intakePortL: 0.40,
  // Exhaust: stock cast manifold with its close-coupled cat, primary (effective slug length, as above) and collector (estimates).
  primaryM: 0.15, primaryDiaM: 0.036, exhaustPortL: 0.35, collectorL: 1.6,
  // The rest of the stock exhaust (cat, resonator, muffler) as one orifice with this effective
  // diameter, sized for about 20 kPa of back pressure at the 7,800 rpm power peak.
  exhaustDiaM: 0.052,
  chargeHeatK: 30,        // how much the intake air warms up in the hot ports at 3,000 rpm (less at higher rpm)
  // Combustion. burnDeg is spark to end of burn for a clean, stoichiometric charge at 4,000 rpm; it gets
  // longer with leftover exhaust in the cylinder (part throttle, big overlap), lean or very rich
  // mixtures, and a little with rpm. Typical spark-ignition values (Heywood, Internal Combustion Engine
  // Fundamentals, ch. 9); the K20's own burn rate isn't published.
  burnDeg: 62,
  heatLoss: 0.30,         // share of the fuel's heat lost to the walls, crevices and incomplete burning
  // Knock: the end gas (unburned mixture ahead of the flame) autoignites when the Livengood–Wu
  // integral of 1/τ reaches 1 before the flame has burned it. τ is the Douaud–Eyzat (1978)
  // autoignition delay for a fuel of a given octane: τ = 17.68 ms × (ON/100)^3.402 × p^-1.7 × e^(3800/T).
  // knockScale shifts it to land the stock engine near where K20s are known to be knock limited.
  knockScale: 2,
};

const R = 287, GAMMA = 1.35, CV = R / (GAMMA - 1), CP = CV + R;   // in-cylinder gas
const LHV = 44e6, STOICH = 14.7, T_WALL_EXH = 700;

// Stock VTC (intake cam advance, crank degrees). Estimate shaped after Hondata's description of the
// stock K20Z3 cam angle maps: little advance at light load, around 40° in the midrange at full throttle,
// and on the high cam about 50° at 6,000 rpm falling to 30° by 8,000.
export function stockVtc(rpm, mapKpa, highCam) {
  const load = clamp((mapKpa - 30) / 60, 0, 1);
  if (highCam) return lookup1([5000, 6000, 7000, 8000, 9000], [45, 50, 40, 30, 25], rpm) * (0.4 + 0.6 * load);
  return lookup1([750, 1500, 2500, 3500, 5000, 6000, 7000], [0, 10, 25, 40, 40, 35, 25], rpm) * load;
}

// Cam angle from the tune if it has cam angle tables, otherwise the stock schedule.
export function vtcFor(tune, rpm, mapKpa, highCam) {
  const t = highCam ? (tune?.vtcHigh || tune?.vtcLow) : tune?.vtcLow;
  return clamp(t ? lookup2(t, rpm, mapKpa) : stockVtc(rpm, mapKpa, highCam), 0, 50);
}

function liftAt(theta, centre, dur, maxLift) {
  const d = ((theta - centre) % 720 + 1080) % 720 - 360;
  if (Math.abs(d) >= dur / 2) return 0;
  return maxLift * Math.sin(Math.PI * (d / dur + 0.5)) ** 1.6;
}

// Mass flow (kg/s) through an orifice; positive from a to b.
function flow(area, pa, Ta, pb, Tb, g = 1.4) {
  if (area <= 0) return 0;
  let sign = 1;
  if (pb > pa) { [pa, pb, Ta] = [pb, pa, Tb]; sign = -1; }
  const pr = pb / pa, crit = (2 / (g + 1)) ** (g / (g - 1));
  let psi;
  if (pr <= crit) psi = Math.sqrt(g * (2 / (g + 1)) ** ((g + 1) / (g - 1)));
  else if (pr > 0.999) psi = Math.sqrt(2 * g / (g - 1) * (0.999 ** (2 / g) - 0.999 ** ((g + 1) / g))) * (1 - pr) / 0.001;   // linear near zero so it doesn't chatter
  else psi = Math.sqrt(2 * g / (g - 1) * (pr ** (2 / g) - pr ** ((g + 1) / g)));
  return sign * area * pa / Math.sqrt(R * Ta) * psi;
}

// Keep one step's flow from overshooting the point where the two volumes reach equal pressure
// (the explicit step would otherwise make flow chatter back and forth across a valve).
function limit(m, pa, Ta, Va, pb, Tb, Vb) {
  const eq = Math.abs(pa - pb) / (1.4 * R * (Ta / Va + (Vb ? Tb / Vb : 0)));
  return clamp(m, -0.7 * eq, 0.7 * eq);
}

// Valve lift (m) for a cylinder at thetaDeg, measured from that cylinder's firing TDC (−360…360,
// overlap TDC at ±360). Same cam lobes and VTC phasing the flow model uses, for renderers and charts.
export function valveLiftM(thetaDeg, intake, highCam, vtc = 0, geom = FLOW) {
  const cam = highCam ? geom.cams.high : geom.cams.low;
  const th = ((thetaDeg + 360) % 720 + 720) % 720 - 360;
  return intake
    ? liftAt(th, -360 + geom.intakeCentre0 - vtc, cam.inDur, cam.inLift)
    : liftAt(th, 360 - geom.exhaustCentre, cam.exDur, cam.exLift);
}

// The cam card in the usual form: lift, duration at 1 mm of valve lift, and lobe centreline (intake:
// degrees after TDC with VTC at 0; exhaust: degrees before TDC).
export function camCard(geom = FLOW) {
  const lobe = (lift, dur, centre) => {
    const x = 2 * Math.asin((0.001 / lift) ** (1 / 1.6)) / Math.PI;   // where the sine^1.6 profile crosses 1 mm
    return { liftMm: +(lift * 1000).toFixed(2), durationDeg: +(dur * (1 - x)).toFixed(1), durationSeatDeg: dur, centerlineDeg: centre };
  };
  const { low, high } = geom.cams;
  return {
    intakeLow: lobe(low.inLift, low.inDur, geom.intakeCentre0), intakeHigh: lobe(high.inLift, high.inDur, geom.intakeCentre0),
    exhaustLow: lobe(low.exLift, low.exDur, geom.exhaustCentre), exhaustHigh: lobe(high.exLift, high.exDur, geom.exhaustCentre),
  };
}

export function createGasflow(geom = FLOW) {
  const G = geom;
  const ap = Math.PI / 4 * G.boreM ** 2, vd = ap * G.strokeM;
  return {
    G, ap, vd, vc: vd / (G.compression - 1),
    // cylinder
    m: 0, T: 800, xb: 0.1,
    // intake port (isothermal at the manifold temperature), runner flow
    pIn: 100e3, xbIn: 0, mRun: 0,
    // exhaust port and collector (mass and temperature), primary flow
    pEx: 105e3, TEx: 900, pCol: 105e3, TCol: 900, mPri: 0,
    out: null,
    primed: false,
  };
}

function volumeAt(s, thetaDeg) {
  const r = s.G.strokeM / 2, l = s.G.rodM, a = thetaDeg * Math.PI / 180;
  const x = r * (1 - Math.cos(a)) + l - Math.sqrt(l * l - (r * Math.sin(a)) ** 2);
  return s.vc + s.ap * x;
}

// Run one full cycle (overlap TDC to overlap TDC, firing TDC in the middle at 0°).
// c: { rpm, mapKpa, iatC, highCam, vtc, ignDeg, lambda, fire, outKpa, outAreaM2, energy, coolK }
// energy: the fuel's heat per kg of air at stoichiometric (LHV ÷ AFR) relative to the calibration fuel.
// coolK: extra charge cooling from the fuel evaporating, relative to the calibration fuel (src/fuels.js);
// a quarter of it cools the air in the port (denser charge), a quarter the trapped charge (end gas), and
// the rest comes off the hot port walls and valve (port injection wets them; estimate).
// outAreaM2 is the whole engine's exhaust outlet; this cylinder gets a quarter of it and a quarter
// of the collector, which stands in for the other three cylinders sharing the manifold.
export function runCycle(s, c, stepDeg = 1) {
  const G = s.G, rpm = Math.max(c.rpm, 300);
  const cam = c.highCam ? G.cams.high : G.cams.low;
  // Port walls heat the incoming air in proportion to how hot they are (head at coolant temperature,
  // calibrated at 90 °C), and cold walls take more heat out of combustion.
  const wallC = c.wallC ?? 90, wallF = Math.max(0, (wallC - 25) / 65), heatLoss = G.heatLoss * (1 + 0.004 * (90 - wallC));
  const coolK = c.fire ? (c.coolK ?? 0) : 0;
  const p0 = c.mapKpa * 1000, Tm = 273 + c.iatC, T0 = Tm + G.chargeHeatK * wallF * (3000 / rpm) ** 0.3 - 0.25 * coolK, pOut = (c.outKpa ?? 101.3) * 1000;
  const outArea = (c.outAreaM2 ?? Math.PI / 4 * G.exhaustDiaM ** 2) / 4;
  const inCentre = -360 + G.intakeCentre0 - (c.vtc ?? 0), exCentre = 360 - G.exhaustCentre;
  // Economy i-VTEC heads open one intake valve only a crack on the low cam: cam.inValves counts the
  // intake valves' share of the full curtain and throat (2 when both open fully).
  const inValves = cam.inValves ?? G.valves;
  const throatIn = inValves * (Math.PI / 4 * (0.88 * G.inValveM) ** 2 - Math.PI / 4 * G.stemM ** 2);
  const throatEx = G.valves * (Math.PI / 4 * (0.88 * G.exValveM) ** 2 - Math.PI / 4 * G.stemM ** 2);
  const aRun = Math.PI / 4 * G.runnerDiaM ** 2, aPri = Math.PI / 4 * G.primaryDiaM ** 2;
  // The collector is shared by four cylinders, so it sees four smaller pulses per cycle. Giving this
  // cylinder the whole volume with a quarter of the outlet smooths its single pulse about as much.
  const vIn = G.intakePortL / 1000, vEx = G.exhaustPortL / 1000, vCol = G.collectorL / 1000;
  const dt = stepDeg / (6 * rpm);

  if (!s.primed) {                                // start from a sensible state the first time
    s.pIn = p0; s.pEx = s.pCol = Math.max(pOut, 101.3e3); s.m = p0 * volumeAt(s, -360) / (R * 900);
    s.T = 900; s.primed = true;
  }

  let V = volumeAt(s, -360);
  let mFresh = s.m * (1 - s.xb), mTot = s.m;   // fresh and total mass in the cylinder
  let pumpW = 0, grossW = 0, pmax = 0, pmaxDeg = 0, qTotal = 0, xbPrev = 0, trappedFresh = 0, trappedTot = 0;
  let inFresh = 0, exOutMass = 0, exOutEnergy = 0, colOut = 0, colP = 0, colPT = 0, n = 0;
  const thetaS = -(c.ignDeg ?? 20), octane = c.octane ?? 91;
  let refTaken = false, pRef = 0, tRef = 0, residual = 0, burnDur = G.burnDeg, ca50 = null;
  let knockI = 0, knockAt = null, knockUnburned = 0;
  let mEx = s.pEx * vEx / (R * s.TEx), mCol = s.pCol * vCol / (R * s.TCol);
  let mInPort = s.pIn * vIn / (R * T0);

  for (let th = -360; th < 360; th += stepDeg) {
    const thm = th + stepDeg / 2;
    const V2 = volumeAt(s, th + stepDeg), dV = V2 - V;
    const p = mTot * R * s.T / V;

    // Valve areas
    const li = liftAt(thm, inCentre, cam.inDur, cam.inLift), le = liftAt(thm, exCentre, cam.exDur, cam.exLift);
    const aIv = li > 1e-4 ? G.valveCd * Math.min(inValves * Math.PI * G.inValveM * li, throatIn) : 0;
    const aEv = le > 1e-4 ? G.valveCd * Math.min(G.valves * Math.PI * G.exValveM * le, throatEx) : 0;

    // Flows this step (kg)
    const fIv = limit(flow(aIv, s.pIn, T0, p, s.T, 1.4) * dt, s.pIn, T0, vIn, p, s.T, V);       // + into the cylinder
    const fEv = limit(flow(aEv, p, s.T, s.pEx, s.TEx, GAMMA) * dt, p, s.T, V, s.pEx, s.TEx, vEx); // + out to the exhaust port
    const fOut = limit(flow(outArea, s.pCol, s.TCol, pOut, 300, 1.33) * dt, s.pCol, s.TCol, vCol, pOut, 300, 0);

    // Intake runner: air column pushed by the pressure difference, slowed by wall friction.
    const rhoRun = p0 / (R * Tm), uRun = s.mRun / (rhoRun * aRun);
    s.mRun += (aRun / G.runnerM) * ((p0 - s.pIn) - 0.02 * (G.runnerM / G.runnerDiaM) * 0.5 * rhoRun * uRun * Math.abs(uRun)) * dt;
    const rhoPri = s.pEx / (R * s.TEx), uPri = s.mPri / (rhoPri * aPri);
    s.mPri += (aPri / G.primaryM) * ((s.pEx - s.pCol) - 0.03 * (G.primaryM / G.primaryDiaM) * 0.5 * rhoPri * uPri * Math.abs(uPri)) * dt;
    const fRun = s.mRun * dt, fPri = s.mPri * dt;

    // Intake port: runner in, valve out. Track exhaust that backflowed into it.
    const xbCyl = mTot > 0 ? 1 - mFresh / mTot : 0;
    const portBurned = s.xbIn * mInPort - (fIv > 0 ? fIv * s.xbIn : fIv * xbCyl) + (fRun < 0 ? fRun * s.xbIn : 0);
    mInPort = Math.max(1e-7, mInPort + fRun - fIv);
    s.xbIn = clamp(portBurned / mInPort, 0, 1);
    s.pIn = mInPort * R * T0 / vIn;

    // Cylinder: mass and energy.
    const freshFrac = mTot > 0 ? mFresh / mTot : 0;
    let dU = -p * dV;
    if (fIv > 0) { dU += 1.4 / 0.4 * R * T0 * fIv; mFresh += fIv * (1 - s.xbIn); }
    else { dU += CP * s.T * fIv; mFresh += fIv * freshFrac; }
    if (fEv > 0) { dU -= CP * s.T * fEv; mFresh -= fEv * freshFrac; exOutMass += fEv; exOutEnergy += fEv * s.T; }
    else { dU -= CP * s.TEx * fEv; }
    if (fIv > 0) inFresh += fIv * (1 - s.xbIn);
    else inFresh += fIv * freshFrac;

    // Trapped charge, and the reference point the end gas is compressed from.
    if (!refTaken && (th >= -60 || (c.fire && th >= thetaS))) {
      refTaken = true; trappedFresh = mFresh; trappedTot = mTot;
      pRef = p; tRef = s.T - (c.fire ? 30 * clamp((1 - (c.lambda ?? 1)) / 0.2, 0, 1) : 0) - 0.25 * coolK;   // rich mixtures cool the charge as the fuel evaporates
      residual = 1 - mFresh / Math.max(mTot, 1e-9);
    }

    // Combustion: a Wiebe heat-release curve that starts at the spark, only with fuel.
    if (c.fire && th >= thetaS && xbPrev < 0.9999) {
      if (qTotal === 0) {
        const lam = Math.max(c.lambda ?? 1, 0.5);
        burnDur = G.burnDeg * (1 + 2.5 * Math.max(0, residual - 0.04)) * (1 + 1.5 * Math.max(0, lam - 1) + 0.8 * Math.max(0, 0.8 - lam)) * (rpm / 4000) ** 0.25;
        // Heat released: the fuel that can burn (rich mixtures are short of air). Mixtures a little rich of
        // stoichiometric make the most power (charge cooling, more gas moles, less dissociation).
        const richGain = lam < 1 ? clamp(1 + 0.045 * (1 - ((lam - 0.87) / 0.13) ** 2), 0.85, 1.045) : 1;
        const misfire = lam > 1.35 ? clamp(1 - (lam - 1.35) * 4, 0, 1) : 1;
        qTotal = Math.min(trappedFresh / (STOICH * lam), trappedFresh / STOICH) * LHV * (c.energy ?? 1) * (1 - heatLoss) * richGain * misfire;
      }
      const x = clamp((th + stepDeg - thetaS) / burnDur, 0, 1), xb = 1 - Math.exp(-6.9 * x ** 3);
      dU += qTotal * (xb - xbPrev);
      mFresh -= trappedFresh * (xb - xbPrev) * Math.min(1, 1 / Math.max(c.lambda ?? 1, 1));
      xbPrev = xb;
      if (xb > 0.1 && xb < 0.9) ca50 = ca50 ?? (xb >= 0.5 ? th + stepDeg : null);
    }
    // Knock integral over the end gas until 90% of the charge has burned.
    if (c.fire && refTaken && xbPrev < 0.9 && p > pRef) {
      const tU = tRef * (p / pRef) ** ((GAMMA - 1) / GAMMA);
      const tau = G.knockScale * 17.68e-3 * (octane / 100) ** 3.402 * (p / 101325) ** -1.7 * Math.exp(3800 / tU);
      knockI += dt / tau;
      if (knockI >= 1 && knockAt === null) { knockAt = th; knockUnburned = 1 - xbPrev; }
    }

    const U = CV * mTot * s.T + dU;
    mTot = Math.max(1e-7, mTot + fIv - fEv);
    mFresh = clamp(mFresh, 0, mTot);
    s.T = clamp(U / (CV * mTot), 250, 4500);
    const p2 = mTot * R * s.T / V2;
    if (th >= -180 && th < 180) grossW += 0.5 * (p + p2) * dV; else pumpW += 0.5 * (p + p2) * dV;
    if (p2 > pmax) { pmax = p2; pmaxDeg = th + stepDeg; }
    V = V2;

    // Exhaust port: cylinder in, primary out. Collector: primary in, exhaust system out. Walls cool both.
    const UEx = CV * mEx * s.TEx + (fEv > 0 ? CP * s.T * fEv : CP * s.TEx * fEv) - CP * (fPri > 0 ? s.TEx : s.TCol) * fPri;
    mEx = Math.max(1e-7, mEx + fEv - fPri);
    s.TEx = clamp(UEx / (CV * mEx), 300, 2000);
    s.TEx += (T_WALL_EXH - s.TEx) * Math.min(1, dt * 40);
    s.pEx = mEx * R * s.TEx / vEx;
    const UCol = CV * mCol * s.TCol + CP * (fPri > 0 ? s.TEx : s.TCol) * fPri - CP * s.TCol * fOut;
    mCol = Math.max(1e-7, mCol + fPri - fOut);
    s.TCol = clamp(UCol / (CV * mCol), 300, 2000);
    s.TCol += (T_WALL_EXH - s.TCol) * Math.min(1, dt * 8);
    s.pCol = mCol * R * s.TCol / vCol;
    colOut += fOut; colP += s.pCol; colPT += s.TCol; n++;
  }
  s.m = mTot;
  s.xb = mTot > 0 ? 1 - mFresh / mTot : 0;

  const rho0 = p0 / (R * Tm), cycleS = 120 / rpm;
  s.out = {
    ve: trappedFresh / (rho0 * s.vd),                // fresh air trapped ÷ what the swept volume holds at MAP/IAT
    veDelivered: inFresh / (rho0 * s.vd),            // fresh air through the intake valve, including any that blows through
    residual,
    burnDeg: burnDur, ca50Deg: ca50 ?? NaN,          // combustion phasing: crank angle where half the charge has burned
    knockIndex: knockI,                               // ≥ 1: the end gas autoignites before the flame reaches it
    knockAtDeg: knockAt ?? NaN, knockUnburned,        // when, and how much charge was left to detonate
    imepKpa: grossW / s.vd / 1000,
    pmepKpa: -pumpW / s.vd / 1000,                    // positive = work spent pumping
    pmaxBar: pmax / 1e5, pmaxDeg,
    egtC: (exOutMass > 0 ? exOutEnergy / exOutMass : s.TEx) - 273,
    exhKpa: colP / n / 1000,                          // mean exhaust manifold (collector) pressure
    exhTempK: colPT / n,
    exhGs: colOut * 4 / cycleS * 1000,                // whole engine
  };
  return s.out;
}

// Run cycles until the result stops changing (used by tools and tests, not in real time).
export function settle(s, c, maxCycles = 40) {
  let prev = null;
  for (let i = 0; i < maxCycles; i++) {
    const o = runCycle(s, c);
    if (prev && Math.abs(o.ve - prev.ve) < 2e-4 && Math.abs(o.exhKpa - prev.exhKpa) < 0.05) break;
    prev = { ...o };
  }
  return s.out;
}
