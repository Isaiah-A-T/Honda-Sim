// A stand-in for Honda Sim's state (sim.eng and sim.running, the fields stateFromSim reads), for the demo and
// for building views before the real models exist. step(dt, input) runs a scripted pull, or follows
// input = { rpm, throttle 0–1, boostKpa } when one is given.

export function createFakeSim({ turbo = true } = {}) {
  const tune = { revLimit: 8000, torqueLimitNm: 350, clutchNm: 450, overboostKpa: 200,
    turbo: { enabled: turbo, maxBoostKpa: 165, spoolRpm: 3400 } };
  const eng = { tune, rpm: 800, tps: 0, map: 30, boost: 0, highCam: false, vtcDeg: 0, ign: 15, mbt: 20, knocking: false,
    knockRetard: 0, lambda: 1, iat: 25, torqueNm: 0, airGs: 3, revCut: false, dfco: false, boostCut: false };
  const sim = { eng, running: true, t: 0, knockUntil: 0 };

  sim.knock = (seconds = 1.2) => { sim.knockUntil = sim.t + seconds; };

  sim.step = (dt, input = null) => {
    sim.t += dt;
    let pedal, targetRpm;
    if (!input) {
      const p = sim.t % 12;                               // idle, full-throttle pull to the limiter, lift and coast
      if (p < 2) { pedal = 0; targetRpm = 850; }
      else if (p < 8.5) { pedal = 1; targetRpm = Math.min(2500 + (p - 2) * 950, 8050); }
      else { pedal = 0; targetRpm = 8000 - (p - 8.5) * 2000; }
      eng.rpm += (Math.max(targetRpm, 800) - eng.rpm) * Math.min(1, dt * 4);
    } else {
      pedal = input.throttle;
      eng.rpm += (input.rpm - eng.rpm) * Math.min(1, dt * 5);
    }
    const rpm = eng.rpm;
    eng.tps = pedal * 100;
    const spool = Math.min(1, Math.max(0, (rpm - 2400) / 2200)) ** 1.6 * pedal;
    const boostTarget = tune.turbo.enabled ? (input?.boostKpa ?? 110) * spool : 0;
    eng.boost += (boostTarget - eng.boost) * Math.min(1, dt * (boostTarget > eng.boost ? 2.5 : 8));
    eng.map = 25 + 75 * pedal + eng.boost;
    eng.highCam = rpm > 5800 && eng.map > 70;
    // VTC: advance the intake cam most in the midrange under load, back to zero at idle and high rpm.
    const vtcTarget = pedal > 0.3 ? 45 * Math.exp(-(((rpm - 4200) / 2200) ** 2)) : 5;
    eng.vtcDeg += (vtcTarget - eng.vtcDeg) * Math.min(1, dt * 3);
    eng.revCut = rpm > 8000;
    eng.dfco = pedal < 0.02 && rpm > 1500;
    eng.lambda = eng.dfco || eng.revCut ? 9.99 : eng.map > 90 ? 0.82 : 1.0;
    eng.mbt = 17 + 3 * Math.min(rpm, 6500) / 1000 + (100 - eng.map) * 0.13;
    eng.knocking = sim.t < sim.knockUntil;
    eng.knockRetard = eng.knocking ? Math.min(eng.knockRetard + dt * 10, 8) : Math.max(eng.knockRetard - dt * 0.5, 0);
    eng.ign = Math.max(8, eng.mbt - 2 - eng.boost * 0.06) - eng.knockRetard;
    eng.iat += ((25 + eng.boost * 0.12) - eng.iat) * Math.min(1, dt * 0.3);
    const ve = eng.highCam ? 0.70 + 0.31 * Math.exp(-(((rpm / 1000 - 7) / 2.6) ** 2)) : 0.58 + 0.29 * Math.exp(-(((rpm / 1000 - 4.4) / 2.4) ** 2));
    eng.airGs = ve * 1.998 * 1.184 * eng.map / 101.3 * rpm / 120;
    eng.torqueNm = eng.dfco || eng.revCut ? -20 : 235 * ve * eng.map / 100 - 15;
  };
  return sim;
}
