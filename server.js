// Local server. Serves the app (browsers won't load ES modules from file://) and is the hub other
// programs, like the 3D renderer, connect to for the live sim state:
//
//   ws://127.0.0.1:8080/ws     WebSocket. The running sim page sends state frames; every other client
//                              receives them. JSON text frames: { type: 'state', v: 1, t, state }.
//   GET /state                 The latest state frame as JSON (for simple polling clients).
//
// `npm start` runs it on its own; the desktop app (desktop/main.mjs) starts it inside the exe.
// Only listens on 127.0.0.1, so nothing outside this computer can reach it.
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';

const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.txt': 'text/plain', '.png': 'image/png' };

export function startServer({ port = Number(process.env.PORT) || 8080, root = fileURLToPath(new URL('.', import.meta.url)) } = {}) {
  root = normalize(root.endsWith(sep) ? root : root + sep);
  const clients = new Set();
  let latest = null;

  const server = createServer(async (req, res) => {
    const url = new URL(req.url, 'http://x');
    if (url.pathname === '/state') {
      res.writeHead(latest ? 200 : 204, { 'content-type': 'application/json', 'cache-control': 'no-store' }).end(latest ?? '');
      return;
    }
    const path = normalize(decodeURIComponent(url.pathname)).replace(/^([/\\])+/, '');
    const file = normalize(join(root, path === '' ? 'index.html' : path));
    if (!file.startsWith(root)) { res.writeHead(403).end(); return; }
    try {
      const body = await readFile(file);
      res.writeHead(200, { 'content-type': types[extname(file)] || 'application/octet-stream' }).end(body);
    } catch {
      res.writeHead(404).end('not found');
    }
  });

  // Minimal WebSocket (RFC 6455): text frames, ping/pong, close. No extensions.
  server.on('upgrade', (req, socket) => {
    if (new URL(req.url, 'http://x').pathname !== '/ws' || !req.headers['sec-websocket-key']) { socket.destroy(); return; }
    const accept = createHash('sha1').update(req.headers['sec-websocket-key'] + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64');
    socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
    socket.setNoDelay(true);
    const client = { socket, buf: Buffer.alloc(0) };
    clients.add(client);
    if (latest) send(client, latest);
    socket.on('data', chunk => {
      client.buf = Buffer.concat([client.buf, chunk]);
      for (let f; (f = readFrame(client)); ) {
        if (f.op === 0x8) { socket.end(frame(Buffer.alloc(0), 0x8)); return; }
        if (f.op === 0x9) { socket.write(frame(f.data, 0xA)); continue; }
        if (f.op !== 0x1) continue;
        const text = f.data.toString('utf8');
        if (text.startsWith('{"type":"state"')) {          // from the sim page: keep it and pass it on
          latest = text;
          for (const c of clients) if (c !== client) send(c, text);
        }
      }
    });
    const drop = () => clients.delete(client);
    socket.on('close', drop); socket.on('error', drop);
  });

  return new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(port, '127.0.0.1', () => resolve({ server, port: server.address().port }));
  });
}

function frame(data, op = 0x1) {
  const n = data.length, head = n < 126 ? Buffer.from([0x80 | op, n]) : n < 65536 ? Buffer.from([0x80 | op, 126, n >> 8, n & 255]) : Buffer.concat([Buffer.from([0x80 | op, 127]), bigLen(n)]);
  return Buffer.concat([head, data]);
}
function bigLen(n) { const b = Buffer.alloc(8); b.writeBigUInt64BE(BigInt(n)); return b; }
function send(c, text) { if (!c.socket.destroyed && c.socket.writableLength < 1 << 20) c.socket.write(frame(Buffer.from(text))); }
function readFrame(c) {
  const b = c.buf;
  if (b.length < 2) return null;
  const op = b[0] & 0x0f, masked = b[1] & 0x80;
  let len = b[1] & 0x7f, off = 2;
  if (len === 126) { if (b.length < 4) return null; len = b.readUInt16BE(2); off = 4; }
  else if (len === 127) { if (b.length < 10) return null; len = Number(b.readBigUInt64BE(2)); off = 10; }
  const maskOff = off; if (masked) off += 4;
  if (b.length < off + len) return null;
  const data = Buffer.from(b.subarray(off, off + len));
  if (masked) for (let i = 0; i < len; i++) data[i] ^= b[maskOff + (i & 3)];
  c.buf = b.subarray(off + len);
  return { op, data };
}

if (process.argv[1] && fileURLToPath(import.meta.url) === normalize(process.argv[1])) {
  startServer().then(({ port }) => console.log(`Honda Sim running at http://localhost:${port} (state stream: ws://127.0.0.1:${port}/ws)`));
}
