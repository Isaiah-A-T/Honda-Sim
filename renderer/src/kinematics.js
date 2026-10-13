// K20Z3 dimensions and the motion of its parts, with no Three.js so it can be tested in Node.
//
// Lengths are millimetres. Crank angles are degrees, 0 = cylinder 1 at top dead centre on its power stroke,
// counted over the full 720° four-stroke cycle. Dimensions are the commonly published K20 figures; the cam
// timing and lift numbers are rough, picked to look right, not read from a cam card.

export const K20 = {
  bore: 86,
  vtcMaxDeg: 50,              // intake cam phaser range, crank degrees of advance
  stroke: 86,
  rodLength: 139,
  boreSpacing: 96,            // centre to centre between cylinders
  deckHeight: 212,            // crank centre to top of block
  firingOrder: [1, 3, 4, 2],
  // Exhaust and intake lobes: centre of the lift event in cycle degrees (0 = TDC firing) and duration.
  cams: {
    low:  { exhaust: { centre: 250, duration: 220, lift: 9.5 },  intake: { centre: 470, duration: 220, lift: 10.0 } },
    high: { exhaust: { centre: 255, duration: 255, lift: 11.5 }, intake: { centre: 465, duration: 260, lift: 12.0 } },
  },
};

const mod = (a, n) => ((a % n) + n) % n;
const rad = d => d * Math.PI / 180;

// Where cylinder `cyl` (1–4) is in its own cycle when the crank is at `crankDeg`. 0 = TDC firing.
export function cycleAngle(cyl, crankDeg) {
  const slot = K20.firingOrder.indexOf(cyl);
  return mod(crankDeg - slot * 180, 720);
}

// Crank throw angle for a cylinder: 1 and 4 share a throw, 2 and 3 are 180° away.
export function throwAngle(cyl) {
  return cyl === 1 || cyl === 4 ? 0 : 180;
}

// Distance from the crank centre up to the wrist pin, for a crank rotation `thetaDeg` from TDC.
export function pistonPinHeight(thetaDeg) {
  const r = K20.stroke / 2, l = K20.rodLength, t = rad(thetaDeg);
  return r * Math.cos(t) + Math.sqrt(l * l - (r * Math.sin(t)) ** 2);
}

// Crank pin position (x across the engine, y up) and the rod's lean from vertical, in radians.
export function crankPin(thetaDeg) {
  const r = K20.stroke / 2, t = rad(thetaDeg);
  return { x: r * Math.sin(t), y: r * Math.cos(t) };
}
export function rodAngle(thetaDeg) {
  const p = crankPin(thetaDeg);
  return Math.asin(p.x / K20.rodLength);
}

// Valve lift in mm for one valve event. Smooth cosine bump over the duration.
export function eventLift(cycleDeg, ev) {
  let d = mod(cycleDeg - ev.centre + 360, 720) - 360;   // -360..360 around the event centre
  if (Math.abs(d) >= ev.duration / 2) return 0;
  return ev.lift * (0.5 + 0.5 * Math.cos(Math.PI * d / (ev.duration / 2)));
}

// vtcDeg advances the intake cam (K20Z3 VTC, roughly 0–50 crank degrees): the intake event happens earlier.
export function valveLift(cyl, crankDeg, side, vtec, vtcDeg = 0) {
  const shift = side === 'intake' ? vtcDeg : 0;
  return eventLift(cycleAngle(cyl, crankDeg + shift), K20.cams[vtec ? 'high' : 'low'][side]);
}

// Which stroke a cylinder is on, for labels.
export function strokeName(cycleDeg) {
  return ['power', 'exhaust', 'intake', 'compression'][Math.floor(mod(cycleDeg, 720) / 180)];
}

// Advances a crank angle by rpm over dt seconds, slowed down by `slowMo` so the eye can follow it.
export function advanceCrank(crankDeg, rpm, dt, slowMo = 1) {
  return mod(crankDeg + rpm / 60 * 360 * dt * slowMo, 720);
}

// Stress 0–1 (above 1 means past the limit) to a heat colour: neutral, then yellow, orange, red.
// Returns [r, g, b] 0–1 to blend over the part's base colour, and how strongly to blend it.
export function stressTint(s) {
  if (!(s > 0)) return { rgb: [1, 1, 1], amount: 0 };
  const x = Math.min(s, 1.3);
  const stops = [[0.5, [1.0, 0.85, 0.2]], [0.8, [1.0, 0.45, 0.08]], [1.0, [0.9, 0.08, 0.04]], [1.3, [0.75, 0.02, 0.02]]];
  if (x <= 0.5) return { rgb: stops[0][1], amount: x / 0.5 * 0.35 };
  for (let i = 1; i < stops.length; i++) {
    const [x0, c0] = stops[i - 1], [x1, c1] = stops[i];
    if (x <= x1) {
      const f = (x - x0) / (x1 - x0);
      return { rgb: c0.map((v, k) => v + (c1[k] - v) * f), amount: 0.35 + (Math.min(x, 1) - 0.5) / 0.5 * 0.55 + (x > 1 ? 0.1 : 0) };
    }
  }
  return { rgb: stops[3][1], amount: 1 };
}
