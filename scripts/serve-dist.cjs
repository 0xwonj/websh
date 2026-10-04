const fs = require("node:fs");
const http = require("node:http");
const path = require("node:path");

const root = fs.realpathSync(process.argv[2] || "target/verify/dist");
const port = Number(process.argv[3] || 4173);
const types = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".wasm": "application/wasm", ".json": "application/json", ".svg": "image/svg+xml", ".png": "image/png", ".jpg": "image/jpeg", ".woff2": "font/woff2", ".pdf": "application/pdf" };

if (!fs.existsSync(path.join(root, "index.html"))) throw new Error("Build verification assets with just build-check first.");

http.createServer((request, response) => {
  if (request.method !== "GET" && request.method !== "HEAD") {
    response.writeHead(405).end();
    return;
  }
  try {
    const pathname = decodeURIComponent(new URL(request.url, "http://localhost").pathname);
    let file = path.resolve(root, `.${pathname}`);
    if (fs.statSync(file).isDirectory()) file = path.join(file, "index.html");
    file = fs.realpathSync(file);
    if (!file.startsWith(`${root}${path.sep}`)) {
      response.writeHead(403).end();
      return;
    }
    const stat = fs.statSync(file);
    if (!stat.isFile()) throw new Error("not a file");
    response.writeHead(200, { "Content-Type": types[path.extname(file)] || "application/octet-stream", "Content-Length": stat.size, "Cache-Control": "no-store" });
    if (request.method === "HEAD") response.end();
    else fs.createReadStream(file).on("error", () => response.destroy()).pipe(response);
  } catch {
    response.writeHead(404).end();
  }
}).listen(port, "127.0.0.1", () => console.log(`Serving ${root} at http://127.0.0.1:${port}`));
