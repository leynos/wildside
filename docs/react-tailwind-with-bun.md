# React + Tailwind with Bun 1.3.0 — a short, no‑nonsense guide

> **Note:** The playground now ships via a Vite build that targets GitHub
> Pages, but this guide remains as a reference for Bun-centric experiments and
> the optional `server.ts` preview helper.

This is a pragmatic walkthrough for building and serving a tiny React + Tailwind app using **Bun 1.3.0**. It leans on Bun’s built‑in dev server (HMR, React Fast Refresh) and production bundler.

---

## 0) Prerequisites

- Node/npm not required, but fine to have.
- Bun **1.3.0** installed:

```bash
# Linux/macOS
curl -fsSL https://bun.sh/install | bash

# Homebrew (macOS)
brew tap oven-sh/bun
brew install bun

# Windows (PowerShell)
powershell -c "irm bun.sh/install.ps1 | iex"

# Or globally via npm (works, but native installer is preferred)
npm install -g bun
```

Verify:

```bash
bun --version  # expect 1.3.0
```

---

## 1) Scaffold a React + Tailwind project

Bun ships templates. Use the Tailwind variant to avoid manual setup.

```bash
mkdir my-app && cd my-app
bun init --react=tailwind
# Follow the prompts; accept defaults if unsure.
```

What you get:

- React + TS/JS wired up for Bun’s **HTML‑first** dev server.
- Tailwind preconfigured (content paths, `@tailwind` directives, PostCSS config).

---

## 2) Run the dev server (with HMR)

Bun 1.3 can serve HTML entry points directly and handle bundling/transpilation under the hood.

```bash
bun './**/*.html'
# or simply:
bun ./index.html
```

You’ll see a URL like `http://localhost:3000/` and a routes table. Open it in the browser. Edits to React components hot‑reload instantly (React Fast Refresh).

> Tip: Bun pipes browser `console.log` back to your terminal. Handy for quick debugging.

---

## 3) Hello React + Tailwind

The template includes a basic app. If starting from a blank template, here’s the minimal shape.

**`index.html`** (HTML import entry; Bun resolves and bundles dependencies):

```html
<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Bun + React + Tailwind</title>
    <link rel="stylesheet" href="./src/index.css" />
  </head>
  <body class="min-h-screen bg-slate-950 text-slate-100">
    <div id="root"></div>
    <script type="module" src="./src/main.tsx"></script>
  </body>
</html>
```

**`src/index.css`** (Tailwind directives):

```css
@tailwind base;
@tailwind components;
@tailwind utilities;
```

**`src/main.tsx`**:

```tsx
import React from "react";
import { createRoot } from "react-dom/client";

function App() {
  return (
    <main className="grid min-h-screen place-items-center p-6">
      <section className="text-center">
        <h1 className="text-3xl font-bold tracking-tight">Hello, Bun 🥖</h1>
        <p className="mt-2 text-slate-300">
          React + Tailwind served by Bun 1.3 with hot reload.
        </p>
        <button className="mt-6 rounded-xl bg-emerald-500 px-4 py-2 font-medium text-emerald-950 hover:bg-emerald-400">
          Click me
        </button>
      </section>
    </main>
  );
}

createRoot(document.getElementById("root")!).render(<App />);
```

Tailwind config (`tailwind.config.{js,ts}`) should include your content globs, e.g.:

```js
export default {
  content: [
    "./index.html",
    "./src/**/*.{ts,tsx,js,jsx}",
  ],
  theme: { extend: {} },
  plugins: [],
};
```

---

## 4) Build for production

Bun bundles HTML, TS/JS, CSS assets. Use `--production` for minification and tree‑shaking.

```bash
bun build ./index.html --production --outdir=dist
```

You’ll get a *fully bundled* `dist/` directory ready to host anywhere.

---

## 5) Serve the production build

### Easiest: any static file host

- Copy `dist/` to S3/Cloudflare Pages/Netlify/Vercel/nginx/Apache. Done.

### With Bun itself (simple static server)

For local preview or DIY hosting, a tiny Bun server can serve the `dist` folder. Example SPA‑safe server that falls back to `index.html`:

```ts
// server.ts
import { serve } from "bun";

serve({
  port: 3000,
  async fetch(req) {
    const url = new URL(req.url);
    const filePath = url.pathname === "/" ? "/index.html" : url.pathname;
    const file = Bun.file(`./dist${filePath}`);

    if (await file.exists()) return new Response(file);

    // SPA fallback (client‑side routing)
    return new Response(Bun.file("./dist/index.html"), {
      headers: { "Content-Type": "text/html; charset=utf-8" },
    });
  },
});
```

Run it:

```bash
bun run server.ts
```

---

## 6) (Optional) One‑process full‑stack dev

If you prefer a single process serving your SPA **and** APIs during development, use HTML imports + `Bun.serve()` routes.

```ts
// dev-serve.ts
import homepage from "./index.html";
import { serve } from "bun";

serve({
  development: { hmr: true, console: true },
  routes: {
    "/": homepage,
    "/api/health": () => Response.json({ status: "ok" }),
  },
});
```

Start it:

```bash
bun run dev-serve.ts
```

You still get HMR, HTML bundling, and a tidy `/api/*` space without CORS faff.

---

## 7) Troubleshooting

- **Tailwind classes not applying**: check `content` globs; ensure `index.css` is linked in `index.html`.
- **404s in production for client‑side routes**: add the SPA fallback (see server example) or configure your static host’s rewrite rules.
- **HMR not triggering**: ensure you started via `bun './**/*.html'` or a `Bun.serve()` with `development.hmr: true`.
- **TypeScript module quirks**: Bun defaults to `"module": "Preserve"`; avoid incompatible TS transforms in your own config.

---

## 8) Bonus: compile to a single executable (advanced)

You can ship a self‑contained binary that serves your app:

```bash
bun build --compile ./index.html --outfile myapp
./myapp  # starts a server; routes can be added in code
```

Use this for kiosk‑style SPAs or internal tools where “download and run” beats provisioning infra.

---

## That’s it

You’ve got: scaffold → dev server with HMR → Tailwind styling → production bundle → optional Bun‑served hosting. Compact, fast, and pleasantly free of yak‑hair.
