// Sends the live sim state to the local server (server.js), which passes it to other programs such as
// the 3D renderer exe. Uses the same origin the page was served from; does nothing if there's no server.
import { renderState } from './simstate.js';

export function createBridge(sim, { hz = 60 } = {}) {
  let ws = null, lastSent = 0, retry = 0;
  const connect = () => {
    if (typeof WebSocket === 'undefined' || !/^https?:$/.test(location.protocol)) return;
    try { ws = new WebSocket(`ws://${location.host}/ws`); } catch { ws = null; return; }
    ws.onclose = () => { ws = null; retry = performance.now() + 2000; };
    ws.onerror = () => ws?.close();
  };
  connect();
  return {
    tick(now = performance.now()) {
      if (!ws) { if (now > retry) connect(); return; }
      if (ws.readyState !== 1 || now - lastSent < 1000 / hz || ws.bufferedAmount > 1 << 18) return;
      lastSent = now;
      ws.send(JSON.stringify({ type: 'state', v: 1, t: now / 1000, state: renderState(sim) }));
    },
    get connected() { return ws?.readyState === 1; },
  };
}
