/** @file Tests the UX state graph audit helper and CLI entrypoint. */

import fc from "fast-check";
import { execFile } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";
import { describe, expect, it } from "vitest";
import {
  auditStateGraph,
  countTransitions,
  formatAuditRows,
  hasRouteMatch,
  hasStringTransitionEndpoints,
  isRecognisedOptionWithValue,
  normaliseStates,
  normaliseTransitions,
  parseArgs,
  readJson,
  readSitemapRoutes,
  runAudit,
} from "./audit-ux-state-graph.mjs";

const execFileAsync = promisify(execFile);
const repositoryRoot = new URL("../", import.meta.url);
const scriptPath = new URL("./audit-ux-state-graph.mjs", import.meta.url);
const ansiPattern = /\u001b\[[0-9;]*m/g;

/**
 * Remove ANSI colour escapes from subprocess output.
 *
 * @param {string} value Text emitted by a subprocess.
 * @returns {string} Text without ANSI escape sequences.
 */
function stripAnsi(value) {
  return value.replaceAll(ansiPattern, "");
}

/**
 * Create temporary graph and sitemap files for filesystem-backed tests.
 *
 * @param {unknown} graph Graph payload to serialise.
 * @param {string} sitemap Markdown sitemap contents.
 * @returns {Promise<{dir: string, graphPath: string, sitemapPath: string}>} Paths to created fixtures.
 */
async function writeAuditFixtures(graph, sitemap) {
  const dir = await mkdtemp(join(tmpdir(), "wildside-ux-audit-"));
  const graphPath = join(dir, "graph.json");
  const sitemapPath = join(dir, "sitemap.md");

  await writeFile(graphPath, JSON.stringify(graph), "utf8");
  await writeFile(sitemapPath, sitemap, "utf8");

  return { dir, graphPath, sitemapPath };
}

describe("parseArgs", () => {
  it("parses supported graph and sitemap options", () => {
    expect(parseArgs(["--graph", "graph.json", "--sitemap", "sitemap.md"])).toEqual({
      graph: "graph.json",
      sitemap: "sitemap.md",
    });
  });

  it("ignores unsupported options and missing values", () => {
    expect(parseArgs(["--unknown", "value", "--graph", "--sitemap", "sitemap.md"])).toEqual({
      sitemap: "sitemap.md",
    });
  });
});

describe("isRecognisedOptionWithValue", () => {
  it("accepts supported options with values", () => {
    expect(isRecognisedOptionWithValue("--graph", "graph.json")).toBe(true);
    expect(isRecognisedOptionWithValue("--sitemap", "sitemap.md")).toBe(true);
  });

  it("rejects unsupported options and absent values", () => {
    expect(isRecognisedOptionWithValue("--graph", undefined)).toBe(false);
    expect(isRecognisedOptionWithValue("--other", "value")).toBe(false);
  });
});

describe("readJson", () => {
  it("reads valid JSON", async () => {
    const { dir, graphPath } = await writeAuditFixtures({ states: [], transitions: [] }, "");

    try {
      expect(readJson(graphPath)).toEqual({ states: [], transitions: [] });
    } finally {
      await rm(dir, { force: true, recursive: true });
    }
  });

  it("throws for invalid JSON", async () => {
    const dir = await mkdtemp(join(tmpdir(), "wildside-ux-audit-"));
    const graphPath = join(dir, "graph.json");
    await writeFile(graphPath, "{", "utf8");

    try {
      expect(() => readJson(graphPath)).toThrow(`Cannot read valid JSON from ${graphPath}:`);
    } finally {
      await rm(dir, { force: true, recursive: true });
    }
  });
});

describe("readSitemapRoutes", () => {
  it("extracts backtick-quoted absolute routes from Markdown", async () => {
    const { dir, sitemapPath } = await writeAuditFixtures(
      { states: [], transitions: [] },
      "Routes: `/`, `/cards`, `not-a-route`, `/account/settings`.",
    );

    try {
      expect(readSitemapRoutes(sitemapPath)).toEqual(
        new Set(["/", "/cards", "/account/settings"]),
      );
    } finally {
      await rm(dir, { force: true, recursive: true });
    }
  });

  it("throws when the sitemap cannot be read", () => {
    expect(() => readSitemapRoutes("/missing/sitemap.md")).toThrow(
      "Cannot read sitemap from /missing/sitemap.md:",
    );
  });
});

describe("normaliseStates", () => {
  it("returns valid state entries", () => {
    const states = [{ id: "home", route: "/" }];

    expect(normaliseStates({ states })).toEqual(states);
  });

  it("throws when states are missing or malformed", () => {
    expect(() => normaliseStates({})).toThrow("State graph must contain a states array");
    expect(() => normaliseStates({ states: [{}] })).toThrow(
      "Every state graph state must contain a string id",
    );
  });
});

describe("normaliseTransitions", () => {
  it("returns valid transition entries", () => {
    const transitions = [{ from: "home", to: "cards" }];

    expect(normaliseTransitions({ transitions })).toEqual(transitions);
  });

  it("throws when transitions are missing or malformed", () => {
    expect(() => normaliseTransitions({})).toThrow(
      "State graph must contain a transitions array",
    );
    expect(() => normaliseTransitions({ transitions: [{ from: "home" }] })).toThrow(
      "Every state graph transition must contain string from and to fields",
    );
  });
});

describe("hasStringTransitionEndpoints", () => {
  it("recognises transitions with string endpoints", () => {
    expect(hasStringTransitionEndpoints({ from: "home", to: "cards" })).toBe(true);
  });

  it("rejects absent and non-string endpoints", () => {
    expect(hasStringTransitionEndpoints(null)).toBe(false);
    expect(hasStringTransitionEndpoints({ from: "home", to: 2 })).toBe(false);
  });
});

describe("countTransitions", () => {
  it("counts repeated endpoint references", () => {
    const transitions = [
      { from: "home", to: "cards" },
      { from: "home", to: "settings" },
      { from: "cards", to: "settings" },
    ];

    expect(countTransitions(transitions, "from")).toEqual(
      new Map([
        ["home", 2],
        ["cards", 1],
      ]),
    );
    expect(countTransitions(transitions, "to")).toEqual(
      new Map([
        ["cards", 1],
        ["settings", 2],
      ]),
    );
  });

  it("returns an empty map when no transitions exist", () => {
    expect(countTransitions([], "from")).toEqual(new Map());
    expect(countTransitions([], "to")).toEqual(new Map());
  });

  it("counts the requested endpoint field independently", () => {
    const transitions = [
      { from: "shared", to: "one" },
      { from: "shared", to: "two" },
      { from: "other", to: "shared" },
    ];

    expect(countTransitions(transitions, "from").get("shared")).toBe(2);
    expect(countTransitions(transitions, "to").get("shared")).toBe(1);
  });

  it("matches generated endpoint frequencies", () => {
    const endpoint = fc.constantFrom("home", "cards", "settings", "done");

    fc.assert(
      fc.property(
        fc.array(
          fc.record({
            from: endpoint,
            to: endpoint,
          }),
          { maxLength: 30 },
        ),
        fc.constantFrom("from", "to"),
        (transitions, field) => {
          const actual = countTransitions(transitions, field);
          const expected = new Map();

          for (const transition of transitions) {
            expected.set(transition[field], (expected.get(transition[field]) ?? 0) + 1);
          }

          expect(actual).toEqual(expected);
        },
      ),
    );
  });
});

describe("hasRouteMatch", () => {
  const routes = new Set(["/", "/cards", "/settings"]);

  it("matches exact, wildcard base, and hash-stripped routes", () => {
    expect(hasRouteMatch("/cards", routes)).toBe(true);
    expect(hasRouteMatch("/cards/*", routes)).toBe(true);
    expect(hasRouteMatch("/settings#profile", routes)).toBe(true);
  });

  it("rejects routes absent from the sitemap", () => {
    expect(hasRouteMatch("/missing", routes)).toBe(false);
  });

  it("matches generated exact, wildcard, and hash-qualified known routes", () => {
    fc.assert(
      fc.property(fc.constantFrom("/", "/cards", "/settings"), (route) => {
        expect(hasRouteMatch(route, routes)).toBe(true);
        if (route !== "/") {
          expect(hasRouteMatch(`${route}/*`, routes)).toBe(true);
        }
        expect(hasRouteMatch(`${route}#detail`, routes)).toBe(true);
      }),
    );
  });
});

describe("auditStateGraph", () => {
  it("computes connectivity and orphan flags for all states", () => {
    const rows = auditStateGraph(
      {
        initialState: "home",
        states: [
          { id: "home", route: "/" },
          { id: "cards", route: "/cards" },
          { id: "terminal", route: "/cards#done" },
          { id: "missing-route", route: "/missing" },
          { id: "unrouted" },
        ],
        transitions: [
          { from: "home", to: "cards" },
          { from: "cards", to: "terminal" },
          { from: "home", to: "missing-route" },
        ],
      },
      new Set(["/", "/cards"]),
    );

    expect(rows).toEqual([
      { id: "home", inbound: 0, outbound: 2, route: "/", isOrphan: false },
      { id: "cards", inbound: 1, outbound: 1, route: "/cards", isOrphan: false },
      { id: "terminal", inbound: 1, outbound: 0, route: "/cards#done", isOrphan: true },
      {
        id: "missing-route",
        inbound: 1,
        outbound: 0,
        route: "/missing",
        isOrphan: true,
      },
      { id: "unrouted", inbound: 0, outbound: 0, route: "NONE", isOrphan: true },
    ]);
  });

  it("marks a non-initial state with no inbound route as orphan", () => {
    const rows = auditStateGraph(
      {
        initialState: "home",
        states: [
          { id: "home", route: "/" },
          { id: "orphan", route: "/cards" },
          { id: "done", route: "/done" },
        ],
        transitions: [
          { from: "home", to: "done" },
          { from: "orphan", to: "done" },
        ],
      },
      new Set(["/", "/cards", "/done"]),
    );

    expect(rows.find((row) => row.id === "orphan")).toEqual({
      id: "orphan",
      inbound: 0,
      outbound: 1,
      route: "/cards",
      isOrphan: true,
    });
  });

  it("defaults an absent route to NONE independently of orphan status", () => {
    const rows = auditStateGraph(
      {
        initialState: "home",
        states: [{ id: "home", route: "/" }, { id: "unrouted" }, { id: "done" }],
        transitions: [
          { from: "home", to: "unrouted" },
          { from: "unrouted", to: "done" },
        ],
      },
      new Set(["/"]),
    );

    expect(rows.find((row) => row.id === "unrouted")).toEqual({
      id: "unrouted",
      inbound: 1,
      outbound: 1,
      route: "NONE",
      isOrphan: false,
    });
  });

  it("uses the initial state flag only for inbound-zero orphan status", () => {
    const graph = {
      states: [
        { id: "home", route: "/" },
        { id: "done", route: "/done" },
      ],
      transitions: [{ from: "home", to: "done" }],
    };

    expect(
      auditStateGraph({ ...graph, initialState: "home" }, new Set(["/", "/done"])).find(
        (row) => row.id === "home",
      )?.isOrphan,
    ).toBe(false);
    expect(auditStateGraph(graph, new Set(["/", "/done"])).find((row) => row.id === "home"))
      .toMatchObject({ inbound: 0, outbound: 1, isOrphan: true });
  });

  it("preserves orphan invariants for generated valid-route topologies", () => {
    const stateId = fc.constantFrom("home", "cards", "settings", "review", "done");

    fc.assert(
      fc.property(
        fc.uniqueArray(stateId, { minLength: 2, maxLength: 5 }),
        fc.array(
          fc.record({
            from: stateId,
            to: stateId,
          }),
          { maxLength: 30 },
        ),
        (ids, generatedTransitions) => {
          const idSet = new Set(ids);
          const transitions = generatedTransitions.filter(
            ({ from, to }) => idSet.has(from) && idSet.has(to),
          );
          const states = ids.map((id) => ({ id, route: `/${id}` }));
          const initialState = ids[0];
          const sitemapRoutes = new Set(ids.map((id) => `/${id}`));
          const inbound = countTransitions(transitions, "to");
          const outbound = countTransitions(transitions, "from");

          const rows = auditStateGraph(
            {
              initialState,
              states,
              transitions,
            },
            sitemapRoutes,
          );

          for (const row of rows) {
            expect(row.isOrphan).toBe(
              (row.id !== initialState && (inbound.get(row.id) ?? 0) === 0) ||
                (outbound.get(row.id) ?? 0) === 0,
            );
          }
        },
      ),
    );
  });
});

describe("formatAuditRows", () => {
  it("formats deterministic text output", () => {
    expect(
      formatAuditRows([
        { id: "home", inbound: 0, outbound: 1, route: "/", isOrphan: false },
        { id: "terminal", inbound: 1, outbound: 0, route: "NONE", isOrphan: true },
      ]),
    ).toEqual(["home in=0 out=1 route=/", "terminal in=1 out=0 route=NONE [ORPHAN]"]);
  });
});

describe("runAudit", () => {
  it("returns formatted rows for valid inputs", async () => {
    const { dir, graphPath, sitemapPath } = await writeAuditFixtures(
      {
        initialState: "home",
        states: [
          { id: "home", route: "/" },
          { id: "done", route: "/cards" },
        ],
        transitions: [{ from: "home", to: "done" }],
      },
      "Routes: `/`, `/cards`.",
    );

    try {
      expect(runAudit(["--graph", graphPath, "--sitemap", sitemapPath])).toEqual([
        "home in=0 out=1 route=/",
        "done in=1 out=0 route=/cards [ORPHAN]",
      ]);
    } finally {
      await rm(dir, { force: true, recursive: true });
    }
  });

  it("throws usage when required arguments are absent", () => {
    expect(() => runAudit([])).toThrow(
      "Usage:\n  bun run scripts/audit-ux-state-graph.mjs --graph <path> --sitemap <path>",
    );
  });
});

describe("CLI entrypoint", () => {
  it("prints audit rows and exits successfully", async () => {
    const { dir, graphPath, sitemapPath } = await writeAuditFixtures(
      {
        initialState: "home",
        states: [
          { id: "home", route: "/" },
          { id: "done", route: "/cards" },
        ],
        transitions: [{ from: "home", to: "done" }],
      },
      "Routes: `/`, `/cards`.",
    );

    try {
      const { stdout, stderr } = await execFileAsync(
        "bun",
        [scriptPath.pathname, "--graph", graphPath, "--sitemap", sitemapPath],
        { cwd: repositoryRoot },
      );

      expect(stdout).toMatchSnapshot();
      expect(stderr).toMatchSnapshot();
    } finally {
      await rm(dir, { force: true, recursive: true });
    }
  });

  it("prints usage and exits non-zero when arguments are missing", async () => {
    const result = await execFileAsync("bun", [scriptPath.pathname], {
      cwd: repositoryRoot,
    }).catch((error) => ({
      code: error.code,
      stderr: stripAnsi(error.stderr),
    }));

    expect(result).toMatchSnapshot();
  });
});
