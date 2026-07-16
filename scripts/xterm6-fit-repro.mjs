import puppeteer from "puppeteer-core";
import { pathToFileURL } from "node:url";
import path from "node:path";

const browser = await puppeteer.launch({
  executablePath: "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  headless: "new",
  args: ["--no-sandbox", "--window-size=1600,1000", "--force-device-scale-factor=1.5"],
  defaultViewport: { width: 1600, height: 1000, deviceScaleFactor: 1.5 },
});
const page = await browser.newPage();
page.on("console", (m) => console.log("[page]", m.text()));
page.on("pageerror", (e) => console.log("[pageerror]", e.message));
const url = pathToFileURL(path.resolve("scripts/xterm6-fit-repro.html")).href;
await page.goto(url, { waitUntil: "load" });
const result = await page.evaluate(() => window.runTest());
console.log(result);
await browser.close();
