'use strict';
const http = require('node:http');
const fs = require('node:fs');
const { buildReport } = require('./report.cjs');
const server = http.createServer((req, res) => {
  if (req.url === '/api/report') {
    try {
      const report = buildReport({ state: 'ready', items: ['alpha', 'beta', 'gamma'] });
      res.writeHead(200, { 'content-type': 'application/json' });
      res.end(JSON.stringify(report));
    } catch (error) {
      res.writeHead(500, { 'content-type': 'application/json' });
      res.end(JSON.stringify({ error: error.message }));
    }
  } else if (req.url === '/' || req.url === '/help') {
    res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    res.end(fs.readFileSync('index.html'));
  } else { res.writeHead(404); res.end('Not found'); }
});
server.listen(Number(process.env.PORT || 3000), process.env.HOST || '127.0.0.1');
