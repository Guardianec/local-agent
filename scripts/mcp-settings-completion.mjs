// Operate only a Local Agent instance launched with disposable QA storage.
import { chromium, expect } from '@playwright/test';
import path from 'node:path';
async function run() {
const browser = await chromium.connectOverCDP('http://127.0.0.1:9223');
try {
  const page = browser.contexts()[0].pages()[0];
  await page.reload();
  await page.getByRole('button', { name: 'Settings', exact: true }).first().click();
  if (process.argv[2] === 'prepare-auth') {
    await page.getByRole('button', { name: 'Add GitHub', exact: true }).click();
    await expect(page.getByLabel('Protected API key or token (optional)', { exact: true })).toBeVisible();
    console.log('GitHub protected credential form ready in isolated QA app. No credential is read by this script.');
    return;
  }
  if (process.argv[2] === 'restart') {
    await expect(page.locator('.integration-card')).toContainText('Synthetic local tools');
    await expect(page.locator('.integration-card')).toContainText('tested and closed');
    await page.getByRole('button', { name: 'Test / reconnect', exact: true }).click();
    await expect(page.locator('.integration-card')).toContainText('Connection test passed', { timeout: 30000 });
    console.log('PASS: application restart preserves approved configuration and safe diagnostics; connection is lazy and reconnects');
    return;
  }
  const consent = () => page.getByRole('checkbox', { name: /I approve this/ });
  const save = () => page.getByRole('button', { name: 'Save integration', exact: true });
  const add = () => page.getByRole('button', { name: 'Add custom MCP', exact: true }).click();
  await add();
  await page.getByLabel('Server URL', { exact: true }).fill('http://example.com/mcp');
  await consent().check(); await save().click();
  await expect(page.getByRole('alert')).toContainText('Use HTTPS');
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await add(); await page.getByRole('combobox', { name: /Transport/ }).selectOption('stdio');
  await page.getByLabel('Installed executable (.exe)', { exact: true }).fill('C:/missing-fixture.exe');
  await page.getByLabel('Working directory', { exact: true }).fill(process.cwd());
  await consent().check(); await save().click();
  await expect(page.getByRole('alert')).toContainText('Choose an installed');
  await page.getByLabel('Name', { exact: true }).fill('Synthetic local tools');
  await page.getByLabel('Installed executable (.exe)', { exact: true }).fill(process.execPath);
  await page.getByLabel('Arguments (one per line; no secrets)', { exact: true }).fill(`${path.resolve('fixtures/mcp/server.cjs')}\nmodern`);
  await page.getByLabel('Credential environment variable (optional)', { exact: true }).fill('TEST_TOKEN');
  await page.getByLabel('Protected API key or token (optional)', { exact: true }).fill('synthetic-ui-canary-64298');
  await expect(save()).toBeDisabled(); await consent().check(); await save().click();
  await expect(page.locator('.integration-card')).toContainText('Protected credential saved');
  await page.getByRole('button', { name: 'Test / reconnect', exact: true }).click();
  await expect(page.locator('.integration-card')).toContainText('Connection test passed', { timeout: 30000 });
  if ((await page.locator('body').innerText()).includes('synthetic-ui-canary-64298')) throw Error('Credential appeared in visible result');
  await page.getByRole('button', { name: 'Configure', exact: true }).click();
  await page.getByLabel('Remove the saved credential when saving').check();
  await page.getByLabel('Credential environment variable (optional)', { exact: true }).fill('');
  await consent().check(); await save().click();
  await expect(page.locator('.integration-card')).toContainText('No credential');
  await page.getByRole('button', { name: 'Test / reconnect', exact: true }).click();
  await expect(page.locator('.integration-card')).toContainText('Connection test passed', { timeout: 30000 });
  console.log('PASS: invalid URL/executable, explicit command consent, protected credential setup/clearing, bounded stdio discovery');
} finally { await browser.close(); }
}
await run();
