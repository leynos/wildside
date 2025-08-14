/**
 * @file API client functions generated from OpenAPI.
 */
import { z } from 'zod';
import { customFetchParsed } from './fetcher';

export type UserId = string & { readonly brand: 'UserId' };

export interface User {
  id: UserId;
  display_name: string;
}

const userSchema = z.object({
  id: z.string().transform(id => id as UserId),
  display_name: z.string(),
}) satisfies z.ZodType<User>;

export const listUsers = (signal?: AbortSignal) =>
  customFetchParsed('/api/users', z.array(userSchema), { signal });
