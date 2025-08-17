#!/usr/bin/env node
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

/** Resolve `{token.path}` references to concrete hex values, following chains and detecting cycles. */
function getTokenValue(val) {
  let current = val;
  const seen = new Set();
  while (typeof current === 'string') {
    const m = /^\{(.+)\}$/.exec(current.trim());
    if (!m) return current;
    const key = m[1];
    if (seen.has(key)) throw new Error(`Circular token reference detected: "${key}"`);
    seen.add(key);
    const node = key
      .split('.')
      .reduce((obj, k) => {
        if (obj?.[k] == null) throw new Error(`Token "${key}" not found`);
        return obj[k];
      }, tokensJson);
    current = node?.value;
  }
  return current;
}

/** Calculate relative luminance for a hex colour. */
function luminance(hex) {
  if (typeof hex !== 'string') return NaN;
  let hexStr = hex.trim().replace('#', '');
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
  if (!Number.isFinite(l1) || !Number.isFinite(l2)) {
    throw new Error(`Invalid colour(s) for contrast: "${a}" vs "${b}"`);
  }
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
    const fileHint = file instanceof URL ? file.pathname : file;
    if (fg == null || bg == null) {
      throw new Error(`${label} in ${fileHint} is missing a value or contrast token`);
    }
    const fgHex = getTokenValue(fg);
    const bgHex = getTokenValue(bg);
    const ratio = contrast(fgHex, bgHex);
    if (ratio < contrastThreshold) {
      throw new Error(
        `${label} in ${fileHint} fails contrast: ${ratio.toFixed(2)} (threshold: ${contrastThreshold})`
      );
    }
  }
  return true;
}
const themesDir = new URL('../src/themes/', import.meta.url);
const themeFiles = fs
  .readdirSync(themesDir)
  .filter(f => f.endsWith('.json'))
  .map(f => new URL(f, themesDir));
const thresholdArg = parseFloat(process.argv[2]);
const options = Number.isNaN(thresholdArg) ? {} : { contrastThreshold: thresholdArg };
let hadError = false;

for (const file of themeFiles) {
  try {
    const ok = checkTheme(file, options);
    if (ok === false) hadError = true;
  } catch (err) {
    console.error(err instanceof Error ? err.message : err);
    hadError = true;
  }
}

if (hadError) {
  process.exit(1);
}

console.log(
  `Contrast checks passed for themes (threshold: ${options.contrastThreshold ?? 4.5}).`
);
process.exit(0);
