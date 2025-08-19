/**
 * @file API client functions generated from OpenAPI.
 * Uses shared user types from `@app/types`.
 * Endpoint: GET /api/users
 * Invariants: returns a JSON array matching the User schema; throws ZodError on mismatch.
 */
import { customFetchParsed } from "./fetcher";
import { type User, UsersSchema } from "@app/types";

/**
 * Fetch all registered users.
 *
 * @example
 * const users = await listUsers();
 * users.length;
 */
export const listUsers = (signal?: AbortSignal): Promise<User[]> =>
	customFetchParsed("/api/users", UsersSchema, { signal });
