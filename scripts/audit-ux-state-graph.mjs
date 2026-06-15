#!/usr/bin/env bun
/** @file Audits UX state graph connectivity against the documented sitemap. */

import { readFileSync } from "node:fs";

const usage = [
  "Usage:",
  "  bun run scripts/audit-ux-state-graph.mjs --graph <path> --sitemap <path>",
].join("\n");

const args = parseArgs(process.argv.slice(2));

if (!args.graph || !args.sitemap) {
  console.error(usage);
  process.exit(1);
}

const graph = readJson(args.graph);
const sitemapRoutes = readSitemapRoutes(args.sitemap);
const states = normaliseStates(graph);
const transitions = normaliseTransitions(graph);
const initialState = typeof graph.initialState === "string" ? graph.initialState : null;

const inboundCounts = countTransitions(transitions, "to");
const outboundCounts = countTransitions(transitions, "from");

for (const state of states) {
  const inbound = inboundCounts.get(state.id) ?? 0;
  const outbound = outboundCounts.get(state.id) ?? 0;
  const route = typeof state.route === "string" ? state.route : "NONE";
  const isInitial = state.id === initialState;
  const isRouteMissing = route !== "NONE" && !hasRouteMatch(route, sitemapRoutes);
  const isOrphan = (!isInitial && inbound === 0) || outbound === 0 || isRouteMissing;
  const marker = isOrphan ? " [ORPHAN]" : "";

  console.log(`${state.id} in=${inbound} out=${outbound} route=${route}${marker}`);
}

function parseArgs(argv) {
  const parsed = {};

  for (let index = 0; index < argv.length; index += 1) {
    const option = argv[index];
    const value = argv[index + 1];

    if ((option === "--graph" || option === "--sitemap") && value) {
      parsed[option.slice(2)] = value;
      index += 1;
    }
  }

  return parsed;
}

function readJson(path) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    throwInputError(`Cannot read valid JSON from ${path}`, error);
  }
}

function readSitemapRoutes(path) {
  try {
    const markdown = readFileSync(path, "utf8");
    return new Set([...markdown.matchAll(/`(\/[^`\s|]*)`/g)].map((match) => match[1]));
  } catch (error) {
    throwInputError(`Cannot read sitemap from ${path}`, error);
  }
}

function normaliseStates(graph) {
  if (!Array.isArray(graph.states)) {
    throwInputError("State graph must contain a states array");
  }

  return graph.states.map((state) => {
    if (!state || typeof state.id !== "string") {
      throwInputError("Every state graph state must contain a string id");
    }

    return state;
  });
}

function normaliseTransitions(graph) {
  if (!Array.isArray(graph.transitions)) {
    throwInputError("State graph must contain a transitions array");
  }

  return graph.transitions.map((transition) => {
    if (
      !transition ||
      typeof transition.from !== "string" ||
      typeof transition.to !== "string"
    ) {
      throwInputError("Every state graph transition must contain string from and to fields");
    }

    return transition;
  });
}

function countTransitions(transitions, field) {
  const counts = new Map();

  for (const transition of transitions) {
    counts.set(transition[field], (counts.get(transition[field]) ?? 0) + 1);
  }

  return counts;
}

function hasRouteMatch(route, sitemapRoutes) {
  if (sitemapRoutes.has(route)) {
    return true;
  }

  if (route.endsWith("/*")) {
    return sitemapRoutes.has(route.slice(0, -2));
  }

  const [routeWithoutHash] = route.split("#");
  return sitemapRoutes.has(routeWithoutHash);
}

function throwInputError(message, cause) {
  if (cause) {
    console.error(`${message}: ${cause.message}`);
  } else {
    console.error(message);
  }

  process.exit(1);
}
