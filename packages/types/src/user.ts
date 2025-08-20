/** @file User domain types shared between client and server.
 *  Invariants:
 *  - `id` is a branded string (`UserId`) parsed via `UserIdSchema`.
 *  - `display_name` is a trimmed, non-empty string.
 *  These schemas gate I/O at module boundaries to keep types and runtime in sync.
 */
import { z } from "zod";

/** Runtime schema for a branded user identifier. */
export const UserIdSchema = z.string().brand<"UserId">();
/** Unique identifier for a user. */
export type UserId = z.infer<typeof UserIdSchema>;

/** Runtime schema for a user record. */
export const UserSchema = z
	.object({
		id: UserIdSchema,
		display_name: z.string().trim().min(1, "display_name must not be empty"),
	})
	.strict();
/** User record returned from the API. */
export type User = z.infer<typeof UserSchema>;

/** Runtime schema for a list of user records. */
export const UsersSchema = z.array(UserSchema);
/** Collection of user records. */
export type Users = z.infer<typeof UsersSchema>;
