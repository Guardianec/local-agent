'use strict';
// App-owned code only. Model data is never evaluated as JavaScript or a selector program.
const { chromium } = require('playwright-core');
const net = require('node:net');
const fs = require('node:fs');
const readline = require('node:readline');

if (process.argv[2] === '--where') { process.stdout.write(chromium.executablePath()); process.exit(0); }
const plan = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const socket = net.createConnection(plan.pipe);
socket.setEncoding('utf8');
const pending = new Map(); let serial = 0;
const lines = readline.createInterface({ input: socket, crlfDelay: Infinity });
lines.on('line', line => { if (line.length > 1024 * 1024) process.exit(2); const result = JSON.parse(line); const handler = pending.get(result.id); if (handler) { pending.delete(result.id); handler(result); } });
socket.on('close', () => process.exit(2));
socket.on('error', () => process.exit(2));
function broker(data) { return new Promise((resolve, reject) => { const id = ++serial; const timer = setTimeout(() => { pending.delete(id); reject(new Error('Request broker timeout')); }, 25000); pending.set(id, result => { clearTimeout(timer); resolve(result); }); const frame = JSON.stringify({ id, nonce: plan.nonce, ...data }); if (frame.length > 128 * 1024) return reject(new Error('Request exceeds frame limit')); socket.write(frame + '\n'); }); }

(async () => {
  await new Promise((resolve, reject) => { socket.once('connect', resolve); socket.once('error', reject); });
  const browser = await chromium.launch({ executablePath: plan.chromium, headless: true, chromiumSandbox: true, args: ['--disable-background-networking', '--disable-component-update', '--disable-sync', '--force-webrtc-ip-handling-policy=disable_non_proxied_udp'] });
  const context = await browser.newContext({ javaScriptEnabled: plan.spec.profile === 'local_test', serviceWorkers: 'block', acceptDownloads: false, permissions: [], viewport: { width: 1280, height: 800 } });
  context.setDefaultTimeout(6000);
  await context.routeWebSocket('**/*', ws => ws.close());
  let redirect = null; let violations = [];
  await context.route('**/*', async route => {
    try {
      const request = route.request();
      if (!/^https?:\/\//i.test(request.url())) return route.abort();
      const response = await broker({ kind: 'fetch', url: request.url(), method: request.method(), body: request.postData(), headers: { 'content-type': request.headers()['content-type'] ?? '' }, document: request.isNavigationRequest() && request.frame() === request.frame().page().mainFrame() });
      if (response.redirect) { redirect = response.redirect; return route.abort(); }
      if (response.error) { violations.push(response.error); return route.abort(); }
      await route.fulfill({ status: response.status, headers: response.headers, body: Buffer.from(response.body, 'base64') });
    } catch (error) { violations.push(String(error.message)); await route.abort().catch(() => {}); }
  });
  let page = await context.newPage();
  function guardPage(page) {
    page.on('popup', other => void other.close());
    page.on('dialog', dialog => void dialog.dismiss());
    page.on('filechooser', () => { violations.push('File uploads are blocked'); });
    page.on('download', download => void download.cancel());
  }
  guardPage(page);
  let target = plan.spec.url;
  for (let hop = 0; hop <= 5; hop++) { redirect = null; await page.goto(target, { waitUntil: 'domcontentloaded', timeout: 25000 }).catch(error => { if (!redirect) throw error; }); if (!redirect) break; if (hop === 5) throw new Error('Redirect limit exceeded'); target = redirect; await page.close(); page = await context.newPage(); guardPage(page); }
  if (await page.locator('input[type="password"]').count()) throw new Error('Password or sign-in pages are outside this capability');
  const results = [];
  for (const step of plan.spec.steps) {
    if (plan.spec.profile !== 'local_test' && !['assert_text', 'assert_title'].includes(step.action)) throw new Error('Public pages are read-only');
    if (step.action === 'assert_text') { const found = page.getByText(step.value, { exact: false }).first(); await found.waitFor({ state: 'visible' }); }
    else if (step.action === 'assert_title') { if (!(await page.title()).includes(step.value)) throw new Error('Page title did not match'); }
    else {
      if (!['button', 'link', 'textbox', 'checkbox', 'combobox'].includes(step.role)) throw new Error('Unsupported control role');
      const element = page.getByRole(step.role, { name: step.name, exact: true });
      if (await element.count() !== 1) throw new Error('Target must match exactly one accessible control');
      if (await element.getAttribute('type') === 'password' || await element.getAttribute('type') === 'file') throw new Error('Sensitive controls are blocked');
      if (step.action === 'click') {
        const href = await element.getAttribute('href');
        if (href !== null) {
          const destination = new URL(href, page.url());
          if (!['http:', 'https:'].includes(destination.protocol) || destination.origin !== new URL(plan.spec.url).origin || destination.username || destination.password) throw new Error('Link leaves the approved HTTP origin');
        }
        await element.click();
      }
      else if (step.action === 'fill') await element.fill(step.value);
      else if (step.action === 'check') await element.check();
      else if (step.action === 'select') await element.selectOption(step.value);
      else throw new Error('Unsupported browser action');
    }
    results.push({ action: step.action, name: step.name || '', status: 'passed' });
    if (await page.locator('input[type="password"]').count()) throw new Error('Authentication boundary reached');
  }
  const observation = await page.locator('body').innerText({ timeout: 5000 });
  const output = { status: 'passed', title: await page.title(), url: page.url(), text: observation.slice(0, 12000), steps: results, blockedRequests: violations.slice(0, 25) };
  await context.close(); await browser.close();
  process.stdout.write(JSON.stringify(output)); socket.destroy(); process.exit(0);
})().catch(error => { const message = String(error.message); process.stdout.write(JSON.stringify({ status: 'failed', message: message.length > 5000 ? message.slice(0, 500) + '\n[launch log omitted]\n' + message.slice(-4500) : message })); socket.destroy(); process.exit(1); });
