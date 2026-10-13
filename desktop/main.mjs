// Desktop app (Windows .exe): the same sim in its own window, with the local server built in.
// The 3D renderer and other programs read the live state at ws://127.0.0.1:<port>/ws (see docs/MODULES.md).
// Port 8080 unless HONDA_SIM_PORT says otherwise; if it's taken, a free port is used and shown in the title.
import { app, BrowserWindow, shell } from 'electron';
import { fileURLToPath } from 'node:url';
import { startServer } from '../server.js';

const root = fileURLToPath(new URL('..', import.meta.url));
const wanted = Number(process.env.HONDA_SIM_PORT) || 8080;

// Keep the sim running at full rate when its window is behind the 3D view or minimized.
app.commandLine.appendSwitch('disable-background-timer-throttling');
app.commandLine.appendSwitch('disable-renderer-backgrounding');
app.commandLine.appendSwitch('disable-backgrounding-occluded-windows');

if (!app.requestSingleInstanceLock()) app.quit();

app.whenReady().then(async () => {
  let srv;
  try { srv = await startServer({ port: wanted, root }); } catch { srv = await startServer({ port: 0, root }); }
  const win = new BrowserWindow({
    width: 1400, height: 950, title: `Honda Sim (state on port ${srv.port})`, autoHideMenuBar: true, backgroundColor: '#111317',
    webPreferences: { backgroundThrottling: false, contextIsolation: true, nodeIntegration: false, sandbox: true },
  });
  win.on('page-title-updated', ev => ev.preventDefault());
  win.webContents.setWindowOpenHandler(({ url }) => { if (/^https:/.test(url)) shell.openExternal(url); return { action: 'deny' }; });
  win.webContents.on('will-navigate', (ev, url) => { if (!url.startsWith(`http://127.0.0.1:${srv.port}/`)) ev.preventDefault(); });
  await win.loadURL(`http://127.0.0.1:${srv.port}/`);
});
app.on('window-all-closed', () => app.quit());
