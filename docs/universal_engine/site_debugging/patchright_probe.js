import { chromium } from 'patchright';

const url = process.argv[2] || 'https://www.canadagoose.com/';
console.log(`[patchright] target: ${url}`);

const browser = await chromium.launch({ headless: true });
const ctx = await browser.newContext({
  userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36',
  viewport: { width: 1920, height: 1080 },
});
const page = await ctx.newPage();

let firstStatus = null;
page.on('response', (resp) => {
  if (resp.url() === url && firstStatus === null) firstStatus = resp.status();
});

const t0 = Date.now();
try {
  const resp = await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 60000 });
  console.log(`[patchright] goto status=${resp?.status()} after ${Date.now()-t0}ms`);
  await page.waitForTimeout(20000);
  const content = await page.content();
  const title = await page.title();
  const currentUrl = page.url();
  const hasIps = content.includes('/ips.js') || content.includes('/149e9513-');
  console.log(`[patchright] first_status: ${firstStatus}`);
  console.log(`[patchright] final_url: ${currentUrl}`);
  console.log(`[patchright] title: ${title}`);
  console.log(`[patchright] content_len: ${content.length}`);
  console.log(`[patchright] has_ips.js_marker: ${hasIps}`);
  console.log(`[patchright] looks_like_real_homepage: ${!hasIps && content.length > 50000}`);
  const body = await page.evaluate(() => document.body ? document.body.innerText.substring(0, 200) : '');
  console.log(`[patchright] body: ${JSON.stringify(body)}`);
} catch (e) {
  console.log(`[patchright] ERROR: ${e.message}`);
}
await browser.close();
