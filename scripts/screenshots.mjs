// Regenerates docs/screenshots/ from the demo page (demo/). See "Updating the screenshots" in
// the README. Needs `npm run dev` running and playwright-core installed.
import { chromium } from "playwright-core";
import { mkdirSync, readFileSync, renameSync, rmSync } from "node:fs";

const out = "docs/screenshots";
mkdirSync(`${out}/raw`, { recursive: true });
// CHROME=/path/to/chrome overrides the browser Playwright installed.
const browser = await chromium.launch({ executablePath: process.env.CHROME });
const URL = "http://localhost:1420/demo/";

async function app(name, nav, scheme) {
  const page = await browser.newPage({ viewport: { width: 1000, height: 680 }, deviceScaleFactor: 2, colorScheme: scheme });
  await page.clock.setFixedTime(new Date("2026-11-18T10:24:00"));
  await page.goto(URL);
  await page.waitForSelector(".sidebar");
  if (nav) await page.getByRole("button", { name: nav, exact: true }).click();
  await page.waitForTimeout(400);
  await page.screenshot({ path: `${out}/raw/${name}.png` });
  await page.close();
}

async function pill(name, phase, scheme) {
  const page = await browser.newPage({ viewport: { width: 260, height: 48 }, deviceScaleFactor: 2, colorScheme: scheme });
  await page.goto(`${URL}?window=pill&phase=${phase}`);
  await page.waitForSelector(".pill");
  await page.waitForTimeout(1500);
  await page.screenshot({ path: `${out}/raw/${name}.png`, omitBackground: true });
  await page.close();
}

// Frame a raw capture as a macOS window on the brand gradient.
async function frame(name, scheme) {
  const img = readFileSync(`${out}/raw/${name}.png`).toString("base64");
  const dark = scheme === "dark";
  const page = await browser.newPage({ viewport: { width: 1160, height: 840 }, deviceScaleFactor: 2 });
  await page.setContent(`<!doctype html><html><body style="margin:0;width:1160px;height:840px;display:flex;align-items:center;justify-content:center;
    background:${dark ? "linear-gradient(160deg,#2b2970,#121133)" : "linear-gradient(160deg,#8f8df0,#4b47c4)"}">
    <div style="border-radius:12px;overflow:hidden;box-shadow:0 30px 70px rgba(10,8,60,.45),0 0 0 .5px rgba(0,0,0,.3);background:${dark ? "#2a2a2f" : "#ececf0"}">
      <div style="height:28px;display:flex;align-items:center;gap:8px;padding:0 12px;position:relative;font:600 13px -apple-system,sans-serif;color:${dark ? "#ccc" : "#444"}">
        <i style="width:12px;height:12px;border-radius:50%;background:#ff5f57"></i><i style="width:12px;height:12px;border-radius:50%;background:#febc2e"></i><i style="width:12px;height:12px;border-radius:50%;background:#28c840"></i>
        <span style="position:absolute;left:0;right:0;text-align:center">hams</span>
      </div>
      <img src="data:image/png;base64,${img}" style="display:block;width:1000px;height:680px">
    </div></body></html>`);
  await page.screenshot({ path: `${out}/${name}.png` });
  await page.close();
}

const shots = [
  ["home", null, "light"],
  ["home-dark", null, "dark"],
  ["insights", "Insights", "light"],
  ["settings", "Settings", "light"],
  ["dictionary", "Dictionary", "dark"],
];
for (const [name, nav, scheme] of shots) await app(name, nav, scheme);
for (const [name, , scheme] of shots) await frame(name, scheme);
await pill("pill", "recording");
await browser.close();
for (const [name] of shots) rmSync(`${out}/raw/${name}.png`);
for (const extra of ["pill"]) {
  rmSync(`${out}/${extra}.png`, { force: true });
  renameSync(`${out}/raw/${extra}.png`, `${out}/${extra}.png`);
}
rmSync(`${out}/raw`, { recursive: true });
