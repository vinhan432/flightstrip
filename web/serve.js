// A static file server for the replay page. No dependencies: `node web/serve.js`.
//
// It serves the repository root, because the page reads the fixtures from
// ../fixtures/ and must not keep its own copy of them. `/` maps to
// `/web/index.html` so the URL stays short.
'use strict';

const http = require('node:http');
const fs = require('node:fs');
const path = require('node:path');
const url = require('node:url');

const root = path.resolve(__dirname, '..');
const port = Number(process.env.PORT) || 4173;

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.ico': 'image/x-icon',
  '.md': 'text/markdown; charset=utf-8',
};

const server = http.createServer((request, response) => {
  let pathname;
  try {
    pathname = decodeURIComponent(url.parse(request.url).pathname || '/');
  } catch {
    response.writeHead(400).end('bad request');
    return;
  }

  if (pathname === '/') pathname = '/web/index.html';
  if (pathname.endsWith('/')) pathname += 'index.html';

  const file = path.join(root, pathname);
  // Never serve anything outside the repository.
  if (!file.startsWith(root + path.sep) && file !== root) {
    response.writeHead(403).end('forbidden');
    return;
  }

  fs.stat(file, (error, stats) => {
    if (error || !stats.isFile()) {
      response.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
      response.end(`not found: ${pathname}\n`);
      return;
    }
    response.writeHead(200, {
      'content-type': MIME[path.extname(file).toLowerCase()] || 'application/octet-stream',
      'cache-control': 'no-store',
    });
    fs.createReadStream(file).pipe(response);
  });
});

server.listen(port, () => {
  console.log(`Flightstrip replay on http://localhost:${port}/`);
  console.log(`serving ${root}`);
  console.log(`fixtures come from ${path.join(root, 'fixtures')}`);
});
