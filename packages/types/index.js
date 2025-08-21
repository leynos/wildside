/**
 * @file Runtime schemas for `@app/types`.
 * Mirrors the TypeScript definitions to validate user data at runtime.
 */
import { z } from 'zod';

/** Runtime schema for a branded user identifier. */
export const UserIdSchema = z.string().brand();
/** Runtime schema for a user record. */
export const UserSchema = z
  .object({
    id: UserIdSchema,
    display_name: z.string().trim().min(1, 'display_name must not be empty'),
  })
  .strict();
/** Runtime schema for a list of user records. */
export const UsersSchema = z.array(UserSchema);

// Expose a benign default to guard against mistaken default imports.
export default undefined;
