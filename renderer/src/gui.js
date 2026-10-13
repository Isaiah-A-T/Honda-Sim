// Controls for the engine view: mode tabs and camera buttons over the 3D view, and a panel for the current
// mode (internals, block, airflow, stress, materials). Everything it shows comes from the view's state, so it
// works the same on fake data and on the real sim.
//
//   const gui = createEngineGui(view, { panel: document.getElementById('modePanel'), onMaterialsChange });
//
// Leave out `panel` and the panel floats over the top right of the view. Pass `partMaterials`
// ({ pistons: { label, family, detail }, ... }) to show the host's own materials read-only instead of the
// placeholder catalog in materials.js.

import { MODES, VIEWS } from './engine-renderer.js';
import { MATERIALS, FAMILIES, defaultMaterials, materialOf } from './materials.js';
import { K20, valveLift, cycleAngle, strokeName } from './kinematics.js';

const STRESS_PARTS = [['rods', 'Connecting rods'], ['pistons', 'Pistons'], ['crank', 'Crankshaft'], ['valves', 'Valves'],
  ['cams', 'Camshafts'], ['head', 'Head'], ['block', 'Block'], ['exhaust', 'Exhaust heat'], ['intake', 'Intake pressure'],
  ['turbo', 'Turbo'], ['clutch', 'Clutch'], ['flywheel', 'Flywheel']];

const BLURB = {
  full: 'Everything at once. Hover a part for its name and load.',
  internals: 'Crank, rods, pistons and the valvetrain with the castings and plumbing taken away.',
  block: 'The block, head, cam cover and oil pan as solid castings.',
  airflow: 'Charge air in blue, exhaust in orange. Each runner only moves while its valve is open, so the cylinders breathe in firing order.',
  stress: 'Parts tint yellow, orange, then red as they near or pass the limit the sim uses.',
  materials: 'Colour shows each part’s material. Pick a part here or click it in the view to change it.',
};

const CSS = `
.egui { --eg-bg: rgba(250,251,252,.92); --eg-solid: #f7f8f9; --eg-fg: #1b2026; --eg-muted: #5d6874; --eg-line: #cfd5dc;
  --eg-track: #e3e7eb; --eg-on: #1b2026; --eg-on-fg: #f7f8f9; --eg-accent: #c8102e; --eg-ok: #1f9d55; --eg-warn: #d98a00;
  --eg-bad: #d42a1f; --eg-mid: #d9b31f; --eg-intake: #2f8fe0; --eg-exhaust: #e0702a;
  font: 13px/1.4 "Barlow", system-ui, sans-serif; color: var(--eg-fg); }
@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) .egui {
  --eg-bg: rgba(22,26,30,.9); --eg-solid: #181c20; --eg-fg: #e8ecf0; --eg-muted: #8b96a2; --eg-line: #2f363d; --eg-track: #262c32;
  --eg-on: #e8ecf0; --eg-on-fg: #14171a; --eg-accent: #ff3b4f; --eg-ok: #3fcf7f; --eg-warn: #f2a93b; --eg-bad: #ff5a4e;
  --eg-mid: #e8c63a; --eg-intake: #6ec8ff; --eg-exhaust: #ff8a3d; } }
:root[data-theme="dark"] .egui {
  --eg-bg: rgba(22,26,30,.9); --eg-solid: #181c20; --eg-fg: #e8ecf0; --eg-muted: #8b96a2; --eg-line: #2f363d; --eg-track: #262c32;
  --eg-on: #e8ecf0; --eg-on-fg: #14171a; --eg-accent: #ff3b4f; --eg-ok: #3fcf7f; --eg-warn: #f2a93b; --eg-bad: #ff5a4e;
  --eg-mid: #e8c63a; --eg-intake: #6ec8ff; --eg-exhaust: #ff8a3d; }
.egui button, .egui select { font: 500 13px "Barlow", system-ui, sans-serif; color: var(--eg-fg); background: var(--eg-solid);
  border: 1px solid var(--eg-line); border-radius: 6px; padding: 6px 10px; cursor: pointer; }
.egui button:hover { border-color: var(--eg-muted); }
.egui :focus-visible { outline: 2px solid var(--eg-accent); outline-offset: 2px; }
.egui-modes { position: absolute; left: 12px; top: 12px; right: 12px; display: flex; flex-wrap: wrap; gap: 4px; z-index: 2; pointer-events: none; }
.egui-modes button { pointer-events: auto; background: var(--eg-bg); backdrop-filter: blur(6px); }
.egui-modes button[aria-selected="true"] { background: var(--eg-on); color: var(--eg-on-fg); border-color: var(--eg-on); }
.egui-views { position: absolute; left: 12px; bottom: 12px; display: flex; flex-wrap: wrap; gap: 4px; z-index: 2; }
.egui-views button { background: var(--eg-bg); padding: 4px 8px; font-size: 12px; }
.egui-float { position: absolute; right: 12px; top: 56px; width: 270px; max-width: calc(100% - 24px); max-height: calc(100% - 110px);
  overflow-y: auto; background: var(--eg-bg); backdrop-filter: blur(6px); border: 1px solid var(--eg-line); border-radius: 8px; padding: 12px; z-index: 2; }
.egui-panel { display: grid; gap: 12px; }
.egui-panel h3 { margin: 0; font: 700 12px "Barlow Semi Condensed", "Arial Narrow", sans-serif; text-transform: uppercase; letter-spacing: .1em; color: var(--eg-muted); }
.egui-panel p { margin: 0; color: var(--eg-muted); font-size: 12px; }
.egui-rows { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; }
.egui-bar { display: grid; grid-template-columns: minmax(84px, 1fr) minmax(60px, 1.4fr) 42px; align-items: center; gap: 8px; }
.egui-bar b, .egui-num { font: 500 12px "JetBrains Mono", ui-monospace, monospace; font-variant-numeric: tabular-nums; text-align: right; }
.egui-meter { height: 6px; background: var(--eg-track); border-radius: 3px; overflow: hidden; }
.egui-meter i { display: block; height: 100%; width: 0; background: var(--eg-ok); }
.egui-meter i[data-level=mid] { background: var(--eg-mid); } .egui-meter i[data-level=high] { background: var(--eg-warn); }
.egui-meter i[data-level=over] { background: var(--eg-bad); }
.egui-meter i.intake { background: var(--eg-intake); } .egui-meter i.exhaust { background: var(--eg-exhaust); } .egui-meter i.vtc { background: var(--eg-ok); }
.egui-scale { height: 6px; border-radius: 3px; background: linear-gradient(90deg, var(--eg-ok), var(--eg-mid) 45%, var(--eg-warn) 70%, var(--eg-bad) 85%); }
.egui-scale-labels { display: flex; justify-content: space-between; font-size: 11px; color: var(--eg-muted); }
.egui-kv { display: grid; grid-template-columns: 1fr auto; gap: 4px 12px; margin: 0; }
.egui-kv dt { color: var(--eg-muted); } .egui-kv dd { margin: 0; text-align: right; font: 500 12px "JetBrains Mono", ui-monospace, monospace; font-variant-numeric: tabular-nums; }
.egui-lamp { display: inline-block; font: 700 11px "Barlow Semi Condensed", sans-serif; letter-spacing: .08em; padding: 2px 7px;
  border-radius: 4px; border: 1px solid var(--eg-line); color: var(--eg-muted); }
.egui-lamp.on { background: var(--eg-ok); color: #fff; border-color: transparent; }
.egui-cyl { display: grid; grid-template-columns: 22px 78px minmax(0, 1fr) minmax(0, 1fr); align-items: center; gap: 6px; font-size: 12px; }
.egui-cyl.head { color: var(--eg-muted); font-size: 11px; }
.egui-cyl .n { font: 600 13px "JetBrains Mono", ui-monospace, monospace; }
.egui-chips { display: grid; grid-template-columns: repeat(4, 1fr); gap: 4px; }
.egui-chips span { text-align: center; padding: 4px 0; border-radius: 4px; background: var(--eg-track); font-size: 11px; color: var(--eg-muted); }
.egui-chips span.intake { box-shadow: inset 0 -3px 0 var(--eg-intake); color: var(--eg-fg); }
.egui-chips span.exhaust { box-shadow: inset 0 -3px 0 var(--eg-exhaust); color: var(--eg-fg); }
.egui-chips b { display: block; font: 600 13px "JetBrains Mono", ui-monospace, monospace; color: var(--eg-fg); }
.egui-legend { display: flex; flex-wrap: wrap; gap: 4px 10px; font-size: 12px; }
.egui-legend span { display: inline-flex; align-items: center; gap: 5px; }
.egui-legend i { width: 10px; height: 10px; border-radius: 2px; display: inline-block; }
.egui-mat { display: grid; grid-template-columns: 1fr; gap: 3px; padding: 6px; border-radius: 6px; border: 1px solid transparent; }
.egui-mat.sel { border-color: var(--eg-accent); }
.egui-mat label { display: flex; align-items: center; gap: 6px; font-weight: 500; }
.egui-mat select { width: 100%; padding: 4px 6px; }
.egui-mat small { color: var(--eg-muted); font-size: 11px; }
.egui-row { display: flex; gap: 8px; align-items: center; justify-content: space-between; }
`;

const hex = n => `#${n.toString(16).padStart(6, '0')}`;
const level = v => (v >= 1 ? 'over' : v >= 0.8 ? 'high' : v >= 0.5 ? 'mid' : 'low');
const stressVal = (st, k) => { const v = st.stress?.[k]; return Array.isArray(v) ? Math.max(...v) : v ?? 0; };

export function createEngineGui(view, options = {}) {
  const opts = { panel: null, materials: defaultMaterials(), onMaterialsChange: null, partMaterials: null, ...options };
  const materials = { ...defaultMaterials(), ...opts.materials };
  const host = view.container;

  if (!document.getElementById('egui-style')) {
    const style = document.createElement('style');
    style.id = 'egui-style';
    style.textContent = CSS;
    document.head.appendChild(style);
  }

  const modes = document.createElement('div');
  modes.className = 'egui egui-modes';
  modes.setAttribute('role', 'tablist');
  modes.setAttribute('aria-label', 'Engine view mode');
  modes.innerHTML = Object.entries(MODES).map(([k, m]) =>
    `<button role="tab" id="egui-tab-${k}" data-mode="${k}" aria-selected="false">${m.label}</button>`).join('');
  host.appendChild(modes);

  const views = document.createElement('div');
  views.className = 'egui egui-views';
  views.setAttribute('aria-label', 'Camera');
  views.innerHTML = Object.entries(VIEWS).map(([k, v]) => `<button data-view="${k}">${v.label}</button>`).join('');
  host.appendChild(views);

  const panel = opts.panel ?? Object.assign(document.createElement('div'), { className: 'egui-float' });
  if (!opts.panel) host.appendChild(panel);
  panel.classList.add('egui', 'egui-panel');
  panel.setAttribute('role', 'tabpanel');

  let refreshers = [];
  const $ = sel => panel.querySelector(sel);

  function bars(keys, prefix) {
    return `<ul class="egui-rows">${keys.map(([k, label]) =>
      `<li class="egui-bar"><span>${label}</span><div class="egui-meter"><i id="${prefix}-${k}"></i></div><b id="${prefix}v-${k}">0%</b></li>`).join('')}</ul>`;
  }
  function setBar(prefix, k, v, max = 1.2) {
    const el = $(`#${prefix}-${k}`);
    if (!el) return;
    el.style.width = `${Math.min(Math.max(v, 0), max) / max * 100}%`;
    el.dataset.level = level(v);
    $(`#${prefix}v-${k}`).textContent = `${Math.round(v * 100)}%`;
  }

  function render(mode) {
    for (const b of modes.querySelectorAll('button')) b.setAttribute('aria-selected', String(b.dataset.mode === mode));
    panel.setAttribute('aria-labelledby', `egui-tab-${mode}`);
    refreshers = [];
    const blurb = mode === 'materials' && opts.partMaterials ? 'Colour shows each part’s material. Click a part in the view to find it in the list.' : BLURB[mode];
    let html = `<h3>${MODES[mode].label}</h3><p>${blurb}</p>`;

    if (mode === 'full') {
      html += `<div class="egui-row"><span>Block and head</span><select id="egui-xray">
        <option value="true">See-through</option><option value="solid">Solid</option><option value="hidden">Hidden</option></select></div>`;
    }

    if (mode === 'internals') {
      html += `<div class="egui-row"><span class="egui-lamp" id="egui-vtec">VTEC</span>
        <span>VTC intake advance <b class="egui-num" id="egui-vtc">0°</b></span></div>
        <div class="egui-meter" title="VTC advance, 0–${K20.vtcMaxDeg}°"><i class="vtc" id="egui-vtcbar"></i></div>
        <div class="egui-rows">
          <div class="egui-cyl head"><span>Cyl</span><span>Stroke</span><span>Intake lift</span><span>Exhaust lift</span></div>
          ${[1, 2, 3, 4].map(c => `<div class="egui-cyl"><span class="n">${c}</span><span id="egui-st${c}">–</span>
            <div class="egui-meter"><i class="intake" id="egui-il${c}"></i></div><div class="egui-meter"><i class="exhaust" id="egui-el${c}"></i></div></div>`).join('')}
        </div><p>Firing order 1-3-4-2. Lift bars are full at high-cam peak lift.</p>`;
      refreshers.push(st => {
        $('#egui-vtec').classList.toggle('on', !!st.vtec);
        $('#egui-vtc').textContent = `${Math.round(st.vtcDeg ?? 0)}°`;
        $('#egui-vtcbar').style.width = `${Math.min((st.vtcDeg ?? 0) / K20.vtcMaxDeg, 1) * 100}%`;
        for (let c = 1; c <= 4; c++) {
          $(`#egui-st${c}`).textContent = strokeName(cycleAngle(c, st.crankDeg));
          for (const [side, id] of [['intake', 'il'], ['exhaust', 'el']])
            $(`#egui-${id}${c}`).style.width = `${valveLift(c, st.crankDeg, side, st.vtec, st.vtcDeg) / K20.cams.high[side].lift * 100}%`;
        }
      });
    }

    if (mode === 'block') {
      html += `<dl class="egui-kv">
        <dt>Displacement</dt><dd>1,998 cc</dd><dt>Bore × stroke</dt><dd>${K20.bore} × ${K20.stroke} mm</dd>
        <dt>Rod length</dt><dd>${K20.rodLength} mm</dd><dt>Bore spacing</dt><dd>${K20.boreSpacing} mm</dd>
        <dt>Block</dt><dd id="egui-blockmat"></dd><dt>Head</dt><dd id="egui-headmat"></dd></dl>
        ${bars([['block', 'Block'], ['head', 'Head']], 'egui-b')}`;
      refreshers.push(st => {
        const mat = k => (opts.partMaterials ? opts.partMaterials[k] : materialOf(k, materials))?.label ?? '–';
        $('#egui-blockmat').textContent = mat('block');
        $('#egui-headmat').textContent = mat('head');
        for (const k of ['block', 'head']) setBar('egui-b', k, stressVal(st, k));
      });
    }

    if (mode === 'airflow') {
      html += `<div class="egui-legend"><span><i style="background:var(--eg-intake)"></i>Intake charge</span>
        <span><i style="background:var(--eg-exhaust)"></i>Exhaust</span></div>
        <dl class="egui-kv"><dt>Airflow</dt><dd id="egui-air">0 g/s</dd><dt>MAP</dt><dd id="egui-map">0 kPa</dd>
        <dt>Boost</dt><dd id="egui-boost">–</dd><dt>Throttle</dt><dd id="egui-tps">0%</dd></dl>
        <div><h3 style="margin-bottom:6px">Breathing now</h3><div class="egui-chips" id="egui-breath"></div></div>
        ${bars([['exhaust', 'Exhaust heat'], ['intake', 'Intake pressure'], ['turbo', 'Turbo']], 'egui-a')}`;
      refreshers.push(st => {
        $('#egui-air').textContent = `${Math.round(st.airGs ?? 0)} g/s`;
        $('#egui-map').textContent = `${Math.round(st.mapKpa)} kPa`;
        $('#egui-boost').textContent = st.turbo ? `${(st.boostKpa * 0.145).toFixed(1)} psi` : 'NA';
        $('#egui-tps').textContent = `${Math.round(st.throttle * 100)}%`;
        $('#egui-breath').innerHTML = [1, 2, 3, 4].map(c => {
          const i = valveLift(c, st.crankDeg, 'intake', st.vtec, st.vtcDeg), e = valveLift(c, st.crankDeg, 'exhaust', st.vtec);
          const cls = i > e && i > 0.5 ? 'intake' : e > 0.5 ? 'exhaust' : '';
          return `<span class="${cls}"><b>${c}</b>${cls === 'intake' ? 'inhaling' : cls === 'exhaust' ? 'exhaling' : 'sealed'}</span>`;
        }).join('');
        for (const k of ['exhaust', 'intake', 'turbo']) setBar('egui-a', k, stressVal(st, k));
      });
    }

    if (mode === 'stress') {
      html += `<div><div class="egui-scale"></div><div class="egui-scale-labels"><span>0%</span><span>50%</span><span>80%</span><span>limit</span></div></div>
        ${bars(STRESS_PARTS, 'egui-s')}
        <p>Percent of the limit the sim uses (350 Nm stock rods, rev limit, clutch capacity, overboost cut). Stronger materials lower it.</p>`;
      refreshers.push(st => { for (const [k] of STRESS_PARTS) setBar('egui-s', k, stressVal(st, k)); });
    }

    if (mode === 'materials' && opts.partMaterials) {
      const pm = opts.partMaterials;
      const used = new Set(Object.values(pm).map(m => m.family));
      html += `<div class="egui-legend">${Object.entries(FAMILIES).filter(([f]) => used.has(f))
        .map(([, f]) => `<span><i style="background:${hex(f.color)}"></i>${f.label}</span>`).join('')}</div>
        <div class="egui-rows">${Object.entries(pm).map(([k, m]) => `<div class="egui-mat" id="egui-m-${k}">
          <label><i style="width:10px;height:10px;border-radius:2px;background:${hex(FAMILIES[m.family]?.color ?? 0x888888)}"></i>${m.part ?? k}</label>
          <span>${m.label}</span>${m.detail ? `<small>${m.detail}</small>` : ''}</div>`).join('')}</div>
        <p>These are the materials the sim's internals model uses. Click a part in the view to find it here.</p>`;
    } else if (mode === 'materials') {
      const used = new Set(Object.keys(MATERIALS).map(k => materialOf(k, materials).family));
      html += `<div class="egui-legend">${Object.entries(FAMILIES).filter(([f]) => used.has(f))
        .map(([, f]) => `<span><i style="background:${hex(f.color)}"></i>${f.label}</span>`).join('')}</div>
        <div class="egui-rows">${Object.entries(MATERIALS).map(([k, part]) => {
          const cur = materialOf(k, materials);
          return `<div class="egui-mat" id="egui-m-${k}"><label for="egui-ms-${k}">
            <i style="width:10px;height:10px;border-radius:2px;background:${hex(FAMILIES[cur.family].color)}"></i>${part.label}</label>
            <select id="egui-ms-${k}" data-part="${k}">${part.options.map(o =>
              `<option value="${o.id}"${o.id === cur.id ? ' selected' : ''}>${o.label}</option>`).join('')}</select>
            <small>Rough relative strength ×${cur.strength.toFixed(2)}</small></div>`;
        }).join('')}</div>
        <p>Strength numbers are placeholders until the sim models materials; they only scale the stress view.</p>`;
    }

    panel.innerHTML = html;

    if (mode === 'full') {
      const sel = $('#egui-xray');
      sel.value = String(view.getOptions().xray);
      sel.onchange = () => view.setOptions({ xray: sel.value === 'true' ? true : sel.value });
    }
    if (mode === 'materials') {
      for (const sel of panel.querySelectorAll('select[data-part]')) {
        sel.onchange = () => {
          materials[sel.dataset.part] = sel.value;
          view.setState({ materials });
          opts.onMaterialsChange?.({ ...materials });
          select(sel.dataset.part);
          render('materials');
          select(sel.dataset.part);
        };
        sel.onfocus = () => select(sel.dataset.part);
      }
    }
  }

  function select(key) {
    view.select(key);
    for (const el of panel.querySelectorAll('.egui-mat')) el.classList.toggle('sel', el.id === `egui-m-${key}`);
    const row = key && $(`#egui-m-${key}`);
    row?.scrollIntoView({ block: 'nearest' });
  }

  function setMode(mode) {
    view.setMode(mode);
    render(mode);
  }

  modes.addEventListener('click', e => { const m = e.target.closest('[data-mode]')?.dataset.mode; if (m) setMode(m); });
  modes.addEventListener('keydown', e => {
    if (!['ArrowLeft', 'ArrowRight'].includes(e.key)) return;
    const keys = Object.keys(MODES);
    const next = keys[(keys.indexOf(view.mode) + (e.key === 'ArrowRight' ? 1 : keys.length - 1)) % keys.length];
    setMode(next);
    modes.querySelector(`[data-mode="${next}"]`).focus();
  });
  views.addEventListener('click', e => { const v = e.target.closest('[data-view]')?.dataset.view; if (v) view.setView(v); });

  view.setOptions({
    onPick(part) {
      if (view.mode !== 'materials') return;
      const known = opts.partMaterials ?? MATERIALS;
      select(part?.key && known[part.key] ? part.key : null);
    },
  });
  view.setState({ materials, partMaterials: opts.partMaterials });

  let lastRefresh = 0;
  const stop = view.onFrame(v => {
    const now = performance.now();
    if (now - lastRefresh < 90 || !refreshers.length) return;
    lastRefresh = now;
    const st = v.getState();
    for (const fn of refreshers) fn(st);
  });

  render(view.mode);

  return {
    setMode,
    get mode() { return view.mode; },
    get materials() { return { ...materials }; },
    dispose() { stop(); modes.remove(); views.remove(); if (!opts.panel) panel.remove(); else panel.innerHTML = ''; },
  };
}
