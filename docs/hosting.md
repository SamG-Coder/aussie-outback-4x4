# Hosting the game: rust4x4.com

The game's site is on Cloudflare, in two parts.

| Part | What is in it | Where it comes from |
| --- | --- | --- |
| **Pages** project `rust4x4` | The home page, and under `/play/` the browser version: the viewer's program, the game's pages, its pictures. About 13 MB. | `dist/`, built by `scripts/build-game-web.sh --hosted` |
| **R2** bucket `rust4x4-assets` | `scenes/NAME.solstice`, the five maps with only what is their own in them (the town 65 MB, the rest 189 to 245 MB each), and `scenes/shared.solstice`, what they are all read with (263 MB): the truck, and the bush's plants, stone and textures. The browser, Windows and Android versions all read these same six files, 1.2 GB in all. `scenes/maps.txt`, which says what version each is; and `download/`, the Windows version (about 39 MB) and the Android app. | `assets/*.solstice` (`bake.bat`), and `scripts/package-release.py` |

A Pages site's files can be 25 MB at most, which is why the maps are not among them. They are
served from the same address all the same: `site/functions/` are Pages Functions that answer
`/scenes/...` and `/download/...` from the bucket (`site/r2.js`), a range of bytes at a time
when a range is asked for. So the browser version streams: a map needs about 35 MB to start,
and then the parts of it that come into view, which in thick bush is a good deal: some 250 MB
in the first half minute of a map never seen before.

**What it has read it keeps** (`apps/viewer/web/loader.js`). Beside each file in the bucket
is `NAME.solstice.chunks` (`tools/hosting/chunks.py`): the file cut into pieces of a megabyte
or two wherever its own bytes say to, each with its hash, and the browser keeps a piece by
its hash. So a second visit to a map downloads next to nothing. When every map had its own
copy of the truck, the trees and the textures, this was also what kept a browser from
fetching them again with each map (1267 MB of 1870 was all there was, counted once); now
they are in the shared file, once, and the six files have next to nothing in common left to
find: 1185 MB, and 1164 counted once (what is left is the shed's and the town's iron and
timber, which are the same textures). Measured here, from nothing, a half-minute drive on the hub
was 158 requests and 383 MB, the shared file's part of it among them.

The pieces are big on purpose: each is one request to the site's functions, which are counted
(below). A whole map is some 300 pieces, and pieces side by side come in one request.

**What the maps share is in a file of its own** (`assets/shared.solstice`, baked from
`scenes/shared.txt`): the truck and the detector its driver carries, and what every square of
bush is made of, its plants, grasses, boulders and fallen timber and the textures of its
ground. The Windows and Android games read it beside whichever map is played and join the two
(`solstice_asset::join`, `apps/viewer/src/shared.rs`), so a map's file has only its own land
in it, and where things stand: the four came to 1909 MB with everything in each, and are
857 MB beside a shared file of 263, with the town's 65 on top. `maps.txt` lists the shared file first, as a map, and it
is fetched as one. It is baked first, and the maps beside it (`bake.bat`): a map is read
with the shared file it was baked beside, so they all go up together.

The browser version does the same with the same files. Its page reads the shared file's
tables alongside the map's (`openScene` in `loader.js`, given `sharedUrl`), the viewer joins
the two (`WebViewer.create`), and a range of bytes it then asks for from a terabyte on
(`shared_file_start`) is read that far into the shared file: the two files answer as one
(`JoinedFile`).

The Windows version reads its maps from the same place and by lists of the same kind. Its
download is the program, its pages and its sounds, and a `game.txt` that says what the program
does when it is opened (`apps/viewer/src/game.rs`): what it is run with, and the sites its maps
are on. Opened, the game shows its window and its loading screen, and behind it fetches the
maps (`apps/viewer/src/fetch.rs`; by hand, `viewer --fetch-maps SITE...`), saying along the top
how far along each is. That reads `scenes/maps.txt` and, for each map it has not got at that
version, the map's `.chunks`. It puts the map together in its `assets`
folder a piece at a time: a piece that is in a map already there, another map or the last
version of this one, is copied from the disk, and only the rest are downloaded, with `curl`,
pieces side by side in one request, each checked against its hash. So a map baked again costs
its changed pieces, and a download that is stopped carries on where it stopped. (What the maps
share is no longer in each of them to be skipped: it is in the shared file, once.) A new program is a small download.

```text
https://rust4x4.com/                   the home page                      Pages
https://rust4x4.com/play/              the browser version                Pages
https://rust4x4.com/scenes/hub.solstice   a map, read in ranges           Pages Function -> R2
https://rust4x4.com/scenes/shared.solstice   what the maps share, likewise  Pages Function -> R2
https://rust4x4.com/download/AussieOutback4x4-windows.zip                 Pages Function -> R2
```

## Putting a new build up

Wrangler, Cloudflare's tool, does it. It is installed in `target/tools/wrangler`
(`npm install --prefix target/tools/wrangler wrangler`), and signed in with `wrangler login`.

```bash
# The site: the home page and the browser version.
bash scripts/build-game-web.sh --hosted
cd site && ../target/tools/wrangler/node_modules/.bin/wrangler pages deploy --branch main --commit-dirty=true
```

That is all a change to the pages, the menus or the browser's program needs. The build names
each script by the build (`loader.js?v=...`, in `scripts/build-game-web.sh`): Cloudflare has
browsers keep a `.js` file for four hours whatever `_headers` says, while a page is asked for
afresh each time, and without the names a browser that had been here ran the last build's
scripts with the new program. Old deployments can be deleted once a new one is up
(`wrangler pages deployment list`, `wrangler pages deployment delete ID --force`). A new Windows
build is packed and put in R2, which wrangler does itself, as it is small:

```bash
python scripts/package-release.py     # the zip, and maps.txt, in target/release-pack/
wrangler r2 object put rust4x4-assets/download/AussieOutback4x4-windows.zip --file target/release-pack/AussieOutback4x4-windows.zip --content-type application/zip --remote
# The Android app (docs/android.md), the same way.
wrangler r2 object put rust4x4-assets/download/AussieOutback4x4.apk --file target/android-pack/AussieOutback4x4.apk --content-type application/vnd.android.package-archive --remote
# The home page says how big each download is and names its build in its link: after either
# has changed, put the page up again. This does it without building the browser version.
bash scripts/build-home.sh
cd site && ../target/tools/wrangler/node_modules/.bin/wrangler pages deploy --branch main --commit-dirty=true
```

A map baked again has to go to R2 too, with the two lists that say what it is now: `maps.txt`,
which tells the Windows game to fetch it afresh, and its `.chunks`, which the browser reads it
by (a list that does not match its map is noticed, and the map is then read without it).
Wrangler puts nothing over 300 MB into R2. Each of the six files is now under that (the
largest is the shared file, 263 MB), so `wrangler r2 object put` should do for all of them;
none of these has been put up that way yet. A file that is over goes through a small Worker
instead, which takes it in parts and puts them together in the bucket
(`tools/hosting/uploader/`). It is put up when there is such a file to upload and taken down
after:

```bash
cd tools/hosting/uploader
python -c "import secrets, pathlib; pathlib.Path('.key').write_text(secrets.token_urlsafe(40))"
wrangler deploy                      # at https://upload.rust4x4.com, shut until it has a key
wrangler secret put UPLOAD_KEY < .key

python upload.py ../../../assets/hub.solstice=scenes/hub.solstice
wrangler delete                      # and the Worker is gone

# Or, for a file under 300 MB, wrangler itself: what the maps share, and each map.
wrangler r2 object put rust4x4-assets/scenes/shared.solstice --file ../../../assets/shared.solstice --remote

python ../../../scripts/package-release.py
wrangler r2 object put rust4x4-assets/scenes/maps.txt --file ../../../target/release-pack/maps.txt --content-type "text/plain; charset=utf-8" --remote

# Every map's pieces, all together so that what they share is cut alike. Needs numpy.
python ../chunks.py ../../../assets/shared.solstice ../../../assets/hub.solstice ../../../assets/outback.solstice ../../../assets/granite.solstice ../../../assets/blackwood.solstice
wrangler r2 object put rust4x4-assets/scenes/hub.solstice.chunks --file ../../../assets/hub.solstice.chunks --content-type "text/plain; charset=utf-8" --remote
# ...and the same for shared, outback, granite and blackwood.
```

`.key` is the Worker's password: made here, never shown, and not kept in the repository.

## What it costs to run

- **Pages**: free. Its files are served without counting against anything.
- **R2**: storage of about 1.2 GB, within the 10 GB that is free, and no charge for what is
  downloaded.
- **The functions**: every range of a map that is read, and every download, is one request to
  a Pages Function, and the free plan allows 100,000 of those a day across the account. A
  player new to a map makes about 180 in the first half minute and 300 at the very most; one
  who has been there before, hardly any. The Windows game fetching all four maps makes about
  sixty, in pieces of up to 32 MB. If that is ever not enough, give the bucket
  an address of its own (R2, the bucket, Settings, Custom Domains: `assets.rust4x4.com`), allow
  the site to read it (a CORS rule for `https://rust4x4.com` with the `Range` header), and say
  so in `scripts/build-game-web.sh`: `<meta name="solstice-scenes" content="https://assets.rust4x4.com/scenes/">`.
  Then maps are read straight from R2, and counted nowhere.

## The licence

The truck's model is CC BY-NC-SA 4.0, so the site is free and carries no ads, and the home
page and the download's `CREDITS.txt` say whose the model is and that the adapted model is
shared alike. Keep both if either is changed.
