#!/usr/bin/env bun
/**
 * @file Supports the front-end source contradictions catalogue audit by checking
 * the documented UX state graph against the published sitemap routes.
 *
 * The CLI requires `--graph <path>` for a JSON state graph and
 * `--sitemap <path>` for the Markdown sitemap. It reports inbound and outbound
 * transition counts for each state and marks states as orphaned when they are
 * unreachable, terminal, or reference a route that is absent from the sitemap.
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const usage = [
  "Usage:",
  "  bun run scripts/audit-ux-state-graph.mjs --graph <path> --sitemap <path>",
].join("\n");

/**
 * Parse supported CLI options from an argv tail.
 *
 * @param {string[]} argv Command-line arguments excluding the executable and script path.
 * @returns {{graph?: string, sitemap?: string}} Parsed option values.
 */
export function parseArgs(argv) {
  const parsed = {};

  for (let index = 0; index < argv.length; index += 1) {
    const option = argv[index];
    const value = argv[index + 1];

    if (isRecognisedOptionWithValue(option, value)) {
      parsed[option.slice(2)] = value;
      index += 1;
    }
  }

  return parsed;
}

/**
 * Check whether an option accepts a following value.
 *
 * @param {string} option Candidate option name.
 * @param {string | undefined} value Candidate option value.
 * @returns {boolean} True when the option is supported and has a value.
 */
export function isRecognisedOptionWithValue(option, value) {
  return (
    (option === "--graph" || option === "--sitemap") &&
    typeof value === "string" &&
    !value.startsWith("--")
  );
}

/**
 * Read and parse a JSON file.
 *
 * @param {string} path File path to read.
 * @returns {unknown} Parsed JSON payload.
 * @throws {Error} When the file cannot be read or parsed.
 */
export function readJson(path) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    throw new Error(`Cannot read valid JSON from ${path}: ${error.message}`);
  }
}

/**
 * Extract backtick-quoted absolute routes from a Markdown sitemap.
 *
 * @param {string} path Markdown sitemap path.
 * @returns {Set<string>} Routes cited in the sitemap.
 * @throws {Error} When the sitemap cannot be read.
 */
export function readSitemapRoutes(path) {
  try {
    const markdown = readFileSync(path, "utf8");
    return new Set([...markdown.matchAll(/`(\/[^`\s|]*)`/g)].map((match) => match[1]));
  } catch (error) {
    throw new Error(`Cannot read sitemap from ${path}: ${error.message}`);
  }
}

/**
 * Validate and return state entries from the graph payload.
 *
 * @param {unknown} graph Parsed state graph payload.
 * @returns {{id: string, route?: string}[]} State entries.
 * @throws {Error} When the graph has no valid states array.
 */
export function normaliseStates(graph) {
  if (!Array.isArray(graph.states)) {
    throw new Error("State graph must contain a states array");
  }

  return graph.states.map((state) => {
    if (!state || typeof state.id !== "string") {
      throw new Error("Every state graph state must contain a string id");
    }

    return state;
  });
}

/**
 * Validate and return transition entries from the graph payload.
 *
 * @param {unknown} graph Parsed state graph payload.
 * @returns {{from: string, to: string}[]} Transition entries.
 * @throws {Error} When transitions are absent or malformed.
 */
export function normaliseTransitions(graph) {
  if (!Array.isArray(graph.transitions)) {
    throw new Error("State graph must contain a transitions array");
  }

  return graph.transitions.map((transition) => {
    if (!hasStringTransitionEndpoints(transition)) {
      throw new Error("Every state graph transition must contain string from and to fields");
    }

    return transition;
  });
}

/**
 * Check whether a transition declares string endpoints.
 *
 * @param {unknown} transition Candidate transition.
 * @returns {boolean} True when `from` and `to` are strings.
 */
export function hasStringTransitionEndpoints(transition) {
  return (
    Boolean(transition) &&
    typeof transition.from === "string" &&
    typeof transition.to === "string"
  );
}

/**
 * Count transition endpoint references by field.
 *
 * @param {{from: string, to: string}[]} transitions Normalised transitions.
 * @param {"from" | "to"} field Endpoint field to count.
 * @returns {Map<string, number>} Count per referenced state id.
 */
export function countTransitions(transitions, field) {
  const counts = new Map();

  for (const transition of transitions) {
    counts.set(transition[field], (counts.get(transition[field]) ?? 0) + 1);
  }

  return counts;
}

/**
 * Check whether a state route is present in the sitemap.
 *
 * @param {string} route State route, optionally with a hash or wildcard suffix.
 * @param {Set<string>} sitemapRoutes Routes extracted from the sitemap.
 * @returns {boolean} True when the route maps to a sitemap route.
 */
export function hasRouteMatch(route, sitemapRoutes) {
  if (sitemapRoutes.has(route)) {
    return true;
  }

  if (route.endsWith("/*")) {
    return sitemapRoutes.has(route.slice(0, -2));
  }

  const [routeWithoutHash] = route.split("#");
  return sitemapRoutes.has(routeWithoutHash);
}

/**
 * Compute audit rows for every state in a graph.
 *
 * @param {unknown} graph Parsed state graph payload.
 * @param {Set<string>} sitemapRoutes Routes extracted from the sitemap.
 * @returns {{id: string, inbound: number, outbound: number, route: string, isOrphan: boolean}[]} Audit rows.
 */
export function auditStateGraph(graph, sitemapRoutes) {
  const states = normaliseStates(graph);
  const transitions = normaliseTransitions(graph);
  const initialState = typeof graph.initialState === "string" ? graph.initialState : null;
  const inboundCounts = countTransitions(transitions, "to");
  const outboundCounts = countTransitions(transitions, "from");

  return states.map((state) => {
    const inbound = inboundCounts.get(state.id) ?? 0;
    const outbound = outboundCounts.get(state.id) ?? 0;
    const route = typeof state.route === "string" ? state.route : "NONE";
    const isInitial = state.id === initialState;
    const isRouteMissing = route !== "NONE" && !hasRouteMatch(route, sitemapRoutes);
    const isOrphan = (!isInitial && inbound === 0) || outbound === 0 || isRouteMissing;

    return { id: state.id, inbound, outbound, route, isOrphan };
  });
}

/**
 * Format audit rows for deterministic CLI output.
 *
 * @param {{id: string, inbound: number, outbound: number, route: string, isOrphan: boolean}[]} rows Audit rows.
 * @returns {string[]} CLI output lines.
 */
export function formatAuditRows(rows) {
  return rows.map((row) => {
    const marker = row.isOrphan ? " [ORPHAN]" : "";

    return `${row.id} in=${row.inbound} out=${row.outbound} route=${row.route}${marker}`;
  });
}

/**
 * Run the audit from parsed CLI arguments.
 *
 * @param {string[]} argv Command-line arguments excluding executable and script path.
 * @returns {string[]} Formatted audit lines.
 * @throws {Error} When required arguments are absent or inputs are invalid.
 */
export function runAudit(argv) {
  const args = parseArgs(argv);

  if (!args.graph || !args.sitemap) {
    throw new Error(usage);
  }

  return formatAuditRows(auditStateGraph(readJson(args.graph), readSitemapRoutes(args.sitemap)));
}

/**
 * Execute the CLI and set a process exit code instead of terminating directly.
 *
 * @param {string[]} argv Command-line arguments excluding executable and script path.
 * @returns {number} Exit code.
 */
export function main(argv) {
  try {
    for (const line of runAudit(argv)) {
      console.log(line);
    }

    return 0;
  } catch (error) {
    console.error(error.message);

    return 1;
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  process.exitCode = main(process.argv.slice(2));
}
