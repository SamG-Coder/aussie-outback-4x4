# Hosting the game: rust4x4.com

The game's site is on Cloudflare, in two parts.

| Part | What is in it | Where it comes from |
| --- | --- | --- |
| **Pages** project `rust4x4` | The home page, and under `/play/` the browser version: the viewer's program, the game's pages, its pictures. About 13 MB. | `dist/`, built by `scripts/build-game-web.sh --hosted` |
| **R2** bucket `rust4x4-assets` | `scenes/NAME.solstice`, the four maps (450 to 490 MB each); `scenes/maps.txt`, which says what version each is; and `download/AussieOutback4x4-windows.zip`, the Windows version (about 37 MB). | `assets/*.solstice`, and `scripts/package-release.py` |

A Pages site's files can be 25 MB at most, which is why the maps are not among them. They are
served from the same address all the same: `site/functions/` are Pages Functions that answer
`/scenes/...` and `/download/...` from the bucket (`site/r2.js`), a range of bytes at a time
when a range is asked for. So the browser version streams: a map needs about 35 MB to start,
and then the parts of it that come into view, which in thick bush is a good deal: some 250 MB
in the first half minute of a map never seen before.

**What it has read it keeps, and shares between maps** (`apps/viewer/web/loader.js`). Beside
each map in the bucket is `NAME.solstice.chunks` (`tools/hosting/chunks.py`): the map's file
cut into pieces of a megabyte or two wherever its own bytes say to, each with its hash. The
maps have much in common, the same truck and textures and trees, each file with its own copy;
cut this way the same bytes are the same pieces in every file, and the browser keeps a piece
by its hash. So a second visit to a map downloads next to nothing, and a first visit to
another map does not fetch again what an earlier map brought: of 1870 MB in the four maps,
1267 MB is all there is once what they share is counted once. Measured on the site, a
half-minute drive on the hub from nothing was 180 requests and 360 MB; then Wombat Creek, 153
and 248 MB; then the hub again, 9 and 13 MB.

The pieces are big on purpose: each is one request to the site's functions, which are counted
(below). A whole map is some 300 pieces, and pieces side by side come in one request.

The Windows version is the same maps, read from the same place and by the same lists. Its
download is the program, its pages and its sounds, and a `game.txt` that says what the program
does when it is opened (`apps/viewer/src/game.rs`): what it is run with, and the sites its maps
are on. Opened, the game shows its window and its loading screen, and behind it fetches the
maps (`apps/viewer/src/fetch.rs`; by hand, `viewer --fetch-maps SITE...`), saying along the top
how far along each is. That reads `scenes/maps.txt` and, for each map it has not got at that
version, the map's `.chunks`. It puts the map together in its `assets`
folder a piece at a time: a piece that is in a map already there, another map or the last
version of this one, is copied from the disk, and only the rest are downloaded, with `curl`,
pieces side by side in one request, each checked against its hash. So what the maps share is
downloaded once (1267 MB for the four, not 1870), a map baked again costs its changed pieces,
and a download that is stopped carries on where it stopped. A new program is a small download.

```text
https://rust4x4.com/                   the home page                      Pages
https://rust4x4.com/play/              the browser version                Pages
https://rust4x4.com/scenes/hub.solstice   a map, read in ranges           Pages Function -> R2
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

That is all a change to the pages, the menus or the browser's program needs. A new Windows
build is packed and put in R2, which wrangler does itself, as it is small:

```bash
python scripts/package-release.py     # the zip, and maps.txt, in target/release-pack/
wrangler r2 object put rust4x4-assets/download/AussieOutback4x4-windows.zip --file target/release-pack/AussieOutback4x4-windows.zip --content-type application/zip --remote
```

A map baked again has to go to R2 too, with the two lists that say what it is now: `maps.txt`,
which tells the Windows game to fetch it afresh, and its `.chunks`, which the browser reads it
by (a list that does not match its map is noticed, and the map is then read without it). A map is
too big for wrangler, which puts nothing over 300 MB into R2. It goes through a small Worker
instead, which takes a file in parts and puts them together in the bucket
(`tools/hosting/uploader/`). It is put up when there is a map to upload and taken down after:

```bash
cd tools/hosting/uploader
python -c "import secrets, pathlib; pathlib.Path('.key').write_text(secrets.token_urlsafe(40))"
wrangler deploy                      # at https://upload.rust4x4.com, shut until it has a key
wrangler secret put UPLOAD_KEY < .key

python upload.py ../../../assets/hub.solstice=scenes/hub.solstice
wrangler delete                      # and the Worker is gone

python ../../../scripts/package-release.py
wrangler r2 object put rust4x4-assets/scenes/maps.txt --file ../../../target/release-pack/maps.txt --content-type "text/plain; charset=utf-8" --remote

# Every map's pieces, all four together so that what they share is cut alike. Needs numpy.
python ../chunks.py ../../../assets/hub.solstice ../../../assets/outback.solstice ../../../assets/granite.solstice ../../../assets/blackwood.solstice
wrangler r2 object put rust4x4-assets/scenes/hub.solstice.chunks --file ../../../assets/hub.solstice.chunks --content-type "text/plain; charset=utf-8" --remote
# ...and the same for outback, granite and blackwood.
```

`.key` is the Worker's password: made here, never shown, and not kept in the repository.

## What it costs to run

- **Pages**: free. Its files are served without counting against anything.
- **R2**: storage of about 1.9 GB, within what is free, and no charge for what is downloaded.
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
