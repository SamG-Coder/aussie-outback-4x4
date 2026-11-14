// Drives the browser version in a headless Edge of its own (its own profile, no window) and
// reports what a drive downloads: node measure.mjs BASE_URL SECONDS BLOCK_KB...
import { spawn } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const [base, seconds, ...sizes] = process.argv.slice(2);
const port = 9400 + Math.floor(Math.random() * 400);
// PROFILE=DIR keeps the browser's own storage from one run to the next, to see what a second visit costs.
const profile = process.env.PROFILE || mkdtempSync(join(tmpdir(), "outback-measure-"));
const edge = spawn("C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe", [
  "--headless=new", `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`, "--no-first-run",
  "--enable-unsafe-webgpu", "--window-size=1600,900", "--mute-audio", "about:blank",
], { stdio: "ignore" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function tab() {
  for (let i = 0; i < 40; i++) {
    try {
      const list = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
      const page = list.find((t) => t.type === "page");
      if (page) return page.webSocketDebuggerUrl;
    } catch { /* not up yet */ }
    await sleep(250);
  }
  throw new Error("Edge did not start");
}

const socket = new WebSocket(await tab());
await new Promise((r) => socket.addEventListener("open", r, { once: true }));
let next = 1;
const waiting = new Map();
const urls = new Map();
let failures = 0;
socket.addEventListener("message", ({ data }) => {
  const message = JSON.parse(data);
  if (message.method === "Network.requestWillBeSent") urls.set(message.params.requestId, `${message.params.request.method} ${message.params.request.url} ${message.params.request.headers.Range ?? ""}`);
  if (message.method === "Network.loadingFailed" && failures++ < 4) console.log("failed:", message.params.errorText, message.params.blockedReason ?? "", urls.get(message.params.requestId));
  if (message.method === "Runtime.consoleAPICalled" && ["warning", "error"].includes(message.params.type) && failures++ < 8) console.log("console:", message.params.args.map((a) => a.value ?? a.description).join(" ").slice(0, 300), "| from:", (message.params.stackTrace?.callFrames ?? []).slice(0, 4).map((f) => `${f.functionName || "?"}@${f.url.split("/").pop()}:${f.lineNumber + 1}`).join(" < "));
  if (message.id && waiting.has(message.id)) { waiting.get(message.id)(message); waiting.delete(message.id); }
});
const send = (method, params = {}) => new Promise((resolve) => {
  const id = next++;
  waiting.set(id, resolve);
  socket.send(JSON.stringify({ id, method, params }));
});
const run = async (expression) => {
  const answer = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (answer.result?.exceptionDetails) throw new Error(JSON.stringify(answer.result.exceptionDetails).slice(0, 300));
  return answer.result?.result?.value;
};

try {
  await send("Page.enable");
  await send("Network.enable");
  await send("Runtime.enable");
  for (const size of sizes) {
    await send("Page.navigate", { url: base });
    let ready = false;
    for (let i = 0; i < 120 && !ready; i++) {
      await sleep(500);
      ready = await run("!!(window.solstice && window.uiHost)").catch(() => false);
    }
    if (!ready) {
      console.log(JSON.stringify({ size, failed: await run("document.getElementById('message')?.textContent").catch(() => "no page") }));
      continue;
    }
    const start = await run("({ requests: solstice.file.requests, mb: solstice.file.bytesFetched / 1e6 })");
    await run("uiHost.say('do start'); solstice.viewer.key('KeyP', true); setTimeout(() => solstice.viewer.key('KeyP', false), 200); 1");
    await sleep(Number(seconds) * 1000);
    const end = await run(`(() => { const f = solstice.file, v = solstice.viewer, ui = document.getElementById('ui').contentDocument;
      return { requests: f.requests, mb: f.bytesFetched / 1e6, used: f.bytesKept / 1e6, shared: (f.bytesShared || 0) / 1e6, pages: v.pages_installed(), total: v.pages_total(),
        reading: f.reading, frames: solstice.frames, speed: ui.getElementById('speed').textContent,
        strip: ui.getElementById('stream').hidden ? 'hidden' : ui.getElementById('stream-what').textContent + ' | ' + ui.getElementById('stream-much').textContent }; })()`);
    console.log(JSON.stringify({
      run: size, toStart: { requests: start.requests, MB: Math.round(start.mb) },
      afterDrive: { requests: end.requests, MB: Math.round(end.mb), usedMB: Math.round(end.used), alreadyHereMB: Math.round(end.shared), pages: `${end.pages} of ${end.total}`,
        stillReading: end.reading, frames: end.frames, kmh: end.speed, strip: end.strip },
    }));
  }
} finally {
  // The browser and everything it started, and nothing else: by the process this began.
  await send("Browser.close").catch(() => {});
  spawn("taskkill", ["/PID", String(edge.pid), "/T", "/F"], { stdio: "ignore" });
  await sleep(1500);
  if (!process.env.PROFILE) try { rmSync(profile, { recursive: true, force: true }); } catch { /* still in use: the system clears it */ }
}
