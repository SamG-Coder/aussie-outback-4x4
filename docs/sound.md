# Sound

How the game's sounds are made, and how they are played.

## Where the sounds come from

Every sound is made from a description by a text-to-audio model run on this machine: the bush
(wind in gum leaves, the creek, rain, cicadas), what a tyre rolls on (clay, gravel, mud, leaf
litter, rock, water), what happens once (a splash, brush snapping, a suspension bottoming
out), and the engine.

**The engine** is the one that has to follow the revs and the throttle from one moment to the
next, and a recording is one fixed performance. So it is made as five loops, each of the engine
held steady: an idle, three speeds above it, and one on the overrun. The game plays the two
nearest the revs it is at, each sped up or slowed to match, and fades between them; the
overrun loop comes in as the throttle shuts. A loop comes out at whatever revs it comes out
at, whatever its words ask for, so each is measured (`tools/audio/engine_pitch.py`: a V8's
firing note at F Hz is F x 15 rpm) and the revs heard are what `ENGINE_LAYERS` in
`apps/viewer/src/sound.rs` has; an `engine.txt` beside the sounds (`NAME RPM` a line,
`overrun NAME RPM`) overrides them without a rebuild. Several takes were made of each and the
ones with a clear, steady note kept. An engine synthesised from the firing of its
cylinders was tried first and is still there for when the loops are missing, but it does not
sound like one.

### Making them

`tools/audio/sounds.txt` lists every sound: its name, whether it loops, how long it is, and
the description it is made from. `tools/audio/generate.py` makes each one that is not made
yet, into `assets/sounds/NAME.wav` (48 kHz, mono, 16-bit), and:

- takes out the model's own whistles: it builds a sound fifty frames to the second and leaves
  faint steady tones at multiples of 50 Hz from about 4 kHz up, which in a quiet sound are
  heard as a buzz;
- fades a loop's tail into its head, so that it plays round with no seam;
- trims the silence from either end of a one-shot;
- sets each to an even loudness, so that the game does the mixing.

```bash
..\audio\venv\Scripts\python tools\audio\generate.py                    # whatever is missing
..\audio\venv\Scripts\python tools\audio\generate.py --only creek --takes 4   # four takes to choose from
..\audio\venv\Scripts\python tools\audio\generate.py --only creek --redo      # after changing its seed or its words
```

A sound that is not right is fixed in `sounds.txt`: another seed gives another take of the
same description, and plainer words usually give a plainer sound. The sounds themselves are
not kept in the repository (`assets/` is not); they are made again from the list.

### The model

[MOSS-SoundEffect v2.0](https://huggingface.co/OpenMOSS-Team/MOSS-SoundEffect-v2.0), from
OpenMOSS: Apache-2.0, not gated, 48 kHz mono, clips up to 30 seconds. It needs a CUDA GPU with
about 16 GB, Python 3.12, and 11 GB of disk for its weights. Set up beside the repository,
in `../audio/`:

```bash
py -3.12 -m venv ..\audio\venv
git clone --depth 1 https://github.com/OpenMOSS/MOSS-TTS.git ..\audio\MOSS-TTS
..\audio\venv\Scripts\python -m pip install --extra-index-url https://download.pytorch.org/whl/cu128 -e "..\audio\MOSS-TTS\moss_soundeffect_v2[torch-cu128]"
python -c "from huggingface_hub import snapshot_download; snapshot_download('OpenMOSS-Team/MOSS-SoundEffect-v2.0', local_dir='../audio/models/MOSS-SoundEffect-v2.0')"
```

Others looked at on 14 November 2026: Stable Audio Open 1.0 (stereo, 47 s, but gated, and
free commercially only under a revenue limit); Sony's Woosh, TangoFlux and Meta's AudioGen
(all with weights for non-commercial use only).

It all but fills a 16 GB card. With the game or a browser open beside it the model spills into
the machine's own memory and runs twenty times slower: close them first. A sound takes about
15 seconds to make when it has the card to itself.

## Playing them

`apps/viewer/src/sound.rs` is a mixer on the machine's audio output: voices that play a loop
or a shot at a volume and a pitch, each eased so that nothing clicks, and the engine's loops.
`Soundscape` in `apps/viewer/src/play.rs` asks it each frame for what should be heard, all of
it from what the game already knows:

- **The engine**, from its revs, the throttle and the limiter.
- **Tyres**, from each wheel (`HudWheel`: the surface it is on, how fast its tread passes over
  the ground, how hard it is pressed): the loop of its surface, louder and higher the faster
  it rolls; a spin loop as it slips; water in a ford.
- **What happens once**: a splash on entering water, brush snapping as scrub is flattened, a
  thud when a wheel reaches its bump stop or drops off a ledge.
- **The bush**: beds chosen by the weather the renderer already has (rain, wind speed).

### In a browser

The same `Soundscape` asks the same things, but nothing is mixed in the game: what it asks is
written down (`apps/viewer/src/sound_web.rs`) and the page plays it with the browser's own
mixer (`apps/viewer/web/sound.js`), which runs away from the page and is not held up by a slow
frame. Each loop is a source that goes round for good with its volume and its speed eased to
what is asked; the engine is its loops, weighted and pitched by the same table as the
desktop's (`sound_table.rs`).

The sounds go to the web squeezed (`scripts/build-game-web.sh`): 33 MB as they are made, 4.6 MB
on the site. The engine's loops are FLAC, exact, since a steady note shows any flaw where a
loop comes round; the rest are Opus.

Still to do: the creek by how near a waterhole is, cicadas by the time of day, rain on the
cab's roof, and thunder at the delay the renderer already works out.
