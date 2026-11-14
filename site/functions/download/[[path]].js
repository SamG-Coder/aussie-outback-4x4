// /download/NAME: the game to download, from the bucket's download/ folder.
//
// Never kept as it is: `no-cache` has a browser, and Cloudflare's own cache in front of the
// site, ask each time whether what it has is still the file (it is told by the ETag, and is
// not sent the file again if so). Kept for an hour, as it once was, a new build put up was
// not the build that came down: the site's address went on handing out the old one. The home
// page's link names the build as well (`?v=`, scripts/build-game-web.sh), so that a copy kept
// anywhere under the bare name is not what the link asks for.
import { serve } from "../../r2.js";

export function onRequest({ request, env, params }) {
  const name = [].concat(params.path).join("/");
  return serve(request, env, `download/${name}`, {
    "cache-control": "no-cache",
    "content-disposition": `attachment; filename="${name.split("/").pop()}"`,
  });
}
