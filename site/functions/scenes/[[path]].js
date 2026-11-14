// /scenes/NAME.solstice: a map, from the bucket's scenes/ folder. A map changes only when it
// is baked again, so a browser may keep what it has read of it for a day.
import { serve } from "../../r2.js";

export function onRequest({ request, env, params }) {
  const name = [].concat(params.path).join("/");
  // The lists that say what the maps are now (maps.txt, and each map's .chunks) are asked for
  // afresh every time: an old list beside a new map would be wrong.
  const list = name.endsWith(".chunks") || name.endsWith(".txt");
  return serve(request, env, `scenes/${name}`, { "cache-control": list ? "no-cache" : "public, max-age=86400" });
}
