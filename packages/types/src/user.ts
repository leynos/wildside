/** @file User domain types shared between client and server. */
import { z } from 'zod';

/** Runtime schema for a branded user identifier. */
export const UserIdSchema = z.string().brand<'UserId'>();
/** Unique identifier for a user. */
export type UserId = z.infer<typeof UserIdSchema>;

/** Runtime schema for a user record. */
export const UserSchema = z.object({
  id: UserIdSchema,
  display_name: z.string(),
});
/** User record returned from the API. */
export type User = z.infer<typeof UserSchema>;
