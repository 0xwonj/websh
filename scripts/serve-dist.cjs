const fs = require("node:fs");
const http = require("node:http");
const path = require("node:path");

const types = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".wasm": "application/wasm", ".json": "application/json", ".svg": "image/svg+xml", ".png": "image/png", ".jpg": "image/jpeg", ".woff2": "font/woff2", ".pdf": "application/pdf" };

function serve(directory, port, preview) {
  const root = fs.realpathSync(directory);
  if (!fs.existsSync(path.join(root, "index.html"))) throw new Error("Build the app distribution first.");
  return http.createServer((request, response) => {
    if (!["GET", "HEAD"].includes(request.method)) return response.writeHead(405).end();
    try {
      const pathname = decodeURIComponent(new URL(request.url, "http://localhost").pathname);
      let body, type;
      if (preview && pathname.startsWith("/_preview/")) {
        body = preview.responses.get(pathname.slice("/_preview".length));
        if (!body) return response.writeHead(404).end();
        type = types[path.extname(pathname)] || "application/octet-stream";
      } else {
        let file = path.resolve(root, `.${pathname}`);
        if (fs.statSync(file).isDirectory()) file = path.join(file, "index.html");
        file = fs.realpathSync(file);
        if (!file.startsWith(`${root}${path.sep}`)) return response.writeHead(403).end();
        body = fs.readFileSync(file);
        type = types[path.extname(file)] || "application/octet-stream";
        if (preview && file === path.join(root, "index.html")) {
          body = Buffer.from(body.toString().replace("<head>", `<head>${preview.adapter}`));
        }
      }
      response.writeHead(200, { "Content-Type": type, "Content-Length": body.length, "Cache-Control": "no-store" });
      response.end(request.method === "HEAD" ? undefined : body);
    } catch {
      response.writeHead(404).end();
    }
  }).listen(port, "127.0.0.1", () => console.log(`Serving ${root} at http://127.0.0.1:${port}`));
}
module.exports = { serve };
if (require.main === module) serve(process.argv[2] || "target/verify/dist", Number(process.argv[3] || 4173));
