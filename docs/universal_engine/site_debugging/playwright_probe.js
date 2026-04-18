import { chromium } from 'playwright';

const url = process.argv[2] || 'https://www.canadagoose.com/';
console.log(`[probe] target: ${url}`);

const browser = await chromium.launch({
  headless: true,
  args: ['--no-sandbox', '--disable-blink-features=AutomationControlled'],
});
const ctx = await browser.newContext({
  userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36',
  viewport: { width: 1920, height: 1080 },
});
const page = await ctx.newPage();

let firstStatus = null, finalStatus = null;
page.on('response', (resp) => {
  if (resp.url() === url && firstStatus === null) firstStatus = resp.status();
});

const t0 = Date.now();
try {
  const resp = await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 60000 });
  finalStatus = resp ? resp.status() : null;
  console.log(`[probe] goto returned status=${finalStatus} after ${Date.now()-t0}ms`);
  // Wait for JS challenge to settle
  await page.waitForTimeout(15000);
  const content = await page.content();
  const title = await page.title();
  const currentUrl = page.url();
  console.log(`[probe] elapsed: ${Date.now()-t0}ms`);
  console.log(`[probe] first_status: ${firstStatus}`);
  console.log(`[probe] final_url: ${currentUrl}`);
  console.log(`[probe] title: ${title}`);
  console.log(`[probe] content_len: ${content.length}`);
  const hasIps = content.includes('/ips.js') || content.includes('/149e9513-');
  const hasKP = content.includes('KPSDK');
  console.log(`[probe] has_ips.js_marker: ${hasIps}`);
  console.log(`[probe] has_KPSDK_marker: ${hasKP}`);
  console.log(`[probe] looks_like_real_homepage: ${!hasIps && content.length > 50000}`);
  const body = await page.evaluate(() => document.body ? document.body.innerText.substring(0, 300) : '');
  console.log(`[probe] body_first_300: ${JSON.stringify(body)}`);
} catch (e) {
  console.log(`[probe] ERROR: ${e.message}`);
}
await browser.close();
