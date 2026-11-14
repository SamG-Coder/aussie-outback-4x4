// The game's pages: shows what the game says there is to show, and tells the game what was
// done (docs/ui.md).
//
// The game calls `ui.receive(state)`: which screen there is, what the instruments read, and
// which way a menu was moved by the keys or a controller. The page says back, a line at a
// time: `key CODE 1|0` and `mouse X Y` for what it was given that is the game's to use,
// `do WHAT` for what was chosen in a menu, and `set WHAT WHICH` for a choice made.
//
// On the desktop the page is in a web view over the game's window, and says things through
// `window.ipc`. In a browser the game is a canvas under the page, and the page that holds
// both sets `window.uiHost = { say(line) }`. With neither, the page shows itself with made-up
// readings, to be worked on: `index.html?screen=drive`.
"use strict";

const el = (id) => document.getElementById(id);
const body = document.body;

const TIPS = [
  "Low range (4L) multiplies the engine's pull. Use it for climbs, bog holes and rock steps.",
  "Lock the diffs before the obstacle, not in the middle of it.",
  "Aired-down tyres spread out and float over soft ground. Air back up for the hard stuff.",
  "Hill descent control holds the truck back on steep drops. Stay off the brake and let it work.",
  "Walk it through water: a steady bow wave, not a splash.",
  "Momentum gets you through mud. Wheelspin digs you into it.",
  "Past forty degrees of lean a truck goes over. Watch the inclinometer on side slopes.",
];

let state = { screen: "boot" };
// A screen the page opens over the game's own, such as the controls.
let opened = null;
// Which button of the screen's menu is in hand: its row, and how far along the row.
let held = [0, 0];
// What has been chosen at a trailhead: which time of day, and which weather.
const picked = { time: 1, weather: 1 };

// ---- Saying things to the game. In a browser the page is in a frame over the game's, which
// sets `uiHost` once the game is running: what is said before then is not heard.
const framed = window.parent !== window;
const host = () => window.uiHost || (framed ? window.parent.uiHost : null);
const mock = !window.ipc && !host() && !framed;
function say(line) {
  if (window.ipc) window.ipc.postMessage(line);
  else if (host()) host().say(line);
  else if (mock) pretend(line);
}

// A mistake in the page is told to the game, which writes it down.
addEventListener("error", (event) => say(`log error ${event.message} at ${event.filename}:${event.lineno}`));

// ---- Menus. A screen's buttons are in rows: a list down the page is a row a button, and a
// `nav` marked `data-row` is one row of buttons side by side.
const screenShown = () => opened || state.screen;
function rows() {
  const section = el(screenShown());
  if (!section) return [];
  const all = [];
  for (const nav of section.querySelectorAll("nav")) {
    const buttons = [...nav.querySelectorAll("button")].filter((b) => b.offsetParent !== null);
    if (buttons.length === 0) continue;
    if (nav.dataset.row !== undefined) all.push(buttons);
    else for (const button of buttons) all.push([button]);
  }
  return all;
}
function hold(row, column) {
  const all = rows();
  for (const button of document.querySelectorAll("button.held")) button.classList.remove("held");
  if (all.length === 0) return;
  row = (row + all.length) % all.length;
  column = Math.max(0, Math.min(column, all[row].length - 1));
  held = [row, column];
  all[row][column].classList.add("held");
}
function holdButton(button) {
  for (const [row, buttons] of rows().entries()) {
    const column = buttons.indexOf(button);
    if (column >= 0) hold(row, column);
  }
}
function back() {
  if (opened) { opened = null; show(); return; }
  if (state.screen === "pause") say("do resume");
  if (state.screen === "offer") say("do leave");
}
// One of a row of choices, picked: the game is told, and the row shows it.
function pick(button) {
  const what = button.dataset.set, which = Number(button.dataset.value);
  picked[what] = which;
  say(`set ${what} ${which}`);
  for (const other of button.parentElement.children) other.classList.toggle("picked", other === button);
}
function choose(button) {
  if (button.dataset.set) pick(button);
  else if (button.dataset.open) { opened = button.dataset.open; show(); }
  else if (button.dataset.close) back();
  else if (button.dataset.do) say(`do ${button.dataset.do}`);
}
function navigate(way) {
  const all = rows();
  if (way === "back") return back();
  if (all.length === 0) return;
  const [row, column] = held;
  const here = all[row] && all[row][column];
  // Going to another row: on a row of choices, to the one that is picked.
  const go = (to) => {
    const set = all[(to + all.length) % all.length][0].dataset.set;
    hold(to, set ? picked[set] : column);
  };
  if (way === "accept") {
    // On a row of choices the one in hand is picked already: go on down.
    if (here && here.dataset.set) go(row + 1);
    else if (here) choose(here);
  } else if (way === "up") go(row - 1);
  else if (way === "down") go(row + 1);
  else {
    const along = all[row].length;
    if (along === 1) return hold(way === "left" ? row - 1 : row + 1, 0);
    hold(row, (column + (way === "left" ? along - 1 : 1)) % along);
    const now = all[row][held[1]];
    // Moving along a row of choices picks as it goes.
    if (now.dataset.set) pick(now);
  }
}
function wire(button) {
  button.addEventListener("mouseenter", () => holdButton(button));
  button.addEventListener("click", () => choose(button));
}
for (const button of document.querySelectorAll("nav button")) wire(button);
el("banner").addEventListener("click", () => say("do enter"));

// A row of choices, made from what the game says there is to choose from.
function chips(id, what, names, which) {
  const nav = el(id);
  if (nav.children.length !== names.length) {
    nav.replaceChildren();
    for (const [index, name] of names.entries()) {
      const button = document.createElement("button");
      button.textContent = name;
      button.dataset.set = what;
      button.dataset.value = String(index);
      wire(button);
      nav.append(button);
    }
  }
  picked[what] = which;
  for (const [index, button] of [...nav.children].entries()) button.classList.toggle("picked", index === which);
}

// A map's picture, if it has one: `art/NAME.jpg`. One with none is not shown.
function art(id, name) {
  const image = el(id);
  const wanted = name ? `art/${name}.jpg` : "";
  if (image.dataset.wanted === wanted) return;
  image.dataset.wanted = wanted;
  image.classList.add("none");
  if (!wanted) return;
  image.onload = () => image.classList.remove("none");
  image.src = wanted;
}

// ---- Showing things.
function lapTime(seconds) {
  if (seconds === null || seconds === undefined) return "-:--.---";
  const minutes = Math.floor(seconds / 60);
  return `${minutes}:${(seconds - minutes * 60).toFixed(3).padStart(6, "0")}`;
}
const setText = (id, text) => { const e = el(id); if (e.textContent !== text) e.textContent = text; };

// The screen there is now: its section shown, its words filled in, its first button in hand.
let shown = null;
function show() {
  const screen = screenShown();
  body.dataset.screen = screen;
  body.classList.toggle("pad", !!state.pad);
  for (const hint of document.querySelectorAll("[data-hint]")) {
    const accept = hint.dataset.hint === "accept";
    hint.dataset.key = state.pad ? (accept ? "✕" : "○") : (accept ? "Enter" : "Esc");
  }
  const map = state.map || {};
  setText("where-title", map.title || "");
  setText("where-mode", state.hub ? "Free roam" : state.mode === "laps" ? "Timed laps" : "Free roam");
  el("laps").hidden = state.mode !== "laps" || !state.lap;
  setText("pause-map", map.title || "");
  el("pause-hub").hidden = !!state.hub;
  if (state.offer) {
    const offer = state.offer;
    setText("offer-title", offer.title || "");
    setText("offer-about", offer.about || "");
    setText("offer-kind", offer.kind || "-");
    setText("offer-length", offer.length || "-");
    setText("offer-best", lapTime(offer.best));
    el("offer-built").hidden = !offer.built;
    el("offer-unbuilt").hidden = !!offer.built;
    art("offer-art", offer.built ? offer.name : "");
    if (screen === "offer" && shown !== "offer") {
      chips("offer-times", "time", state.times || [], state.time || 0);
      chips("offer-weathers", "weather", state.weathers || [], state.weather || 0);
    }
  }
  if (screen === "loading" && shown !== "loading") {
    setText("loading-title", map.title || "");
    // What it is, and what it is being driven in.
    const facts = [map.kind, map.length];
    if (!state.hub && state.times && state.weathers) facts.push(state.times[picked.time], state.weathers[picked.weather]);
    setText("loading-facts", facts.filter(Boolean).join("  ·  "));
    setText("loading-tip", TIPS[Math.floor(Math.random() * TIPS.length)]);
    art("loading-art", map.name);
  }
  if (screen !== shown) {
    shown = screen;
    if (screen !== "drive" && document.pointerLockElement) document.exitPointerLock();
    // A screen starts with its first button in hand, or the row marked as where to start.
    const all = rows();
    const first = all.findIndex((row) => row[0].parentElement.dataset.first !== undefined);
    hold(Math.max(first, 0), 0);
  } else {
    hold(held[0], held[1]);
  }
}

// The rev counter's bars.
const BARS = 28, RED = 4;
for (let i = 0; i < BARS; i++) {
  const bar = document.createElement("i");
  if (i >= BARS - RED) bar.className = "red";
  el("revs").append(bar);
}
const bars = [...el("revs").children];

function instruments() {
  const hud = state.hud;
  if (!hud) return;
  setText("speed", String(hud.speed));
  setText("gear", hud.gear);
  const lit = Math.round(Math.min(hud.rpm / Math.max(hud.limit, 1), 1) * BARS);
  for (const [i, bar] of bars.entries()) bar.classList.toggle("lit", i < lit);
  el("lamp-abs").classList.toggle("on", hud.abs);
  el("lamp-tc").classList.toggle("on", hud.traction);
  el("lamp-hdc").classList.toggle("on", hud.descent);
  el("driveline").hidden = !hud.four_wheel_drive;
  el("tilt").hidden = !hud.four_wheel_drive;
  for (const range of el("driveline").querySelectorAll("[data-range]")) {
    range.classList.toggle("on", range.dataset.range === hud.drive);
  }
  el("lock-front").classList.toggle("on", hud.front_locked);
  el("lock-rear").classList.toggle("on", hud.rear_locked);
  el("tyres").classList.toggle("on", hud.aired_down);
  // The inclinometer. Nose up tips the side view back; a lean to the right tips the rear view.
  const tip = (id, truck, degrees, steep) => {
    setText(id, `${degrees > 0 ? "+" : ""}${degrees}°`);
    el(truck).style.transform = `rotate(${id === "pitch" ? -degrees : degrees}deg)`;
    el(truck).closest("figure").classList.toggle("steep", Math.abs(degrees) >= steep);
  };
  tip("pitch", "pitch-truck", hud.pitch, 30);
  tip("roll", "roll-truck", hud.roll, 28);

  const lap = state.lap;
  if (lap) {
    setText("lap-count", String(lap.count + 1));
    setText("lap-time", lapTime(lap.time));
    el("lap-time").classList.toggle("off", !lap.valid);
    setText("lap-last", lapTime(lap.last));
    el("lap-last").classList.toggle("off", lap.last !== null && !lap.last_valid);
    setText("lap-best", lapTime(lap.best));
  }
  // The nearest trailhead: the way to it, or, in its ring, how to take it.
  const near = state.near;
  el("marker").hidden = !near || near.inside;
  el("banner").hidden = !near || !near.inside;
  if (near && !near.inside) {
    setText("marker-title", near.title);
    setText("marker-distance", near.distance >= 1000 ? `${(near.distance / 1000).toFixed(1)} km` : `${near.distance} m`);
    el("marker-arrow").style.transform = `rotate(${near.bearing}deg)`;
  }
  if (near && near.inside) {
    setText("banner-title", near.title);
    setText("banner-key", state.pad ? "✕" : "Enter");
    setText("banner-words", state.pad ? "Hold to enter" : "Hold to enter, or click here");
    el("banner-hold").style.width = `${Math.round((near.hold || 0) * 100)}%`;
  }
}

// What is being downloaded, along the top: `what` it is, how `much` of it has come, as words,
// what `share` of it that is from 0 to 1, and whether more is coming now. Nothing coming, the
// line is still, and after a few seconds it goes.
let streamGone = 0;
function stream(told) {
  const bar = el("stream");
  if (!told) { bar.hidden = true; return; }
  clearTimeout(streamGone);
  bar.hidden = false;
  bar.classList.toggle("busy", !!told.busy);
  setText("stream-what", told.what || "");
  setText("stream-much", told.much || "");
  // With no telling how much there is to come, the whole line, moving.
  const share = told.share || (told.busy ? 1 : 0);
  el("stream-bar").style.width = `${Math.round(Math.max(0, Math.min(share, 1)) * 1000) / 10}%`;
  if (!told.busy && told.fades !== false) streamGone = setTimeout(() => { bar.hidden = true; }, 4000);
}

window.ui = {
  receive(told) {
    const nav = told.nav || [];
    state = told;
    show();
    instruments();
    for (const way of nav) navigate(way);
  },
  stream,
};

// ---- What the page is given that is the game's: the keys, and the mouse while it has it.
addEventListener("keydown", (event) => {
  if (!event.repeat) say(`key ${event.code} 1`);
  event.preventDefault();
});
addEventListener("keyup", (event) => { say(`key ${event.code} 0`); event.preventDefault(); });
addEventListener("blur", () => say("blur"));
addEventListener("contextmenu", (event) => event.preventDefault());

// Driving, a click takes hold of the mouse, and the mouse looks round the truck. Escape lets
// go of it, which the page is not told of as a key, so letting go while driving pauses.
let moved = [0, 0];
addEventListener("mousedown", (event) => {
  if (state.screen === "drive" && !opened && !event.target.closest("button")) body.requestPointerLock();
});
addEventListener("mousemove", (event) => {
  if (document.pointerLockElement) { moved[0] += event.movementX; moved[1] += event.movementY; }
});
document.addEventListener("pointerlockchange", () => {
  if (!document.pointerLockElement && state.screen === "drive" && shown === "drive") say("do pause");
});
function passOn() {
  if (moved[0] !== 0 || moved[1] !== 0) {
    say(`mouse ${moved[0]} ${moved[1]}`);
    moved = [0, 0];
  }
  requestAnimationFrame(passOn);
}
requestAnimationFrame(passOn);

// ---- With no game: made-up readings, and menus that go where they would.
function pretend(line) {
  const [word, what, down] = line.split(" ");
  if (word === "key" && down === "1") {
    const way = { ArrowUp: "up", ArrowDown: "down", ArrowLeft: "left", ArrowRight: "right", Enter: "accept", Escape: "back" }[what];
    if (way === "back" && state.screen === "drive") window.ui.receive({ ...state, screen: "pause" });
    else if (way) navigate(way);
  }
  if (word !== "do") return;
  const to = { start: "drive", resume: "drive", pause: "pause", title: "title", leave: "drive", enter: "offer", roam: "loading", laps: "loading", hub: "loading", restart: "loading" }[what];
  if (!to) return;
  const next = { ...state, screen: to, nav: [] };
  if (what === "laps") next.mode = "laps";
  if (what === "roam" || what === "hub") next.mode = "roam";
  if (what === "laps" || what === "roam") { next.map = state.offer; next.hub = false; }
  window.ui.receive(next);
  if (to === "loading" && !new URLSearchParams(location.search).has("still")) {
    setTimeout(() => window.ui.receive({ ...next, screen: "drive" }), 4000);
  }
}
if (mock) {
  body.classList.add("mock");
  const wanted = new URLSearchParams(location.search);
  if (wanted.has("still")) body.classList.add("still");
  if (wanted.has("stream")) stream({ what: "Streaming map", much: "38%", share: 0.38, busy: true });
  const wombat = { name: "wombat", title: "Wombat Creek Track", about: "A washed-out fire trail through stringybark and manna gum: two creek fords, bog holes, rock steps and ruts cut by a wet winter.", kind: "Fire trail", length: "3.5 km", best: 312.482, built: true };
  window.ui.receive({
    screen: wanted.get("screen") || "title", mode: wanted.get("mode") || "roam", pad: wanted.has("pad"), nav: [],
    hub: !wanted.has("map"), map: wanted.has("map") ? wombat : { name: "crossroads", title: "The Crossroads" },
    offer: wanted.has("unbuilt") ? { name: "granite", title: "Granite Ridge", built: false } : wombat,
    times: ["Dawn", "Day", "Dusk", "Night"], weathers: ["Clear", "Fair", "Overcast", "Storm"], time: 1, weather: 1,
    hud: { speed: 37, gear: "2", rpm: 3100, limit: 6800, abs: false, traction: true, four_wheel_drive: true, drive: "4L",
      front_locked: false, rear_locked: true, descent: false, aired_down: true, pitch: 12, roll: -31 },
    lap: { count: 1, time: 83.412, last: 318.07, best: 312.482, valid: true, last_valid: true },
    near: { title: "Wombat Creek Track", distance: 240, bearing: 35, inside: wanted.has("inside"), hold: 0.6 },
  });
}

say("hello");
