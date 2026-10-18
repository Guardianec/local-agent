import { chromium, expect } from '@playwright/test';
import fs from 'node:fs/promises';

// Run only against the disposable verify_interaction history, never personal data.
const browser = await chromium.connectOverCDP('http://127.0.0.1:9223');
const page = browser.contexts()[0].pages()[0];
await page.reload();
await page.getByRole('button', { name: 'What is a Fenwick tree?', exact: true }).click();
await expect(page.locator('.task-outcome')).toContainText('Fenwick');
await expect(page.locator('.task-outcome')).not.toContainText('Prepared 0 file');
await expect(page.getByRole('button', { name: 'Copy diagnostics', exact: true })).toBeVisible();
await expect(page.getByRole('button', { name: 'Retry', exact: true })).toBeVisible();
await page.getByText('Technical details', { exact: true }).click();
await expect(page.locator('.task-outcome details')).toContainText('"intent": "converse"');
await expect(page.locator('.task-outcome details')).not.toContainText('What is a Fenwick');
await fs.mkdir('.local-data/screenshots', { recursive: true });
await page.screenshot({ path: '.local-data/screenshots/stabilization-conversation.png' });
await page.getByRole('button', { name: 'Check whether the tests pass.', exact: true }).click();
await expect(page.locator('.task-outcome')).toBeVisible();
await expect(page.locator('.task-status')).toContainText('Report ready');
await page.getByRole('button', { name: 'New task', exact: true }).click();
await expect(page.getByLabel('Interaction')).toHaveValue('task');
await page.getByLabel('Interaction').selectOption('ask');
await expect(page.getByLabel('Interaction')).toHaveValue('ask');
console.log('PASS: native conversation/report rendering, terminal actions, safe diagnostics and Auto/Ask controls');
await browser.close();
