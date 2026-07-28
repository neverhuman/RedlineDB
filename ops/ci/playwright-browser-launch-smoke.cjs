"use strict";

const { chromium } = require("playwright");

async function main() {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  await page.setContent("<main>sealed chromium</main>");
  const content = await page.textContent("main");
  await browser.close();
  if (content !== "sealed chromium") {
    throw new Error("Chromium did not return the expected page content");
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
