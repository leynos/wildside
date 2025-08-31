/** @file Build Style Dictionary outputs and derive framework presets.
 * Converts design token sources into CSS variables, Tailwind presets, and
 * daisyUI themes. Shared utilities live in `../build-utils` to keep scripts
 * small and focused.
 */
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import StyleDictionary from 'style-dictionary';
import { readJson } from '../build-utils/read-json.js';

const sd = new StyleDictionary({
  source: ['src/tokens.json', 'src/themes/*.json'],
  platforms: {
    css: {
      transformGroup: 'css',
      buildPath: 'dist/css/',
      files: [{ destination: 'variables.css', format: 'css/variables' }],
    },
    tailwind: {
      transformGroup: 'js',
      buildPath: 'dist/tw/',
      files: [{ destination: 'preset.js', format: 'javascript/module' }],
    },
    daisy: {
      transformGroup: 'js',
      buildPath: 'dist/daisy/',
      files: [{ destination: 'theme.js', format: 'javascript/module' }],
    },
  },
});

sd.buildAllPlatforms();

// Map tokens into Tailwind and DaisyUI presets
/**
 * Tokens source loaded from disk.
 *
 * @type {Record<string, unknown>}
 */
const tokens = readJson(new URL('../src/tokens.json', import.meta.url));

/**
 * Recursively strip `value` wrappers from tokens.
 *
 * @param {unknown} input - Token node to unwrap.
 * @returns {unknown} Unwrapped token tree.
 * @example
 * ```js
 * unwrap({ size: { sm: { value: '1rem' } } });
 * //=> { size: { sm: '1rem' } }
 * unwrap([{ value: '1px' }]);
 * //=> ['1px']
 * ```
 */
function unwrap(input) {
  if (input == null || typeof input !== 'object') {
    return input;
  }
  if (Array.isArray(input)) {
    return input.map(unwrap);
  }
  if ('value' in input) {
    return input.value;
  }
  return Object.fromEntries(
    Object.entries(input).map(([k, v]) => [k, unwrap(v)]),
  );
}

const preset = {
  theme: {
    extend: {
      spacing: unwrap(tokens.space ?? {}),
      borderRadius: unwrap(tokens.radius ?? {}),
      colors: Object.fromEntries(
        Object.entries(tokens.color ?? {}).map(([k, v]) => [k, unwrap(v)]),
      ),
    },
  },
};
fs.mkdirSync('dist/tw', { recursive: true });
fs.writeFileSync('dist/tw/preset.js', `export default ${JSON.stringify(preset)};\n`, 'utf-8');

const themesUrl = new URL('../src/themes/', import.meta.url);
// Convert the URL to a file-system path via `fileURLToPath` for cross-platform compatibility.
const themeFiles = fs
  .readdirSync(fileURLToPath(themesUrl))
  .filter((f) => f.endsWith('.json'));
const daisyThemes = themeFiles.map((file) => {
  const json = readJson(new URL(file, themesUrl));
  const semantic = unwrap(json.semantic ?? {});
  return {
    ...(json.name ? { name: json.name } : {}),
    primary: semantic?.brand?.default ?? '#000000',
    'primary-focus': semantic?.brand?.hover ?? '#000000',
    'primary-content': semantic?.brand?.contrast ?? '#111111',
    'base-100': semantic?.bg?.default ?? '#ffffff',
    'base-200': semantic?.bg?.subtle ?? '#f4f4f5',
    'base-content': semantic?.fg?.default ?? '#111111',
    'base-content-muted': semantic?.fg?.muted ?? '#4b5563',
  };
});
fs.mkdirSync('dist/daisy', { recursive: true });
fs.writeFileSync(
  'dist/daisy/theme.js',
  `export default {themes: ${JSON.stringify(daisyThemes)}};\n`,
  'utf-8',
);
