// Puppeteer Extra + Stealth Plugin benchmark
const puppeteer = require('puppeteer-extra');
const StealthPlugin = require('puppeteer-extra-plugin-stealth');
puppeteer.use(StealthPlugin());

const STEALTH_CHECKS = [
    ["webdriver", "typeof navigator.webdriver", "undefined"],
    ["chrome_obj", "typeof window.chrome", "object"],
    ["plugins", "navigator.plugins.length > 0", "true"],
    ["languages", "navigator.languages.length > 0", "true"],
    ["vendor", "navigator.vendor", "Google Inc."],
    ["platform", "typeof navigator.platform", "string"],
    ["hardwareConcurrency", "navigator.hardwareConcurrency > 0", "true"],
    ["ua_chrome", "/Chrome/.test(navigator.userAgent)", "true"],
    ["webrtc", "typeof RTCPeerConnection", "function"],
    ["fonts_api", "typeof document.fonts", "object"],
    ["permissions", "typeof navigator.permissions.query", "function"],
    ["battery", "typeof navigator.getBattery", "function"],
    ["speech_voices", "speechSynthesis.getVoices().length > 0", "true"],
    ["media_source", "typeof MediaSource.isTypeSupported", "function"],
    ["codec_h264", 'MediaSource.isTypeSupported(\'video/mp4; codecs="avc1.42E01E"\')', "true"],
    ["eventsource", "typeof EventSource", "function"],
    ["websocket", "typeof WebSocket", "function"],
    ["deviceMemory", "navigator.deviceMemory > 0", "true"],
];

(async () => {
    const start = Date.now();
    const browser = await puppeteer.launch({ headless: 'new', args: ['--no-sandbox'] });
    const startup = Date.now() - start;

    const page = await browser.newPage();
    await page.goto('about:blank');

    const results = { name: "Puppeteer+Stealth", stealth: {}, timing: {}, memory: {} };
    results.timing.startup = `${startup}ms`;

    // Stealth checks
    let passed = 0;
    for (const [name, js, expected] of STEALTH_CHECKS) {
        try {
            const result = String(await page.evaluate(js));
            const ok = result === expected;
            results.stealth[name] = ok ? "PASS" : `FAIL (${result})`;
            if (ok) passed++;
        } catch (e) {
            results.stealth[name] = `ERR (${e.message})`;
        }
    }
    results.stealth._score = `${passed}/${STEALTH_CHECKS.length}`;

    // Memory
    const proc = require('process');
    results.memory.rss_mb = `${Math.round(proc.memoryUsage().rss / 1024 / 1024)}`;

    // Page load
    const navStart = Date.now();
    await page.goto('https://example.com', { waitUntil: 'load', timeout: 10000 });
    results.timing['example.com'] = `${Date.now() - navStart}ms`;
    results.timing.title = await page.title();

    await browser.close();

    // Output JSON
    console.log(JSON.stringify(results, null, 2));
})();
