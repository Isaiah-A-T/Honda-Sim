import { test } from 'node:test';
import assert from 'node:assert/strict';
import { parseTable, parseCurve, lookup1, lookup2, activeCell, guessFuelUnits } from '../src/tables.js';

test('parses a tab-separated paste with an empty corner cell', () => {
  const t = parseTable('\t30\t60\t100\n1000\t20\t18\t15\n3000\t30\t26\t22\n');
  assert.deepEqual(t.cols, [30, 60, 100]);
  assert.deepEqual(t.rows, [1000, 3000]);
  assert.deepEqual(t.data, [[20, 18, 15], [30, 26, 22]]);
});

test('parses a header row that is one cell shorter, with units and CRLF', () => {
  const t = parseTable('30 kPa\t60 kPa\r\n1000 rpm\t0.98\t0.95\r\n2000 rpm\t1.00\t0.90\r\n');
  assert.deepEqual(t.cols, [30, 60]);
  assert.deepEqual(t.rows, [1000, 2000]);
  assert.equal(t.data[1][1], 0.9);
});

test('explains a row with the wrong number of values', () => {
  assert.throws(() => parseTable('\t30\t60\n1000\t1\n'), /header has 2 columns/);
});

test('parses a curve as two rows or two columns', () => {
  assert.deepEqual(parseCurve('1 2 3\n10 20 30'), { axis: [1, 2, 3], values: [10, 20, 30] });
  assert.deepEqual(parseCurve('1\t10\n2\t20\n3\t30'), { axis: [1, 2, 3], values: [10, 20, 30] });
});

test('interpolates and clamps', () => {
  assert.equal(lookup1([0, 10], [0, 100], 5), 50);
  assert.equal(lookup1([0, 10], [0, 100], 20), 100);
  const t = { rows: [1000, 2000], cols: [20, 100], data: [[0, 10], [20, 30]] };
  assert.equal(lookup2(t, 1500, 60), 15);
  assert.equal(lookup2(t, 9000, 0), 20);
  assert.deepEqual(activeCell(t, 1900, 30), [1, 0]);
});

test('guesses fuel table units', () => {
  const tbl = data => ({ rows: [1], cols: [1, 2], data: [data] });
  assert.equal(guessFuelUnits(tbl([0.85, 1.0])), 'lambda');
  assert.equal(guessFuelUnits(tbl([12.5, 14.7])), 'afr');
  assert.equal(guessFuelUnits(tbl([2.1, 8.4])), 'ms');
});
