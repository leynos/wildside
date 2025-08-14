import preset from '@app/tokens/dist/tw/preset.js';
import daisy from '@app/tokens/dist/daisy/theme.js';
import daisyui from 'daisyui';

/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  presets: [preset],
  plugins: [daisyui],
  daisyui: daisy
};
