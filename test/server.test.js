import { test } from 'node:test';
import assert from 'node:assert/strict';
import { startServer } from '../server.js';

const open = url => new Promise((res, rej) => { const ws = new WebSocket(url); ws.onopen = () => res(ws); ws.onerror = rej; });

test('the local server passes sim state to other programs over WebSocket and /state', async () => {
  const { server, port } = await startServer({ port: 0 });
  try {
    const sim = await open(`ws://127.0.0.1:${port}/ws`), viewer = await open(`ws://127.0.0.1:${port}/ws`);
    const got = new Promise(res => { viewer.onmessage = ev => res(JSON.parse(ev.data)); });
    const big = { type: 'state', v: 1, t: 1, state: { rpm: 3000, pad: 'x'.repeat(70000) } };   // also exercises 64-bit frame lengths
    sim.send(JSON.stringify(big));
    const msg = await got;
    assert.equal(msg.state.rpm, 3000);
    const r = await fetch(`http://127.0.0.1:${port}/state`);
    assert.equal((await r.json()).state.rpm, 3000);
    const page = await fetch(`http://127.0.0.1:${port}/`);
    assert.match(await page.text(), /Honda Sim/);
    sim.close(); viewer.close();
  } finally { server.close(); server.closeAllConnections?.(); }
});
