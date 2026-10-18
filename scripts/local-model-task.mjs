import { modelIds } from './model-registry.mjs';
import { chromium, expect } from '@playwright/test';
import fs from 'node:fs/promises';
const browser = await chromium.connectOverCDP('http://127.0.0.1:9223');
const page = browser.contexts()[0].pages()[0];
const original = '.local-data/ui-project/math.cjs';
const before = await fs.readFile(original, 'utf8');
await page.getByRole('button', { name: 'New task', exact: true }).click();
await page.getByLabel('Task description').fill('Fix addition in math.cjs and run the existing tests. Change only math.cjs.');
await page.getByLabel('Permission mode').selectOption('assisted');
await page.getByRole('button', { name: 'Start task', exact: false }).click();
let edits = 0, commands = 0;
const end = Date.now() + 120000;
while (Date.now() < end) {
  const state = await page.locator('.task-status').innerText();
  if (state.includes('Ready for review')) break;
  if (state.includes('Needs attention') || state.includes('Handoff recommended')) throw Error(await page.locator('.task-outcome').innerText());
  const dialog = page.getByRole('dialog');
  if (await dialog.count()) {
    const title = await dialog.getByRole('heading').innerText();
    if (title === 'Change this file in the task copy?') {
      await expect(dialog).toContainText('math.cjs');
      if (await fs.readFile(original, 'utf8') !== before) throw Error('Original changed before approval');
      edits++;
    } else if (title === "Run this project's code on your PC?") { commands++; }
    else throw Error('Unexpected approval: ' + title);
    await dialog.getByRole('button', { name: 'Allow once', exact: true }).click();
  }
  await page.waitForTimeout(150);
}
await expect(page.locator('.task-status')).toContainText('Ready for review');
if (!edits || !commands) throw Error('The real model task did not exercise both edit and host-code approvals');
if (await fs.readFile(original, 'utf8') !== before) throw Error('Original changed before Apply');
await page.getByRole('button', { name: 'Activity', exact: true }).click();
await expect(page.locator('.timeline')).toContainText('Using balanced');
await expect(page.locator('.timeline')).toContainText(modelIds.balanced);
await page.screenshot({ animations: 'disabled', path: '.local-data/screenshots/real-model-activity.png' });
await page.getByRole('button', { name: 'Checks 1', exact: true }).click();
await expect(page.locator('.check-row')).toContainText('passed');
await page.getByRole('button', { name: 'Changes 1', exact: true }).click();
await page.locator('.monaco-diff-editor').waitFor();
await page.screenshot({ animations: 'disabled', path: '.local-data/screenshots/real-model-diff.png' });
await page.getByRole('button', { name: 'Apply changes', exact: true }).click();
await page.getByRole('dialog').getByRole('button', { name: 'Apply changes', exact: true }).click();
await expect(page.getByRole('button', { name: 'Undo applied changes' })).toBeVisible();
if (!(await fs.readFile(original, 'utf8')).includes('a + b')) throw Error('Real model proposal did not reach the original file');
await page.getByRole('button', { name: 'Undo applied changes' }).click();
await page.getByRole('dialog').getByRole('button', { name: 'Undo changes', exact: true }).click();
await expect(page.locator('.review-footer')).toContainText('restored');
if (await fs.readFile(original, 'utf8') !== before) throw Error('Undo failed exact-byte restoration');
console.log('PASS: real Ollama model, automatic Balanced fallback, explicit edit/command approvals, passing tests, reviewed Apply and exact Undo');
await browser.close();
