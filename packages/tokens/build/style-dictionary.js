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
const tokens = JSON.parse(fs.readFileSync('src/tokens.json'));
fs.writeFileSync('dist/tw/preset.cjs', `module.exports={theme:{extend:{spacing:${JSON.stringify(tokens.space)},borderRadius:${JSON.stringify(tokens.radius)},colors:${JSON.stringify(tokens.color)}}}`);
const theme = {
  primary: tokens.semantic.brand.default,
  'base-100': tokens.semantic.bg.default,
  'base-content': tokens.semantic.fg.default
};
fs.writeFileSync('dist/daisy/theme.cjs', `module.exports={themes:[${JSON.stringify(theme)}]}`);
