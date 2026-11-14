// The game's sound in a browser (docs/sound.md).
//
// The game says each frame what should be heard: every loop's volume and pitch, and what to
// play once (`viewer.sound_asks()`). This plays it with the browser's own mixer, which runs
// away from the page and is not held up when a frame is slow: each loop is a source that goes
// round for good, with a volume that is eased to what is asked, and each one-off is a source
// that plays and is gone.
//
// A browser lets a page make no sound until whoever is at it has pressed or clicked
// something, so `wake()` is called on every key and click, and the sounds are fetched then.
// And it reads one map a page, so the page that plays is inside another that stays
// (`shell.html`), and the sound is that one's: it goes on from map to map, is fetched once,
// and needs waking once.

/// How quickly a loop's volume and pitch follow what is asked, in seconds: quick enough to
/// follow the revs, slow enough that nothing clicks.
const EASE = 0.05;

export function makeSound(base = "sounds/") {
  let context = null;
  let master = null;
  const buffers = new Map();
  const loops = new Map();

  // Every sound the site has, by name, fetched and unpacked a few at a time: the engine and
  // the bush first, which are heard at once.
  async function load() {
    const list = await (await fetch(`${base}sounds.json`)).json();
    const names = Object.keys(list).sort((a, b) => first(b) - first(a));
    let next = 0;
    const worker = async () => {
      while (next < names.length) {
        const name = names[next++];
        try {
          const packed = await (await fetch(base + list[name])).arrayBuffer();
          buffers.set(name, await context.decodeAudioData(packed));
        } catch (error) {
          console.warn(`sound ${name}: ${error.message ?? error}`);
        }
      }
    };
    await Promise.all([worker(), worker(), worker(), worker()]);
    console.info(`sound: ${buffers.size} sounds`);
  }
  const first = (name) => (name.startsWith("engine") ? 2 : name.startsWith("bush") ? 1 : 0);

  function wake() {
    if (!context) {
      context = new AudioContext({ latencyHint: "interactive" });
      master = context.createGain();
      master.gain.value = 0.8;
      // Loud moments are held down rather than clipped.
      const limiter = context.createDynamicsCompressor();
      limiter.threshold.value = -9;
      limiter.knee.value = 8;
      limiter.ratio.value = 12;
      limiter.attack.value = 0.003;
      limiter.release.value = 0.2;
      master.connect(limiter).connect(context.destination);
      load();
    }
    if (context.state === "suspended" && !document.hidden) context.resume();
  }

  // A loop, going round from somewhere of its own so that loops begun together don't beat.
  function begin(name) {
    const buffer = buffers.get(name);
    if (!buffer) return null;
    const source = context.createBufferSource();
    source.buffer = buffer;
    source.loop = true;
    const gain = context.createGain();
    gain.gain.value = 0;
    source.connect(gain).connect(master);
    source.start(0, (loops.size * 1.37) % buffer.duration);
    const loop = { source, gain };
    loops.set(name, loop);
    return loop;
  }

  /// Plays what the game asks: `asked` is `viewer.sound_asks()`, JSON, or nothing.
  function tick(asked) {
    if (!context || !asked || context.state !== "running") return;
    const { loops: looping, shots } = JSON.parse(asked);
    const now = context.currentTime;
    for (const [name, volume, pitch] of looping) {
      // One nobody has heard yet is not begun until it is to be heard.
      const loop = loops.get(name) ?? (volume > 0 ? begin(name) : null);
      if (!loop) continue;
      loop.gain.gain.setTargetAtTime(volume, now, EASE);
      loop.source.playbackRate.setTargetAtTime(pitch, now, EASE);
    }
    for (const [name, volume, pitch] of shots) {
      const buffer = buffers.get(name);
      if (!buffer) continue;
      const source = context.createBufferSource();
      source.buffer = buffer;
      source.playbackRate.value = pitch;
      const gain = context.createGain();
      gain.gain.value = volume;
      source.connect(gain).connect(master);
      source.start();
    }
  }

  /// Nothing is asked for any more, for now: a map is being left. Everything dies away.
  function hush() {
    if (!context) return;
    for (const loop of loops.values()) loop.gain.gain.setTargetAtTime(0, context.currentTime, 0.15);
  }

  // The page hidden, nothing is heard; shown again, it is.
  document.addEventListener("visibilitychange", () => {
    if (!context) return;
    if (document.hidden) context.suspend();
    else context.resume();
  });

  /// For trying it: whether it is running, how many sounds it has, and how many loops are going.
  const state = () => ({ running: context?.state ?? "asleep", sounds: buffers.size, loops: loops.size });

  return { wake, tick, hush, state };
}
