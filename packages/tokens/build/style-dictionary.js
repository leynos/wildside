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

// naive tw + daisy mappers (replace with richer mapping later)
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

const light = JSON.parse(fs.readFileSync('src/themes/light.json', 'utf-8'));
const semantic = unwrap(light.semantic);
const theme = {
  primary: semantic.brand.default,
  'primary-focus': semantic.brand.hover,
  'primary-content': semantic.brand.contrast,
  'base-100': semantic.bg.default,
  'base-200': semantic.bg.subtle,
  'base-content': semantic.fg.default,
  'base-content-muted': semantic.fg.muted
};
fs.writeFileSync('dist/daisy/theme.cjs', `module.exports={themes:[${JSON.stringify(theme)}]}`);
