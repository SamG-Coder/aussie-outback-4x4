// A way in for files too big for wrangler to put in R2 itself: they come in parts, each part a
// request, and are put together in the bucket. Only for whoever has the key; deleted once the
// files are up.
export default {
  async fetch(request, env) {
    if (!env.UPLOAD_KEY || request.headers.get("x-upload-key") !== env.UPLOAD_KEY) {
      return new Response("not for you", { status: 403 });
    }
    const url = new URL(request.url);
    const key = url.searchParams.get("key");
    const id = url.searchParams.get("uploadId");
    try {
      switch (url.pathname) {
        case "/create": {
          const upload = await env.BUCKET.createMultipartUpload(key, {
            httpMetadata: { contentType: url.searchParams.get("type") || "application/octet-stream" },
          });
          return Response.json({ uploadId: upload.uploadId });
        }
        case "/part": {
          const upload = env.BUCKET.resumeMultipartUpload(key, id);
          const part = await upload.uploadPart(Number(url.searchParams.get("n")), request.body);
          return Response.json(part);
        }
        case "/complete": {
          const upload = env.BUCKET.resumeMultipartUpload(key, id);
          const object = await upload.complete(await request.json());
          return Response.json({ size: object.size, etag: object.httpEtag });
        }
        case "/abort": {
          await env.BUCKET.resumeMultipartUpload(key, id).abort();
          return Response.json({ aborted: true });
        }
      }
      return new Response("no such thing", { status: 404 });
    } catch (error) {
      return new Response(String(error && error.message || error), { status: 500 });
    }
  },
};
