/**
 * @file API client functions generated from OpenAPI.
 * Uses shared user types from `@app/types`.
 */
import { z } from 'zod';
import { customFetchParsed } from './fetcher';
import { User, UserSchema } from '@app/types';

/** Fetch all registered users. */
export const listUsers = (
  signal?: AbortSignal,
): Promise<User[]> => customFetchParsed('/api/users', z.array(UserSchema), { signal });
