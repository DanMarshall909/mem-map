import assert from 'node:assert/strict';

// A runtime gate, not a reliance on Playwright's current default launch mode.
export async function requireHeadless(browser) {
  const session = await browser.newBrowserCDPSession();
  try {
    const { product, userAgent } = await session.send('Browser.getVersion');
    // Branded Edge reports product=Edg even in headless mode; its engine UA
    // exposes HeadlessChrome. The product label alone is not the mode oracle.
    assert.match(userAgent, /HeadlessChrome\//, 'Automated browser must actually run headlessly');
    return { product, headless: true };
  } finally { await session.detach(); }
}

export async function launchHeadless(chromium) {
  const executablePath = process.env.MEMMAP_BROWSER_EXECUTABLE;
  const channel = process.env.MEMMAP_BROWSER_CHANNEL || (process.platform === 'win32' ? 'msedge' : 'chromium');
  const browser = await chromium.launch({ headless: true, ...(executablePath ? { executablePath } : { channel }) });
  try { await requireHeadless(browser); return browser; }
  catch (error) { await browser.close(); throw error; }
}
