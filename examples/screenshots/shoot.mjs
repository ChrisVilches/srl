// Takes the screenshots with Firefox, driven by Puppeteer over WebDriver
// BiDi, so no browser is downloaded. Tried on Linux only.
//
//   node shoot.mjs tui ANSI OUT   draws each ANSI/*.ansi (from capture-tui.sh)
//                                 in tui.html and saves OUT/NAME.png
//   node shoot.mjs web OUT        studies, browses and adds in srl-web, which
//                                 must already be running on PORT
//
// Set FIREFOX to the Firefox program if it isn't /usr/bin/firefox.
import puppeteer from 'puppeteer-core';
import fs from 'node:fs';
import path from 'node:path';

const here = path.dirname(new URL(import.meta.url).pathname);
const firefox = process.env.FIREFOX ?? '/usr/bin/firefox';
const PORT = 4417;
// The color schemes to take the web shots in: 'dark', 'light' or both.
const SCHEMES = ['dark'];
// Page widths in CSS pixels. Browse needs more room: below about 1000 px
// srl-web stacks its list above the card.
const WIDTH = 980;
const BROWSE_WIDTH = 1280;
// Columns of the terminal in capture-tui.sh.
const COLS = 108;

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// Firefox over BiDi can't emulate prefers-color-scheme, so each scheme gets
// a browser of its own with the content override pref (0 dark, 1 light).
const launch = (scheme) => puppeteer.launch({
  browser: 'firefox',
  executablePath: firefox,
  headless: true,
  extraPrefsFirefox: { 'layout.css.prefers-color-scheme.content-override': scheme === 'dark' ? 0 : 1 },
});

async function tui(ansiDir, out) {
  const browser = await launch('dark');
  const page = await browser.newPage();
  // Scale 1 with a 30 px font in tui.html: xterm's canvas renderer draws
  // twice too big when the page is zoomed with deviceScaleFactor.
  await page.setViewport({ width: 2800, height: 2400, deviceScaleFactor: 1 });
  for (const file of fs.readdirSync(ansiDir).filter((f) => f.endsWith('.ansi')).sort()) {
    // Blank rows at the bottom are left out, so the image ends with the keys.
    const text = fs.readFileSync(path.join(ansiDir, file), 'utf8').replace(/(\s*\n)+$/, '');
    const rows = text.split('\n').length;
    await page.goto('file://' + path.join(here, 'tui.html'));
    await page.evaluate((t, c, r) => window.render(t, c, r), text, COLS, rows);
    const frame = await page.$('#frame');
    const png = path.join(out, file.replace(/\.ansi$/, '.png'));
    await frame.screenshot({ path: png });
    console.log(png);
  }
  await browser.close();
}

async function web(out) {
  const url = `http://localhost:${PORT}/`;
  for (const scheme of SCHEMES) {
    const browser = await launch(scheme);
    const page = await browser.newPage();
    const size = (width) => page.setViewport({ width, height: 900, deviceScaleFactor: 2 });
    const shot = async (name) => {
      // Firefox's scrollbars stay light in the dark scheme.
      await page.addStyleTag({ content: '* { scrollbar-width: none !important; }' });
      await sleep(700);
      const png = path.join(out, `web-${name}-${scheme}.png`);
      await page.screenshot({ path: png, fullPage: true });
      console.log(png);
    };
    // Study the cards of a selector, typed into the page's selector box.
    const study = async (selector) => {
      await page.goto(url);
      await sleep(800);
      const input = await page.waitForSelector('input[placeholder^="selector, e.g."]');
      await input.click({ count: 3 });
      await input.type(selector);
      await page.keyboard.press('Enter');
      await sleep(900);
      await page.evaluate(() => document.activeElement.blur());
    };
    const reveal = () => page.keyboard.press(' ');

    await size(WIDTH);
    await study('in:grammar');
    await shot('1-study-question');
    await reveal();
    await shot('2-study-answer');

    for (const [name, selector] of [
      ['3-rust-answer', 'in:rust-ownership'],
      ['4-japanese-words', 'in:vocabulary'],
      ['5-chinese-tones', 'in:tones'],
      ['6-electrical', 'in:electrical'],
    ]) {
      await study(selector);
      await reveal();
      await shot(name);
    }

    await size(BROWSE_WIDTH);
    await page.goto(url + '#browse');
    await sleep(1200);
    const card = await page.waitForSelector('::-p-text(道頓堀で)');
    await card.click();
    await sleep(2500);
    await reveal();
    await shot('7-browse');

    await size(WIDTH);
    await page.goto(url + '#add');
    await sleep(1200);
    await shot('8-add');

    await browser.close();
  }
}

const [what, ...args] = process.argv.slice(2);
if (what === 'tui' && args.length === 2) {
  await tui(path.resolve(args[0]), path.resolve(args[1]));
} else if (what === 'web' && args.length === 1) {
  await web(path.resolve(args[0]));
} else {
  console.error('usage: node shoot.mjs tui ANSI OUT | node shoot.mjs web OUT');
  process.exit(2);
}
