// Calibration tables: lookups with interpolation, and parsing tables pasted out of FlashPro Manager.
//
// FlashPro .fpcal files are encrypted, so tables come in as text: select a table in FlashPro Manager,
// Ctrl+A, Ctrl+C, and paste. The parser accepts tab, comma or space separated numbers with the column
// axis on the first row and the row axis in the first column (the usual spreadsheet layout).

export const clamp = (x, lo, hi) => Math.min(hi, Math.max(lo, x));

// Index and blend fraction of x within an ascending (or descending) axis, clamped at the ends.
function locate(axis, x) {
  const n = axis.length;
  if (n === 1) return [0, 0];
  const asc = axis[n - 1] >= axis[0];
  const v = asc ? x : -x, a = i => (asc ? axis[i] : -axis[i]);
  if (v <= a(0)) return [0, 0];
  if (v >= a(n - 1)) return [n - 2, 1];
  let i = 0;
  while (i < n - 2 && v > a(i + 1)) i++;
  return [i, (v - a(i)) / (a(i + 1) - a(i))];
}

// 1D curve: lookup1(axis, values, x)
export function lookup1(axis, values, x) {
  if (axis.length === 1) return values[0];
  const [i, f] = locate(axis, x);
  return values[i] + (values[i + 1] - values[i]) * f;
}

// 2D table: rows indexed by rowAxis (RPM), columns by colAxis (MAP / load).
export function lookup2(table, row, col) {
  const { rows, cols, data } = table;
  const [ri, rf] = rows.length > 1 ? locate(rows, row) : [0, 0];
  const [ci, cf] = cols.length > 1 ? locate(cols, col) : [0, 0];
  const r1 = Math.min(ri + 1, rows.length - 1), c1 = Math.min(ci + 1, cols.length - 1);
  const top = data[ri][ci] + (data[ri][c1] - data[ri][ci]) * cf;
  const bot = data[r1][ci] + (data[r1][c1] - data[r1][ci]) * cf;
  return top + (bot - top) * rf;
}

// Which cell is "active" for a row/col value (nearest breakpoint), for highlighting like a tuning UI.
export function activeCell(table, row, col) {
  const near = (axis, x) => axis.reduce((best, v, i) => (Math.abs(v - x) < Math.abs(axis[best] - x) ? i : best), 0);
  return [near(table.rows, row), near(table.cols, col)];
}

const NUM = /^[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?$/;

function splitLine(line) {
  const sep = line.includes('\t') ? '\t' : line.includes(',') ? ',' : /\s+/;
  return line.split(sep).map(s => s.replace(/[%°]|kpa|rpm|ms/gi, '').trim());
}

// Parse pasted text into { rows, cols, data }. Throws with a readable message when the shape is wrong.
export function parseTable(text) {
  // Keep a leading tab: it is the empty corner cell above the row axis.
  const lines = String(text).replace(/\r/g, '').split('\n').map(l => (l.includes('\t') ? l.replace(/\s+$/, '') : l.trim())).filter(l => l.trim());
  if (lines.length < 2) throw new Error('Paste needs at least a header row and one data row.');
  const grid = lines.map(splitLine);
  const header = grid[0];
  // A header row either starts with an empty/label cell, or is one cell shorter than the data rows.
  const dataWidth = grid[1].length;
  const headerCells = header.length === dataWidth ? header.slice(1) : header;
  const cols = headerCells.filter(s => s !== '').map(Number);
  if (!cols.length || cols.some(v => Number.isNaN(v))) throw new Error('First row should be the column axis (e.g. MAP in kPa).');

  const rows = [], data = [];
  for (const [i, cells] of grid.slice(1).entries()) {
    const nums = cells.filter(s => s !== '');
    if (!nums.every(s => NUM.test(s))) throw new Error(`Row ${i + 2} has something that isn't a number: "${lines[i + 1]}"`);
    if (nums.length !== cols.length + 1) throw new Error(`Row ${i + 2} has ${nums.length - 1} values but the header has ${cols.length} columns.`);
    rows.push(Number(nums[0]));
    data.push(nums.slice(1).map(Number));
  }
  return { rows, cols, data };
}

// Parse a 1D curve: two lines (axis, values) or two columns (axis<TAB>value per line).
export function parseCurve(text) {
  const lines = String(text).replace(/\r/g, '').split('\n').map(l => l.trim()).filter(Boolean).map(splitLine);
  const nums = lines.map(cells => cells.filter(s => s !== '').map(Number));
  if (nums.length === 2 && nums[0].length === nums[1].length && nums[0].length > 1) return { axis: nums[0], values: nums[1] };
  if (nums.every(r => r.length === 2)) return { axis: nums.map(r => r[0]), values: nums.map(r => r[1]) };
  throw new Error('Curve should be two rows (axis, values) or two columns.');
}

// Guess what a fuel table holds from its values: lambda target, AFR target, or injector pulse width (ms).
export function guessFuelUnits(table) {
  const all = table.data.flat();
  const lo = Math.min(...all), hi = Math.max(...all);
  if (lo >= 0.5 && hi <= 1.3) return 'lambda';
  if (lo >= 9 && hi <= 20) return 'afr';
  return 'ms';
}
