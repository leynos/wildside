# React and Tailwind with Bun

This guide records the Bun-centric React and Tailwind workflow kept for
experiments and the optional static preview helper. The production Wildside PWA
uses the Vite-based front-end stack described in `docs/v2a-front-end-stack.md`.

## Prerequisites

- Bun 1.3.x or the version pinned by the repository tooling.
- A React entry point and Tailwind stylesheet.
- Makefile targets for project-local validation.

## Scaffold a small experiment

```bash
mkdir my-app
cd my-app
bun init --react=tailwind
```

The generated project contains React, Tailwind and Bun HTML-entry development
support.

## Run the dev server

Bun can serve an HTML entry point directly:

```bash
bun './**/*.html'
```

The command prints a local URL and route table. React component edits should hot
reload through React Fast Refresh.

## Minimal entry shape

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Bun React Tailwind experiment</title>
    <link rel="stylesheet" href="./src/index.css" />
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="./src/main.tsx"></script>
  </body>
</html>
```

```css
@import "tailwindcss";
```

## Build for production

```bash
bun build ./index.html --production --outdir=dist
```

The output is a bundled `dist/` directory suitable for static hosting.

## Static preview helper

A small Bun server can serve `dist/` and fall back to `index.html` for
client-side routes:

```ts
import { serve } from "bun";

serve({
  port: 3000,
  async fetch(req) {
    const url = new URL(req.url);
    const path = url.pathname === "/" ? "/index.html" : url.pathname;
    const file = Bun.file(`./dist${path}`);

    if (await file.exists()) return new Response(file);

    return new Response(Bun.file("./dist/index.html"), {
      headers: { "Content-Type": "text/html; charset=utf-8" },
    });
  },
});
```

## Troubleshooting

- **Tailwind classes missing:** check source globs and stylesheet imports.
- **Client-side routes return 404:** add a static-host rewrite or the SPA
  fallback above.
- **Hot reload inactive:** start from an HTML entry point or a Bun server with
  development HMR enabled.
- **TypeScript transform mismatch:** keep Bun experiment settings separate from
  the Vite production workspace.
