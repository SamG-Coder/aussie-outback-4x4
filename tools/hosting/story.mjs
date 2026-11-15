// Begins a story in the browser version, in a headless Edge of its own (its own profile, no
// window), and says how far it got: node story.mjs BASE_URL [OUT_FOLDER]
//
//   node tools/hosting/story.mjs https://test.rust4x4.pages.dev/play/game.html target/story
//
// A new game is begun in the first place, the first of the trucks offered is taken, and the
// game is left to come to the hub in it; then the town is opened as a free drive, to see the
// used-truck yard. A picture is kept of each step, and what the pages were told is printed.
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const [base, out = "target/story"] = process.argv.slice(2);
mkdirSync(out, { recursive: true });
const port = 9400 + Math.floor(Math.random() * 400);
const profile = mkdtempSync(join(tmpdir(), "outback-measure-"));
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
let said = 0;
socket.addEventListener("message", ({ data }) => {
  const message = JSON.parse(data);
  if (message.method === "Runtime.consoleAPICalled" && ["warning", "error"].includes(message.params.type) && said++ < 12) {
    console.log("console:", message.params.args.map((a) => a.value ?? a.description).join(" ").slice(0, 300));
  }
  if (message.method === "Runtime.exceptionThrown" && said++ < 12) {
    console.log("exception:", JSON.stringify(message.params.exceptionDetails).slice(0, 400));
  }
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
// What the game's pages were last told, as far as it says where the game is.
const told = () => run(`(() => { const ui = document.getElementById('ui')?.contentWindow; if (!ui) return null;
  return ui.eval("JSON.stringify({ screen: state.screen, map: state.map && state.map.name, story: !!(state.story && state.story.on), page: state.yard && state.yard.page, " +
    "start: state.yard && (state.yard.start || []).map((t) => t.name), trucks: state.yard && (state.yard.trucks || []).map((t) => t.name), saves: (state.saves || []).length, tag: state.tag || null })"); })()`)
  .then((text) => (text ? JSON.parse(text) : null)).catch(() => null);
// Waits for the page that is there to be a game that is running, on one of these screens.
async function until(screens, seconds = 120) {
  for (let i = 0; i < seconds * 2; i++) {
    await sleep(500);
    const ready = await run("!!(window.solstice && window.uiHost)").catch(() => false);
    const now = ready ? await told() : null;
    if (now && screens.includes(now.screen)) return now;
    const failed = await run("document.getElementById('message')?.className === 'error' ? document.getElementById('message').textContent : ''").catch(() => "");
    if (failed) return { failed };
  }
  return { failed: `never came to ${screens.join(" or ")}: ${JSON.stringify(await told())}` };
}
async function picture(name) {
  const shot = await send("Page.captureScreenshot", { format: "png" });
  if (shot.result?.data) writeFileSync(join(out, `${name}.png`), Buffer.from(shot.result.data, "base64"));
}
const step = (name, what) => console.log(`${name}: ${JSON.stringify(what)}`);

try {
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Page.navigate", { url: base });
  const title = await until(["title"]);
  step("title", title);
  await picture("1-title");
  // FREE=TRUCK sets up a free drive instead of a story: that truck, as a monster truck, in
  // the arena at dusk in a storm, each step a page of the browser.
  if (process.env.FREE && !title.failed) {
    const say = (line) => run(`uiHost.say('do ${line}'); 1`);
    await say("free_page");
    await sleep(1500);
    step("which truck", await until(["yard"]));
    await say("monster:on");
    await sleep(1500);
    step("as monsters", await until(["yard"]));
    await sleep(5000);
    await picture("f1-trucks");
    await say(`free:${process.env.FREE}`);
    await sleep(1500);
    step("build it", await until(["yard"]));
    await sleep(5000);
    await picture("f2-build");
    for (const line of ["page:where", "where:arena", "time:2", "weather:3"]) { await say(line); await sleep(400); }
    step("where and when", await told());
    await say("drive");
    await sleep(1500);
    step("driving", await until(["drive"]));
    await sleep(12000);
    await picture("f3-arena");
  } else if (!title.failed) {
    // A new game, in the first place: its first truck is chosen in the showroom.
    await run("uiHost.say('do save_new:1'); 1");
    await sleep(1500);
    const start = await until(["yard"]);
    step("first truck", start);
    await sleep(6000);
    await picture("2-first-truck");
    if (!start.failed && start.start?.length) {
      await run(`uiHost.say('do choose:${start.start[0]}'); 1`);
      await sleep(2500);
      step("its colour", await told());
      await picture("3-colour");
      await run("uiHost.say('do take:b01818'); 1");
      await sleep(1500);
      const hub = await until(["drive"]);
      step("the hub", hub);
      await sleep(8000);
      await picture("4-hub");
      step("kept", await run("Object.keys(localStorage).map((key) => key + ' ' + (localStorage.getItem(key) || '').length)"));
      // The page read again where it is: the story goes on, in the same truck.
      await send("Page.reload");
      const again = await until(["drive", "title"]);
      step("read again", again);
    }
  }
  // The town, by its address: the day's used trucks are parked across the road.
  if (!process.env.FREE) {
    const town = new URL(base);
    town.search = "?scene=town&map=town&drive";
    await send("Page.navigate", { url: town.toString() });
    const there = await until(["drive"]);
    step("the town", there);
    await sleep(10000);
    await picture("5-town");
  }
} finally {
  await send("Browser.close").catch(() => {});
  spawn("taskkill", ["/PID", String(edge.pid), "/T", "/F"], { stdio: "ignore" });
  await sleep(1500);
  try { rmSync(profile, { recursive: true, force: true }); } catch { /* still in use: the system clears it */ }
}
