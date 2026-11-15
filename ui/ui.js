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

// (A button a tip speaks of is drawn as it is on what is in hand: `{ACTIONS}`, as below. One
// that speaks of a button not yet known is left out.)
const TIPS = [
  "Low range (4L) multiplies the engine's pull. Use it for climbs, bog holes and rock steps.",
  "Lock the diffs before the obstacle, not in the middle of it.",
  "Aired-down tyres spread out and float over soft ground. Air back up for the hard stuff.",
  "Hill descent control holds the truck back on steep drops. Stay off the brake and let it work.",
  "Walk it through water: a steady bow wave, not a splash.",
  "Momentum gets you through mud. Wheelspin digs you into it.",
  "Past forty degrees of lean a truck goes over. Watch the inclinometer on side slopes.",
  "{drive_mode} moves the transfer case on: two-wheel drive, four high, four low.",
  "{diff_lock} locks the rear diff, then both, then neither.",
  "{tyres} lets the tyres down for mud and sand, and pumps them up again for hard ground.",
  "Off the track, or on your roof? {reset} puts the truck back on it.",
  "Stopped, hold {enter~KeyE} to get out and walk. Hold it again beside the truck to get back in.",
];

let state = { screen: "boot" };
// A screen the page opens over the game's own, such as the controls.
let opened = null;
// Which button of the screen's menu is in hand: its row, and how far along the row.
let held = [0, 0];
// What has been chosen at a trailhead: which time of day, which weather, and which class.
const picked = { time: 1, weather: 1, class: 0 };

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

// ---- Buttons (docs/ui.md, Buttons). Whatever is pressed for something is drawn as it is on
// what is in hand: a key's cap or the mouse, an Xbox controller's buttons, or a PlayStation's.
// The game says which is in hand (`pad`, `pad_kind`) and what is bound to what (`binds`: an
// action's controls on the keys, the mouse and a controller), and nothing here names a key of
// its own: an element marked `data-glyph="ACTIONS"` is filled with the buttons for them, one
// marked `data-hint` has them put before its words, and `data-words` is words with
// `{ACTIONS}` among them. They are all drawn again the moment another device is in hand.
//
// ACTIONS is terms with spaces between: `throttle` is the action's first control (for one
// that goes two ways on keys, such as `steer`, the key each way); `forward+` and `forward-`
// are one way of it; `run|sprint` is the first of them that is bound on this device;
// `enter~KeyE` is that control if it is one of the action's, else its first; `hold:enter`
// has the word Hold before it; `@mouse:left` is a control itself, bound or not.
let binds = {}, bindsTold = 0;
// Whose controller it is, `xbox` or `playstation`, and whether one was in hand when the page
// was last told: kept from one page to the next, since a browser reads a page for each map.
let padKind = "xbox", padHeld = false;
const HANDS = "aussie-outback-4x4.hands";
try {
  const kept = JSON.parse(localStorage.getItem(HANDS) || "null");
  if (kept) { binds = kept.binds || {}; padKind = kept.kind || "xbox"; padHeld = !!kept.pad; }
} catch { /* Nothing kept, or nowhere to keep it. */ }
let handsKept = "";
function keepHands() {
  const kept = JSON.stringify({ binds, kind: padKind, pad: padHeld });
  if (kept === handsKept) return;
  handsKept = kept;
  try { localStorage.setItem(HANDS, kept); } catch { /* Nowhere to keep it. */ }
}
// What is in hand: `keys`, the keyboard and mouse, or a controller's family.
const kitNow = () => (state.pad ? padKind : "keys");

// The drawings. Each is 32 high, and as wide as it needs; its size on the page is `--glyph`.
const gSvg = (width, name, inner) => `<svg class="g" viewBox="0 0 ${width} 32" role="img" aria-label="${name}"><title>${name}</title>${inner}</svg>`;
const gText = (x, y, size, text, more = "") => `<text class="g-t" x="${x}" y="${y}" font-size="${size}"${more}>${text}</text>`;
const G_DISC = '<circle class="g-b" cx="16" cy="16" r="13.5"/>';
// The four buttons under the right thumb. An Xbox's are letters, a PlayStation's are shapes,
// each in its own colour.
const G_FACE = {
  xbox: { south: ["A", "#6cc24a"], east: ["B", "#ea5a44"], west: ["X", "#4a9fee"], north: ["Y", "#f2b632"] },
  playstation: {
    south: ["Cross", "#86aef5", '<path d="M10.4 10.4 L21.6 21.6 M21.6 10.4 L10.4 21.6"/>'],
    east: ["Circle", "#f2756e", '<circle cx="16" cy="16" r="6.6"/>'],
    west: ["Square", "#e99fd3", '<rect x="10.2" y="10.2" width="11.6" height="11.6" rx="0.8"/>'],
    north: ["Triangle", "#55d3a4", '<path d="M16 8.8 L23.2 21.2 H8.8 Z"/>'],
  },
};
function gFace(kit, which) {
  const [name, colour, shape] = G_FACE[kit][which];
  if (shape) return gSvg(32, name, `${G_DISC}<g class="g-s" style="stroke:${colour}">${shape}</g>`);
  return gSvg(32, name, `<circle class="g-b" cx="16" cy="16" r="13.5" style="stroke:${colour}"/>${gText(16, 16.8, 17, name, ` style="fill:${colour}"`)}`);
}
// The bumpers, wide and flat with the outer corner rounded off, and the triggers, tall.
const G_SHOULDER = { xbox: { l1: "LB", r1: "RB", l2: "LT", r2: "RT" }, playstation: { l1: "L1", r1: "R1", l2: "L2", r2: "R2" } };
function gShoulder(kit, which) {
  const name = G_SHOULDER[kit][which];
  if (which.endsWith("2")) return gSvg(34, name, `<path class="g-b" d="M8 29.5 L6 14.5 A11 11.5 0 0 1 28 14.5 L26 29.5 Z"/>${gText(17, 18.6, 11.5, name)}`);
  const turned = which === "r1" ? ' transform="matrix(-1 0 0 1 44 0)"' : "";
  return gSvg(44, name, `<path class="g-b" d="M4.5 24.5 V15.5 A8 8 0 0 1 12.5 7.5 H36 A3.5 3.5 0 0 1 39.5 11 V24.5 Z"${turned}/>${gText(22, 16.6, 11.5, name)}`);
}
// A stick: its ring, and its cap with which it is. Pushed along one line, arrows that way;
// pressed in, the cap is lit, with an arrow down on to it.
const G_WAYS = { left: "M4.2 16 L7.3 13.1 V18.9 Z", right: "M27.8 16 L24.7 13.1 V18.9 Z", up: "M16 4.2 L13.1 7.3 H18.9 Z", down: "M16 27.8 L13.1 24.7 H18.9 Z" };
function gStick(kit, side, how) {
  const letter = side === "left" ? "L" : "R", pressed = how === "press";
  const label = pressed && kit === "playstation" ? `${letter}3` : letter;
  const name = `${side === "left" ? "Left" : "Right"} stick${pressed ? ", pressed" : ""}`;
  const arrows = how === "x" ? [G_WAYS.left, G_WAYS.right] : how === "y" ? [G_WAYS.up, G_WAYS.down] : pressed ? ["M16 7.6 L13.1 4.5 H18.9 Z"] : [];
  return gSvg(32, name, `${G_DISC}<circle class="${pressed ? "g-f" : "g-c"}" cx="16" cy="16" r="8"/>`
    + gText(16, 16.5, label.length > 1 ? 8.6 : 10.5, label, pressed ? ' style="fill:var(--g-ink)"' : "")
    + arrows.map((arrow) => `<path class="g-f" d="${arrow}"/>`).join(""));
}
// The d-pad, with the arms that are meant lit.
const G_ARMS = { up: "M12.9 4.2 H19.1 V10.9 L16 14 L12.9 10.9 Z", down: "M12.9 27.8 H19.1 V21.1 L16 18 L12.9 21.1 Z",
  left: "M4.2 12.9 V19.1 H10.9 L14 16 L10.9 12.9 Z", right: "M27.8 12.9 V19.1 H21.1 L18 16 L21.1 12.9 Z" };
const gDpad = (ways) => gSvg(32, `D-pad ${ways.join(" and ")}`,
  '<path class="g-b" d="M11.5 2.5 H20.5 V11.5 H29.5 V20.5 H20.5 V29.5 H11.5 V20.5 H2.5 V11.5 H11.5 Z"/>'
  + ways.map((way) => `<path class="g-f" d="${G_ARMS[way]}"/>`).join(""));
// The two small buttons in the middle: View and Menu on an Xbox, Create and Options on a
// PlayStation, each with the mark it has on the controller.
const G_LINES = '<path class="g-s" d="M10.5 11.5 H21.5 M10.5 16 H21.5 M10.5 20.5 H21.5"/>';
const G_MIDDLE = {
  xbox: { select: ["View", '<rect class="g-s" x="9.5" y="9.5" width="9.5" height="8" rx="1"/><rect class="g-s" x="13" y="14.5" width="9.5" height="8" rx="1" style="fill:var(--g-ink)"/>'],
    start: ["Menu", G_LINES] },
  playstation: { select: ["Create", '<rect class="g-s" x="13" y="13.5" width="6" height="10.5" rx="3"/><path class="g-s" d="M16 6.8 V10 M10.6 8.6 L12.4 11.2 M21.4 8.6 L19.6 11.2"/>'],
    start: ["Options", G_LINES] },
};
const gMiddle = (kit, which) => gSvg(32, G_MIDDLE[kit][which][0], G_DISC + G_MIDDLE[kit][which][1]);
// The mouse: a button of it lit, or arrows either side of it for moving it.
function gMouse(which) {
  const moved = which === "move", at = moved ? 9 : 0;
  const lit = { left: `<path class="g-f" d="M${at + 10.1} 3.5 V12.1 H${at + 4} V10.4 A7 7 0 0 1 ${at + 10.1} 3.5 Z"/>`,
    right: `<path class="g-f" d="M${at + 11.9} 3.5 V12.1 H${at + 18} V10.4 A7 7 0 0 0 ${at + 11.9} 3.5 Z"/>`,
    middle: `<rect class="g-f" x="${at + 9.2}" y="4.8" width="3.6" height="7" rx="1.8" style="stroke:var(--g-ink);stroke-width:1.2"/>`,
    move: '<path class="g-s" d="M6 11.6 L2 16 L6 20.4 M34 11.6 L38 16 L34 20.4"/>' }[which];
  const name = moved ? "Move the mouse" : `${which[0].toUpperCase()}${which.slice(1)} mouse button`;
  return gSvg(moved ? 40 : 22, name, `<rect class="g-b" x="${at + 3}" y="2.5" width="16" height="27" rx="8"/>`
    + `<path class="g-l" d="M${at + 11} 2.5 V13 M${at + 3} 13 H${at + 19}"/>${lit}`);
}
// A key: its cap, with what is on it. The arrows are arrows.
const G_KEYS = { Space: "Space", Enter: "Enter", NumpadEnter: "Enter", Escape: "Esc", Backspace: "Bksp", Tab: "Tab", CapsLock: "Caps",
  ShiftLeft: "Shift", ShiftRight: "Shift", ControlLeft: "Ctrl", ControlRight: "Ctrl", AltLeft: "Alt", AltRight: "Alt",
  Delete: "Del", Insert: "Ins", Home: "Home", End: "End", PageUp: "PgUp", PageDown: "PgDn", Minus: "-", Equal: "=",
  BracketLeft: "[", BracketRight: "]", Semicolon: ";", Quote: "'", Comma: ",", Period: ".", Slash: "/", Backslash: "\\", Backquote: "`" };
const G_ARROWS = { ArrowUp: 0, ArrowRight: 90, ArrowDown: 180, ArrowLeft: 270 };
function gKey(code) {
  if (code in G_ARROWS) {
    const way = code.slice(5);
    return `<kbd class="g-key" title="${way} arrow"><svg viewBox="0 0 16 16" role="img" aria-label="${way} arrow"><path d="M8 3.2 L13.4 11.8 H2.6 Z" transform="rotate(${G_ARROWS[code]} 8 8)"/></svg></kbd>`;
  }
  const label = G_KEYS[code] || code.replace(/^(Key|Digit)/, "").replace(/^Numpad/, "Num ");
  return `<kbd class="g-key">${label.replace("&", "&amp;").replace("<", "&lt;")}</kbd>`;
}
// One control drawn: `key:KeyW`, `mouse:left`, `pad:south`, and what `gather` makes of a
// stick's axes, the d-pad's arms and the mouse's two ways.
function gDraw(control, kit) {
  const [device, what, how] = control.split(":");
  if (device === "key") return gKey(what);
  if (device === "mouse") return gMouse(what);
  if (device === "dpad") return gDpad(what.split("+"));
  if (device === "stick") return gStick(kit, what, how || "");
  if (what in G_FACE[kit]) return gFace(kit, what);
  if (what in G_SHOULDER[kit]) return gShoulder(kit, what);
  if (what in G_MIDDLE[kit]) return gMiddle(kit, what);
  if (what === "l3" || what === "r3") return gStick(kit, what === "l3" ? "left" : "right", "press");
  return `<kbd class="g-key">${what}</kbd>`;
}
// A stick's axis and the mouse's movement go both ways whatever way they are bound; a key or
// a button bound backwards (`-KeyA`) is the other way of its action.
const G_AXES = ["left_x", "left_y", "right_x", "right_y", "x", "y"];
// The controls a term stands for, on a device: `[device, control]` each.
function gTerm(term, kit) {
  if (term.startsWith("@")) {
    const [device, control] = term.slice(1).split(":");
    return (device === "pad") === (kit !== "keys") ? [[device, control]] : [];
  }
  for (const each of term.split("|")) {
    const [named, prefer] = each.split("~");
    const way = /[+-]$/.test(named) ? named.slice(-1) : "", bound = binds[way ? named.slice(0, -1) : named] || {};
    const all = kit === "keys" ? [...(bound.key || []).map((c) => ["key", c]), ...(bound.mouse || []).map((c) => ["mouse", c])]
      : (bound.pad || []).map((c) => ["pad", c]);
    if (all.length === 0) continue;
    const bare = ([device, control]) => [device, control.replace(/^-/, "")];
    const axis = all.find(([, control]) => G_AXES.includes(control.replace(/^-/, "")));
    const on = all.find(([, control]) => !control.startsWith("-") && !G_AXES.includes(control));
    const back = all.find(([, control]) => control.startsWith("-") && !G_AXES.includes(control.slice(1)));
    const wanted = prefer && all.find(([, control]) => control.replace(/^-/, "") === prefer);
    if (wanted) return [bare(wanted)];
    if (way) return [bare((way === "+" ? on : back) || axis || all[0])];
    // (What is bound first is what is shown: a stick, or else a button, both ways if it has two.)
    if (axis === all[0] || !on) return [bare(all[0])];
    return back ? [bare(back), bare(on)] : [bare(on)];
  }
  return [];
}
// The controls of a hint, as they are drawn: none twice, a stick's two axes as the stick,
// the d-pad's arms as one d-pad, and the mouse's two ways as the mouse.
function gather(controls) {
  const drawn = [], has = (device, control) => controls.some(([d, c]) => d === device && c === control);
  for (const [device, control] of controls) {
    let made = `${device}:${control}`;
    const stick = device === "pad" && /^(left|right)_([xy])$/.exec(control);
    if (stick) made = `stick:${stick[1]}${has("pad", `${stick[1]}_x`) && has("pad", `${stick[1]}_y`) ? "" : `:${stick[2]}`}`;
    else if (device === "pad" && control in G_ARMS) made = `dpad:${Object.keys(G_ARMS).filter((way) => has("pad", way)).join("+")}`;
    else if (device === "mouse" && (control === "x" || control === "y")) made = "mouse:move";
    if (!drawn.includes(made)) drawn.push(made);
  }
  return drawn;
}
// The buttons for ACTIONS, drawn: or nothing, if nothing on this device does any of them.
function glyphs(spec, kit = kitNow()) {
  const controls = [];
  let held = false;
  for (const term of spec.split(/\s+/).filter(Boolean)) {
    const hold = term.startsWith("hold:");
    const found = gTerm(hold ? term.slice(5) : term, kit);
    held = held || (hold && found.length > 0);
    controls.push(...found);
  }
  const drawn = gather(controls).map((control) => gDraw(control, kit)).join("");
  return drawn && held ? `<span class="g-word">Hold</span>${drawn}` : drawn;
}
// Words with buttons among them: `Hold {enter} to stop here`. Nothing at all, if a button
// they speak of is not bound on this device.
function glyphWords(words, kit = kitNow()) {
  let whole = true;
  const html = words.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/\{([^}]+)\}/g, (_, spec) => {
    const drawn = glyphs(spec, kit);
    if (!drawn) whole = false;
    return `<span class="glyphs">${drawn}</span>`;
  });
  return whole ? html : null;
}
// What a `data-hint` is short for.
const HINTS = { accept: "menu_accept", back: "menu_back", tabs: "menu_prev menu_next", browse: "menu_left menu_right" };
// Every button on the page drawn as it is on what is in hand now: only those that are not
// already, so this is called whenever anything is shown.
function dress(within = document) {
  const kit = kitNow(), stamp = `${kit} ${bindsTold} `;
  body.dataset.kit = kit;
  for (const hint of within.querySelectorAll("[data-hint]")) {
    if (hint.firstElementChild && hint.firstElementChild.classList.contains("glyphs")) continue;
    const holder = document.createElement("span");
    holder.className = "glyphs";
    holder.dataset.glyph = HINTS[hint.dataset.hint] || hint.dataset.hint;
    hint.prepend(holder);
  }
  for (const holder of within.querySelectorAll("[data-glyph]")) {
    // (One that is always a device's own, whatever is in hand, says so: `data-kit`.)
    const mine = holder.dataset.kit === "pad" ? padKind : holder.dataset.kit || kit;
    const wanted = `${stamp}${mine} ${holder.dataset.glyph}`;
    if (holder.dataset.drawn === wanted) continue;
    holder.dataset.drawn = wanted;
    holder.innerHTML = glyphs(holder.dataset.glyph, mine);
    // A hint with no button on this device is not a hint.
    holder.classList.toggle("unbound", holder.innerHTML === "");
    if (holder.parentElement.dataset.hint !== undefined) holder.parentElement.classList.toggle("unbound", holder.innerHTML === "");
  }
  for (const text of within.querySelectorAll("[data-words]")) {
    const wanted = `${stamp}${text.dataset.words}`;
    if (text.dataset.drawn === wanted) continue;
    text.dataset.drawn = wanted;
    text.innerHTML = glyphWords(text.dataset.words, kit) ?? text.dataset.words.replace(/\{[^}]+\}/g, "");
  }
}

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
  yardHeld(all[row][column]);
}
function holdButton(button) {
  for (const [row, buttons] of rows().entries()) {
    const column = buttons.indexOf(button);
    if (column >= 0) hold(row, column);
  }
}
function back() {
  if (opened) { opened = null; saveSure = ""; show(); return; }
  // (A bill is turned down. Out of a section of the auto shop is back to its sections,
  // not out of the shop.)
  if (state.screen === "yard" && state.yard && state.yard.quote) return say("do reject");
  if (state.screen === "yard" && shopOpen) return shopLeave();
  if (state.screen === "pause") say("do resume");
  if (state.screen === "offer" || state.screen === "shed" || state.screen === "buyer" || state.screen === "yard") say("do leave");
}
// One of a row of choices, picked: the game is told, and the row shows it.
function pick(button) {
  const what = button.dataset.set, which = Number(button.dataset.value);
  picked[what] = which;
  say(`set ${what} ${which}`);
  for (const other of button.parentElement.children) other.classList.toggle("picked", other === button);
  if (what === "class") offerClass();
}
function choose(button) {
  if (button.dataset.set) pick(button);
  else if (button.dataset.tab) { optionsTab = button.dataset.tab; show(); holdButton(button); }
  else if (button.dataset.open) { opened = button.dataset.open; show(); }
  else if (button.dataset.close) back();
  else if (button.dataset.do) say(`do ${button.dataset.do}`);
  else if (button.ask) button.ask();
}
function navigate(way) {
  const all = rows();
  if (way === "back") return back();
  if (way === "prev" || way === "next") return turnTab(way === "next" ? 1 : -1);
  if (all.length === 0) return;
  const [row, column] = held;
  const here = all[row] && all[row][column];
  // Going to another row: on a row of choices, to the one that is picked.
  const go = (to) => {
    const there = all[(to + all.length) % all.length];
    const set = there[0].dataset.set;
    // (On a row that is set as it is moved along, to the one that is set.)
    const live = there[0].dataset.live !== undefined ? there.findIndex((b) => b.classList.contains("picked")) : -1;
    hold(to, set ? picked[set] : live >= 0 ? live : column);
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
    else if (now.dataset.live !== undefined) choose(now);
  }
}
// A screen with tabs: turned to the next of them or the one before, wherever the hand is.
function turnTab(by) {
  const section = el(screenShown());
  const tabs = section ? [...section.querySelectorAll("nav.tabs button")] : [];
  if (tabs.length === 0) return;
  const at = tabs.findIndex((tab) => tab.classList.contains("picked"));
  choose(tabs[(at + by + tabs.length) % tabs.length]);
}
function wire(button) {
  button.addEventListener("mouseenter", () => holdButton(button));
  button.addEventListener("click", () => choose(button));
}
for (const button of document.querySelectorAll("nav button")) wire(button);
el("banner").addEventListener("click", () => say("do enter"));

// A row of choices, made from what the game says there is to choose from.
function chips(id, what, names, which, dress) {
  const nav = el(id);
  if (nav.children.length !== names.length) {
    nav.replaceChildren();
    for (const [index, name] of names.entries()) {
      const button = document.createElement("button");
      button.textContent = name;
      button.dataset.set = what;
      button.dataset.value = String(index);
      if (dress) dress(button, index);
      wire(button);
      nav.append(button);
    }
  }
  picked[what] = which;
  for (const [index, button] of [...nav.children].entries()) button.classList.toggle("picked", index === which);
}
// What each of a row of choices adds to a purse, written on it, and which of them is today's.
function tagChips(id, bonus, today) {
  for (const [index, button] of [...el(id).children].entries()) {
    let tag = button.querySelector("small");
    const extra = bonus ? bonus[index] : 0;
    if (extra && !tag) { tag = document.createElement("small"); button.append(tag); }
    if (tag && !extra) tag.remove();
    else if (tag) tag.textContent = `+${extra}%`;
    button.classList.toggle("today", index === today);
  }
}

// ---- Timed runs: medals, classes, and money.
const MEDALS = ["bronze", "silver", "gold"];
const MEDAL_NAMES = { bronze: "Bronze", silver: "Silver", gold: "Gold" };
const SKY_NAMES = { clear: "Clear", fair: "Fair", overcast: "Overcast", storm: "Storm" };
// How good a medal is: none is below bronze.
const rank = (medal) => MEDALS.indexOf(medal);
const money = (amount) => `$${Math.round(amount || 0).toLocaleString("en-AU")}`;
// A medal's time, to the tenth it is set to.
function medalTime(seconds) {
  if (seconds === null || seconds === undefined) return "-:--.-";
  const tenths = Math.round(seconds * 10), minutes = Math.floor(tenths / 600);
  return `${minutes}:${((tenths - minutes * 600) / 10).toFixed(1).padStart(4, "0")}`;
}
const gapWords = (gap) => `${gap > 0 ? "+" : "−"}${Math.abs(gap).toFixed(1)}`;
// A class among a row of choices: its truck, and its number.
function dressClass(button, index) {
  button.insertAdjacentHTML("afterbegin", `<svg aria-hidden="true"><use href="#class-${index + 1}"/></svg>`);
}
// At a trailhead, the class chosen: what it is called, its three times, and what each pays
// or that it has been won.
function offerClass() {
  const classes = state.offer && state.offer.timed !== false ? state.classes : null;
  el("offer-timed").hidden = !classes;
  if (!classes) return;
  const chosen = classes[state.class_fixed ? state.class : picked.class] || classes[0];
  setText("offer-class", state.class_fixed ? `Class ${chosen.class + 1}  ·  ${chosen.name}` : chosen.name);
  for (const box of el("offer-medals").querySelectorAll("[data-medal]")) {
    const index = rank(box.dataset.medal), won = rank(chosen.held) >= index;
    box.querySelector("b").textContent = medalTime(chosen.medals && chosen.medals[index]);
    box.classList.toggle("won", won);
    box.querySelector("span").textContent = won ? "Won" : chosen.purses ? money(chosen.purses[index]) : MEDAL_NAMES[box.dataset.medal];
  }
  setText("offer-medals-note", chosen.medals ? "" : "Its times are set the first time you drive out there.");
}
// The calendar: a square a day, and what is due next.
function calendar() {
  const told = state.calendar;
  if (!told) return;
  const grid = el("calendar-days"), wanted = JSON.stringify(told);
  if (grid.dataset.wanted === wanted) return;
  grid.dataset.wanted = wanted;
  setText("calendar-day", String(told.day));
  setText("calendar-title", told.months);
  const labels = { today: "Today", forecast: "Forecast", outlook: "Outlook" };
  grid.replaceChildren(...told.days.map((day) => {
    const square = document.createElement("div");
    square.className = `${day.when} ${day.sky || ""}`;
    square.innerHTML = `<b>${day.date}${day.month ? `<small>${day.month}</small>` : ""}</b>`
      + (day.sky ? `<svg aria-hidden="true"><use href="#sky-${day.sky}"/></svg>` : "")
      + (labels[day.when] ? `<span class="when">${labels[day.when]}</span>` : "")
      + (day.due ? `<p class="due"><span>Due</span>${money(day.due)}</p>` : "");
    return square;
  }));
  setText("calendar-today", `${told.weekday} ${told.date} ${told.month}`);
  el("calendar-sky-icon").setAttribute("href", `#sky-${told.sky}`);
  el("calendar-sky").querySelector("span").textContent = SKY_NAMES[told.sky] || "";
  const next = told.next;
  setText("calendar-next", `${next.weekday} ${next.date} ${next.month}`);
  setText("calendar-in", next.in === 0 ? "Today" : next.in === 1 ? "Tomorrow" : `In ${next.in} days`);
  // What the bank takes, and what is owed it in all; or that nothing is.
  const dues = next.total ? [...next.items, { what: "Still owed", dollars: next.owed, total: true }] : [{ what: "Nothing is owed", dollars: 0, total: true }];
  el("calendar-dues").replaceChildren(...dues.map((item) => {
    const line = document.createElement("li");
    if (item.total) line.className = "total";
    line.innerHTML = `<span>${item.what}</span><b>${next.total ? money(item.dollars) : ""}</b>`;
    return line;
  }));
}
// A timed run or practice: the line-up, another driver's run being watched, and the result.
function timed() {
  const run = state.run, map = state.map || {};
  const counts = state.mode === "timed";
  if (run) {
    el("lineup-icon").setAttribute("href", `#class-${run.class + 1}`);
    setText("lineup-kicker", `${counts ? "Timed run" : "Practice"}  ·  Class ${run.class + 1}  ·  ${run.name}`);
    setText("lineup-title", map.title || "");
    setText("lineup-sky", [run.hour, run.weather, counts && run.bonus ? `purses +${run.bonus}%` : null].filter(Boolean).join("  ·  "));
    setText("lineup-about", counts
      ? "One lap from a rolling start: the clock starts as you cross the line. Beat a time for its medal and its purse, paid in gold into your tray. The track is driven through its gates in turn; being put back on it puts you at the start of the section you are in, and adds 10 seconds."
      : "Against the clock, lap after lap, with the medal times to measure yourself by. Nothing is paid, and nothing is lost.");
    setText("lineup-go", counts ? "One lap. It counts." : "Lap after lap. Nothing at stake.");
    for (const row of el("lineup").querySelectorAll(".rivals button")) {
      const index = rank(row.dataset.medal), rival = run.rivals[index], won = rank(run.held) >= index;
      row.querySelector("i").style.background = rival.paint;
      row.querySelector("b").textContent = rival.name;
      row.querySelector("small").textContent = `${rival.truck}  ·  ${MEDAL_NAMES[rival.medal]}`;
      row.querySelector("em").textContent = medalTime(rival.time);
      row.classList.toggle("won", won);
      row.querySelector("strong").textContent = !counts ? "" : won ? "Won" : money(run.purses[index] * (1 + run.bonus / 100));
    }
  }
  const watch = state.watch;
  if (watch) {
    document.querySelector("#watch .watching").className = `watching ${watch.medal}`;
    setText("watch-kicker", `Watching  ·  ${MEDAL_NAMES[watch.medal]} run  ·  ${watch.shot} of ${watch.shots}`);
    setText("watch-name", watch.name);
    setText("watch-truck", `${watch.truck}  ·  Class ${watch.class + 1}`);
    setText("watch-clock", medalTime(watch.clock));
    setText("watch-time", `of ${medalTime(watch.time)}`);
  }
  const result = state.result;
  if (result) {
    el("result-icon").setAttribute("href", `#class-${result.class + 1}`);
    setText("result-kicker", `Timed run  ·  Class ${result.class + 1}${run ? `  ·  ${run.name}` : ""}  ·  ${map.title || ""}`);
    el("result-outcome").className = `outcome ${result.medal || ""}`;
    setText("result-time", lapTime(result.time));
    setText("result-words", !result.valid ? "No time  ·  the truck was put back on the track"
      : [result.medal ? MEDAL_NAMES[result.medal] : "No medal", result.record ? "your best yet" : null,
        result.resets ? `put back ${result.resets === 1 ? "once" : `${result.resets} times`}: +${Math.round(result.penalty)} s` : null].filter(Boolean).join("  ·  "));
    for (const box of el("result-medals").querySelectorAll("[data-medal]")) {
      const index = rank(box.dataset.medal), gap = result.time - result.medals[index];
      box.querySelector("dd").textContent = medalTime(result.medals[index]);
      box.querySelector("small").textContent = gapWords(gap);
      box.classList.toggle("won", result.valid && gap <= 0);
    }
    setText("result-best", lapTime(result.valid && result.record ? result.time : result.best));
    const purse = el("result-purse"), wanted = JSON.stringify(result);
    if (purse.dataset.wanted !== wanted) {
      purse.dataset.wanted = wanted;
      const lines = result.firsts.map((first) => [`${MEDAL_NAMES[first.medal]}  ·  first win`, money(first.purse)]);
      if (result.repeat) lines.push([`${MEDAL_NAMES[result.medal]} again  ·  a tenth of its purse`, money(result.repeat)]);
      if (result.night) lines.push(["At night  ·  half as much again", `+${money(result.night)}`]);
      if (result.rain) lines.push(["In a storm  ·  half as much again", `+${money(result.rain)}`]);
      if (result.paid) lines.push(["Paid", money(result.paid), "total"]);
      purse.replaceChildren(...lines.map(([what, much, kind]) => {
        const line = document.createElement("tr");
        if (kind) line.className = kind;
        line.innerHTML = `<td>${what}</td><td>${much}</td>`;
        return line;
      }));
    }
    setText("result-paid", !result.valid ? "Being put back on the track costs a run its medal. The line is where you left it."
      : !result.paid ? "Outside the bronze time: nothing is paid. The others' runs can be watched from the line."
      : result.grams ? `${result.grams} g of gold, weighed out at the buyer's price, is in your tray. It is yours when it is sold: get it to town.`
      : `${money(result.paid)} is in the bank.`);
  }
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

// ---- The map: whole, on its own screen, and small, in the corner while driving.
// How much ground the small one shows across, in metres.
const MINIMAP_ACROSS = 260;
function chart() {
  const drawn = state.chart, at = state.at;
  const driving = state.screen === "drive" && !opened;
  el("minimap").hidden = !(drawn && at && driving);
  el("pause-chart").hidden = !drawn;
  if (!drawn) return;
  // Where a place on the ground is on the picture, from 0 to 1 across and down. North is up.
  const where = (x, z) => [(x + drawn.half) / (2 * drawn.half), (z + drawn.half) / (2 * drawn.half)];
  const image = el("chart-img");
  if (image.dataset.wanted !== drawn.file) {
    image.dataset.wanted = drawn.file;
    image.src = drawn.file;
    el("minimap-chart").style.backgroundImage = `url("${drawn.file}")`;
  }
  if (at && driving) {
    const box = el("minimap").clientWidth, [u, v] = where(at.x, at.z);
    const size = box * (2 * drawn.half) / MINIMAP_ACROSS;
    const small = el("minimap-chart").style;
    small.backgroundSize = `${size}px ${size}px`;
    small.backgroundPosition = `${box / 2 - u * size}px ${box / 2 - v * size}px`;
    el("minimap-arrow").style.transform = `rotate(${at.heading}deg)`;
  }
  // The others, against the clock: a dot each where they would be by now. One off the edge
  // of the little map is held at its edge, the way it lies.
  const ghosts = (state.pace && state.pace.ghosts) || [];
  for (const dot of el("minimap").querySelectorAll(".ghost")) {
    const ghost = ghosts.find((other) => other.medal === dot.dataset.medal);
    dot.hidden = !(ghost && at && driving) || ghost.done;
    if (dot.hidden) continue;
    const box = el("minimap").clientWidth, edge = box / 2 - 7;
    let across = (ghost.x - at.x) / MINIMAP_ACROSS * box, down = (ghost.z - at.z) / MINIMAP_ACROSS * box;
    const reach = Math.max(Math.abs(across), Math.abs(down));
    if (reach > edge) { across *= edge / reach; down *= edge / reach; }
    dot.classList.toggle("far", reach > edge);
    dot.style.left = `${box / 2 + across}px`;
    dot.style.top = `${box / 2 + down}px`;
  }
  if (screenShown() !== "map") return;
  setText("chart-title", (state.map || {}).title || "");
  const arrow = el("chart-arrow");
  arrow.style.display = at ? "" : "none";
  if (at) {
    const [u, v] = where(at.x, at.z);
    arrow.style.left = `${u * 100}%`;
    arrow.style.top = `${v * 100}%`;
    arrow.style.transform = `rotate(${at.heading}deg)`;
  }
  // Where the driver has said they are going: a flag there, and the place of that name lit.
  const going = state.waypoint, flag = el("chart-flag");
  flag.hidden = !going;
  if (going) {
    const [u, v] = where(going.x, going.z);
    flag.style.left = `${u * 100}%`;
    flag.style.top = `${v * 100}%`;
  }
  setText("chart-going", going ? `Going to: ${going.title}` : "Choose where to go: a place in the list, or anywhere on the map.");
  const labels = el("chart-labels"), places = state.places || [];
  const wanted = JSON.stringify(places) + JSON.stringify(going || null);
  if (labels.dataset.wanted !== wanted) {
    labels.dataset.wanted = wanted;
    // The places there are to go, as a list to choose from: so a controller can.
    const list = el("chart-places"), was = held.slice();
    list.replaceChildren(...places.map((place) => {
      const is = going && Math.hypot(going.x - place.x, going.z - place.z) < 6;
      const made = button(`<span>${place.title}</span>`, null, is ? "going" : "");
      made.ask = () => say(`set waypoint ${place.x.toFixed(1)} ${place.z.toFixed(1)}`);
      return made;
    }));
    if (screenShown() === "map") hold(was[0], was[1]);
    labels.replaceChildren(...places.map((place) => {
      const label = document.createElement("span"), [u, v] = where(place.x, place.z);
      label.textContent = place.title;
      label.style.left = `${u * 100}%`;
      label.style.top = `${v * 100}%`;
      return label;
    }));
  }
}

// Clicked, the map says where to go: there.
el("chart").addEventListener("click", (event) => {
  const drawn = state.chart, box = el("chart-img").getBoundingClientRect();
  if (!drawn || !box.width) return;
  const u = (event.clientX - box.left) / box.width, v = (event.clientY - box.top) / box.height;
  if (u < 0 || u > 1 || v < 0 || v > 1) return;
  say(`set waypoint ${(u * 2 * drawn.half - drawn.half).toFixed(1)} ${(v * 2 * drawn.half - drawn.half).toFixed(1)}`);
});

// ---- Showing things.
function lapTime(seconds) {
  if (seconds === null || seconds === undefined) return "-:--.---";
  const minutes = Math.floor(seconds / 60);
  return `${minutes}:${(seconds - minutes * 60).toFixed(3).padStart(6, "0")}`;
}
const setTextOf = (e, text) => { if (e.textContent !== text) e.textContent = text; };
const setText = (id, text) => setTextOf(el(id), text);

// The screen there is now: its section shown, its words filled in, its first button in hand.
let shown = null;
// ---- The yard: money and trucks. The truck in hand stands in the showroom behind the page
// (the game puts it there when it is told which is in hand), and the page is made from what
// the game says: the trucks to choose from along the bottom, what the one in hand is down
// the side, and what can be done with it.
const cash = (n) => `$${Number(n || 0).toLocaleString("en-AU")}`;
const far = (n) => `${Number(n || 0).toLocaleString("en-AU")} km`;
const PARTS = { bullbar: "Bull bar", spots: "Spot lamps", snorkel: "Snorkel", rack: "Roof rack", lightbar: "Light bar", arches: "Arches",
  steps: "Steps", suspension: "Suspension", tyres: "Tyres", rims: "Rims", tub: "Tub top", tray: "Tray", spare: "Spare wheel" };
const KINDS = { tube: "Tube bar", plate: "Winch bar", nudge: "Nudge bar", high: "On the bar", low: "Low", fitted: "Fitted", roof: "On the roof",
  rack: "On the rack", lip: "Standard", bolted: "Bolt-on flares", moulded: "Moulded flares", steps: "Side steps", sliders: "Rock sliders",
  stock: "Standard", heavy: "Heavy duty", lift: "50 mm lift", long: "Long travel", road: "Road", at: "All-terrain", mud: "Mud",
  slot8: "Slotted steel", sunraysia: "Sunraysia", spoke5: "Five spoke", spoke6: "Six spoke", mesh: "Mesh", beadlock: "Beadlock",
  sportsbar: "Sports bar", tonneau: "Tonneau", canopy: "Canopy", timber: "Timber deck", boxes: "Tool boxes" };
// A group that a truck always has one of: nothing is not a choice of it.
const ALWAYS = ["arches", "suspension", "tyres", "rims"];
const ROLLS = ["Rough", "Tired", "Fair", "Good", "Very good", "As new"];
const condition = (roll) => ROLLS[Math.min(ROLLS.length - 1, Math.floor((roll || 0) * ROLLS.length))];
// What each truck is, beyond what the game says of it: its body, its engine, its weight.
let SPECS = {};
fetch("trucks/specs.json").then((got) => got.json()).then((specs) => { SPECS = specs; yardKey = ""; if (state.screen === "yard") show(); }).catch(() => {});
let yardKey = "", yardPage = "", yardPick = 0, yardShown = -1, yardTimer = 0;
function yardCards() {
  const yard = state.yard || {};
  return yard.page === "start" ? yard.start || [] : yard.page === "garage" ? yard.trucks || [] : yard.page === "used" ? yard.used || []
    : yard.page === "free" ? yard.free || [] : yard.page === "dealer" ? yard.dealer || [] : [];
}
function button(html, act, classes) {
  const made = document.createElement("button");
  made.innerHTML = html;
  if (act) made.dataset.do = act;
  if (classes) made.className = classes;
  wire(made);
  return made;
}
// The truck in hand: what is said of it, what it is, and what can be done with it.
function yardActs() {
  const yard = state.yard;
  if (!yard) return;
  const acts = el("yard-acts"), cards = yardCards();
  const own = (yard.trucks || []).find((truck) => truck.driving);
  const setting = yard.page === "tune" || yard.page === "where";
  const card = yard.page === "colour" ? yard.chosen : setting ? yard.free_truck : cards.length ? cards[Math.min(yardPick, cards.length - 1)] : own;
  acts.replaceChildren();
  let detail = "";
  const spec = (card && SPECS[card.name]) || {};
  const lines = [];
  if (card) {
    if (spec.about) lines.push(`<p>${spec.about}</p>`);
    lines.push(`<dt>Year</dt><dd>${card.year}</dd>`, `<dt>Grade</dt><dd>${card.grade} of 5</dd>`);
    if (spec.body) lines.push(`<dt>Body</dt><dd>${spec.body}</dd>`);
    if (spec.kw) lines.push(`<dt>Engine</dt><dd>${spec.kw} kW ${spec.engine}</dd>`, `<dt>Weight</dt><dd>${Number(spec.kg).toLocaleString("en-AU")} kg</dd>`, `<dt>Tyres</dt><dd>${spec.tyres} inch</dd>`);
    if (card.km !== undefined) lines.push(`<dt>Driven</dt><dd>${far(card.km)}</dd>`, `<dt>Condition</dt><dd>${condition(card.roll)}</dd>`);
    if (card.oil !== undefined) lines.push(`<dt>Oil</dt><dd>${card.oil > 0 ? `${far(card.oil)} overdue` : `due in ${far(-card.oil)}`}</dd>`);
  }
  if (card && (yard.page === "start" || yard.page === "colour")) {
    detail = `${card.year}  ·  ${cash(card.price)} on finance: ${cash(yard.block + card.price)} owed in all`;
    lines.push(`<dt>Price</dt><dd class="big">${cash(card.price)}</dd>`);

  } else if (card && yard.page === "garage") {
    detail = `${card.year}  ·  ${far(card.km)}${card.driving ? "  ·  the one you drive" : ""}`;
    lines.push(`<dt>Worth</dt><dd class="big">${cash(card.fetches)}</dd>`);

    if (cards.length > 1) acts.append(button(`<span>Sell it</span><small>${cash(card.fetches)} in your hand</small>`, `sell:${card.index}`));
  } else if (card && yard.page === "used") {
    detail = card.here ? `${card.year}  ·  ${far(card.km)}  ·  ${condition(card.roll)}${card.bargain ? "  ·  a good one, going cheap" : ""}` : "Sold: it is in your shed";
    lines.push(`<dt>Price</dt><dd class="big">${cash(card.price)}</dd>`);
    if (card.here && card.price > yard.money) detail += "  ·  what you do not have goes on the debt";
  } else if (card && yard.page === "dealer") {
    detail = `${card.year}  ·  Grade ${card.grade}  ·  new, with nothing on the clock`;
    lines.push(`<dt>Price</dt><dd class="big">${cash(card.price)}</dd>`);
  } else if (card && yard.page === "free") {
    detail = `${card.year}  ·  Grade ${card.grade}  ·  press to build it`;
  } else if (card && setting) {
    detail = `${card.year}  ·  Grade ${card.grade}${yard.monster ? "  ·  monster truck" : ""}`;
  } else if (card) {
    detail = `${card.year}  ·  ${far(card.km)}  ·  the one you drive`;
  }
  el("yard-spec").innerHTML = lines.join("");
  // (Its colour being chosen, the colour in hand is named with it.)
  const tint = yard.page === "colour" ? (yard.colours || [])[yardPick] : null;
  setText("yard-name", card ? `${card.title}${tint ? `  ·  ${tint.name}` : ""}` : "");
  // (In the shop and at the bank, what is in hand in the strip is what is named.)
  const inHand = el("yard-lot").children[yardPick];
  if ((yard.page === "shop" || yard.page === "bank" || setting) && inHand) {
    const words = [...inHand.querySelectorAll("b, small")].map((part) => part.textContent).filter(Boolean);
    const wheres = { map: "Map", time: "Time of day", weather: "Weather" };
    const section = yard.page === "bank" ? "Bank"
      : yard.page === "where" ? (card ? card.title : "") + (shopOpen ? `  ·  ${wheres[shopOpen] || shopOpen}` : "")
      : yard.page === "tune" ? (card ? card.title : "") + (!shopOpen ? "" : shopOpen === "paint" ? "  ·  Colour" : `  ·  ${PARTS[shopOpen] || shopOpen}`)
      : !shopOpen ? "Motor Spares" : shopOpen === "paint" ? `Respray  ·  ${cash(yard.respray)}` : PARTS[shopOpen] || shopOpen;
    setText("yard-name", section);
    detail = words.join("  ·  ");
    setText("yard-detail", detail);
  }
  setText("yard-detail", detail);
  const picture = el("yard-car");
  if (card && !yard.showroom) picture.src = `trucks/${card.name}.jpg`;
}
// A row of colours to choose from, the one that is worn marked: the shop's respray, and the
// colour a first truck is taken in.
function swatches(nav, colours, worn) {
  for (const colour of colours || []) {
    const is = worn && worn.toLowerCase() === `#${colour.hex}`.toLowerCase();
    const made = button("", `paint:${colour.hex}`, is ? "swatch fitted" : "swatch");
    made.style.background = `#${colour.hex}`; made.title = colour.name;
    nav.append(made);
  }
}
// Another truck is in hand: the page says what it is at once, and the game is told to stand
// it in the showroom once the hand has stopped moving along the row.
// A free drive's starting map, down the side: the one in hand in the row of maps, or else
// the one that is chosen. Its picture is `art/NAME.jpg`, if it has one.
function yardStart(inHand) {
  const yard = state.yard || {}, panel = el("yard-start");
  const maps = yard.page === "where" ? yard.maps || [] : [];
  const map = maps.find((map) => map.name === inHand) || maps.find((map) => map.name === yard.start_map);
  panel.hidden = !map || state.screen !== "yard";
  if (!map) return;
  art("yard-map", map.name);
  setText("yard-map-title", map.title);
  setText("yard-map-kind", map.name === yard.start_map ? map.kind || "" : `${map.kind || ""}  ·  press to start here`);
  setText("yard-map-about", map.about || "");
}
function yardHeld(held) {
  yardStart(held && held.dataset.map);
  if (!held || held.dataset.pick === undefined || state.screen !== "yard") return;
  const pick = Number(held.dataset.pick);
  // The row slides to put it in the middle.
  el("yard-lot").style.setProperty("--at", String(pick));
  if (pick !== yardPick) { yardPick = pick; yardActs(); }
  if (pick !== yardShown) {
    clearTimeout(yardTimer);
    // A truck in hand is stood in the showroom; a colour or a part in hand is put on the
    // one there, to be seen; a section, or something at the bank's counter, is only held.
    const data = held.dataset;
    const tell = data.colour ? `do paint:${data.colour}` : data.tint ? `do tint:${data.tint}` : data.try ? `do try:${data.try}`
      : data.only !== undefined ? null : `do show:${pick}`;
    yardTimer = setTimeout(() => { yardShown = pick; if (tell) say(tell); }, tell && tell.startsWith("do show") ? 260 : 120);
  }
}
// The auto shop: which of its sections is open (a group of parts, or `paint`), if one is.
let shopOpen = null;
function shopLeave() {
  shopOpen = null;
  yardKey = "";
  say("do focus:off");
  say("do untry");
  show();
  hold(0, yardBack);
}
let yardBack = 0;
// A tile of the strip that is words, not a picture: what it is, and a line under it.
function tile(name, note, act, classes) {
  return button(`<b>${name}</b><small>${note || ""}</small>`, act, `tile ${classes || ""}`);
}
function yard() {
  const yard = state.yard;
  if (!yard) { yardKey = ""; yardPage = ""; return; }
  const key = JSON.stringify(yard) + Object.keys(SPECS).length;
  if (key === yardKey) return;
  yardKey = key;
  const fresh = yard.page !== yardPage;
  yardPage = yard.page;
  // (On a new page the first thing is in hand and is what is shown; a first colour is not
  // on the truck yet, and is put on it.)
  if (fresh) { yardPick = 0; yardShown = yard.page === "colour" ? -1 : 0; shopOpen = null; }
  const driven = (yard.trucks || []).find((truck) => truck.driving);
  el("yard").classList.toggle("flat", !yard.showroom);
  const words = {
    free: ["Free drive", "Choose a truck", "Any of the game's trucks. Next you build it, and choose where and when to drive it. Nothing is owed, and nothing is kept."],
    tune: ["Free drive", "Build it", "Fit what you like from its kit, choose its colour, or stand it on a monster truck's frame and tyres. Nothing costs anything here."],
    where: ["Free drive", "Where and when", "The map to start on, the hour, and the weather. It keeps that weather wherever it goes."],
    colour: ["Day one", "Choose its colour", "Any of these, at no cost. A respray later is paid for at Motor Spares."],
    start: ["Day one", "Choose your first truck", `You have bought a block in the high country with a shed on it, and you owe the bank ${cash(yard.block)} for it. You need a truck to work it. These three you can get on finance.`],
    garage: ["Your shed", "Garage", "The trucks you own. One is the one you drive."],
    shop: ["Ironbark Flat  ·  Motor Spares", "Auto shop", "A part you have bought is yours: taking it off and putting it back costs nothing."],
    used: ["Ironbark Flat  ·  Ironbark Motors", "Used trucks", "What is on the yard today. It is different every day, and now and then there is a good one going cheap."],
    dealer: ["Ironbark Flat  ·  Ironbark Motors", "New trucks", "Any truck there is, new, at its price. What you do not have in hand goes on the debt, if the bank will lend it."],
    bank: ["Ironbark Flat  ·  The Bank", "Bank", `You can owe up to ${cash(yard.limit)}. Every Friday ${cash(yard.repayment)} comes out. Pay it and your credit goes up; miss it and it goes down.`],
  }[yard.page] || ["", "", ""];
  setText("yard-kicker", words[0]); setText("yard-title", words[1]); setText("yard-about", words[2]);
  el("yard-facts").innerHTML = `<div><dt>In hand</dt><dd>${cash(yard.money)}</dd></div><div><dt>Owed</dt><dd class="owed">${cash(yard.page === "start" || yard.page === "colour" ? yard.block : yard.debt)}</dd></div>`
    + (yard.page === "start" || yard.page === "colour" || yard.page === "free" || yard.page === "tune" || yard.page === "where" ? "" : `<div><dt>Credit</dt><dd>${yard.credit}</dd></div><div><dt>Fridays</dt><dd>${cash(yard.repayment)}</dd></div><div><dt>Interest</dt><dd>${yard.interest_year}%</dd></div>`);
  // The strip of trucks.
  const lot = el("yard-lot");
  lot.replaceChildren();
  // The second step of a first truck: the colours, in the strip the trucks were in.
  if (yard.page === "colour") {
    for (const [index, colour] of (yard.colours || []).entries()) {
      const made = button(`<i style="background:#${colour.hex}"></i><b>${colour.name}</b>`, `take:${colour.hex}`, "colour");
      made.dataset.colour = colour.hex;
      made.dataset.pick = String(index);
      lot.append(made);
    }
  }
  // The auto shop: its sections, each a group of parts, the respray and the oil; or, a
  // section opened, what is in it. What is in hand is seen on the truck before it is bought.
  const cart = yard.cart || {};
  const add = (made, index, only) => { made.dataset.pick = String(index); if (only) made.dataset.only = ""; lot.append(made); };
  // (The view goes to a part of the truck that is opened; a map or an hour is not one.)
  const opens = (made, what, index, looks = true) => { made.ask = () => { shopOpen = what; yardBack = index; yardKey = ""; yardPick = 0; yardShown = -1; if (looks) say(`do focus:${what}`); show(); }; return made; };
  // A free drive's truck is built: its kit's groups, its colour, and whether it is stood on
  // a monster truck's frame and tyres. Nothing has a price.
  if (yard.page === "tune") {
    const groups = (yard.tune && yard.tune.groups) || [];
    const open = groups.find((group) => group.group === shopOpen);
    if (open) {
      const choices = ALWAYS.includes(open.group) ? [] : [{ choice: "none", fitted: open.bare }];
      choices.push(...open.choices);
      for (const [index, choice] of choices.entries()) {
        const made = tile(choice.choice === "none" ? "None" : KINDS[choice.choice] || choice.choice, choice.fitted ? "Fitted" : "", `fit:${open.group}:${choice.choice}`, choice.fitted ? "fitted" : "");
        made.dataset.try = `${open.group}:${choice.choice}`;
        add(made, index);
      }
    } else if (shopOpen === "paint") {
      for (const [index, colour] of (yard.colours || []).entries()) {
        const worn = yard.free_paint === colour.hex;
        const made = button(`<i style="background:#${colour.hex}"></i><b>${colour.name}</b>`, `paint:${colour.hex}`, worn ? "colour fitted" : "colour");
        made.dataset.tint = colour.hex;
        add(made, index);
      }
    } else {
      shopOpen = null;
      let index = 0;
      for (const group of groups) {
        const fitted = group.choices.find((choice) => choice.fitted);
        add(opens(tile(PARTS[group.group] || group.group, fitted ? KINDS[fitted.choice] || fitted.choice : "None"), group.group, index), index, true);
        index += 1;
      }
      const named = (yard.colours || []).find((colour) => colour.hex === yard.free_paint);
      add(opens(tile("Colour", named ? named.name : "As it was made"), "paint", index), index, true);
      add(tile("Monster truck", yard.monster ? "On: on the frame and tyres" : "Off", `monster:${yard.monster ? "off" : "on"}`, yard.monster ? "fitted" : ""), index + 1, true);
      add(tile("Where and when", "The map, the hour, the weather", "page:where", "counter"), index + 2, true);
    }
  }
  // And where and when it is driven: the map it starts on, the hour, and the weather.
  if (yard.page === "where") {
    const maps = yard.maps || [], times = yard.times || [], weathers = yard.weathers || [];
    const chosen = (is) => (is ? "fitted" : "");
    if (shopOpen === "map") {
      for (const [index, map] of maps.entries()) {
        const made = tile(map.title, map.kind || "", `where:${map.name}`, chosen(map.name === yard.start_map));
        made.dataset.map = map.name;
        add(made, index, true);
      }
    } else if (shopOpen === "time") {
      for (const [index, name] of times.entries()) add(tile(name, index === yard.time ? "Chosen" : "", `time:${index}`, chosen(index === yard.time)), index, true);
    } else if (shopOpen === "weather") {
      for (const [index, name] of weathers.entries()) add(tile(name, index === yard.weather ? "Chosen" : "", `weather:${index}`, chosen(index === yard.weather)), index, true);
    } else {
      shopOpen = null;
      const map = maps.find((map) => map.name === yard.start_map);
      add(opens(tile("Map", map ? map.title : ""), "map", 0, false), 0, true);
      add(opens(tile("Time of day", times[yard.time] || ""), "time", 1, false), 1, true);
      add(opens(tile("Weather", weathers[yard.weather] || ""), "weather", 2, false), 2, true);
      add(tile("Damage", yard.damage === false ? "Off: nothing breaks" : "On", `damage:${yard.damage === false ? "on" : "off"}`, yard.damage === false ? "" : "fitted"), 3, true);
      add(tile("Drive", "Start the free drive", "drive", "counter"), 4, true);
    }
  }
  if (yard.page === "shop") {
    const groups = (yard.shop && yard.shop.groups) || [];
    const open = groups.find((group) => group.group === shopOpen);
    if (open) {
      const choices = ALWAYS.includes(open.group) ? [] : [{ choice: "none", fitted: open.bare, note: open.bare ? "As it is" : "Take it off" }];
      choices.push(...open.choices);
      for (const [index, choice] of choices.entries()) {
        const note = choice.note || (choice.fitted ? "Fitted" : choice.owned ? "Yours: fit it" : choice.cart ? `In the cart  ·  ${cash(choice.price)}` : cash(choice.price));
        const made = tile(choice.choice === "none" ? "None" : KINDS[choice.choice] || choice.choice, note, `fit:${open.group}:${choice.choice}`, choice.fitted ? "fitted" : choice.cart ? "carted" : "");
        made.dataset.try = `${open.group}:${choice.choice}`;
        add(made, index);
      }
    } else if (shopOpen === "paint") {
      for (const [index, colour] of (yard.colours || []).entries()) {
        const worn = driven && driven.paint && driven.paint.toLowerCase() === `#${colour.hex}`;
        const carted = cart.paint === colour.hex;
        const made = button(`<i style="background:#${colour.hex}"></i><b>${colour.name}${carted ? "  ·  in the cart" : ""}</b>`, `paint:${colour.hex}`, worn ? "colour fitted" : "colour");
        made.dataset.tint = colour.hex;
        add(made, index);
      }
    } else {
      shopOpen = null;
      let index = 0;
      for (const group of groups) {
        const fitted = group.choices.find((choice) => choice.fitted), carted = group.choices.find((choice) => choice.cart);
        const note = carted ? `In the cart: ${KINDS[carted.choice] || carted.choice}` : fitted ? KINDS[fitted.choice] || fitted.choice : "None";
        add(opens(tile(PARTS[group.group] || group.group, note, null, carted ? "carted" : ""), group.group, index), index, true);
        index += 1;
      }
      add(opens(tile("Respray", cart.paint ? "In the cart" : cash(yard.respray), null, cart.paint ? "carted" : ""), "paint", index), index, true);
      const oil = driven && driven.oil > 0 ? `${far(driven.oil)} overdue` : driven ? `due in ${far(-driven.oil)}` : "";
      add(tile("Oil and filter", cart.oil ? `In the cart  ·  ${cash(yard.oil_change)}` : `${cash(yard.oil_change)}  ·  ${oil}`, "oil", cart.oil ? "carted" : ""), index + 1, true);
      // What it has hit is mended here: every hurt piece, by how far gone it is.
      const hurt = (yard.repairs && yard.repairs.hurt) || 0, mend = cash((yard.repairs && yard.repairs.cost) || 0);
      const pieces = `${hurt} ${hurt === 1 ? "piece" : "pieces"}`;
      add(tile("Repairs", cart.repair ? `In the cart  ·  ${mend}` : hurt ? `${mend}  ·  ${pieces} hurt` : "Nothing to mend", "repair", cart.repair ? "carted" : ""), index + 2, true);
      // The counter: what is in the cart is paid for there, or put back.
      if (cart.count) {
        add(tile("To the counter", `${cart.count} in the cart  ·  ${cash(cart.total)}`, "checkout", "counter"), index + 3, true);
        add(tile("Empty the cart", "Put it all back"), index + 4, true);
        lot.lastChild.dataset.do = "empty";
      }
    }
  }
  // The bank's counter: what is borrowed, and what is paid off.
  if (yard.page === "bank") {
    let index = 0;
    for (const amount of [1000, 5000, 20000]) { const made = tile("Borrow", cash(amount), `borrow:${amount}`); made.dataset.pick = String(index++); made.dataset.only = ""; lot.append(made); }
    for (const amount of [1000, 5000, 20000]) { const made = tile("Pay off", cash(amount), `repay:${amount}`); made.dataset.pick = String(index++); made.dataset.only = ""; lot.append(made); }
    const all = tile("Pay off", "All I can", "repay:all"); all.dataset.pick = String(index); all.dataset.only = ""; lot.append(all);
  }
  for (const [index, card] of yardCards().entries()) {
    const dot = card.paint ? `<span class="dot" style="background:${card.paint}"></span>` : "";
    const tag = card.bargain && card.here ? '<span class="tag">Bargain</span>' : card.driving ? '<span class="tag">Driving</span>' : "";
    // Chosen where it stands: it is taken, taken out, bought or driven by pressing on it.
    const act = yard.page === "start" ? `choose:${card.name}` : yard.page === "free" ? `free:${card.name}`
      : yard.page === "garage" ? (card.driving ? null : `drive:${card.index}`) : yard.page === "used" && card.here ? `buy:${card.place}`
      : yard.page === "dealer" ? `new:${card.name}` : null;
    const made = button(`<img src="trucks/${card.name}_small.jpg" alt="${card.title}">${tag}${dot}`, act, card.here === false ? "gone" : "");
    made.dataset.pick = String(index);
    lot.append(made);
  }
  lot.style.setProperty("--at", String(Math.min(yardPick, Math.max(lot.children.length - 1, 0))));
  yardActs();
  // (Nothing is down the side but what the truck is: what there is to choose is in the strip.)
  el("yard-rows").replaceChildren();
  setText("yard-said", yard.said || "");
  // The bill for what was asked for, if there is one: over everything else on the page.
  const quote = yard.quote, sheet = el("yard-quote");
  const quoting = !!quote, was = !sheet.hidden;
  sheet.hidden = !quoting;
  el("yard").classList.toggle("quoting", quoting);
  if (quote) {
    setText("quote-kicker", quote.what === "cart" ? "Motor Spares" : "Ironbark Motors");
    const named = (item) => item.truck ? `${item.truck}${item.used ? ", used" : ", new"}` : item.paint ? "Respray" : item.oil ? "Oil and filter"
      : item.repair ? `Repairs: ${item.repair} ${item.repair === 1 ? "piece" : "pieces"}`
      : `${PARTS[item.group] || item.group}: ${KINDS[item.choice] || item.choice}`;
    el("quote-lines").innerHTML = quote.items.map((item) => `<tr><td>${named(item)}</td><td>${cash(item.price)}</td></tr>`).join("")
      + `<tr class="total"><td>In all</td><td>${cash(quote.total)}</td></tr>`;
    const more = quote.repayment_after > quote.repayment;
    el("quote-facts").innerHTML = `<div><dt>From what is in hand</dt><dd>${cash(quote.cash)}</dd></div>`
      + `<div><dt>On the debt</dt><dd class="${quote.financed ? "more" : ""}">${cash(quote.financed)}</dd></div>`
      + `<div><dt>Owed after</dt><dd>${cash(quote.debt_after)}</dd></div>`
      + `<div><dt>Fridays after</dt><dd class="${more ? "more" : ""}">${cash(quote.repayment_after)}${more ? `  (now ${cash(quote.repayment)})` : ""}</dd></div>`
      + `<div><dt>Interest</dt><dd>${quote.interest_year}% a year</dd></div>`;
    setText("quote-why", quote.ok ? "" : `The bank will not lend that much: you can owe ${cash(quote.limit)} in all.`);
    el("quote-yes").hidden = !quote.ok;
  }
  // The other pages, and the way out.
  const tabs = el("yard-tabs");
  tabs.replaceChildren();
  if (yard.page === "free" || yard.page === "dealer") {
    for (const grade of [1, 2, 3, 4, 5]) if (grade !== yard.grade) tabs.append(button(`Grade ${grade}`, `grade:${grade}`));
    // A free drive's truck can be stood on a monster truck's frame and tyres, whichever it is.
    if (yard.page === "free") tabs.append(button(yard.monster ? "Monster truck: on" : "Monster truck: off", `monster:${yard.monster ? "off" : "on"}`));
  }
  // The yard sells used trucks and new ones: the one is a step from the other.
  if (yard.page === "used") tabs.append(button("New trucks", "page:dealer"));
  if (yard.page === "dealer") tabs.append(button("Used trucks", "page:used"));
  if ((yard.page === "shop" || yard.page === "tune" || yard.page === "where") && shopOpen) {
    // Out of a section of the shop: back to its sections, the truck as it was.
    const out = button("Back", null);
    out.ask = shopLeave;
    tabs.append(out);
  } else {
    tabs.append(button(yard.from === "title" || yard.page === "tune" || yard.page === "where" ? "Back" : yard.page === "garage" ? "Back to the shed" : "Leave", "leave"));
  }
  // What was in hand is in hand still, if it is still there: on a new page, the first thing.
  if (fresh) hold(0, 0);
  else if ((yard.page === "shop" || yard.page === "tune" || yard.page === "where") && yardShown === -1) {
    // A section just opened: what is on the truck is what is in hand.
    const worn = Math.max(0, [...lot.children].findIndex((made) => made.classList.contains("fitted")));
    yardPick = yardShown = worn;
    hold(0, worn);
  } else if (quoting !== was) hold(0, quoting ? 0 : yardPick);
  else hold(held[0], held[1]);
}

// ---- The kept games: three slots, a tile each. A new game is begun in one, and one that
// has a game in it is gone on with or deleted. What cannot be undone is asked twice: the
// first press only makes the button say so.
let savesKey = "", saveSure = "";
function saves() {
  const games = state.saves || [];
  const key = JSON.stringify(games) + saveSure + screenShown();
  if (key === savesKey) return;
  savesKey = key;
  // (Asked by whatever chooses the button: a click, the Enter key or a controller's button.)
  const asks = (made, id) => { made.ask = () => {
    saveSure = id; savesKey = ""; saves();
    holdButton(document.querySelector(`#${screenShown()} .sure`));
  }; };
  // A slot's tile: its number, its truck's picture, and what is in it; and, on a tile that
  // is pressed, what pressing it does.
  const tile = (game, word, act, sure) => {
    const kept = game.game || {};
    const picture = kept.truck ? `<img src="trucks/${kept.truck}_small.jpg" alt="">`
      : `<div class="blank">${game.empty ? "Empty" : "No truck yet"}</div>`;
    const about = game.empty ? "<strong>Empty slot</strong><i>Nothing is kept here</i>"
      : `<strong>${kept.title || "A new game"}</strong><i>Day ${kept.day || 1}</i>`;
    const facts = game.empty ? "" : `<dl><div><dt>In hand</dt><dd>${cash(kept.money)}</dd></div>`
      + (kept.title ? `<div><dt>Owed</dt><dd>${cash(kept.debt)}</dd></div><div><dt>Trucks</dt><dd>${kept.trucks}</dd></div>` : "") + "</dl>";
    const html = `<b>Slot ${game.slot}</b>${picture}${about}${facts}${word ? `<em>${word}</em>` : ""}`;
    if (word) return button(html, act, sure ? "slot sure" : "slot");
    const made = document.createElement("div");
    made.className = "slot empty";
    made.innerHTML = html;
    return made;
  };
  const fresh = el("newgame-list"), kept = el("loadgame-list"), acts = el("loadgame-acts");
  fresh.replaceChildren(); kept.replaceChildren(); acts.replaceChildren();
  for (const game of games) {
    const begin = `save_new:${game.slot}`;
    if (game.empty) fresh.append(tile(game, "Start here", begin));
    else if (saveSure === `new${game.slot}`) fresh.append(tile(game, "Replace this game?", begin, true));
    else { const made = tile(game, "Start here", null); asks(made, `new${game.slot}`); fresh.append(made); }
    kept.append(game.empty ? tile(game) : tile(game, "Load", `save_use:${game.slot}`));
    if (game.empty) acts.append(document.createElement("span"));
    else if (saveSure === `delete${game.slot}`) acts.append(button("Delete for good?", `save_delete:${game.slot}`, "sure"));
    else { const made = button("Delete", null); asks(made, `delete${game.slot}`); acts.append(made); }
  }
}

// ---- The options: which tab is shown, and what each option is set to.
let optionsTab = "keys";
// What is done with the keys and with a controller, as the two tabs list it: what it is, the
// actions that do it (as `data-glyph` has them), and a word more for one device or the other.
// What is drawn beside each is whatever the game says is bound to those actions.
const CONTROLS = {
  driving: ["Driving",
    ["Throttle, brake", "throttle brake"],
    ["Steer", "steer"],
    ["Handbrake", "handbrake"],
    ["Gear up, down", "shift_up shift_down"],
    ["2H, 4H, 4L", "drive_mode"],
    ["Diff locks", "diff_lock"],
    ["Air the tyres down, up", "tyres"],
    ["Look round", "orbit orbit_up look_round look_over", { keys: "click first" }],
    ["Look behind", "look_back"],
    ["Change view", "camera"],
    ["Back on the track", "reset"],
    ["Pause, and the map", "pause"]],
  foot: ["On foot",
    ["Get out, get in", "hold:enter~KeyE"],
    ["Walk", "forward+ right- forward- right+"],
    ["Run", "run|sprint"],
    ["Look round", "turn look_up"],
    ["Jump", "jump"],
    ["Dig, pick up, put in, go in a door", "enter~KeyE"],
    ["Detector or empty hands", "swap"],
    ["Put down what you hold", "drop"]],
};
// The two tabs' tables, made once: each row's buttons are `data-glyph`, of the tab's device
// whatever is in hand, so `dress` draws them and draws them again when they are bound anew.
function controlsTables() {
  for (const table of document.querySelectorAll("#controls table[data-controls]")) {
    if (table.rows.length) continue;
    const kit = table.closest(".tab").dataset.panel === "pad" ? "pad" : "keys";
    const [title, ...rows] = CONTROLS[table.dataset.controls];
    table.innerHTML = `<thead><tr><th>${title}</th><th></th></tr></thead><tbody>${rows.map(([what, actions, more]) =>
      `<tr><td>${what}</td><td><span class="glyphs" data-glyph="${actions}" data-kit="${kit}"></span>${more && more[kit] ? `<small>${more[kit]}</small>` : ""}</td></tr>`).join("")}</tbody>`;
  }
}
function options() {
  const set = state.options || { volume: 10, fullscreen: false };
  controlsTables();
  for (const tab of document.querySelectorAll("#option-tabs button")) tab.classList.toggle("picked", tab.dataset.tab === optionsTab);
  for (const panel of document.querySelectorAll("#controls .tab")) panel.classList.toggle("shown", panel.dataset.panel === optionsTab);
  const pips = el("option-volume");
  if (pips.children.length === 0) {
    for (let share = 0; share <= 10; share += 1) {
      const pip = button("", `option:volume:${share}`);
      pip.dataset.live = "";
      pips.append(pip);
    }
  }
  for (const [share, pip] of [...pips.children].entries()) {
    pip.classList.toggle("on", share <= set.volume && set.volume > 0);
    pip.classList.toggle("picked", share === set.volume);
  }
  setText("option-volume-says", set.volume === 0 ? "Off" : `${set.volume * 10}%`);
  // How fast the right stick looks: ten steps, the sixth as it is to begin with.
  const looks = el("option-look"), look = set.look || 6;
  if (looks.children.length === 0) {
    for (let speed = 1; speed <= 10; speed += 1) {
      const pip = button("", `option:look:${speed}`);
      pip.dataset.live = "";
      looks.append(pip);
    }
  }
  for (const [index, pip] of [...looks.children].entries()) {
    pip.classList.toggle("on", index + 1 <= look);
    pip.classList.toggle("picked", index + 1 === look);
  }
  setText("option-look-says", String(look));
  for (const [index, choice] of [...el("option-screen").children].entries()) choice.classList.toggle("picked", (index === 1) === !!set.fullscreen);
}

function show() {
  const screen = screenShown();
  // Paused in a story, the yard is a button away.
  // Paused: a run is started again; a track is left for the Crossroads.
  // (A free drive's map is started again too: where it starts, mended.)
  const timedRun = state.mode === "practice" || state.mode === "timed";
  const freeDrive = !(state.story && state.story.on);
  el("pause-restart").hidden = !(timedRun || freeDrive);
  setText("pause-restart-says", timedRun ? "From the start of this run." : "From the start of this map, with the truck as new.");
  // And its damage can be switched off, and on again.
  el("pause-damage").hidden = !freeDrive;
  setText("pause-damage-says", state.harm === false ? "Off: nothing you hit hurts the truck. Press to turn it on." : "On: what you hit breaks the truck. Press to turn it off.");
  // (Only from a map with no trailhead home: where there is one, it is driven to.)
  el("pause-hub").hidden = state.way_home !== false;
  const kept = !!(state.story && state.story.on);
  el("pause-title").querySelector("small").textContent = kept ? "The game is kept as you play." : "A free drive is not kept.";
  // What a sleep can be till: each time of day six hours off or more, the first of them
  // on the button there is, and the rest on buttons of their own after it.
  const sleeps = state.sleeps || [], wantedSleeps = JSON.stringify(sleeps) + kept;
  const first = el("shed-sleep");
  if (first.dataset.wanted !== wantedSleeps) {
    first.dataset.wanted = wantedSleeps;
    for (const old of document.querySelectorAll("#shed .sleep-more")) old.remove();
    const words = (sleep) => [`Sleep till ${sleep.name.toLowerCase()}`,
      `${sleep.hour}${sleep.tomorrow ? " tomorrow: the day ends" : " today"}, ${sleep.off} hours on.${kept && sleep.tomorrow ? " The game is saved." : ""}`];
    first.hidden = sleeps.length === 0;
    let after = first;
    for (const [place, sleep] of sleeps.entries()) {
      const [name, about] = words(sleep);
      const made = place === 0 ? first : button(`<span>${name}</span><small>${about}</small>`, `sleep:${sleep.index}`, "sleep-more");
      if (place === 0) {
        first.dataset.do = `sleep:${sleep.index}`;
        first.querySelector("span").textContent = name;
        first.querySelector("small").textContent = about;
      } else {
        after.after(made);
      }
      after = made;
    }
  }
  // On foot there is no getting out, and no driving on: there is leaving.
  for (const id of ["shed-walk", "buyer-walk"]) el(id).hidden = !!state.afoot;
  setText("sleep-till", `${state.slept_till || "Morning"}  ·  ${kept ? "saved" : "rested"}`);
  el("shed-yard").hidden = !(state.story && state.story.on);
  body.dataset.screen = screen;
  // (Once its screen is the one shown: what is in hand is found among what can be seen.)
  yard();
  saves();
  options();
  body.classList.toggle("pad", !!state.pad);
  // On foot, out of the truck: no dashboard, and what the button does instead.
  const afoot = !!state.afoot && screen === "drive";
  body.classList.toggle("afoot", afoot);
  el("foot").hidden = !afoot;
  el("foot-in").hidden = !state.by_truck;
  // What the driver's hands hold decides what the other two buttons do: the detector is
  // slung or taken in hand again, and whatever is held is put down.
  const holding = state.holding || "nothing";
  el("foot-does").hidden = !state.use;
  el("foot-swaps").hidden = holding === "nugget" || state.detector === "left";
  setText("foot-swap", holding === "detector" ? "Empty hands" : "Detector");
  el("foot-drops").hidden = holding === "nothing";
  setText("foot-drop", holding === "detector" ? "Put the detector down" : "Put down");
  chart();
  setText("touch-use", !afoot ? "Get out" : state.by_truck ? "Get in" : "Use");
  setText("touch-steer-words", afoot ? "Walk" : "Steer");
  // Each hint's button, and every other button the pages draw, as it is on what is in hand.
  dress();
  const map = state.map || {};
  setText("where-title", map.title || "");
  const clocked = !state.hub && (state.mode === "practice" || state.mode === "timed");
  setText("where-mode", !clocked ? "Free roam"
    : `${state.mode === "timed" ? "Timed run" : "Practice"}${state.run ? `  ·  Class ${state.run.class + 1}` : ""}`);
  el("laps").hidden = !clocked || !state.lap;
  el("laps").classList.toggle("once", state.mode === "timed");
  el("targets").hidden = !clocked || !state.lap || !(state.run && state.run.medals);
  timed();
  calendar();
  // The gold: what is in the truck's tray and in the chest, and, on foot, the detector, what is
  // in the hand, and what the button does where the driver stands.
  const [inTray, trayKilos] = state.tray || [0, 0], [inChest, chestKilos] = state.chest || [0, 0];
  const weighed = (count, kilos) => count === 0 ? "Nothing" : `${count} \u00b7 ${kilos.toFixed(1)} kg`;
  el("haul").hidden = inTray === 0;
  if (String(inTray) !== el("haul-count").textContent) {
    setText("haul-count", String(inTray));
    el("haul").classList.remove("new");
    if (inTray > 0 && shown === "drive") { void el("haul").offsetWidth; el("haul").classList.add("new"); }
  }
  setText("haul-words", `${trayKilos.toFixed(1)} kg in the tray`);
  for (const id of ["shed-day", "shed-day-2", "sleep-day"]) setText(id, String(state.day || 1));
  setText("shed-tray", weighed(inTray, trayKilos));
  setText("shed-chest", weighed(inChest, chestKilos));
  // What gold has been sold for, and, at the buyer's, what is on the scales and what it fetches.
  const dollars = (amount) => `$${Math.round(amount || 0).toLocaleString("en-AU")}`;
  for (const id of ["shed-money", "buyer-money"]) setText(id, dollars(state.money));
  setText("buyer-day", String(state.day || 1));
  setText("buyer-town", (state.map || {}).title || "Town");
  const buyer = state.buyer;
  if (buyer) {
    setText("buyer-tray", weighed(buyer.count, buyer.kilos));
    setText("buyer-rate", `${dollars(buyer.rate)} a kilo`);
    setText("buyer-pays", buyer.count ? dollars(buyer.pays) : "-");
    const sold = buyer.sold;
    setText("buyer-about", sold
      ? `Sold: ${sold.count} ${sold.count === 1 ? "nugget" : "nuggets"}, ${sold.kilos.toFixed(1)} kg on the scales, ${dollars(sold.paid)} paid. The tray is empty: there is more where that came from.`
      : "Gold bought by weight, for cash. What is lying in the tray of your truck is put on the scales: what bounced out on the way in is still out there on the track.");
    el("buyer-sell").disabled = buyer.count === 0;
    setText("buyer-sell-words", buyer.count
      ? `${buyer.count} ${buyer.count === 1 ? "nugget" : "nuggets"}, ${buyer.kilos.toFixed(1)} kg, for ${dollars(buyer.pays)}.`
      : sold ? "Nothing left to sell." : "There is no gold in the tray.");
  }
  const signal = state.signal ?? null;
  el("detector").hidden = signal === null || screen !== "drive";
  el("detector-bar").style.width = `${Math.round((signal || 0) * 100)}%`;
  el("detector").classList.toggle("over", (signal || 0) > 0.88);
  // What it hears, in words: how far off the gold is, and what to do about it.
  const away = state.gold_away ?? null, over = (signal || 0) > 0.88;
  setText("detector-hand", state.hand ? `${state.hand.toFixed(1)} kg in hand` : over ? "Dig here"
    : away === null ? "No signal" : away < 5 ? "Gold a few steps off" : `Gold about ${away} m off`);
  setText("detector-tip", over ? "You are over it." : state.gold_here === false ? "This ground is dug out for now. It is found here again in a few days."
    : away === null ? "Keep walking beside the road or the track: it hears gold within 60 metres."
    : "Turn on the spot: the beeps are fastest facing the gold. Walk that way.");
  // What the last blow did to the truck.
  const blow = screen === "drive" ? state.blow : null;
  el("blow").hidden = !blow;
  if (blow) setText("blow-said", blow);
  // By a truck in the used-truck yard: its tag, in place of the detector's panel.
  const tag = screen === "drive" ? state.tag : null;
  el("tag").hidden = !tag;
  if (tag) {
    el("detector").hidden = true;
    setText("tag-kicker", tag.bargain ? "Going cheap" : "For sale");
    el("tag").classList.toggle("bargain", !!tag.bargain);
    setText("tag-price", dollars(tag.price));
    setText("tag-title", tag.title || "");
    setText("tag-facts", `${tag.year}  ·  ${far(tag.km)}  ·  ${condition(tag.roll)}`);
  }
  const uses = { dig: "Dig", pick: "Pick up", tray: "Put in the tray", chest: "Put in the chest", untray: "Take from the tray",
    unchest: "Take from the chest", drop: "Put down", take: "Pick up the detector", open: "Open the door", shut: "Shut the door",
    door_shop: "Go in: Auto shop", door_used: "Ask about used trucks", door_bank: "Go in: Bank",
    door_shed: "Go in: The shed", door_buyer: "Go in: Gold buyer", truck_look: "Look it over",
    stow: "Put in the truck", unstow: "Take from the truck" };
  setText("foot-use", uses[state.use] || "Use");
  if (state.afoot && screen === "drive") setText("touch-use", state.by_truck && !state.use ? "Get in" : uses[state.use] || "Use");
  if (state.offer) {
    const offer = state.offer;
    setText("offer-title", offer.title || "");
    setText("offer-about", offer.about || "");
    setText("offer-kind", offer.kind || "-");
    setText("offer-length", offer.length || "-");
    setText("offer-best", lapTime(offer.best));
    el("offer-built").hidden = !offer.built;
    // Somewhere with no track to time, a town or the road, is only driven to: as the day
    // is, with no clock, so there is no hour, weather or best lap to speak of.
    const driven = offer.timed === false;
    for (const what of ["practice", "timed"]) document.querySelector(`#offer [data-do="${what}"]`).hidden = driven;
    for (const id of ["offer-time-row", "offer-weather-row", "offer-best-fact"]) el(id).hidden = driven;
    const roam = document.querySelector('#offer [data-do="roam"]');
    roam.querySelector("span").textContent = driven ? "Drive there" : "Free roam";
    roam.querySelector("small").textContent = driven ? "By the track, at your own pace." : "No clock. The track, and the bush round it, at your own pace.";
    setText("offer-kicker", driven ? "Gate" : "Trailhead");
    // One of the game's trucks runs in its own class: it is said, not chosen.
    el("offer-classes").hidden = !!state.class_fixed;
    setText("offer-class-label", state.class_fixed ? "Your truck's class" : "Class");
    el("offer-unbuilt").hidden = !!offer.built;
    art("offer-art", offer.built ? offer.name : "");
    if (screen === "offer" && shown !== "offer") {
      chips("offer-times", "time", state.times || [], state.time || 0);
      chips("offer-weathers", "weather", state.weathers || [], state.weather || 0);
      if (state.classes) chips("offer-classes", "class", state.classes.map((each) => String(each.class + 1)), state.class || 0, dressClass);
    }
    // What the night and the rain add to a purse, and which weather is today's.
    tagChips("offer-times", offer.timed === false ? null : (state.bonus || {}).time, -1);
    tagChips("offer-weathers", offer.timed === false ? null : (state.bonus || {}).weather, state.today);
    offerClass();
  }
  if (screen === "loading" && shown !== "loading") {
    setText("loading-title", map.title || "");
    // What it is, and what it is being driven in.
    const facts = [map.kind, map.length];
    if (!state.hub && state.times && state.weathers) facts.push(state.times[picked.time], state.weathers[picked.weather]);
    setText("loading-facts", facts.filter(Boolean).join("  ·  "));
    // One of the tips there are, of those whose buttons are known: `dress` writes it out.
    const tips = TIPS.filter((tip) => glyphWords(tip) !== null);
    el("loading-tip").dataset.words = tips[Math.floor(Math.random() * tips.length)];
    dress(el("loading"));
    // (The showroom is the Crossroads: it has its picture.)
    const shown = (map.name || "").startsWith("showroom");
    setText("loading-kicker", shown ? "Opening" : "Heading out to");
    art("loading-art", map.name === "showroom" ? "crossroads" : map.name === "showroom_town" ? "town" : map.name);
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
  el("dash-keys").classList.toggle("two", !hud.four_wheel_drive);
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
    // Which section of the lap it is, and what being put back has cost so far.
    setText("lap-section", lap.sections ? `${lap.section} of ${lap.sections}` : "-");
    setText("lap-penalty", lap.penalty ? `+${Math.round(lap.penalty)} s` : "");
  }
  // Each medal's time, and how far behind it the lap is here, or ahead.
  const run = state.run, pace = state.pace;
  if (run && run.medals) {
    for (const row of el("targets").children) {
      const index = rank(row.dataset.medal), gap = pace ? pace.gaps[index] : null;
      setTextOf(row.querySelector("b"), medalTime(run.medals[index]));
      setTextOf(row.querySelector("span"), gap === null ? "" : gapWords(gap));
      row.classList.toggle("inside", gap !== null && gap <= 0);
    }
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
    el("banner-key").hidden = touched && !state.pad;
    setText("banner-words", state.pad ? "Hold to enter" : touched ? "Touch here to enter" : "Hold to enter, or click here");
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

// On the desktop the page is not given the mouse: the game's window has it, and says here
// what it did, `move`, `down` or `up`, and where. A button under it is taken in hand as the
// mouse passes, and chosen when the mouse is pressed and let go on it. Pressed where there is
// nothing, while driving, the game is asked to take hold of the mouse, to look round with.
let pressed = null;
function pointer(what, x, y) {
  const button = document.elementFromPoint(x, y)?.closest("button") ?? null;
  for (const other of document.querySelectorAll("button.over")) if (other !== button) other.classList.remove("over");
  if (button) button.classList.add("over");
  if (what === "move" && button?.closest("nav")) holdButton(button);
  if (what === "down") {
    pressed = button;
    if (!button && state.screen === "drive" && !opened) say("grab");
  }
  if (what === "up") {
    if (button && button === pressed) button.click();
    pressed = null;
  }
}

// The game cannot start, and why: said on the loading screen, where its tip was, with nothing
// turning. It waits there to be closed.
function stuck(why) {
  el("stream").hidden = true;
  el("loading").classList.add("stuck");
  setText("loading-kicker", "Not yet heading out to");
  setText("loading-tip-head", "No maps yet");
  delete el("loading-tip").dataset.words;
  setText("loading-tip", why);
  setText("loading-foot", "Close the window to leave");
}

window.ui = {
  pointer,
  stuck,
  receive(told) {
    const nav = told.nav || [];
    // A page opened over a screen is shut when the game goes on to another: a game that
    // has been loaded is not behind the list it was chosen from.
    if (told.screen !== state.screen) { opened = null; saveSure = ""; }
    // What is bound to what, and whose controller it is, are kept from when they were last
    // said; and what is in hand, where it is not said (before the game is running).
    if (told.binds) { binds = told.binds; bindsTold += 1; }
    if (told.pad_kind) padKind = told.pad_kind === "playstation" ? "playstation" : "xbox";
    if (told.pad === undefined) told.pad = padHeld;
    padHeld = !!told.pad;
    keepHands();
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
  // In a browser. On the desktop the game's window has the mouse, and this is never heard.
  if (window.ipc) return;
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

// ---- Touch. A screen that is touched and has no mouse, a phone or a tablet, has the truck's
// controls drawn on it (`#touch`). Steering and the pedals are the game's touch stick, which
// it is told of as `touch X Y`, each from -1 to 1: right, and braking, above zero. The rest
// are keys, held as long as they are touched.
const touched = new URLSearchParams(location.search).has("touch")
  || (navigator.maxTouchPoints > 0 && matchMedia("(pointer: coarse)").matches);
if (touched) body.classList.add("touch");
// With a controller in use the controls are not drawn: a touch anywhere asks for them back.
addEventListener("touchstart", () => { if (touched && state.pad) say("touched"); }, { capture: true, passive: true });
const stick = { x: 0, y: 0, gas: 0, brake: 0, said: "touch 0.00 0.00" };
function sayStick() {
  // In the truck the pad steers and the pedals are the rest; on foot the pad is all of it.
  const y = state.afoot ? stick.y : stick.brake - stick.gas;
  const line = `touch ${stick.x.toFixed(2)} ${y.toFixed(2)}`;
  if (line !== stick.said) { stick.said = line; say(line); }
}
// Hears a control: `down` when a finger lands on it, `slid` as that finger moves, `up` when
// it goes. Each control follows its own finger, so that one may steer while another brakes.
function touchable(control, down, slid, up) {
  let finger = null;
  const mine = (event) => [...event.changedTouches].find((touch) => touch.identifier === finger);
  control.addEventListener("touchstart", (event) => {
    event.preventDefault();
    if (finger !== null) return;
    finger = event.changedTouches[0].identifier;
    control.classList.add("down");
    down(event.changedTouches[0]);
  }, { passive: false });
  control.addEventListener("touchmove", (event) => {
    event.preventDefault();
    const touch = mine(event);
    if (touch && slid) slid(touch);
  }, { passive: false });
  const gone = (event) => {
    if (!mine(event)) return;
    finger = null;
    control.classList.remove("down");
    up();
  };
  control.addEventListener("touchend", gone);
  control.addEventListener("touchcancel", gone);
}
{
  const pad = el("touch-steer"), knob = pad.querySelector("i");
  let from = [0, 0];
  const steer = (touch) => {
    // Full lock is a third of the pad's width from where the thumb came down.
    const reach = pad.clientWidth / 3;
    stick.x = Math.max(-1, Math.min(1, (touch.clientX - from[0]) / reach));
    stick.y = state.afoot ? Math.max(-1, Math.min(1, (touch.clientY - from[1]) / (reach * 0.6))) : 0;
    knob.style.transform = `translate(${stick.x * reach}px, ${stick.y * reach * 0.4}px)`;
    sayStick();
  };
  touchable(pad, (touch) => { from = [touch.clientX, touch.clientY]; steer(touch); }, steer, () => {
    stick.x = stick.y = 0;
    knob.style.transform = "";
    sayStick();
  });
  // Looking round, on foot: by how far the finger has moved since it was last told.
  let last = [0, 0];
  const look = (touch) => {
    say(`look ${(touch.clientX - last[0]).toFixed(1)} ${(touch.clientY - last[1]).toFixed(1)}`);
    last = [touch.clientX, touch.clientY];
  };
  touchable(el("touch-look"), (touch) => { last = [touch.clientX, touch.clientY]; }, look, () => {});
  for (const pedal of document.querySelectorAll("#touch [data-pedal]")) {
    const press = (much) => { stick[pedal.dataset.pedal] = much; sayStick(); };
    touchable(pedal, () => press(1), null, () => press(0));
  }
  for (const tap of document.querySelectorAll("#touch [data-key]")) {
    touchable(tap, () => say(`key ${tap.dataset.key} 1`), null, () => say(`key ${tap.dataset.key} 0`));
  }
  for (const tap of document.querySelectorAll("#touch [data-say]")) {
    touchable(tap, () => say(tap.dataset.say), null, () => {});
  }
}

// ---- With no game: made-up readings, and menus that go where they would.
function pretend(line) {
  const [word, what, down] = line.split(" ");
  if (word === "key" && down === "1") {
    // (A key pressed is the keyboard in hand, as the game would say.)
    if (state.pad) window.ui.receive({ ...state, pad: false, nav: [] });
    const way = { ArrowUp: "up", ArrowDown: "down", ArrowLeft: "left", ArrowRight: "right", Enter: "accept", Escape: "back", KeyQ: "prev", KeyE: "next" }[what];
    if (way === "back" && state.screen === "drive") window.ui.receive({ ...state, screen: "pause" });
    else if (way) navigate(way);
  }
  if (word !== "do") return;
  const clocked = state.mode === "practice" || state.mode === "timed";
  const to = { start: "drive", resume: "drive", pause: "pause", title: "title", leave: "drive", enter: "offer",
    roam: state.screen === "offer" ? "loading" : "drive", practice: "loading", timed: "loading", hub: "loading",
    restart: clocked ? "lineup" : "loading", sleep: "sleep", walk: "drive", go: "drive", again: "drive", skip: "lineup",
    watch0: "watch", watch1: "watch", watch2: "watch" }[what];
  if (!to) return;
  const next = { ...state, screen: to, nav: [] };
  if (to === "watch") {
    const rival = state.run.rivals[Number(what.slice(5))];
    next.watch = { ...rival, class: state.run.class, clock: 0, shot: 1, shots: 5 };
  }
  if (what === "walk") { next.afoot = true; next.by_truck = true; next.signal = 0.35; next.use = null; }
  if (what === "sleep") { next.day = (state.day || 1) + 1; setTimeout(() => window.ui.receive({ ...next, screen: "drive" }), 3400); }
  if (what === "practice" || what === "timed") next.mode = what;
  if (what === "roam" || what === "hub") next.mode = "roam";
  if (to === "loading" && what !== "hub" && what !== "restart") { next.map = state.offer; next.hub = false; }
  if (what === "hub") { next.hub = true; next.map = { name: "crossroads", title: "The Crossroads" }; }
  window.ui.receive(next);
  if (to === "loading" && !new URLSearchParams(location.search).has("still")) {
    const then = next.mode === "roam" ? "drive" : "lineup";
    setTimeout(() => window.ui.receive({ ...next, screen: then }), 4000);
  }
}
if (mock) {
  body.classList.add("mock");
  const wanted = new URLSearchParams(location.search);
  if (wanted.has("still")) body.classList.add("still");
  if (wanted.has("stream")) stream({ what: "Streaming map", much: "38%", share: 0.38, busy: true });
  const wombat = { name: "wombat", title: "Wombat Creek Track", about: "A washed-out fire trail through stringybark and manna gum: two creek fords, bog holes, rock steps and ruts cut by a wet winter.", kind: "Fire trail", length: "3.5 km", best: 312.482, built: true };
  // Timed runs, made up: five classes of the one track, each with its three other drivers.
  const names = ["Paddock bashers", "Old tourers", "Workhorses", "Built tourers", "The top end"];
  const others = [["Shaz Pickett", "Licola", "#3d6b3a"], ["Davo Treloar", "Dargo Styleside", "#d9a21b"], ["Narelle Baxter", "Omeo Tray", "#2f5f8a"]];
  const classes = names.map((name, index) => {
    const gold = Math.round((402 - 34 * index) * 10) / 10, pays = [1, 1.5, 2.5, 4, 6][index];
    const medals = [1.3, 1.12, 1].map((slower) => Math.round(gold * slower * 10) / 10);
    const top = Math.round(gold * 2 * pays / 10) * 10;
    return { class: index, name, medals: wanted.has("unknown") ? null : medals, purses: wanted.has("unknown") ? null : [top / 4, top / 2, top].map((p) => Math.round(p / 10) * 10),
      held: [null, "silver", "bronze", null, "gold"][index], best: index === 1 ? 441.207 : null,
      rivals: others.map(([name, truck, paint], slot) => ({ name, truck, paint, medal: MEDALS[slot], time: medals[slot] })) };
  });
  const chosen = Number(wanted.get("class") || 1), foul = wanted.has("foul");
  const run = { ...classes[chosen], night: foul, rain: foul, bonus: foul ? 100 : 0, hour: foul ? "Night" : "Day", weather: foul ? "Storm" : "Fair" };
  const skies = ["fair", "clear", "overcast", "storm", "fair", "clear", "clear", "fair", "overcast", "overcast", "storm", "fair", "clear"];
  const calendarDays = [...Array(28).keys()].map((index) => {
    const day = 8 + index, date = (11 + index - 1) % 31 + 1, ahead = day - 10;
    const when = ahead < 0 ? "past" : ahead === 0 ? "today" : ahead <= 2 ? "forecast" : ahead <= 6 ? "outlook" : "later";
    return { day, date, month: index === 0 ? "May" : date === 1 ? "Jun" : null, when, sky: when === "later" ? null : skies[index % skies.length], due: index % 7 === 4 ? 1850 : null };
  });
  const result = wanted.has("repeat")
    ? { class: chosen, time: 372.914, valid: true, medal: "gold", held: "gold", best: 371.2, record: false, medals: run.medals, firsts: [], repeat: 70, night: foul ? 35 : null, rain: foul ? 35 : null, paid: foul ? 140 : 70, grams: foul ? 58 : 29 }
    : wanted.has("void")
      ? { class: chosen, time: 512.33, valid: false, medal: null, held: "silver", best: 441.207, record: false, medals: run.medals, firsts: [], repeat: 0, night: null, rain: null, paid: 0, grams: null }
      : { class: chosen, time: run.medals[2] - 2.518, valid: true, medal: "gold", held: "silver", best: 441.207, record: true, medals: run.medals,
        firsts: [{ medal: "gold", purse: run.purses[2] }], repeat: 0, night: foul ? run.purses[2] / 2 : null, rain: foul ? run.purses[2] / 2 : null,
        paid: run.purses[2] * (foul ? 2 : 1), grams: Math.round(run.purses[2] * (foul ? 2 : 1) / 2.4) };
  window.ui.receive({
    class: chosen, classes, bonus: { time: [0, 0, 0, 50], weather: [0, 0, 0, 50] }, today: 2, run,
    pace: { gaps: [-21.4, -6.3, 2.8], on: "silver", ghosts: [{ medal: "bronze", x: -470, z: 120, done: false }, { medal: "silver", x: -430, z: 160, done: false }, { medal: "gold", x: -380, z: 60, done: false }] },
    watch: { ...run.rivals[2], class: chosen, clock: 141.3, shot: 3, shots: 5 },
    result,
    calendar: { day: 10, weekday: "Wednesday", date: 13, month: "May", months: "May and June", sky: "overcast", days: calendarDays,
      next: { day: 12, in: 2, weekday: "Friday", date: 15, month: "May", items: [{ what: "The block", dollars: 1500 }, { what: "The truck", dollars: 350 }], total: 1850 } },
    screen: wanted.get("screen") || "title", mode: wanted.get("mode") || "roam", nav: [],
    // What is in hand: `pad` is an Xbox controller, `pad=ps` a PlayStation's. And what is
    // bound to what, as the game's own files have it (`scenes/outback/game.input`, and the
    // menus' in `shell.rs`).
    pad: wanted.has("pad"), pad_kind: wanted.get("pad") === "ps" ? "playstation" : "xbox",
    binds: {"forward":{"key":["KeyW","ArrowUp","-KeyS","-ArrowDown"],"pad":["-left_y"]},"right":{"key":["KeyD","ArrowRight","-KeyA","-ArrowLeft"],"pad":["left_x"]},"run":{"key":["ShiftLeft"]},"sprint":{"pad":["l3"]},"turn":{"mouse":["-x"],"pad":["-right_x"]},"look_up":{"mouse":["-y"],"pad":["-right_y"]},"jump":{"key":["Space"],"pad":["north"]},"interact":{"key":["KeyE"],"pad":["south"]},"throttle":{"key":["KeyW","ArrowUp"],"pad":["r2"]},"brake":{"key":["KeyS","ArrowDown"],"pad":["l2"]},"steer":{"key":["KeyD","ArrowRight","-KeyA","-ArrowLeft"],"pad":["left_x"]},"handbrake":{"key":["Space"],"pad":["east"]},"shift_up":{"key":["KeyE"],"pad":["r1"]},"shift_down":{"key":["KeyQ"],"pad":["l1"]},"reset":{"key":["KeyR"],"pad":["north"]},"camera":{"key":["KeyC"],"pad":["select"]},"look_back":{"key":["KeyB"],"pad":["r3"]},"look_round":{"pad":["right_x"],"key":["KeyL","-KeyJ"]},"look_over":{"pad":["right_y"],"key":["KeyI","-KeyK"]},"orbit":{"mouse":["-x"]},"orbit_up":{"mouse":["y"]},"autopilot":{"key":["KeyP"],"pad":["start"]},"drive_mode":{"key":["KeyT"],"pad":["west"]},"diff_lock":{"key":["KeyG"],"pad":["south"]},"tyres":{"key":["KeyY"],"pad":["up"]},"menu_up":{"key":["ArrowUp","KeyW"],"pad":["up","-left_y"]},"menu_down":{"key":["ArrowDown","KeyS"],"pad":["down","left_y"]},"menu_left":{"key":["ArrowLeft","KeyA"],"pad":["left","-left_x"]},"menu_right":{"key":["ArrowRight","KeyD"],"pad":["right","left_x"]},"menu_accept":{"key":["Enter","Space"],"pad":["south"]},"menu_back":{"key":["Escape","Backspace"],"pad":["east"]},"menu_prev":{"key":["KeyQ"],"pad":["l1"]},"menu_next":{"key":["KeyE"],"pad":["r1"]},"pause":{"key":["Escape"],"pad":["start"]},"enter":{"key":["Enter","KeyF","KeyE"],"pad":["south"]},"swap":{"key":["KeyQ"],"pad":["west"]},"drop":{"key":["KeyG"],"pad":["east"]}},
    day: 3, tray: wanted.has("empty") ? [0, 0] : [4, 13.6], chest: [9, 41.2], afoot: wanted.has("afoot"), by_truck: false,
    chart: { file: "maps/the-crossroads.png", half: 512 }, at: { x: -417, z: 133, heading: 40, afoot: wanted.has("afoot") },
    places: [{ title: "Wombat Creek Track", kind: "trailhead", x: -252, z: -391 }, { title: "The Shed", kind: "shed", x: -430, z: 132 }],
    holding: wanted.has("hand") ? "nugget" : "detector", detector: wanted.has("hand") ? "slung" : "hand",
    money: 48250, buyer: wanted.has("sold")
      ? { count: 0, kilos: 0, pays: 0, rate: 2400, sold: { count: 4, kilos: 13.6, paid: 32640 } }
      : { count: wanted.has("empty") ? 0 : 4, kilos: wanted.has("empty") ? 0 : 13.6, pays: wanted.has("empty") ? 0 : 32640, rate: 2400, sold: null },
    signal: wanted.has("afoot") ? 0.93 : null, use: wanted.has("afoot") ? "dig" : null, hand: wanted.has("hand") ? 2.4 : null,
    hub: !wanted.has("map"), map: wanted.has("map") ? wombat : { name: "crossroads", title: "The Crossroads" },
    offer: wanted.has("unbuilt") ? { name: "granite", title: "Granite Ridge", built: false } : wombat,
    times: ["Dawn", "Day", "Dusk", "Night"], weathers: ["Clear", "Fair", "Overcast", "Storm"], time: 1, weather: 1,
    hud: { speed: 37, gear: "2", rpm: 3100, limit: 6800, abs: false, traction: true, four_wheel_drive: true, drive: "4L",
      front_locked: false, rear_locked: true, descent: false, aired_down: true, pitch: 12, roll: -31 },
    lap: { count: 1, time: 83.412, last: 318.07, best: 312.482, valid: true, last_valid: true },
    near: { title: "Wombat Creek Track", distance: 240, bearing: 35, inside: wanted.has("inside"), hold: 0.6 },
  });
  // A real controller, plugged in: pressing anything on it is that controller in hand, as
  // the game would say, and its buttons are drawn; a key pressed is the keyboard again
  // (`pretend`). A stick only counts pushed past half way.
  const padPoll = () => {
    for (const pad of navigator.getGamepads ? navigator.getGamepads() : []) {
      if (!pad || !pad.connected) continue;
      if (!pad.buttons.some((b) => b.value > 0.5) && !pad.axes.slice(0, 4).some((v) => Math.abs(v) > 0.5)) continue;
      const kind = /054c|dualsense|dualshock|playstation|sony|ps[345]/i.test(pad.id) ? "playstation" : "xbox";
      if (!state.pad || padKind !== kind) window.ui.receive({ ...state, pad: true, pad_kind: kind, nav: [] });
    }
    requestAnimationFrame(padPoll);
  };
  requestAnimationFrame(padPoll);
}

say("hello");
