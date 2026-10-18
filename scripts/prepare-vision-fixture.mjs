import { mkdir, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
process.env.PLAYWRIGHT_BROWSERS_PATH = resolve('.local-data/browsers');
const { chromium } = await import('@playwright/test');
const root = resolve('.local-data/v03-vision');
await mkdir(root + '/project', { recursive: true });
await mkdir(root + '/read-only-project', { recursive: true });
const code = String(10000 + Math.floor(Math.random() * 89999));
const html = `<!doctype html><meta charset="utf-8"><title>Settings fixture</title><link rel="stylesheet" href="styles.css"><main><h1>Account settings</h1><p>Update your contact email.</p><div class="error">SAVE FAILED · error ${code}</div><form><label for="email">Email address</label><input id="email" value="demo@example.test"><button class="save-button" type="button">Save settings</button></form><p class="hint">The button should sit below the email field.</p></main>`;
const css = 'body{font:18px "Segoe UI",sans-serif;background:#f3f5f8;color:#17202d;padding:40px}main{max-width:650px;background:white;padding:32px;border:1px solid #d3d8e0}h1{font-size:28px}.error{background:#fce4e6;color:#a31325;padding:16px;margin:24px 0;font-weight:600}form{position:relative;padding:16px 0}label{display:block;margin-bottom:8px}input{display:block;width:400px;padding:14px;font:inherit;border:1px solid #9fa9ba}.save-button{position:absolute;top:42px;left:18px;background:#225bc0;color:white;border:0;padding:15px 24px;font:inherit}.hint{color:#53627a;font-size:15px}';
const test = `const fs=require('node:fs'),assert=require('node:assert/strict');const css=fs.readFileSync('styles.css','utf8');const rule=css.match(/\\.save-button\\s*\\{([^}]+)\\}/)[1];assert(!/position\\s*:\\s*absolute/.test(rule),'Save button must use normal flow');assert(!/(?:^|;)\\s*(top|left)\\s*:/.test(rule),'Remove button offsets so the input stays clear');console.log('Layout checks passed');`;
for (const [name, content] of Object.entries({ 'index.html': html, 'styles.css': css, 'test.cjs': test, 'package.json': JSON.stringify({ name: 'local-agent-vision-fixture', version: '1.0.0', scripts: { test: 'node test.cjs' } }) })) await writeFile(root + '/project/' + name, content);
await writeFile(root + '/read-only-project/README.md', 'A disposable project for local image description. No image contents are stored in project files.');
await writeFile(root + '/expected.json', JSON.stringify({ code }));
const browser = await chromium.launch({ headless: true, channel: 'chromium' });
try { const page = await browser.newPage({ viewport: { width: 920, height: 600 }, deviceScaleFactor: 1 }); await page.goto('file:///' + (root + '/project/index.html').replaceAll('\\', '/')); await page.screenshot({ path: root + '/screenshot.png' }); } finally { await browser.close(); }
console.log('Prepared local screenshot and disposable layout project; expected code stored separately.');
