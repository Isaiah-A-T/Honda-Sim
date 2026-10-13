// Browser front end: runs the sim in real time and shows a dash, the tune tables and a dyno chart.

import { parseTable, activeCell, guessFuelUnits } from './tables.js';
import { CAR, placeholderTune } from './k20z3.js';
import { createSim, step, shift, shiftUp, shiftDown, startEngine, snapshot, logToCsv, setTune } from './sim.js';
import { createInput, attachKeyboard, readInput, startLearn, AXES, BUTTONS } from './input.js';

const TUNE_KEY = 'honda-sim.tune.v1';
const NM_TO_LBFT = 0.737562;
const $ = id => document.getElementById(id);

const TABLES = [
  ['ignLow', 'Ignition (low cam)'],
  ['ignHigh', 'Ignition (high cam)'],
  ['fuelLow', 'Fuel (low cam)'],
  ['fuelHigh', 'Fuel (high cam)'],
];

function loadTune() {
  try {
    const t = JSON.parse(localStorage.getItem(TUNE_KEY));
    if (t && t.ignLow && t.fuelLow) return t;
  } catch { /* fall through */ }
  return placeholderTune();
}
function saveTune(t) {
  try { localStorage.setItem(TUNE_KEY, JSON.stringify(t)); } catch { /* not persisted */ }
}

let tune = loadTune();
const sim = createSim(tune);
const inp = createInput();
attachKeyboard(inp);
let shownTable = 'ignLow';
let lastCell = '';
const dyno = { cur: new Map(), prev: null };

// ------------------------------------------------------------------ tables
function renderTabs() {
  $('tableTabs').innerHTML = '';
  for (const [key, label] of TABLES) {
    const b = document.createElement('button');
    b.textContent = label;
    b.className = key === shownTable ? 'active' : '';
    b.onclick = () => { shownTable = key; renderTabs(); renderTable(); };
    $('tableTabs').append(b);
  }
  $('pasteTarget').innerHTML = TABLES.map(([k, l]) => `<option value="${k}"${k === shownTable ? ' selected' : ''}>${l}</option>`).join('');
}

function renderTable() {
  const t = tune[shownTable];
  const head = `<thead><tr><th>rpm \\ kPa</th>${t.cols.map(c => `<th>${c}</th>`).join('')}</tr></thead>`;
  const body = t.rows.map((r, i) => `<tr><th>${r}</th>${t.data[i].map((v, j) => `<td data-c="${i},${j}">${fmt(v)}</td>`).join('')}</tr>`).join('');
  $('tableView').innerHTML = head + `<tbody>${body}</tbody>`;
  lastCell = '';
}
const fmt = v => (Math.abs(v) < 3 ? v.toFixed(2) : v.toFixed(1));

function highlightCell() {
  const e = sim.eng;
  const usingHigh = e.highCam;
  const live = shownTable.endsWith(usingHigh ? 'High' : 'Low');
  const [i, j] = activeCell(tune[shownTable], e.rpm, e.map);
  const key = live ? `${i},${j}` : '';
  if (key === lastCell) return;
  document.querySelectorAll('#tableView td.hi').forEach(td => td.classList.remove('hi'));
  if (key) document.querySelector(`#tableView td[data-c="${key}"]`)?.classList.add('hi');
  lastCell = key;
}

function renderSettings() {
  $('tuneName').textContent = tune.name || '';
  $('setVtec').value = tune.vtecRpm;
  $('setVtecMap').value = tune.vtecMinMap;
  $('setRev').value = tune.revLimit;
  $('setOct').value = tune.octane ?? 91;
  $('setFuelUnits').value = tune.fuelUnits || guessFuelUnits(tune.fuelLow);
}

function applyTune(t) {
  tune = t;
  setTune(sim, tune);
  saveTune(tune);
  renderSettings();
  renderTable();
}

for (const [id, key] of [['setVtec', 'vtecRpm'], ['setVtecMap', 'vtecMinMap'], ['setRev', 'revLimit'], ['setOct', 'octane']]) {
  $(id).addEventListener('change', ev => {
    const v = Number(ev.target.value);
    if (Number.isFinite(v)) { tune[key] = v; saveTune(tune); }
  });
}
$('setFuelUnits').addEventListener('change', ev => { tune.fuelUnits = ev.target.value; saveTune(tune); });

$('btnPaste').onclick = () => {
  const target = $('pasteTarget').value;
  try {
    const t = parseTable($('pasteBox').value);
    tune[target] = t;
    if (target.startsWith('fuel')) tune.fuelUnits = guessFuelUnits(t);
    if (/Placeholder/.test(tune.name)) tune.name = 'Imported from FlashPro Manager';
    shownTable = target;
    applyTune(tune);
    renderTabs();
    $('pasteMsg').textContent = `Loaded ${t.rows.length} × ${t.cols.length} table into ${TABLES.find(x => x[0] === target)[1]}.` +
      (target.startsWith('fuel') ? ` Fuel values look like: ${tune.fuelUnits}. Change it below if that's wrong.` : '');
    $('pasteBox').value = '';
  } catch (err) {
    $('pasteMsg').textContent = err.message;
  }
};

$('btnSaveTune').onclick = () => download(`${(tune.name || 'tune').replace(/\W+/g, '_')}.json`, JSON.stringify(tune, null, 1), 'application/json');
$('fileTune').addEventListener('change', async ev => {
  const f = ev.target.files[0];
  if (!f) return;
  try {
    const t = JSON.parse(await f.text());
    if (!t.ignLow || !t.fuelLow) throw new Error('missing tables');
    applyTune({ ...placeholderTune(), ...t });
  } catch (err) {
    alert(`Couldn't open that tune: ${err.message}`);
  }
  ev.target.value = '';
});
$('btnResetTune').onclick = () => { if (confirm('Replace the loaded tables with the placeholder tune?')) applyTune(placeholderTune()); };

// ------------------------------------------------------------------ controls
function renderLearnRows() {
  const names = [...AXES, ...BUTTONS];
  $('learnRows').innerHTML = names.map(n => {
    const m = inp.mapping[n];
    const what = !m ? '<span class="muted">not set</span>' : m.axis !== undefined ? `axis ${m.axis}` : `button ${m.button}`;
    return `<div><span>${n}</span><span>${what}</span><button data-learn="${n}">Learn</button></div>`;
  }).join('');
  $('learnRows').querySelectorAll('button').forEach(b => {
    b.onclick = () => {
      const n = b.dataset.learn;
      $('learnMsg').textContent = startLearn(inp, AXES.includes(n) ? 'axis' : 'button', n);
    };
  });
}

// ------------------------------------------------------------------ datalog, dyno
$('btnLog').onclick = () => {
  sim.logging = !sim.logging;
  if (sim.logging) sim.log = [];
  $('btnLog').textContent = sim.logging ? 'Stop datalog' : 'Start datalog';
  $('btnCsv').disabled = sim.logging || !sim.log.length;
};
$('btnCsv').onclick = () => download(`honda-sim-log-${new Date().toISOString().slice(0, 19).replace(/:/g, '')}.csv`, logToCsv(sim.log), 'text/csv');
$('btnStart').onclick = () => startEngine(sim);
$('btnDynoClear').onclick = () => { dyno.cur = new Map(); dyno.prev = null; };

function download(name, text, type) {
  const a = document.createElement('a');
  a.href = URL.createObjectURL(new Blob([text], { type }));
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 1000);
}

function recordDyno(throttle) {
  const e = sim.eng;
  if (throttle > 0.95 && sim.car.locked && sim.running && e.rpm > 1500) {
    const bin = Math.round(e.rpm / 100) * 100;
    const best = dyno.cur.get(bin);
    if (!best || e.torqueNm > best) dyno.cur.set(bin, e.torqueNm);
  }
}

function drawDyno() {
  const c = $('dyno'), g = c.getContext('2d');
  const css = getComputedStyle(document.documentElement);
  const W = c.width, H = c.height, pad = 30;
  g.clearRect(0, 0, W, H);
  g.strokeStyle = css.getPropertyValue('--line'); g.fillStyle = css.getPropertyValue('--muted'); g.font = '11px system-ui';
  const x = rpm => pad + (rpm - 1000) / 8000 * (W - pad * 2), y = v => H - pad - v / 220 * (H - pad * 1.5);
  for (let r = 1000; r <= 9000; r += 1000) { g.beginPath(); g.moveTo(x(r), pad / 2); g.lineTo(x(r), H - pad); g.stroke(); g.fillText(`${r / 1000}k`, x(r) - 6, H - pad + 14); }
  for (let v = 0; v <= 200; v += 50) { g.fillText(v, 4, y(v) + 4); }
  const plot = (map, alpha) => {
    const pts = [...map.entries()].sort((a, b) => a[0] - b[0]);
    if (pts.length < 2) return;
    for (const [dash, val] of [[[], ([r, t]) => t * r / 7120.9], [[5, 4], ([, t]) => t * NM_TO_LBFT]]) {
      g.setLineDash(dash); g.globalAlpha = alpha; g.strokeStyle = css.getPropertyValue('--accent'); g.lineWidth = 2;
      g.beginPath(); pts.forEach((p, i) => (i ? g.lineTo : g.moveTo).call(g, x(p[0]), y(val(p)))); g.stroke();
    }
    g.setLineDash([]); g.globalAlpha = 1; g.lineWidth = 1;
  };
  if (dyno.prev) plot(dyno.prev, 0.3);
  plot(dyno.cur, 1);
  const peak = [...dyno.cur.entries()].reduce((b, [r, t]) => (t * r / 7120.9 > b.hp ? { hp: t * r / 7120.9, rpm: r } : b), { hp: 0, rpm: 0 });
  if (peak.hp) { g.fillStyle = css.getPropertyValue('--text'); g.fillText(`peak ${peak.hp.toFixed(0)} hp @ ${peak.rpm}`, W - pad - 130, pad); }
}

// ------------------------------------------------------------------ main loop
function handleEvents(events) {
  for (const ev of events) {
    if (ev === 'shiftUp') shiftUp(sim);
    else if (ev === 'shiftDown') shiftDown(sim);
    else if (ev === 'neutral') shift(sim, 0);
    else if (ev === 'reverse') shift(sim, -1);
    else if (ev === 'start') startEngine(sim);
    else if (/^gear\d$/.test(ev)) shift(sim, Number(ev.slice(4)));
  }
}

function updateDash(input) {
  const s = snapshot(sim, input.throttle), e = sim.eng;
  const kmh = $('kmh').checked;
  $('rpm').textContent = s.rpm;
  $('rpmFill').style.width = `${Math.min(100, s.rpm / 9000 * 100)}%`;
  $('rpmFill').className = s.vtec ? 'high' : '';
  $('rpmLimit').style.left = `${tune.revLimit / 9000 * 100}%`;
  $('vtec').classList.toggle('on', !!s.vtec);
  $('shiftLamp').classList.toggle('on', s.rpm > tune.revLimit - 400);
  $('revcut').classList.toggle('on', !!s.rev_cut);
  $('knockLamp').classList.toggle('on', e.knocking);
  $('gear').textContent = s.gear === 0 ? 'N' : s.gear < 0 ? 'R' : s.gear;
  $('speed').textContent = Math.abs(kmh ? s.vss_kmh : s.vss_kmh / 1.609).toFixed(0);
  $('speedUnit').textContent = kmh ? 'km/h' : 'mph';
  $('tps').textContent = s.tps.toFixed(0);
  $('map').textContent = s.map_kpa.toFixed(0);
  $('afr').textContent = e.revCut || e.dfco ? 'cut' : s.afr.toFixed(1);
  $('lambda').textContent = e.revCut || e.dfco ? '–' : s.lambda.toFixed(2);
  $('lamTarget').textContent = Number.isFinite(e.lambdaTarget) ? e.lambdaTarget.toFixed(2) : 'from ms table';
  $('ign').textContent = s.ign_deg.toFixed(1);
  $('ignTable').textContent = s.ign_table.toFixed(1);
  $('mbt').textContent = s.mbt_deg.toFixed(1);
  $('kl').textContent = s.knock_limit.toFixed(1);
  $('knockCount').textContent = s.knock_count;
  $('retard').textContent = s.knock_retard.toFixed(1);
  $('ipw').textContent = s.inj_ms.toFixed(2);
  $('duty').textContent = s.inj_duty.toFixed(0);
  $('tq').textContent = (s.torque_nm * NM_TO_LBFT).toFixed(0);
  $('hp').textContent = s.hp.toFixed(0);
  $('barThrottle').style.width = `${input.throttle * 100}%`;
  $('barBrake').style.width = `${input.brake * 100}%`;
  $('barClutch').style.width = `${(1 - sim.lastClutch) * 100}%`;
  $('engineState').textContent = sim.running ? 'Engine running' : 'Stalled: press R or Start';
  $('engineState').className = sim.running ? 'pill' : 'pill off';
  $('padName').textContent = inp.padName ? `Detected: ${inp.padName}` : 'No wheel or pedals detected yet. Press any pedal or button.';
}

let last = performance.now(), frame = 0, wasFull = false;
function loop(now) {
  const dt = Math.min(0.05, (now - last) / 1000);
  last = now;
  const input = readInput(inp, dt);
  if (input.message) { $('learnMsg').textContent = input.message; renderLearnRows(); }
  handleEvents(input.events);
  step(sim, input, dt);
  recordDyno(input.throttle);
  const full = input.throttle > 0.95;
  if (wasFull && !full && dyno.cur.size > 10) { dyno.prev = dyno.cur; dyno.cur = new Map(); }
  wasFull = full;
  updateDash(input);
  highlightCell();
  if (frame++ % 6 === 0) drawDyno();
  $('btnCsv').disabled = sim.logging || !sim.log.length;
  requestAnimationFrame(loop);
}

$('carName').textContent = CAR.name;
renderTabs();
renderSettings();
renderTable();
renderLearnRows();
requestAnimationFrame(loop);
