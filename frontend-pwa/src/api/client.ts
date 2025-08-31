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
 *
 * @example
 * useQuery({ queryKey: usersQueryKey, queryFn: listUsers });
 * @see usersQueryKeys for composed keys
 */
export const usersQueryKey = ['users'] as const satisfies QueryKey;
// Freeze to guard against accidental mutation at runtime.
Object.freeze(usersQueryKey);

/**
 * Helpers for composing user query keys.
 */
export const usersQueryKeys = {
  all: usersQueryKey,
  byId: (id: User['id']): readonly [...typeof usersQueryKey, User['id']] =>
    [...usersQueryKey, id] as const,
} as const;
Object.freeze(usersQueryKeys);

/**
 * Fetch all registered users.
 *
 * @example
 * const users = await listUsers();
 * users.length;
 */
export const listUsers = (signal?: AbortSignal): Promise<User[]> =>
  customFetchParsed('/api/users', UsersSchema, { signal });
