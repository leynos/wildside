/** @file User domain types shared between client and server.
 *  Invariants:
 *  - `id` is a branded string (`UserId`) parsed via `UserIdSchema`.
 *  - `display_name` is a trimmed, non-empty string.
 *  These schemas gate I/O at module boundaries to keep types and runtime in sync.
 */
import { z } from 'zod';
/** Runtime schema for a branded user identifier. */
export declare const UserIdSchema: z.ZodBranded<z.ZodString, "UserId">;
/** Unique identifier for a user. */
export type UserId = z.infer<typeof UserIdSchema>;
/** Runtime schema for a user record. */
export declare const UserSchema: z.ZodObject<{
    id: z.ZodBranded<z.ZodString, "UserId">;
    display_name: z.ZodString;
}, "strict", z.ZodTypeAny, {
    id?: string & z.BRAND<"UserId">;
    display_name?: string;
}, {
    id?: string;
    display_name?: string;
}>;
/** User record returned from the API. */
export type User = z.infer<typeof UserSchema>;
/** Runtime schema for a list of user records. */
export declare const UsersSchema: z.ZodArray<z.ZodObject<{
    id: z.ZodBranded<z.ZodString, "UserId">;
    display_name: z.ZodString;
}, "strict", z.ZodTypeAny, {
    id?: string & z.BRAND<"UserId">;
    display_name?: string;
}, {
    id?: string;
    display_name?: string;
}>, "many">;
/** Collection of user records. */
export type Users = z.infer<typeof UsersSchema>;
