const preset = require('@app/tokens/dist/tw/preset.cjs');
const daisy = require('@app/tokens/dist/daisy/theme.cjs');

module.exports = {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  presets: [preset],
  plugins: [require('daisyui')],
  daisyui: daisy
};
