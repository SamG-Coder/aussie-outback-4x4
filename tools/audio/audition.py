"""Writes a page to listen to the game's sounds on: assets/sounds/audition.html, each sound
with a player, whether it loops, and the description it was made from.

    python tools/audio/audition.py

Open the page in a browser. A loop's player repeats, so its seam can be listened for.
"""
import html
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "assets" / "sounds"
LIST = Path(__file__).with_name("sounds.txt")

described = {}
for line in LIST.read_text(encoding="utf-8").splitlines():
    words = line.split("#")[0].split(None, 4)
    if len(words) == 5:
        described[words[0]] = (words[1], words[2], words[3], words[4])

rows = []
for path in sorted(OUT.glob("*.wav")):
    # A take is `name.2.wav`: it is described as its sound is.
    name = path.stem.split(".")[0]
    kind, seconds, seed, prompt = described.get(name, ("", "", "", "not in sounds.txt"))
    loop = " loop" if kind == "loop" else ""
    rows.append(
        f"<tr><td><b>{html.escape(path.stem)}</b><br><small>{kind} {seconds} s, seed {seed}</small></td>"
        f"<td><audio controls{loop} preload='none' src='{html.escape(path.name)}'></audio></td>"
        f"<td>{html.escape(prompt)}</td></tr>"
    )

page = f"""<!doctype html>
<meta charset="utf-8">
<title>Outback sounds</title>
<style>
  body {{ font: 15px/1.45 system-ui, sans-serif; margin: 24px auto; max-width: 1100px; padding: 0 16px;
         background: #1b1a17; color: #e8e2d6; }}
  h1 {{ font-size: 20px; }}
  table {{ border-collapse: collapse; width: 100%; }}
  td {{ border-top: 1px solid #3a372f; padding: 10px 8px; vertical-align: middle; }}
  small {{ color: #a39b8a; }}
  audio {{ width: 300px; }}
</style>
<h1>Outback sounds: {len(rows)} made</h1>
<p>Loops repeat, so listen for a seam. To change one, edit its seed or its words in
<code>tools/audio/sounds.txt</code> and run <code>generate.py --only NAME --redo</code>.</p>
<table>{''.join(rows)}</table>
"""
OUT.mkdir(parents=True, exist_ok=True)
(OUT / "audition.html").write_text(page, encoding="utf-8")
print(f"{OUT / 'audition.html'}: {len(rows)} sounds")
