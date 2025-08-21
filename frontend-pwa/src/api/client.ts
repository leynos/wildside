/**
 * @file API client functions generated from OpenAPI.
 * Uses shared user types from `@app/types`.
 * Endpoint: GET /api/users
 * Invariants: returns a JSON array matching the User schema; throws ZodError on mismatch.
 */
import { type User, UsersSchema } from '@app/types';
import type { QueryKey } from '@tanstack/react-query';
import { customFetchParsed } from './fetcher';

/**
 * Query key for user listings.
 */
export const USERS_QK = ['users'] as const satisfies QueryKey;

/**
 * Helpers for composing user query keys.
 */
export const usersQK = {
  all: USERS_QK,
  byId: (id: User['id']) => [...USERS_QK, id] as const,
} as const;
// Freeze to guard against runtime mutation.
Object.freeze(usersQK);

/**
 * Fetch all registered users.
 *
 * @example
 * const users = await listUsers();
 * users.length;
 */
export const listUsers = (signal?: AbortSignal): Promise<User[]> =>
  customFetchParsed('/api/users', UsersSchema, { signal });
