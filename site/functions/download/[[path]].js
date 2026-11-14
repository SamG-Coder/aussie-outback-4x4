// /download/NAME: the game to download, from the bucket's download/ folder.
import { serve } from "../../r2.js";

export function onRequest({ request, env, params }) {
  const name = [].concat(params.path).join("/");
  return serve(request, env, `download/${name}`, {
    "cache-control": "public, max-age=3600",
    "content-disposition": `attachment; filename="${name.split("/").pop()}"`,
  });
}
