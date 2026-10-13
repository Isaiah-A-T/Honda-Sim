// Demo driver: a fake Honda Sim (scripted pulls or sliders) fed to the engine view through the same adapter
// the real sim will use, with the mode GUI on top.

import { createEngineRenderer } from './src/engine-renderer.js';
import { createEngineGui } from './src/gui.js';
import { stateFromSim } from './src/sim-adapter.js';
import { createFakeSim } from './src/fake-sim.js';

const $ = id => document.getElementById(id);
const view = createEngineRenderer($('viewport'), { slowMo: 0.02 });
const gui = createEngineGui(view, { panel: $('modePanel') });
const sim = createFakeSim({ turbo: true });
window.engineView = view;   // handy from the console
window.engineGui = gui;

let mode = 'pull';
let last = performance.now(), hudT = 0;
function loop(now) {
  const dt = Math.min((now - last) / 1000, 0.05);
  last = now;
  sim.step(dt, mode === 'manual' ? { rpm: +$('inRpm').value, throttle: +$('inThrottle').value / 100, boostKpa: +$('inBoost').value } : null);
  const s = stateFromSim(sim, { materials: gui.materials });
  view.setState(s);
  hudT += dt;
  if (hudT > 0.08) { hudT = 0; drawHud(s); }
  requestAnimationFrame(loop);
}

function drawHud(s) {
  $('hRpm').textContent = Math.round(s.rpm).toLocaleString();
  $('hBoost').textContent = s.turbo ? (s.boostKpa * 0.145).toFixed(1) : '–';
  $('hIgn').textContent = s.ignDeg.toFixed(1);
  $('hMap').textContent = Math.round(s.mapKpa);
  $('hVtec').classList.toggle('on', s.vtec);
  $('hKnock').classList.toggle('on', s.knock);
  $('hCut').classList.toggle('on', s.fuelCut && s.rpm > 7900);
  $('rpmFill').style.width = `${Math.min(s.rpm / 8500, 1) * 100}%`;
}

for (const b of document.querySelectorAll('[name=mode]')) b.onchange = () => {
  mode = document.querySelector('[name=mode]:checked').value;
  $('manual').hidden = mode !== 'manual';
};
const showSlow = () => { const v = +$('inSlow').value; view.setOptions({ slowMo: v / 1000 });
  $('slowVal').textContent = v >= 1000 ? 'real time' : `${(v / 10).toFixed(1)}%`; };
$('inSlow').oninput = showSlow;
$('inTurbo').onchange = e => { sim.eng.tune.turbo.enabled = e.target.checked; $('boostRow').hidden = !e.target.checked; };
$('inSpin').onchange = e => view.setOptions({ autoRotate: e.target.checked });
$('btnKnock').onclick = () => sim.knock();
for (const id of ['inRpm', 'inThrottle', 'inBoost']) {
  const out = $(id + 'Val');
  $(id).addEventListener('input', () => { out.textContent = $(id).value; });
  out.textContent = $(id).value;
}
showSlow();
requestAnimationFrame(loop);
