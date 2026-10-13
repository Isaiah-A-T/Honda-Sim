// Browser front end: runs the sim in real time and shows a dash, the tune tables and a dyno chart.

import { parseTable, activeCell, guessFuelUnits } from './tables.js';
import { CAR, placeholderTune, turboSetup } from './k20z3.js';
import { createSim, step, shift, shiftUp, shiftDown, startEngine, rebuildEngine, snapshot, logToCsv, setTune } from './sim.js';
import { PARTS, MATERIALS, partStatus, materialsFor, partMaterial } from './internals.js';
import { FLOW } from './gasflow.js';
import { createInput, attachKeyboard, readInput, startLearn, AXES, BUTTONS } from './input.js';

const TUNE_KEY = 'honda-sim.tune.v1';
const NM_TO_LBFT = 0.737562;
const $ = id => document.getElementById(id);

const TABLES = [
  ['ignLow', 'Ignition (low cam)'],
  ['ignHigh', 'Ignition (high cam)'],
  ['fuelLow', 'Fuel (low cam)'],
  ['fuelHigh', 'Fuel (high cam)'],
  ['boostTarget', 'Boost target'],
  ['wgDuty', 'Wastegate duty'],
  ['vtcLow', 'Cam angle (low cam)'],
  ['vtcHigh', 'Cam angle (high cam)'],
];
// Paste/CSV targets: "both cams" writes the same table into low and high cam.
const TARGETS = [
  ['ignBoth', 'Ignition (both cams)', ['ignLow', 'ignHigh']],
  ['fuelBoth', 'Fuel (both cams)', ['fuelLow', 'fuelHigh']],
  ...TABLES.map(([k, l]) => [k, l, [k]]),
];
// Which table a CSV file goes to, from its name.
function targetForFile(name) {
  const n = name.toLowerCase();
  if (/wg|wastegate|duty/.test(n)) return 'wgDuty';
  if (/boost/.test(n)) return 'boostTarget';
  if (/cam|vtc/.test(n)) return /high/.test(n) ? 'vtcHigh' : 'vtcLow';
  if (/ign|timing/.test(n)) return /high/.test(n) ? 'ignHigh' : /low/.test(n) ? 'ignLow' : 'ignBoth';
  if (/afr|fuel|lambda/.test(n)) return /high/.test(n) ? 'fuelHigh' : /low/.test(n) ? 'fuelLow' : 'fuelBoth';
  return null;
}

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
  for (const [key, label] of TABLES.filter(([k]) => tune[k])) {
    const b = document.createElement('button');
    b.textContent = label;
    b.className = key === shownTable ? 'active' : '';
    b.onclick = () => { shownTable = key; renderTabs(); renderTable(); };
    $('tableTabs').append(b);
  }
  $('pasteTarget').innerHTML = TARGETS.map(([k, l]) => `<option value="${k}"${k === shownTable ? ' selected' : ''}>${l}</option>`).join('');
}

function renderTable() {
  if (!tune[shownTable]) shownTable = 'ignLow';
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
  const camTable = /Low$|High$/.test(shownTable);
  const live = !camTable || shownTable.endsWith(usingHigh && tune[shownTable.replace(/Low$/, 'High')] ? 'High' : 'Low');
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
  const T = tune.turbo || {};
  $('setTurbo').checked = !!T.enabled;
  for (const [id, v] of [['setInjCc', tune.injectorCc], ['setDead', tune.injectorDeadMs], ['setOverboost', tune.overboostKpa], ['setTorqueLim', tune.torqueLimitNm],
    ['setClutch', tune.clutchNm], ['setSpring', T.springKpa], ['setMaxBoost', T.maxBoostKpa], ['setTurbArea', T.turbineAreaCm2], ['setWgArea', T.wastegateAreaCm2], ['setCompTip', T.compressorTipMm], ['setWgGain', T.wgGain ?? 1]]) $(id).value = v ?? '';
  const F = { ...FLOW, ...(tune.flow || {}) };
  $('setTb').value = Math.round(F.throttleDiaM * 1000); $('setRunner').value = Math.round(F.runnerM * 1000); $('setExhaust').value = Math.round(F.exhaustDiaM * 1000);
}

function applyTune(t) {
  tune = t;
  setTune(sim, tune);
  saveTune(tune);
  renderSettings();
  renderTable();
  renderMaterials();
}

for (const [id, key] of [['setVtec', 'vtecRpm'], ['setVtecMap', 'vtecMinMap'], ['setRev', 'revLimit'], ['setOct', 'octane']]) {
  $(id).addEventListener('change', ev => {
    const v = Number(ev.target.value);
    if (Number.isFinite(v)) { tune[key] = v; saveTune(tune); }
  });
}
for (const [id, key] of [['setInjCc', 'injectorCc'], ['setDead', 'injectorDeadMs'], ['setOverboost', 'overboostKpa'], ['setTorqueLim', 'torqueLimitNm'], ['setClutch', 'clutchNm']]) {
  $(id).addEventListener('change', ev => {
    const v = ev.target.value === '' ? undefined : Number(ev.target.value);
    if (v === undefined || Number.isFinite(v)) { tune[key] = v; saveTune(tune); }
  });
}
for (const [id, key] of [['setTb', 'throttleDiaM'], ['setRunner', 'runnerM'], ['setExhaust', 'exhaustDiaM']]) {
  $(id).addEventListener('change', ev => {
    const v = Number(ev.target.value) / 1000;
    if (Number.isFinite(v) && v > 0) { tune.flow = { ...(tune.flow || {}), [key]: v }; saveTune(tune); }
  });
}
for (const [id, key] of [['setSpring', 'springKpa'], ['setMaxBoost', 'maxBoostKpa'], ['setTurbArea', 'turbineAreaCm2'], ['setWgArea', 'wastegateAreaCm2'], ['setCompTip', 'compressorTipMm'], ['setWgGain', 'wgGain']]) {
  $(id).addEventListener('change', ev => {
    const v = Number(ev.target.value);
    if (Number.isFinite(v)) { tune.turbo = { ...tune.turbo, [key]: v }; saveTune(tune); }
  });
}
$('setTurbo').addEventListener('change', ev => { tune.turbo = { ...tune.turbo, enabled: ev.target.checked }; saveTune(tune); });
$('btnTurbo').onclick = () => { applyTune(turboSetup(tune)); renderTabs(); };
$('fileCsv').addEventListener('change', async ev => {
  const msgs = [];
  for (const f of ev.target.files) {
    const target = targetForFile(f.name);
    if (!target) { if (/\.csv$/i.test(f.name)) msgs.push(`${f.name}: couldn't tell which table it is (name it afr, ignition, boost or wg_duty).`); continue; }
    try { loadInto(target, parseTable(await f.text())); msgs.push(`${f.name} → ${TARGETS.find(x => x[0] === target)[1]}`); }
    catch (err) { msgs.push(`${f.name}: ${err.message}`); }
  }
  applyTune(tune);
  renderTabs();
  $('pasteMsg').textContent = msgs.join(' · ');
  $('pasteMsg').closest('details').open = true;
  ev.target.value = '';
});

function loadInto(target, t) {
  for (const k of TARGETS.find(x => x[0] === target)[2]) tune[k] = t;
  if (target.startsWith('fuel')) tune.fuelUnits = guessFuelUnits(t);
  if (/Placeholder/.test(tune.name)) tune.name = 'Imported tables';
  shownTable = TARGETS.find(x => x[0] === target)[2][0];
}

$('setFuelUnits').addEventListener('change', ev => { tune.fuelUnits = ev.target.value; saveTune(tune); });

$('btnPaste').onclick = () => {
  const target = $('pasteTarget').value;
  try {
    const t = parseTable($('pasteBox').value);
    loadInto(target, t);
    applyTune(tune);
    renderTabs();
    $('pasteMsg').textContent = `Loaded ${t.rows.length} × ${t.cols.length} table into ${TARGETS.find(x => x[0] === target)[1]}.` +
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
  if (throttle > 0.95 && sim.car.locked && sim.running && e.rpm > 1500 && !e.revCut && !e.boostCut) {
    const bin = Math.round(e.rpm / 100) * 100;
    const best = dyno.cur.get(bin);
    if (!best || e.torqueNm > best) dyno.cur.set(bin, e.torqueNm);
  }
}

function drawDyno() {
  const c = $('dyno'), g = c.getContext('2d');
  const css = getComputedStyle(document.documentElement);
  const W = c.width, H = c.height, pad = 30;
  const peakVal = Math.max(200, ...[...dyno.cur, ...(dyno.prev || [])].map(([r, t]) => Math.max(t * r / 7120.9, t * NM_TO_LBFT)));
  const yMax = Math.ceil(peakVal / 50) * 50;
  g.clearRect(0, 0, W, H);
  g.strokeStyle = css.getPropertyValue('--line'); g.fillStyle = css.getPropertyValue('--muted'); g.font = '11px system-ui';
  const x = rpm => pad + (rpm - 1000) / 8000 * (W - pad * 2), y = v => H - pad - v / yMax * (H - pad * 1.5);
  for (let r = 1000; r <= 9000; r += 1000) { g.beginPath(); g.moveTo(x(r), pad / 2); g.lineTo(x(r), H - pad); g.stroke(); g.fillText(`${r / 1000}k`, x(r) - 6, H - pad + 14); }
  for (let v = 0; v <= yMax; v += 50) { g.fillText(v, 4, y(v) + 4); }
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

// ------------------------------------------------------------------ internals
const esc = t => String(t).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
function matCell(m, f, digits = 0) {
  const v = m[f];
  if (v === undefined || v === null) return '<td class="num muted">–</td>';
  const est = m.estimated?.[f];
  return `<td class="num"${est ? ` title="Estimated: ${esc(est)}"` : ''}>${typeof v === 'number' ? v.toFixed(digits) : esc(v)}${est ? ' <span class="muted">est.</span>' : ''}</td>`;
}
function renderMaterials() {
  const cols = [['densityKgM3', 'Density kg/m³'], ['modulusGPa', 'Modulus GPa'], ['yieldMPa', 'Yield MPa'], ['utsMPa', 'Tensile MPa'], ['fatigueMPa', 'Fatigue MPa'],
    ['elongationPct', 'Elong. %', 1], ['thermalExpansionUmMK', 'Expansion µm/m·K', 1], ['conductivityWmK', 'Conductivity W/m·K'], ['maxServiceTempC', 'Max °C']];
  $('matTable').innerHTML = `<thead><tr><th>Part</th><th>Alloy</th>${cols.map(c => `<th>${c[1]}</th>`).join('')}<th>Hot strength</th><th>Sources</th></tr></thead><tbody>` +
    PARTS.map(p => {
      const m = partMaterial(tune, p), id = Object.keys(MATERIALS).find(k => MATERIALS[k] === m);
      const opts = materialsFor(p.category).map(k => `<option value="${k}"${k === id ? ' selected' : ''}>${esc(MATERIALS[k].name)}</option>`).join('');
      const hot = m.strengthVsTempC ? m.strengthVsTempC.filter(([t]) => t >= 150).slice(0, 3).map(([t, f]) => `${(f * 100).toFixed(0)}% @ ${t}°`).join(', ') : '–';
      const src = (m.sources || []).map((u, i) => `<a href="${esc(u)}" target="_blank" rel="noopener">${i + 1}</a>`).join(' ');
      return `<tr><td>${p.name}</td><td><select data-part="${p.key}">${opts}</select></td>${cols.map(([f, , d]) => matCell(m, f, d)).join('')}<td${m.estimated?.strengthVsTempC ? ` title="Estimated: ${esc(m.estimated.strengthVsTempC)}"` : ''}>${hot}</td><td>${src || '–'}</td></tr>`;
    }).join('') + '</tbody>';
  $('matTable').querySelectorAll('select[data-part]').forEach(sel => sel.addEventListener('change', ev => {
    tune.materials = { ...(tune.materials || {}), [ev.target.dataset.part]: ev.target.value };
    saveTune(tune); renderMaterials();
  }));
  $('partsTable').innerHTML = '<thead><tr><th>Part</th><th>Load now</th><th></th><th>Life used</th><th>Status</th></tr></thead><tbody>' +
    PARTS.map(p => `<tr><td>${p.name}</td><td><span class="loadbar"><div id="lb-${p.key}"></div></span></td><td class="num" id="lv-${p.key}"></td><td class="num" id="dm-${p.key}"></td><td id="st-${p.key}"></td></tr>`).join('') + '</tbody>';
}
$('btnRebuild').onclick = () => { rebuildEngine(sim); startEngine(sim); };

function updateInternals() {
  const s = sim.internals;
  $('pmax').textContent = s.pmaxBar.toFixed(0);
  $('pistonC').textContent = s.pistonC.toFixed(0);
  $('rodComp').textContent = (s.rodCompN / 1000).toFixed(1);
  $('rodTen').textContent = (s.rodTensionN / 1000).toFixed(1);
  for (const p of PARTS) {
    const load = s.load[p.key], dmg = s.damage[p.key], st = partStatus(load, dmg);
    const bar = $(`lb-${p.key}`);
    bar.style.width = `${Math.min(100, load / 1.5 * 100)}%`;
    bar.className = st;
    $(`lv-${p.key}`).textContent = `${(load * 100).toFixed(0)}%`;
    $(`dm-${p.key}`).textContent = dmg > 0 && dmg < 0.001 ? '<0.1%' : `${(dmg * 100).toFixed(1)}%`;
    const cell = $(`st-${p.key}`);
    cell.textContent = { ok: 'OK', warn: 'Fatigue', danger: 'Overloaded', failed: 'FAILED' }[st];
    cell.className = `st-${st}`;
  }
  $('failBanner').hidden = !s.failed;
  if (s.failed) $('failBanner').textContent = `Engine failure: ${s.failed}. Press Rebuild engine to fit fresh parts.`;
}

// ------------------------------------------------------------------ 3D view (loaded on demand so the rest works without WebGL)
let r3d = null, r3dVisible = true;
import('./render3d.js').then(({ createRenderer }) => {
  try {
    r3d = createRenderer($('r3d'));
    r3d.setTimeScale(Number($('r3dSpeed').value));
  } catch (err) {
    $('r3d').textContent = `3D view unavailable: ${err.message}`;
  }
}).catch(err => { $('r3d').textContent = `3D view unavailable: ${err.message}`; });
$('r3dSpeed').addEventListener('change', ev => r3d?.setTimeScale(Number(ev.target.value)));
$('r3dStress').addEventListener('change', ev => r3d?.setStress(ev.target.checked));
$('r3dCut').addEventListener('change', ev => r3d?.setCutaway(ev.target.checked));
new IntersectionObserver(([en]) => { r3dVisible = en.isIntersecting; }).observe($('r3d'));

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
  $('boost').textContent = s.boost_psi.toFixed(1);
  $('boostTarget').textContent = s.boost_target_kpa === '' ? '–' : `${((s.boost_target_kpa - 101.3) * 0.145).toFixed(1)} psi`;
  $('wgDuty').textContent = s.wg_duty.toFixed(0);
  $('iat').textContent = s.iat_c.toFixed(0);
  $('boostCutLamp').classList.toggle('on', !!s.boost_cut);
  $('torqueLamp').classList.toggle('on', !!s.over_torque);
  const cut = e.revCut || e.dfco || e.boostCut;
  $('afr').textContent = cut ? 'cut' : s.afr.toFixed(1);
  $('lambda').textContent = cut ? '–' : s.lambda.toFixed(2);
  $('lamTarget').textContent = Number.isFinite(e.lambdaTarget) ? e.lambdaTarget.toFixed(2) : 'from ms table';
  $('ign').textContent = s.ign_deg.toFixed(1);
  $('ignTable').textContent = s.ign_table.toFixed(1);
  $('mbt').textContent = s.mbt_deg.toFixed(1);
  $('kl').textContent = s.knock_limit.toFixed(1);
  $('knockCount').textContent = s.knock_count;
  $('retard').textContent = s.knock_retard.toFixed(1);
  $('vtc').textContent = s.vtc_deg.toFixed(0);
  $('ve').textContent = (s.ve * 100).toFixed(0);
  $('ca50').textContent = cut || s.ca50_deg === '' ? '–' : s.ca50_deg.toFixed(0);
  $('knockIdx').textContent = s.knock_index.toFixed(2);
  $('egt').textContent = s.egt_c;
  $('exh').textContent = s.exh_kpa.toFixed(0);
  $('pmep').textContent = s.pmep_kpa;
  $('resid').textContent = s.residual_pct.toFixed(0);
  $('turboK').textContent = s.turbo_krpm;
  $('wgOpen').textContent = s.wg_open_pct;
  $('ipw').textContent = s.inj_ms.toFixed(2);
  $('duty').textContent = s.inj_duty.toFixed(0);
  $('tq').textContent = (s.torque_nm * NM_TO_LBFT).toFixed(0);
  $('hp').textContent = s.hp.toFixed(0);
  $('barThrottle').style.width = `${input.throttle * 100}%`;
  $('barBrake').style.width = `${input.brake * 100}%`;
  $('barClutch').style.width = `${(1 - sim.lastClutch) * 100}%`;
  $('engineState').textContent = sim.internals.failed ? `Engine failed: ${sim.internals.failed}` : sim.running ? 'Engine running' : 'Stalled: press R or Start';
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
  if (frame++ % 6 === 0) { drawDyno(); updateInternals(); }
  if (r3d && r3dVisible) r3d.update(sim, dt);
  $('btnCsv').disabled = sim.logging || !sim.log.length;
  requestAnimationFrame(loop);
}

$('carName').textContent = CAR.name;
renderTabs();
renderSettings();
renderTable();
renderLearnRows();
renderMaterials();
requestAnimationFrame(loop);
