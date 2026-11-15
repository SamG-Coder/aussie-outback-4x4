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
      ? "One lap from a rolling start: the clock starts as you cross the line. Beat a time for its medal and its purse, paid in gold into your tray. Being put back on the track costs the run its medal."
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
    setText("watch-key", state.pad ? "✕" : "Enter");
  }
  const result = state.result;
  if (result) {
    el("result-icon").setAttribute("href", `#class-${result.class + 1}`);
    setText("result-kicker", `Timed run  ·  Class ${result.class + 1}${run ? `  ·  ${run.name}` : ""}  ·  ${map.title || ""}`);
    el("result-outcome").className = `outcome ${result.medal || ""}`;
    setText("result-time", lapTime(result.time));
    setText("result-words", !result.valid ? "No time  ·  the truck was put back on the track"
      : [result.medal ? MEDAL_NAMES[result.medal] : "No medal", result.record ? "your best yet" : null].filter(Boolean).join("  ·  "));
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
  const labels = el("chart-labels"), places = state.places || [];
  const wanted = JSON.stringify(places);
  if (labels.dataset.wanted !== wanted) {
    labels.dataset.wanted = wanted;
    labels.replaceChildren(...places.map((place) => {
      const label = document.createElement("span"), [u, v] = where(place.x, place.z);
      label.textContent = place.title;
      label.style.left = `${u * 100}%`;
      label.style.top = `${v * 100}%`;
      return label;
    }));
  }
}

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
    : yard.page === "free" ? yard.free || [] : [];
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
  const card = yard.page === "colour" ? yard.chosen : cards.length ? cards[Math.min(yardPick, cards.length - 1)] : own;
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
  } else if (card && yard.page === "free") {
    detail = `${card.year}  ·  Grade ${card.grade}  ·  ${cash(card.price)}`;
  } else if (card) {
    detail = `${card.year}  ·  ${far(card.km)}  ·  the one you drive`;
  }
  el("yard-spec").innerHTML = lines.join("");
  // (Its colour being chosen, the colour in hand is named with it.)
  const tint = yard.page === "colour" ? (yard.colours || [])[yardPick] : null;
  setText("yard-name", card ? `${card.title}${tint ? `  ·  ${tint.name}` : ""}` : "");
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
function yardHeld(held) {
  if (!held || held.dataset.pick === undefined || state.screen !== "yard") return;
  const pick = Number(held.dataset.pick);
  // The row slides to put it in the middle.
  el("yard-lot").style.setProperty("--at", String(pick));
  if (pick !== yardPick) { yardPick = pick; yardActs(); }
  if (pick !== yardShown) {
    clearTimeout(yardTimer);
    // A truck in hand is stood in the showroom; a colour in hand is put on the one there.
    const tell = held.dataset.colour ? `do paint:${held.dataset.colour}` : `do show:${pick}`;
    yardTimer = setTimeout(() => { yardShown = pick; say(tell); }, held.dataset.colour ? 120 : 260);
  }
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
  if (fresh) { yardPick = 0; yardShown = yard.page === "colour" ? -1 : 0; }
  el("yard").classList.toggle("flat", !yard.showroom);
  const words = {
    free: ["Free drive", "Choose a truck", "Any of the game's trucks, as it is sold. Nothing is owed, and nothing is kept."],
    colour: ["Day one", "Choose its colour", "Any of these, at no cost. A respray later is paid for at Motor Spares."],
    start: ["Day one", "Choose your first truck", `You have bought a block in the high country with a shed on it, and you owe the bank ${cash(yard.block)} for it. You need a truck to work it. These three you can get on finance.`],
    garage: ["Your shed", "Garage", "The trucks you own. One is the one you drive."],
    shop: ["Ironbark Flat  ·  Motor Spares", "Auto shop", "A part you have bought is yours: taking it off and putting it back costs nothing."],
    used: ["Ironbark Flat  ·  The Hotel", "Used trucks", "What the publican has to sell today. It is different every day, and now and then there is a good one going cheap."],
    bank: ["Ironbark Flat  ·  The Bank", "Bank", `Every Friday ${cash(yard.repayment)} comes out. Pay it and your credit goes up; miss it and it goes down.`],
  }[yard.page] || ["", "", ""];
  setText("yard-kicker", words[0]); setText("yard-title", words[1]); setText("yard-about", words[2]);
  el("yard-facts").innerHTML = `<div><dt>In hand</dt><dd>${cash(yard.money)}</dd></div><div><dt>Owed</dt><dd class="owed">${cash(yard.page === "start" || yard.page === "colour" ? yard.block : yard.debt)}</dd></div>`
    + (yard.page === "start" || yard.page === "colour" || yard.page === "free" ? "" : `<div><dt>Credit</dt><dd>${yard.credit}</dd></div><div><dt>Fridays</dt><dd>${cash(yard.repayment)}</dd></div>`);
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
  for (const [index, card] of yardCards().entries()) {
    const dot = card.paint ? `<span class="dot" style="background:${card.paint}"></span>` : "";
    const tag = card.bargain && card.here ? '<span class="tag">Bargain</span>' : card.driving ? '<span class="tag">Driving</span>' : "";
    // Chosen where it stands: it is taken, taken out, bought or driven by pressing on it.
    const act = yard.page === "start" ? `choose:${card.name}` : yard.page === "free" ? `free:${card.name}`
      : yard.page === "garage" ? (card.driving ? null : `drive:${card.index}`) : yard.page === "used" && card.here ? `buy:${card.place}` : null;
    const made = button(`<img src="trucks/${card.name}_small.jpg" alt="${card.title}">${tag}${dot}`, act, card.here === false ? "gone" : "");
    made.dataset.pick = String(index);
    lot.append(made);
  }
  lot.style.setProperty("--at", String(Math.min(yardPick, Math.max(lot.children.length - 1, 0))));
  yardActs();
  // The shop's parts and paint, and the bank's counter, down the side.
  const rows = el("yard-rows");
  rows.replaceChildren();
  const row = (label) => {
    const nav = document.createElement("nav");
    nav.className = "part-row"; nav.dataset.row = "";
    nav.innerHTML = `<span>${label}</span>`;
    rows.append(nav);
    return nav;
  };
  if (yard.page === "shop") {
    const driven = (yard.trucks || []).find((truck) => truck.driving);
    for (const group of (yard.shop && yard.shop.groups) || []) {
      const nav = row(PARTS[group.group] || group.group);
      if (!ALWAYS.includes(group.group)) nav.append(button(`None<small>${group.bare ? "As it is" : "Take it off"}</small>`, `fit:${group.group}:none`, group.bare ? "fitted" : ""));
      for (const choice of group.choices) {
        const note = choice.fitted ? "Fitted" : choice.owned ? "Yours: fit it" : cash(choice.price);
        nav.append(button(`${KINDS[choice.choice] || choice.choice}<small>${note}</small>`, `fit:${group.group}:${choice.choice}`, choice.fitted ? "fitted" : ""));
      }
    }
    swatches(row(`Respray  ·  ${cash(yard.respray)}`), yard.colours, driven && driven.paint);
    const oil = driven && driven.oil > 0 ? `${far(driven.oil)} overdue` : driven ? `due in ${far(-driven.oil)}` : "";
    row("Oil").append(button(`Oil and filter<small>${cash(yard.oil_change)}, ${oil}</small>`, "oil"));
  }
  if (yard.page === "bank") {
    const borrow = row(`Borrow  ·  you can owe up to ${cash(yard.limit)}`);
    for (const amount of [1000, 5000, 20000]) borrow.append(button(`${cash(amount)}`, `borrow:${amount}`));
    const repay = row("Pay off");
    for (const amount of [1000, 5000, 20000]) repay.append(button(`${cash(amount)}`, `repay:${amount}`));
    repay.append(button("All I can", "repay:all"));
  }
  setText("yard-said", yard.said || "");
  // The other pages, and the way out.
  const tabs = el("yard-tabs");
  tabs.replaceChildren();
  if (yard.page === "free") {
    for (const grade of [1, 2, 3, 4, 5]) if (grade !== yard.grade) tabs.append(button(`Grade ${grade}`, `grade:${grade}`));
  }
  tabs.append(button(yard.from === "title" ? "Back" : yard.page === "garage" ? "Back to the shed" : "Leave", "leave"));
  // What was in hand is in hand still, if it is still there: on a new page, the first thing.
  if (fresh) hold(0, 0); else hold(held[0], held[1]);
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
function options() {
  const set = state.options || { volume: 10, fullscreen: false };
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
  for (const [index, choice] of [...el("option-screen").children].entries()) choice.classList.toggle("picked", (index === 1) === !!set.fullscreen);
}

function show() {
  const screen = screenShown();
  // Paused in a story, the yard is a button away.
  // In a browser there are no kept games to begin or go on with: it is a free drive.
  el("title-new").hidden = el("title-load").hidden = !!state.web;
  // Paused: a run is started again; a track is left for the Crossroads.
  el("pause-restart").hidden = !(state.mode === "practice" || state.mode === "timed");
  // (Only from a map with no trailhead home: where there is one, it is driven to.)
  el("pause-hub").hidden = state.way_home !== false;
  const kept = !!(state.story && state.story.on);
  el("pause-title").querySelector("small").textContent = kept ? "The game is kept as you play." : "A free drive is not kept.";
  el("shed-sleep").querySelector("small").textContent = kept ? "The day ends. The game is saved, and it is half past six tomorrow." : "The day ends, and it is half past six tomorrow.";
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
  setText("foot-key", state.pad ? "✕" : "E");
  setText("foot-hold", state.pad ? "Hold ✕" : "Hold E");
  setText("foot-swap-key", state.pad ? "□" : "Q");
  setText("foot-drop-key", state.pad ? "○" : "G");
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
  for (const hint of document.querySelectorAll("[data-hint]")) {
    const accept = hint.dataset.hint === "accept";
    hint.dataset.key = state.pad ? (accept ? "✕" : "○") : (accept ? "Enter" : "Esc");
  }
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
  setText("detector-tip", over ? "You are over it." : away === null ? "Keep walking: it hears gold within 60 metres."
    : "Turn on the spot: the beeps are fastest facing the gold. Walk that way.");
  const uses = { dig: "Dig", pick: "Pick up", tray: "Put in the tray", chest: "Put in the chest", untray: "Take from the tray",
    unchest: "Take from the chest", drop: "Put down", take: "Pick up the detector", open: "Open the door", shut: "Shut the door",
    door_shop: "Go in: Auto shop", door_used: "Ask about used trucks", door_bank: "Go in: Bank",
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
    setText("loading-tip", TIPS[Math.floor(Math.random() * TIPS.length)]);
    // (The showroom is the Crossroads: it has its picture.)
    setText("loading-kicker", map.name === "showroom" ? "Opening" : "Heading out to");
    art("loading-art", map.name === "showroom" ? "crossroads" : map.name);
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
    setText("banner-key", state.pad ? "✕" : "Enter");
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
    const way = { ArrowUp: "up", ArrowDown: "down", ArrowLeft: "left", ArrowRight: "right", Enter: "accept", Escape: "back" }[what];
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
    screen: wanted.get("screen") || "title", mode: wanted.get("mode") || "roam", pad: wanted.has("pad"), nav: [],
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
}

say("hello");
