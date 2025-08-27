/** @file Build Style Dictionary outputs and derive framework presets.
 * Converts design token sources into CSS variables, Tailwind presets, and
 * daisyUI themes. Utility functions live alongside build scripts to keep them
 * small and focused.
 */
import fs from 'node:fs';
import StyleDictionary from 'style-dictionary';
import { readJson } from './read-json.js';

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
const tokens = readJson('src/tokens.json');
const unwrap = (input) =>
  Object.fromEntries(
    Object.entries(input).map(([k, v]) => [
      k,
      typeof v === 'object' && 'value' in v ? v.value : unwrap(v),
    ]),
  );

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

const themesDir = 'src/themes';
const themeFiles = fs.readdirSync(themesDir).filter((f) => f.endsWith('.json'));
const daisyThemes = themeFiles.map((file) => {
  const json = readJson(`${themesDir}/${file}`);
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
