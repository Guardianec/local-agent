import { chromium, expect } from '@playwright/test';
import { readFile, mkdir } from 'node:fs/promises';
const registry = JSON.parse(await readFile('assets/model-registry.json', 'utf8'));
const original = await readFile('.local-data/v03-vision/project/styles.css');
const screenshot = (await readFile('.local-data/v03-vision/screenshot.png')).toString('base64');
const browser = await chromium.connectOverCDP('http://127.0.0.1:9223');
const page = browser.contexts()[0].pages()[0];
const errors = []; page.on('pageerror', error => errors.push(error.message));
try {
  if (!process.argv.includes('--resume-review')) {
  await page.getByRole('button', { name: 'Auto routing', exact: true }).click();
  for (const spec of registry.tiers) await expect(page.getByLabel(spec.tier[0].toUpperCase() + spec.tier.slice(1) + ' model')).toHaveValue(spec.modelId);
  await expect(page.getByText('Strong uses GPU and system RAM;', { exact: false })).toBeVisible();
  await mkdir('.local-data/screenshots', { recursive: true });
  await page.screenshot({ path: '.local-data/screenshots/v03-models.png' });
  await page.getByRole('button', { name: 'Done', exact: true }).click();
  await page.getByRole('button', { name: 'New task', exact: true }).click();
  await page.getByLabel('Task model').selectOption('@profile/balanced');
  await page.getByLabel('Permission mode').selectOption('project');
  await page.evaluate(base64 => {
    const data = Uint8Array.from(atob(base64), c => c.charCodeAt(0));
    const transfer = new DataTransfer(); transfer.items.add(new File([data], 'screenshot.png', { type: 'image/png' }));
    document.querySelector('textarea').dispatchEvent(new ClipboardEvent('paste', { bubbles: true, clipboardData: transfer }));
  }, screenshot);
  await expect(page.getByRole('button', { name: 'Remove screenshot.png' })).toBeVisible();
  await page.getByRole('button', { name: 'Remove screenshot.png' }).click();
  await expect(page.getByRole('button', { name: 'Remove screenshot.png' })).toHaveCount(0);
  await page.evaluate(base64 => {
    const transfer = new DataTransfer(); transfer.items.add(new File([Uint8Array.from(atob(base64), c => c.charCodeAt(0))], 'screenshot.png', { type: 'image/png' }));
    document.querySelector('.home-composer').dispatchEvent(new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer: transfer }));
  }, screenshot);
  await expect(page.getByRole('img', { name: 'screenshot.png', exact: true })).toBeVisible();
  await page.getByLabel('Task description').fill('Fix the layout issue shown in the screenshot. Inspect project files and change only styles.css so Save settings uses normal flow below the email field and does not cover it. Remove obsolete button offsets. Keep HTML, tests and package.json unchanged, and run the existing tests.');
  await expect(page.getByRole('button', { name: 'Start task' })).toBeEnabled();
  await page.screenshot({ path: '.local-data/screenshots/v03-image-composer.png' });
  await page.getByRole('button', { name: 'Start task' }).click();
  const end = Date.now() + 180000;
  let done = false;
  while (Date.now() < end) {
    const permission = page.getByRole('dialog');
    if (await permission.count()) {
      const title = await permission.locator('h2').textContent();
      if (!title?.includes("Run this project's code on your PC?")) throw Error('Unexpected approval: ' + title);
      await expect(permission).toContainText('test.cjs');
      await permission.getByRole('button', { name: 'Allow once', exact: true }).click();
    }
    if (await page.getByText('Ready for review', { exact: true }).count()) { done = true; break; }
    if (await page.getByText('Needs attention', { exact: true }).count() || await page.getByText('Handoff recommended', { exact: true }).count()) throw Error(await page.locator('.task-outcome').innerText());
    await page.waitForTimeout(250);
  }
  if (!done) throw Error('The real vision/coding task did not finish');
  }
  if (!(await readFile('.local-data/v03-vision/project/styles.css')).equals(original)) throw Error('Original changed before Apply');
  await page.getByRole('button', { name: /^Checks/ }).click();
  await expect(page.locator('.check-row').last()).toContainText('passed');
  await page.getByRole('button', { name: /^Changes/ }).click();
  await expect(page.locator('.changed-file-list')).toContainText('styles.css');
  await page.screenshot({ path: '.local-data/screenshots/v03-vision-review.png' });
  await page.getByRole('button', { name: 'Apply changes', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Apply changes', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Undo applied changes', exact: true })).toBeVisible();
  if ((await readFile('.local-data/v03-vision/project/styles.css')).equals(original)) throw Error('Apply did not update CSS');
  await page.getByRole('button', { name: 'Undo applied changes', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Undo changes', exact: true }).click();
  await expect.poll(async () => (await readFile('.local-data/v03-vision/project/styles.css')).equals(original)).toBe(true);
  if (errors.length) throw Error(errors.join('\n'));
  console.log('PASS: real native paste/drop/removal, local Balanced vision+coding, command approval, passed checks, review, Apply and exact Undo.');
} finally { await browser.close(); }
