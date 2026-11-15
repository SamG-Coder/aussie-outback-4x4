// The game round the driving, in a browser (docs/ui.md): `index.html?world=outback.world&play`.
//
// The viewer's page draws the scene on its canvas. This lays the game's own pages over it, in
// a frame the size of the window, and passes between them and the game's shell, which is in
// the viewer: what the shell has to tell the pages, what the pages say back, and what the
// shell asks of the page, which is to go to another map. A browser reads one scene a page, so
// going to a map loads the page again with that map's scene, and the shell takes the game up
// from what the page before wrote down as it left (`ui_carry`, kept for the tab), or, with
// nothing written down, from what the address says: which map, what for, and that it is
// being driven.
//
// A game's trucks are not in its maps. Each is a pack beside them (`trucks/NAME.truck`), put
// together as the map is read: so before the scene is made, the game says which packs it is
// to be read with (`WebGame`), and they are fetched here (`trucks`).

import { makeSound } from "./sound.js";

// What a page wrote down of the game as it left for another map, for that page to take up.
const CARRIED = "aussie-outback-4x4.carried";

// The game's maps, as the world file names them: `map NAME SCENE TITLE`, and which is the hub.
function readWorld(text) {
  const world = { hub: "", maps: {} };
  for (const line of text.split("\n")) {
    const words = line.split("#")[0].trim().split(/\s+/);
    if (words[0] === "hub") world.hub = words[1];
    if (words[0] === "map" && words.length >= 3) {
      world.maps[words[1]] = { name: words[1], scene: words[2], title: words.slice(3).join(" ") || words[1] };
    }
    if (["kind", "length"].includes(words[0]) && world.maps[words[1]]) {
      world.maps[words[1]][words[0]] = words.slice(2).join(" ");
    }
  }
  return world;
}

// A scene's name, as the viewer's page is given it: `assets/hub.solstice` is `hub`.
const sceneName = (scene) => scene.split("/").pop().replace(/\.solstice$/, "");

/// Before anything is downloaded: reads the game's maps, and shows its loading screen over
/// the page. Returns what `attach` needs, and the scene to read if the address named none.
export async function prepare(params) {
  const text = await (await fetch(params.get("world"))).text();
  const world = readWorld(text);
  const map = world.maps[params.get("map") || world.hub] ?? { name: world.hub, title: "" };
  document.body.classList.add("game");
  const frame = document.createElement("iframe");
  frame.id = "ui";
  frame.src = "ui/index.html";
  frame.allow = "fullscreen";
  document.body.append(frame);
  await new Promise((loaded) => frame.addEventListener("load", loaded, { once: true }));
  const tell = (state) => frame.contentWindow.ui?.receive(state);
  tell({ screen: "loading", nav: [], hub: map.name === world.hub, map });
  // What is being downloaded, along the top of the game's pages: what it is, and how much of
  // it has come, in megabytes if that is known.
  const megabytes = (bytes) => `${(bytes / 1e6).toFixed(0)} MB`;
  const progress = (what, received, total) => frame.contentWindow.ui?.stream?.({
    what: what.replace(/…$/, ""),
    much: received === undefined ? "" : total ? `${megabytes(received)} of ${megabytes(total)}` : megabytes(received),
    share: total ? received / total : 0,
    busy: true,
    fades: false,
  });
  progress("Starting");
  return { text, world, map, frame, tell, progress, scene: sceneName(map.scene ?? "") };
}

/// Before the scene is made: the game, taken up where the page before left it, with the
/// packs of the trucks its scene is to be read with, fetched from `base` (where the maps
/// are). `wasm` is the viewer's module, and `game` what `prepare` gave.
export async function trucks(wasm, params, game, base) {
  // Only a page that was come to from another takes up what that one was doing: the game's
  // address by itself is its title.
  const carried = params.has("carried") ? sessionStorage.getItem(CARRIED) ?? "" : "";
  if (!params.has("carried")) sessionStorage.removeItem(CARRIED);
  const made = new wasm.WebGame(
    game.text,
    carried,
    params.get("map") ?? "",
    params.get("mode") ?? "roam",
    params.has("drive"),
  );
  const names = made.packs_wanted();
  let received = 0;
  await Promise.all(names.map(async (name) => {
    const got = await fetch(`${base}trucks/${name}.truck`);
    if (!got.ok) throw new Error(`The truck ${name} could not be fetched (${got.status}).`);
    const bytes = new Uint8Array(await got.arrayBuffer());
    received += bytes.length;
    game.progress(names.length > 1 ? "Fetching trucks" : "Fetching the truck", received);
    made.give_pack(name, bytes);
  }));
  return made;
}

/// Once the scene's game is running: starts the shell, and returns what to call each frame.
export function attach(viewer, params, game, file) {
  const { text, world, frame, tell } = game;
  // (The game the scene was made with is the one started: `trucks`.)
  viewer.ui_start(
    text,
    "",
    params.get("map") ?? "",
    params.get("mode") ?? "roam",
    params.has("drive"),
  );
  if (Number(params.get("soaked")) > 0) viewer.soak(Number(params.get("soaked")));
  // Time passes as it does: a minute in a minute. Driven on into the evening, it gets dark,
  // and the lamps come on.
  viewer.set_day_length(24 * 60 * 60);
  // The game's sound, which a browser lets be heard once something has been pressed: a key
  // or a click, in the game's pages or on this one. It is the page's that this one is in
  // (shell.html), where it goes on from map to map; or, with no such page, this one's own.
  const outer = window.parent !== window ? window.parent.outbackSound : null;
  const sound = outer ?? makeSound("sounds/");
  viewer.sound_start();
  addEventListener("keydown", sound.wake);
  addEventListener("pointerdown", sound.wake);
  // Leaving for another map, what this one asked for dies away.
  addEventListener("pagehide", sound.hush);
  // Already woken, on an earlier map: heard from the start.
  sound.wake();
  window.uiHost = {
    say(line) {
      if (line.startsWith("key ") || line.startsWith("do ") || line.startsWith("set ")) sound.wake();
      viewer.ui_say(line);
    },
  };
  // The pages have been waiting: tell them everything.
  viewer.ui_say("hello");
  frame.contentWindow.focus();

  // Another map: the same page, with its scene, and the game taken up in it. What the address
  // already says is kept, and what this page was given besides (`params`), such as which game
  // it is, so that the next page is this game too.
  function go(map, scene, mode, sky) {
    const next = new URLSearchParams(params);
    next.set("scene", sceneName(scene));
    next.set("map", map);
    next.set("mode", mode);
    next.set("drive", "");
    // What the game is doing goes with it: the next page takes it up.
    next.set("carried", "");
    for (const name of ["time", "weather", "soaked"]) next.delete(name);
    if (sky.length === 3) {
      const hours = Number(sky[0]);
      next.set("time", `${Math.floor(hours)}:${String(Math.round((hours % 1) * 60)).padStart(2, "0")}`);
      next.set("weather", sky[1]);
      next.set("soaked", sky[2]);
    }
    location.search = next.toString();
  }

  // While it is played a map goes on coming, a part at a time, as more of it comes into view.
  // Twice a second: whether anything is being downloaded now, how much has been, and how
  // much of the map is here. Said for as long as something is coming, and once more when it
  // has all come, which lets the line fade.
  let looked = performance.now();
  let quiet = 2;
  function streaming(now) {
    if (now - looked < 500) return;
    looked = now;
    const busy = file.reading > 0;
    quiet = busy ? 0 : quiet + 1;
    if (quiet > 1) return;
    const share = viewer.pages_installed() / Math.max(viewer.pages_total(), 1);
    // How much has been downloaded, and how much did not need to be: it was here already,
    // from an earlier visit or from another map.
    const megabytes = `${(file.bytesFetched / 1e6).toFixed(0)} MB`;
    const spared = file.bytesShared > 5e6 ? ` · ${(file.bytesShared / 1e6).toFixed(0)} MB already here` : "";
    frame.contentWindow.ui?.stream?.({
      what: busy ? "Streaming map" : "Map loaded",
      much: busy ? `${megabytes} · ${Math.round(share * 100)}%${spared}` : `${megabytes}${spared}`,
      share,
      busy,
    });
  }

  return {
    tick() {
      streaming(performance.now());
      const told = viewer.ui_told();
      if (told) tell(JSON.parse(told));
      for (const order of viewer.ui_orders()) {
        const [what, map, scene, mode, ...sky] = order.split(" ");
        if (what === "load") {
          try {
            sessionStorage.setItem(CARRIED, viewer.ui_carry());
          } catch (error) {
            console.warn(`the game could not be carried to the next map: ${error.message ?? error}`);
          }
          go(map, scene, mode, sky);
        }
        // A page cannot close itself: quitting goes back to the title, in the hub.
        if (what === "quit") {
          sessionStorage.removeItem(CARRIED);
          location.search = "";
        }
      }
      // (The game keeps itself, in the browser's storage, in the place of the one played.)
      viewer.ui_keep();
      sound.tick(viewer.sound_asks());
    },
  };
}
