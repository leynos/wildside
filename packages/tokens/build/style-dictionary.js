import StyleDictionary from 'style-dictionary';
import fs from 'node:fs';

const sd = StyleDictionary.extend({
  source: ['src/tokens.json', 'src/themes/*.json'],
  platforms: {
    css: {
      transformGroup: 'css',
      buildPath: 'dist/css/',
      files: [{ destination: 'variables.css', format: 'css/variables' }]
    },
    tailwind: {
      transformGroup: 'js',
      buildPath: 'dist/tw/',
      files: [{ destination: 'preset.cjs', format: 'javascript/module' }]
    },
    daisy: {
      transformGroup: 'js',
      buildPath: 'dist/daisy/',
      files: [{ destination: 'theme.cjs', format: 'javascript/module' }]
    }
  }
});

sd.buildAllPlatforms();

// Map tokens into Tailwind and DaisyUI presets
const tokens = JSON.parse(fs.readFileSync('src/tokens.json', 'utf-8'));
const unwrap = input =>
  Object.fromEntries(
    Object.entries(input).map(([k, v]) => [
      k,
      typeof v === 'object' && 'value' in v ? v.value : unwrap(v)
    ])
  );

const preset = {
  theme: {
    extend: {
      spacing: unwrap(tokens.space),
      borderRadius: unwrap(tokens.radius),
      colors: Object.fromEntries(
        Object.entries(tokens.color).map(([k, v]) => [k, unwrap(v)])
      )
    }
  }
};
fs.writeFileSync('dist/tw/preset.cjs', `module.exports=${JSON.stringify(preset)}`);

const themesDir = 'src/themes';
const themeFiles = fs.readdirSync(themesDir).filter(f => f.endsWith('.json'));
const daisyThemes = themeFiles.map(file => {
  const json = JSON.parse(fs.readFileSync(`${themesDir}/${file}`, 'utf-8'));
  const semantic = unwrap(json.semantic ?? {});
  return {
    ...(json.name ? { name: json.name } : {}),
    primary: semantic?.brand?.default ?? '#000000',
    'primary-focus': semantic?.brand?.hover ?? '#000000',
    'primary-content': semantic?.brand?.contrast ?? '#111111',
    'base-100': semantic?.bg?.default ?? '#ffffff',
    'base-200': semantic?.bg?.subtle ?? '#f4f4f5',
    'base-content': semantic?.fg?.default ?? '#111111',
    'base-content-muted': semantic?.fg?.muted ?? '#4b5563'
  };
});
fs.mkdirSync('dist/daisy', { recursive: true });
fs.writeFileSync(
  'dist/daisy/theme.cjs',
  `module.exports = {themes: ${JSON.stringify(daisyThemes)}};\n`,
  'utf-8'
);
