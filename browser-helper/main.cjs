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
let commandHandler;
const queuedCommands = [];
const lines = readline.createInterface({ input: socket, crlfDelay: Infinity });
lines.on('line', line => { if (line.length > 1024 * 1024) process.exit(2); const result = JSON.parse(line); if (result.command) { if (commandHandler) commandHandler(result.command); else if (queuedCommands.length < 1) queuedCommands.push(result.command); else process.exit(2); return; } const handler = pending.get(result.id); if (handler) { pending.delete(result.id); handler(result); } });
socket.on('close', () => process.exit(2));
socket.on('error', () => process.exit(2));
function broker(data) { return new Promise((resolve, reject) => { const id = ++serial; const timer = setTimeout(() => { pending.delete(id); reject(new Error('Request broker timeout')); }, 25000); pending.set(id, result => { clearTimeout(timer); resolve(result); }); const frame = JSON.stringify({ id, nonce: plan.nonce, ...data }); if (frame.length > 128 * 1024) return reject(new Error('Request exceeds frame limit')); socket.write(frame + '\n'); }); }

(async () => {
  await new Promise((resolve, reject) => { socket.once('connect', resolve); socket.once('error', reject); });
  const browser = await chromium.launch({ executablePath: plan.chromium, headless: true, chromiumSandbox: true, args: ['--disable-background-networking', '--disable-component-update', '--disable-sync', '--force-webrtc-ip-handling-policy=disable_non_proxied_udp'] });
  const context = await browser.newContext({ javaScriptEnabled: ['local_test','local_read','public_web'].includes(plan.spec.profile), serviceWorkers: 'block', acceptDownloads: false, permissions: [], viewport: { width: 1280, height: 800 } });
  context.setDefaultTimeout(6000);
  await context.addInitScript(() => {
    for (const name of ['RTCPeerConnection','webkitRTCPeerConnection','WebTransport']) Object.defineProperty(globalThis,name,{value:undefined,writable:false,configurable:false});
  });
  await context.routeWebSocket('**/*', ws => ws.close());
  let redirect = null; let violations = [];
  await context.route('**/*', async route => {
    try {
      const request = route.request();
      if (!/^https?:\/\//i.test(request.url())) return route.abort();
      const response = await broker({ kind: 'fetch', url: request.url(), method: request.method(), body: request.postData(), headers: { 'content-type': request.headers()['content-type'] ?? '' }, document: request.isNavigationRequest(), mainDocument: request.isNavigationRequest() && request.frame() === request.frame().page().mainFrame() });
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
  if (plan.session) {
    await operateSession(page, context, () => violations, value => { redirect = value; }, () => redirect);
    return;
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

async function operateSession(page, context, violations, setRedirect, getRedirect) {
  function clip(text, maxBytes) { text=String(text); let bytes=0,end=0; for (const char of text) {const next=Buffer.byteLength(char);if(bytes+next>maxBytes)break;bytes+=next;end+=char.length;} return text.slice(0,end); }
  const sensitiveSelector = 'input[type="password"],input[autocomplete="current-password"],input[autocomplete="new-password"],input[name*="password" i],input[id*="password" i]';
  let generation = 0, references = new Map(), snapshotTree = '', snapshotUrl = '', actions = 0, navigations = 0;
  const tabs = new Map([[1,page]]); let nextTab = 1, activeTab = 1;
  let openingExplicit = false, unexpectedPages = 0;
  context.on('page', other => { if (!openingExplicit) { void other.close(); if (++unexpectedPages > 8) socket.destroy(); } });
  let allowedOrigins = [new URL(plan.spec.url).origin];
  const consoleErrors = [], networkFailures = [];
  const add = (items, item) => { items.push(item); if (items.length > 12) items.shift(); };
  const cleanUrl = raw => { try { const url = new URL(raw); return clip(url.origin + url.pathname,400); } catch { return '[invalid URL]'; } };
  function observe(page) {
  page.on('framenavigated', frame => { if (frame === page.mainFrame() && ++navigations > 20) socket.destroy(); });
  page.on('popup', other => void other.close().catch(()=>{}));
  page.on('dialog', dialog => void dialog.dismiss().catch(()=>{}));
  page.on('filechooser', () => violations().push('File uploads are blocked'));
  page.on('download', download => void download.cancel().catch(()=>{}));
  page.on('pageerror', error => add(consoleErrors, String(error.message).slice(0, 300)));
  page.on('console', message => { if (['error', 'warning'].includes(message.type())) add(consoleErrors, message.text().slice(0, 300)); });
  page.on('response', response => { if (response.status() >= 400) add(networkFailures, {method: response.request().method(), url: cleanUrl(response.url()), status: response.status()}); });
  page.on('requestfailed', request => add(networkFailures, {method: request.method(), url: cleanUrl(request.url()), error: String(request.failure()?.errorText || 'failed').slice(0, 100)}));
  }
  observe(page);
  async function clearReferences() { for (const handle of references.values()) await handle.dispose(); references = new Map(); snapshotTree = ''; }
  async function guard() { if (await page.locator('input[type="file"]').count()) { await clearReferences(); throw new Error('SENSITIVE_PAGE: Upload controls are outside this capability'); } }
  async function pageTree() {
    if (!await page.locator(sensitiveSelector).count()) return page.locator('body').ariaSnapshot({timeout:5000});
    // Never serialize password controls through the accessibility value path.
    return page.evaluate(selector => {
      const visible = node => !!node.getClientRects().length && getComputedStyle(node).visibility !== 'hidden';
      const lines = [...document.querySelectorAll('h1,h2,h3')].filter(visible).slice(0,20).map(node=>'- heading '+JSON.stringify(node.innerText.slice(0,200)));
      for (const node of [...document.querySelectorAll('button,a[href],input,textarea,select,[role="button"],[role="link"],[role="textbox"],[role="checkbox"],[role="combobox"]')].slice(0,80)) {
        if (!visible(node) || node.matches(selector) || ['hidden','file'].includes(node.type)) continue;
        const role = node.getAttribute('role') || (node.tagName==='BUTTON'?'button':node.tagName==='A'?'link':node.tagName==='SELECT'?'combobox':node.type==='checkbox'?'checkbox':['INPUT','TEXTAREA'].includes(node.tagName)?'textbox':'');
        const name = node.getAttribute('aria-label') || [...(node.labels || [])].map(label=>label.innerText).join(' ') || node.innerText || node.getAttribute('placeholder') || '';
        if (role && name.length<=160) lines.push('- '+role+' '+JSON.stringify(name.trim()));
      }
      lines.push('- text: '+JSON.stringify(document.body.innerText.slice(0,6000)));
      return lines.join('\n');
    }, sensitiveSelector);
  }
  async function navigate(url) {
    const destination = new URL(url);
    if (!['http:', 'https:'].includes(destination.protocol) || !allowedOrigins.includes(destination.origin) || destination.username || destination.password) throw new Error('BROWSER_ORIGIN: Navigation leaves approved origins');
    if (url === page.url()) return;
    await clearReferences();
    for (let hop = 0; hop <= 5; hop++) {
      if (navigations >= 20) throw new Error('BROWSER_LIMIT: Navigation limit reached');
      setRedirect(null);
      await page.goto(url, {waitUntil: 'domcontentloaded', timeout: 25000}).catch(error => { if (!getRedirect()) throw error; });
      if (!getRedirect()) return;
      if (hop === 5) throw new Error('BROWSER_REDIRECT: Redirect limit reached');
      url = getRedirect();
    }
  }
  async function snapshot(read = false) {
    await guard();
    await clearReferences();
    generation++;
    snapshotTree = await pageTree();
    snapshotUrl = page.url();
    const elements = [];
    for (const line of snapshotTree.split('\n')) {
      const match = line.match(/^\s*- (button|link|textbox|checkbox|combobox) ("(?:[^"\\]|\\.)*")/);
      if (!match || elements.length >= 40) continue;
      let name; try { name = JSON.parse(match[2]); } catch { continue; }
      if (name.length > 160) continue;
      const locator = page.getByRole(match[1], {name, exact: true});
      if (await locator.count() !== 1 || !await locator.isVisible()) continue;
      const handle = await locator.elementHandle();
      if (!handle || ['password', 'file', 'hidden'].includes(await handle.getAttribute('type')) || await handle.evaluate((node,selector)=>node.matches(selector),sensitiveSelector)) continue;
      const reference = `${generation}:${elements.length + 1}`;
      references.set(reference, handle);
      elements.push({element: reference, role: match[1], name});
    }
    const main = page.locator('main:visible,article:visible,[role="main"]:visible').first();
    const readingRoot = await main.count() ? main : page.locator('body');
    const output = {status:'passed',session:plan.session,activeTab,tabCount:tabs.size,title:clip(await page.title(),200),url:cleanUrl(page.url()),text:clip(await readingRoot.innerText({timeout:5000}),read?7000:4000),structure:clip(snapshotTree,2500),elements,consoleErrors:consoleErrors.slice(-4).map(s=>clip(s,300)),networkFailures:networkFailures.slice(-4),blockedRequests:violations().slice(-4).map(s=>clip(s,200))};
    while (Buffer.byteLength(JSON.stringify(output))>22000) {
      if (output.text.length>1000) output.text=output.text.slice(0,Math.floor(output.text.length/2));
      else if (output.structure.length>1000) output.structure=output.structure.slice(0,Math.floor(output.structure.length/2));
      else if (elements.length>5) {const removed=elements.pop();await references.get(removed.element).dispose();references.delete(removed.element);}
      else if (output.consoleErrors.length>1) output.consoleErrors.shift();
      else if (output.networkFailures.length>1) output.networkFailures.shift();
      else break;
    }
    return output;
  }
  async function perform(command) {
    if (++actions > 30) throw new Error('BROWSER_LIMIT: Action limit reached');
    if (page.url() === 'about:blank') await navigate(plan.spec.url);
    const {action} = command;
    if (action === 'tabs') return {status:'passed', activeTab, tabs:await Promise.all([...tabs].map(async ([tab,p])=>({tab,url:cleanUrl(p.url()),title:(await p.title()).slice(0,200)})))};
    if (action === 'open_tab') {
      if (tabs.size >= 4) throw new Error('BROWSER_TABS: At most four tabs');
      await clearReferences(); openingExplicit=true;
      try {page=await context.newPage();} finally {openingExplicit=false;}
      activeTab=++nextTab; tabs.set(activeTab,page); observe(page);
      await navigate(command.url); return snapshot();
    }
    if (['switch_tab','close_tab'].includes(action)) {
      if (!tabs.has(command.tab) || (action==='close_tab' && tabs.size===1)) throw new Error('BROWSER_TAB: Select an existing tab; retain at least one');
      await clearReferences();
      if (action==='close_tab') { await tabs.get(command.tab).close(); tabs.delete(command.tab); if (activeTab===command.tab) activeTab=tabs.keys().next().value; }
      else activeTab=command.tab;
      page=tabs.get(activeTab); return snapshot();
    }
    if (action === 'screenshot') {
      await guard();
      const bytes = await page.screenshot({type:'jpeg', quality:45, fullPage:false, timeout:5000, mask:[page.locator(sensitiveSelector)],maskColor:'#111111'});
      if (bytes.length > 80000) throw new Error('BROWSER_IMAGE_LIMIT: Viewport image exceeds capture budget');
      return {status:'passed', session:plan.session, screenshot:bytes.toString('base64')};
    }
    if (['snapshot', 'read', 'diagnostics'].includes(action)) return snapshot(action==='read');
    if (action === 'navigate') await navigate(command.url);
    else if (['back', 'forward', 'reload'].includes(action)) {
      if (navigations >= 20) throw new Error('BROWSER_LIMIT: Navigation limit reached');
      await clearReferences();
      setRedirect(null);
      await page[action === 'back' ? 'goBack' : action === 'forward' ? 'goForward' : 'reload']({waitUntil: 'domcontentloaded', timeout: 25000}).catch(error => { if (!getRedirect()) throw error; });
      if (getRedirect()) await navigate(getRedirect());
    } else if (action === 'wait') {
      if (!Number.isInteger(command.milliseconds) || command.milliseconds < 0 || command.milliseconds > 5000) throw new Error('BROWSER_WAIT');
      await page.waitForTimeout(command.milliseconds);
    } else {
      await guard();
      if (await page.locator(sensitiveSelector).count()) throw new Error('AUTHENTICATION_BOUNDARY: Login pages can be observed with masked passwords; model interaction is blocked');
      if (plan.spec.profile !== 'local_test' && !(plan.spec.profile === 'public_read' && ['fill','select','check','prepare_submit'].includes(action))) throw new Error('BROWSER_EFFECT: Use explicit reviewed submission for public forms');
      const handle = references.get(command.element);
      if (!handle || snapshotUrl !== page.url() || snapshotTree !== await pageTree() || !await handle.evaluate(node => node.isConnected)) {
        await clearReferences();
        return {status:'stale_reference', message:'Page changed. Use references from this fresh snapshot.', fresh: await snapshot()};
      }
      if (['password','file','hidden'].includes(await handle.getAttribute('type'))) throw new Error('SENSITIVE_CONTROL');
      if (plan.spec.profile === 'public_read' && action !== 'prepare_submit') {
        const safe = await handle.evaluate((node, action) => {
          if (action === 'check') return node.tagName === 'INPUT' && node.type === 'checkbox';
          if (action === 'select') return node.tagName === 'SELECT';
          if (action === 'fill') return node.tagName === 'TEXTAREA' || (node.tagName === 'INPUT' && ['text','search','email','tel','url','number'].includes(node.type));
          return false;
        }, action);
        if (!safe) throw new Error('BROWSER_CONTROL: Public fields require ordinary non-submitting HTML controls');
      }
      if (action === 'prepare_submit') {
        if (plan.spec.profile !== 'public_read') throw new Error('BROWSER_FORM: Review submission is limited to static anonymous forms');
        const submission = await handle.evaluate(button => {
          const form = button.form;
          if (!form || !['BUTTON','INPUT'].includes(button.tagName) || !['submit','image'].includes(button.type) || button.type==='image') throw new Error('Select a submit button');
          if (['formaction','formmethod','formenctype','formtarget'].some(name=>button.hasAttribute(name)) || !['get','post'].includes(form.method) || form.enctype!=='application/x-www-form-urlencoded' || (form.target && form.target!=='_self')) throw new Error('Unsupported form encoding or override');
          const controls = [...form.elements];
          if (controls.length>20 || controls.some(e=>['password','file','hidden'].includes(e.type))) throw new Error('Sensitive, hidden or oversized forms are blocked');
          const fields = [...new FormData(form,button).entries()].map(([name,value])=>{if(typeof value!=='string'||name.length>120||value.length>2000)throw new Error('Unsupported form value');return {name,value};});
          return {url:form.action,method:form.method.toUpperCase(),fields};
        });
        return {status:'prepared', submission};
      } else if (action === 'click') {
        const href = await handle.getAttribute('href');
        if (href !== null) { const url = new URL(href,page.url()); if (!['http:','https:'].includes(url.protocol) || !allowedOrigins.includes(url.origin) || url.username || url.password) throw new Error('BROWSER_ORIGIN'); }
        await handle.click();
      } else if (action === 'fill') await handle.fill(command.value);
      else if (action === 'select') await handle.selectOption(command.value);
      else if (action === 'check') await handle.check();
      else if (action === 'press' && ['Tab','Escape','ArrowDown','ArrowUp','Enter','Space'].includes(command.key)) await handle.press(command.key);
      else throw new Error('BROWSER_ACTION');
    }
    if (plan.spec.profile === 'local_test') await page.waitForTimeout(150);
    return snapshot();
  }
  let busy = false;
  commandHandler = async command => {
    if (busy) { socket.destroy(); return; }
    busy = true;
    allowedOrigins = command.allowedOrigins;
    let result;
    try { result = await perform(command); }
    catch (error) { await clearReferences(); result = {status:'failed', message:String(error.message).slice(0, 1000)}; }
    const frame = JSON.stringify({nonce:plan.nonce, kind:'result', result});
    socket.write((Buffer.byteLength(frame) > (result.screenshot ? 120000 : 24000) ? JSON.stringify({nonce:plan.nonce,kind:'result',result:{status:'failed',message:'Observation exceeded byte budget'}}) : frame) + '\n');
    busy = false;
  };
  if (queuedCommands.length) commandHandler(queuedCommands.shift());
  await new Promise(() => {});
}
