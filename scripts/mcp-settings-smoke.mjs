// Native candidate QA only. Launch with disposable LOCAL_AGENT_TEST_DATA first.
import { chromium, expect } from '@playwright/test';
const browser = await chromium.connectOverCDP('http://127.0.0.1:9223');
try {
  const page = browser.contexts()[0].pages()[0];
  await page.reload();
  await page.getByRole('button', { name: 'Settings', exact: true }).first().click();
  await page.getByRole('button', { name: 'Add Context7', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Save integration', exact: true })).toBeDisabled();
  await page.getByLabel('I approve this endpoint and sending its configured credential when connecting.').check();
  await page.getByRole('button', { name: 'Save integration', exact: true }).click();
  await expect(page.locator('.integration-card')).toContainText('Context7');
  await page.getByRole('button', { name: 'Test / reconnect', exact: true }).click();
  await expect(page.locator('.integration-card')).toContainText('Connection test passed', { timeout: 30000 });
  await page.getByRole('button', { name: 'Disable', exact: true }).click();
  await expect(page.locator('.integration-card')).toContainText('Disabled');
  await page.reload();
  await page.getByRole('button', { name: 'Settings', exact: true }).first().click();
  await expect(page.locator('.integration-card')).toContainText('Disabled');
  await page.getByRole('button', { name: 'Enable', exact: true }).click();
  await expect(page.locator('.integration-card')).toContainText('Enabled');
  await page.getByRole('button', { name: 'Remove', exact: true }).click();
  await expect(page.locator('.integration-card')).toHaveCount(0);
  console.log('PASS: native Context7 configuration, explicit consent, anonymous discovery/cleanup, disable/reload/enable/remove');
} finally { await browser.close(); }
