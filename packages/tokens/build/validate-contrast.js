/** @file Validate theme colour contrast.
 * Ensures brand and accent colours in theme tokens meet WCAG AA contrast
 * against their paired text colour. This guards against inaccessible colour
 * combinations slipping into the design system.
 */
import fs from 'node:fs';

// Load the complete token tree once for reference resolution.
const tokensJson = JSON.parse(
  fs.readFileSync(new URL('../src/tokens.json', import.meta.url), 'utf8')
);

/** Resolve `{token.path}` references to concrete hex values. */
function getTokenValue(val) {
  const match = /^\{(.+)\}$/.exec(val);
  if (!match) return val;
  return match[1]
    .split('.')
    .reduce((obj, key) => {
      if (obj?.[key] == null) throw new Error(`Token "${match[1]}" not found`);
      return obj[key];
    }, tokensJson).value;
}

/** Calculate relative luminance for a hex colour. */
function luminance(hex) {
  let hexStr = hex.replace('#', '');
  if (hexStr.length === 3) {
    hexStr = hexStr.split('').map(c => c + c).join('');
  }
  if (!/^[0-9a-fA-F]{6}$/.test(hexStr)) return NaN;
  const [r, g, b] = [0, 2, 4]
    .map(i => parseInt(hexStr.slice(i, i + 2), 16) / 255)
    .map(c => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** Compute WCAG contrast ratio between two hex colours. */
function contrast(a, b) {
  const l1 = luminance(a);
  const l2 = luminance(b);
  const [lighter, darker] = l1 > l2 ? [l1, l2] : [l2, l1];
  return (lighter + 0.05) / (darker + 0.05);
}

function checkTheme(file, options = {}) {
  const json = JSON.parse(fs.readFileSync(file, 'utf8'));
  const brand = json.semantic?.brand;
  const accent = json.semantic?.accent;
  if (!brand || !accent) {
    console.error(`Missing brand/accent in ${file}`);
    return false;
  }
  const pairs = [
    ['brand default', brand.default?.value, brand.contrast?.value],
    ['brand hover', brand.hover?.value, brand.contrast?.value],
    ['accent default', accent.default?.value, accent.contrast?.value],
    ['accent hover', accent.hover?.value, accent.contrast?.value]
  ];
  const contrastThreshold =
    typeof options.contrastThreshold === 'number'
      ? options.contrastThreshold
      : 4.5;
  for (const [label, fg, bg] of pairs) {
    const fgHex = getTokenValue(fg);
    const bgHex = getTokenValue(bg);
    const ratio = contrast(fgHex, bgHex);
    if (ratio < contrastThreshold) {
      throw new Error(
        `${label} in ${file} fails contrast: ${ratio.toFixed(2)} (threshold: ${contrastThreshold})`
      );
    }
  }
  return true;
}

const themes = ['dark', 'light'];
const thresholdArg = parseFloat(process.argv[2]);
const options = Number.isNaN(thresholdArg) ? {} : { contrastThreshold: thresholdArg };
let hadError = false;

for (const theme of themes) {
  try {
    const ok = checkTheme(
      new URL(`../src/themes/${theme}.json`, import.meta.url),
      options
    );
    if (ok === false) hadError = true;
  } catch (err) {
    console.error(err instanceof Error ? err.message : err);
    hadError = true;
  }
}

if (hadError) {
  process.exit(1);
}

console.log('Contrast checks passed for themes.');
process.exit(0);
