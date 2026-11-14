// Serves a file from the site's R2 bucket: the game's maps, which are far too big to be files
// of the site itself, and its download. A map is read a part at a time, so a request for a
// range of bytes is answered with just that range.
export async function serve(request, env, key, extra = {}) {
  if (request.method !== "GET" && request.method !== "HEAD") {
    return new Response("GET or HEAD only", { status: 405, headers: { allow: "GET, HEAD" } });
  }
  const headers = new Headers({ "accept-ranges": "bytes", ...extra });
  if (request.method === "HEAD") {
    const found = await env.BUCKET.head(key);
    if (!found) return new Response("not found", { status: 404 });
    found.writeHttpMetadata(headers);
    headers.set("etag", found.httpEtag);
    headers.set("content-length", String(found.size));
    return new Response(null, { headers });
  }
  const object = await env.BUCKET.get(key, { range: request.headers, onlyIf: request.headers });
  if (!object) return new Response("not found", { status: 404 });
  object.writeHttpMetadata(headers);
  headers.set("etag", object.httpEtag);
  // Unchanged since the browser last had it.
  if (!("body" in object)) return new Response(null, { status: 304, headers });
  const whole = object.size;
  const range = object.range;
  if (request.headers.has("range") && range) {
    const offset = range.offset ?? Math.max(whole - (range.suffix ?? 0), 0);
    const length = range.length ?? whole - offset;
    headers.set("content-range", `bytes ${offset}-${offset + length - 1}/${whole}`);
    headers.set("content-length", String(length));
    return new Response(object.body, { status: 206, headers });
  }
  headers.set("content-length", String(whole));
  return new Response(object.body, { headers });
}
