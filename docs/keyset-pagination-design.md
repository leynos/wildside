# Design: Keyset Pagination Crate for Wildside Backend

## Overview and Goals

In the Wildside backend, we need a robust **cursor-based pagination** system to
efficiently navigate large datasets (like the user list) without the pitfalls
of offset-based paging. The solution will be implemented as a standalone crate
(`backend/crates/pagination`) so it can be reused across endpoints. Key design
goals include:

- **Keyset (Cursor) Pagination:** Use a cursor strategy instead of numeric
  offsets, avoiding performance issues and data inconsistencies on updates.
  Cursors will be based on a **stable total ordering** of records (e.g. by
  creation time and unique ID).

- **Envelope Response Format:** Paginated endpoints will return an **envelope**
  containing the data array and pagination metadata, rather than a raw array.
  This makes it easy to include additional info.

- **Hypermedia Navigation Links:** Each response includes `self`, `next`, and
  `prev` links (when applicable) so clients can retrieve adjacent pages without
  constructing URLs manually.

- **Opaque Cursors:** The cursor tokens will not expose internal IDs or SQL
  offsets. Instead, they will be **base64url-encoded JSON** strings that the
  server interprets. Clients should treat them as opaque.

- **No Total Counts:** We will **avoid returning total item counts** in the
  response to prevent expensive `COUNT(*)` queries on large tables. Clients can
  detect the end of data by the absence of a `next` link.

These goals summarize the expectations captured in issue #52
requirements[^issue-52-requirements].

This design focuses initially on the `GET /api/users` endpoint (replacing the
current fixed-limit list), but the crate will be generic and extensible to
other list endpoints (e.g. POIs, routes) with minimal effort. We assume a
PostgreSQL database with Diesel ORM (using **`diesel_async`** for async queries
in Actix-web), and a connection pool (such as Deadpool or **bb8**) for database
connections.

## Pagination Crate API and Types

The `pagination` crate provides core types and functions for implementing
cursor pagination. The API is designed to be generic over different data models
and key types, while ensuring type safety and ease of integration with Diesel.
Key components of the crate include:

- **Cursor Representation:** An opaque cursor token encapsulates a position in
  the ordered dataset. Internally, we'll represent this with a `Cursor<K>`
  struct (where `K` is a struct or tuple of key fields), plus an enumeration
  for direction (next vs previous). For example:

```rust
use serde::{Serialize, Deserialize};

/// Direction of pagination relative to the cursor.
#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq)]
pub enum Direction {
    Next,    // forward in the sort order (e.g. newer items if sorting ascending)
    Prev,    // backward in the sort order (e.g. older items)
}

/// Generic cursor containing a sort key and direction.
#[derive(Serialize, Deserialize, Debug)]
struct Cursor<K> {
    dir: Direction,
    key: K,
}
```

The `key: K` holds the values of the sort key for the boundary item, e.g. for
users it might be `(DateTime, Uuid)`. The `dir` indicates whether this cursor
is meant as the starting point for a **next-page** (`Next`) or a
**previous-page** (`Prev`) query. This allows using a single `cursor` query
parameter for both directions – the server can infer how to apply the key from
the cursor content.

- **Cursor Encoding/Decoding:** We use **JSON encoding** for `Cursor<K>`
  combined with URL-safe Base64 for transport. The crate will provide helper
  methods to encode a cursor to string and decode a cursor from string:

```rust
impl<K: Serialize + for<'de> Deserialize<'de>> Cursor<K> {
    /// Encode the cursor into a base64url string (opaque to clients).
    pub fn encode(&self) -> String {
        let json = serde_json::to_vec(self).expect("Cursor serialization failed");
        base64::encode_config(json, base64::URL_SAFE_NO_PAD)
    }

    /// Decode a cursor from a base64url string.
    pub fn decode(s: &str) -> Result<Cursor<K>, serde_json::Error> {
        let bytes = base64::decode_config(s, base64::URL_SAFE_NO_PAD)
            .map_err(|e| serde_json::Error::custom(format!("Base64 decode error: {e}")))?;
        serde_json::from_slice(&bytes)
    }
}
```

Example: A JSON representation might be
`{"dir":"Next","key":{"created_at":"2025-10-10T19:17:56Z","id":"...uuid..."}}`.
After base64url encoding, the client sees a string like
`**eyJkaXIiOiJOZXh0Iiwia2V5Ijp7ImNyZWF0ZWRfYXQiOiIyMDI1LTEwLTEwVDE5OjE3OjU2WiIsImlkIjoi...**`
 (opaque and not easily guessable). **No signing or encryption** is applied in
this phase (to keep things simple), but the format is designed to be wrapped or
signed later if needed for security.

- **Key Type (`K`) and Trait:** Each paginated endpoint will define its own key
  struct (or use a tuple) corresponding to the sort key. For example, for users
  we might use:

```rust
#[derive(Serialize, Deserialize, Debug)]
struct UserCursorKey {
    created_at: chrono::DateTime<chrono::Utc>,
    id: uuid::Uuid,
}
```

We expect that `(created_at, id)` forms a unique, **totally ordered key** for
the `users` table. In general, **the combination of fields in the key must
correspond to an existing composite index in Postgres** for efficient queries.
Here we assume an index on `users(created_at, id)` so that queries using this
key for pagination are index-assisted.

The crate can provide a marker trait or helper for such key types (e.g., a
trait `PaginationKey` with perhaps an associated Diesel column tuple), but it
may be simplest to rely on explicit usage in each context. For instance, we
might implement a conversion from a `User` model to `UserCursorKey`:

```rust
impl From<&User> for UserCursorKey {
    fn from(u: &User) -> Self {
        UserCursorKey { created_at: u.created_at, id: u.id }
    }
}
```

This makes it easy to get a key from a model instance when generating cursors.

- **Paginated Response Envelope:** The crate defines a generic container for
  paginated responses. We’ll call it `Paginated<T>` with fields for the data
  list, limit, and links:

```rust
use serde::Serialize;

#[derive(Serialize)]
pub struct PaginationLinks {
    pub self_: String,
    pub next: Option<String>,
    pub prev: Option<String>,
}

#[derive(Serialize)]
pub struct Paginated<T: Serialize> {
    pub data: Vec<T>,
    pub limit: u32,
    pub links: PaginationLinks,
}
```

Here `T` is the item type (e.g. `User` DTO). We use `self_` as the field name
for the self link (since `self` is reserved in Rust). The `next` and `prev`
fields are `Option<String>` and omitted (or null in JSON) if no such page
exists. This structure aligns with the intended OpenAPI schema sketched in
issue #52 envelope outline[^issue-52-envelope] and the pagination link
component design in issue #52 link schema[^issue-52-links]:

```yaml
PaginationLinks:
  type: object
  properties:
    self:
      type: string
      description: URL of this page
    next:
      type: string
      description: URL of next page, if any
    prev:
      type: string
      description: URL of previous page, if any

PaginatedUsers:
  type: object
  properties:
    data:
      type: array
      items:
        $ref: "#/components/schemas/User"
      maxItems: 100  # enforce the guardrail of max 100 items per page
    limit:
      type: integer
      description: Number of users requested
    links:
      $ref: "#/components/schemas/PaginationLinks"
```

In implementation, we will likely use derives like `Serialize` and our OpenAPI
tool (e.g. **utoipa** or similar) to generate these schema components. We’ll
ensure `Paginated<T>` implements or derives the appropriate schema trait (e.g.
`utoipa::ToSchema`) so that the OpenAPI spec includes these new components. The
`maxItems: 100` note indicates the maximum page size.

- **Page Parameter Extractor:** For convenience, the crate can define a struct
  to represent incoming pagination query params, e.g.:

```rust
use serde::Deserialize;
#[derive(Deserialize)]
pub struct PageParams {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}
```

This struct can be used with Actix-web’s query extractor:
`web::Query<PageParams>` in handler signatures. The `cursor` will be the opaque
string from the client (if provided), and `limit` is the requested page size
(we’ll apply a default and max as needed). The crate might also provide a
default constant for `DEFAULT_PAGE_SIZE` (e.g. 20) and `MAX_PAGE_SIZE` (100),
or enforce those limits in the handler logic.

## Cursor Semantics and Structure

**Ordering:** The keyset pagination relies on a **total ordering** of records.
For `/api/users`, we use `ORDER BY created_at, id` (both ascending) as the
stable sort. This ensures no two records share the same position: if two
users have the same `created_at`, the tiebreaker `id` (e.g., a UUID or
surrogate primary key) provides a deterministic order. The ordering must remain
**consistent** for all pages of a given endpoint. We will always apply this
ordering to the query (even for the first page) to avoid nondeterministic
results. The crate can enforce this by providing the appropriate Diesel
`.order_by()` calls or by documenting that handlers must always sort by the key
fields.

**Cursor JSON Fields:** The cursor JSON contains the fields of the key. For
composite keys, we have a couple of representation options:

- **Array format:** e.g.
  `{"dir":"Next","key":["2025-10-10T19:17:56Z","<uuid>"]}` – here the array
  order matches the fields `(created_at, id)`. This is concise but relies on
  positional ordering of fields.

- **Object format:** e.g.
  `{"dir":"Next","key":{"created_at":"2025-10-10T19:17:56Z","id":"<uuid>"}}`.
  This is more verbose but self-describing and robust if we extend the key with
  additional fields later. We favor this **object format** for clarity and
  extensibility, at the cost of a slightly longer token.

**Direction Encoding:** As noted, we embed a `dir` field (`"Next"` or `"Prev"`)
in the cursor JSON to differentiate cursors for forward vs. backward
navigation. This way, the same query parameter (`cursor`) can be used for both
`next` and `prev` links. The server, upon decoding the cursor, will inspect
`cursor.dir` to know how to apply the key:

- If `dir == Next`: The cursor’s key represents the **last item of the previous
  page**. The new page should start *after* this key in the sorted order.

- If `dir == Prev`: The cursor’s key represents the **first item of the next
  page** (i.e. the page ahead of the current one). The new page should consist
  of items *before* this key in the sorted order.

This scheme lets us implement `prev` page without a separate parameter. An
alternative design could have been two separate params (e.g. `?after=...` and
`?before=...`), but using one param makes the API simpler and was chosen here
for the envelope links approach.

**Examples:** Suppose the users are sorted by join date. If a client requests
the first page (no cursor), they get the first N users
(`order_by created_at ASC, id ASC`). The response might include a `next` link
like:

```bash
"next": "/api/users?cursor=eyJkaXIiOiJOZXh0Iiwia2V5eyJjcmVhdGVkX2F0Ijoi2023...
```

If they follow that `next` link, the server will decode the cursor to
`{dir: Next, key: {created_at: X, id: Y}}`. It will then fetch users **where
`(created_at, id)` > `(X, Y)`** (continuing forward). The response will include
both a `prev` link (to go back to the earlier page) and possibly another `next`
if more users remain.

Conversely, if a client follows a `prev` link (which might be `cursor` encoding
`{dir: Prev, key: {...}}`), the server will interpret it as “fetch the page
*ending just before this key*”. In practice, the query will retrieve items with
keys < the given key (since we’re moving backwards in time) and then the
results are presented in normal sorted order. More on this query logic below.

**Security Considerations:** The cursor token is **opaque but not securely
protected**. Base64 encoding the JSON hides it from casual observation, but
clients could decode it. Since we are not signing or encrypting it (for now), a
client could also tamper with it (e.g. alter the timestamp or ID inside). This
is generally not harmful – at worst, the client can jump to an arbitrary
position in the list, which is something we allow anyway via the cursor. The
server should treat the decoded values as untrusted input (just as it would
treat a page number or filter parameter) and use them only in the intended
query context. We should:

- Validate that decoded values have the expected types and ranges (the JSON
  decoding step already enforces type structure).

- Not include any sensitive information in the cursor. In our case, `created_at`
  and `id` are fine to expose in this form; they could potentially reveal the
  chronology or density of records but not confidential data.

- In the future, if we want to prevent clients from forging cursors (to e.g.
  enumerate IDs out of context), we could **sign the cursor** (adding an HMAC
  or encryption). The current design is flexible enough to add signing later
  without changing the API (we would simply produce a different opaque string).

**Extensibility:** The cursor format (JSON inside base64) is easily extensible:

- We can add fields to the key if an endpoint requires a compound ordering (e.g.
  `(rating, created_at, id)` for a POI list sorted by rating).

- We could include metadata like the sort direction or filter context if we
  later allow clients to sort by different fields or filter results. For
  example, a cursor could include a field indicating it’s for “name ASC” vs
  “date DESC” sort, ensuring the server applies the same context when the
  cursor is used. This would bloat the token slightly, but keeps things
  consistent.

- The crate’s generic design means you can define a new key struct for each new
  use case. For instance, an endpoint listing posts might use
  `PostCursorKey { score: i32, id: Uuid }` if sorted by score, or
  `PostCursorKey { created_at: DateTime, title: String, id: Uuid }` if sorted
  by multiple fields. As long as the corresponding database index exists and
  the struct implements `Serialize/Deserialize`, everything else can reuse the
  same logic.

## Integrating Pagination in Handlers (Actix-web + Diesel)

To use this in the actual Actix handlers (like for `/api/users`), we will
follow a pattern:

- **Parse Request Query:** Use `web::Query<PageParams>` to get the optional
  `cursor` and `limit`. For example:

```rust
async fn list_users(
    db_pool: Data<DbPool>, 
    query: Query<PageParams>
) -> Result<HttpResponse, ApiError> {
    let PageParams { cursor, limit } = query.into_inner();
    let page_size = limit.unwrap_or(20).min(100) as u32;  // default 20, cap at 100
    // Decode cursor if provided
    let decoded_cursor: Option<Cursor<UserCursorKey>> = 
        cursor.as_deref().map(|c| Cursor::decode(c))
              .transpose()
              .map_err(|e| ApiError::BadRequest("Invalid cursor".into()))?;
    ...
}
```

Here `DbPool` is our async pool (could be Deadpool or bb8 – either yields an
`AsyncPgConnection`). We cap the page size at 100 to honour the guardrail
described in issue #52 page limit guidance[^issue-52-limit] and to match the
client defaults noted in issue #52 client
expectations[^issue-52-client-defaults]. If `cursor` is present, we attempt to
decode it into a `Cursor<UserCursorKey>`.

- **Build Diesel Query:** Using Diesel’s query builder, start from the base
  table and apply filters based on the cursor:

```rust
    use schema::users::dsl as users;  // Diesel schema import
    let mut query = users::users.into_boxed(); // start building a query
 
    if let Some(ref cur) = decoded_cursor {
        match cur.dir {
            Direction::Next => {
                // For next-page cursor: get items with key > cursor.key
                let UserCursorKey { created_at: c_ts, id: c_id } = cur.key;
                query = query.filter(
                    users::created_at.gt(c_ts)
                        .or(users::created_at.eq(c_ts).and(users::id.gt(c_id)))
                );
            }
            Direction::Prev => {
                // For prev-page cursor: get items with key < cursor.key
                let UserCursorKey { created_at: c_ts, id: c_id } = cur.key;
                query = query.filter(
                    users::created_at.lt(c_ts)
                        .or(users::created_at.eq(c_ts).and(users::id.lt(c_id)))
                );
            }
        }
    }
    // Apply ordering and limit (note: important to sort by the same key fields)
    query = query.order_by(users::created_at.asc()).then_order_by(users::id.asc())
                 .limit((page_size + 1) as i64);
```

A few notes on this:

- We use a **lexicographic filter** to implement `>` comparison on the composite
  key. SQL allows `(created_at, id) > (c_ts, c_id)`, but Diesel doesn't
  directly support tuple comparisons in a high-level API. Instead, we use the
  equivalent expression:

- For `Next` (assuming ascending sort):
  `created_at > c_ts OR (created_at = c_ts AND id > c_id)`.

- For `Prev`: `created_at < c_ts OR (created_at = c_ts AND id < c_id)`.

- We use Diesel’s `.into_boxed()` to build the query dynamically (needed because
  we conditionally add filters).

- **Ordering** must match the intended direction:

- For ascending order (oldest first): we do
  `.order_by(created_at.asc()).then_order_by(id.asc())`.

- If we had chosen descending (newest first), we would order by `.desc()`. But
  since our key is defined as (created_at, id) ascending, we stick to that
  here. The direction of navigation (`Next` vs `Prev`) is handled in the filter
  logic, not by flipping the sort order.

- We request `limit = page_size + 1` items. This “one extra” item is used to
  detect if there are more results beyond the current page.

- **Execute Query (Async Diesel):** Acquire a DB connection from the pool and
  load the results:

```rust
    let mut conn = db_pool.get().await?;  // Get an AsyncPgConnection
    use diesel_async::RunQueryDsl;
    let mut users_page: Vec<User> = query.load(&mut conn).await?;
```

This yields up to `page_size + 1` user records in ascending order.

- **Determine Page Boundaries:** After fetching, we determine which links to
  include and trim the results to `page_size`:

```rust
    let mut next_cursor_str = None;
    let mut prev_cursor_str = None;
    if users_page.len() as u32 > page_size {
        // More results exist beyond this page
        users_page.pop();  // remove the extra item
        // The last item of this page will be the basis for the `next` cursor
        if let Some(last_item) = users_page.last() {
            let last_key = UserCursorKey::from(last_item);
            let cursor = Cursor { dir: Direction::Next, key: last_key };
            next_cursor_str = Some(cursor.encode());
        }
    }
    // If a cursor was provided and was a 'Next' (forward) cursor, it means there are earlier items
    if let Some(cur) = decoded_cursor {
        if cur.dir == Direction::Next {
            // User came from some page after the beginning -> prev link needed
            if let Some(first_item) = users_page.first() {
                let first_key = UserCursorKey::from(first_item);
                let cursor = Cursor { dir: Direction::Prev, key: first_key };
                prev_cursor_str = Some(cursor.encode());
            }
        } else if cur.dir == Direction::Prev {
            // User came from a later (newer) page -> next link already set in next_cursor_str? 
            // Actually, in this case, the query was backward, so `users_page` contains older items.
            // We should generate a `next` link pointing to the newest item in this page, 
            // which is actually the *last* item in the returned list (because we sorted asc).
            if let Some(last_item) = users_page.last() {
                let last_key = UserCursorKey::from(last_item);
                let cursor = Cursor { dir: Direction::Next, key: last_key };
                next_cursor_str = Some(cursor.encode());
            }
            // And also determine if more older items exist:
            if users_page.len() as u32 == page_size {
                // We fetched exactly page_size (no extra), so we don't definitively know if more older exist.
                // But if we got exactly the limit without the extra, it likely means there *were* no extra 
                // (or we would have gotten one). So probably no older items remain -> no prev link.
                // If we wanted to be sure, we could fetch page_size+1 in both directions, but that complicates logic.
            }
        } else {
            // if no cursor was provided (first page), prev_cursor_str remains None
        }
    }
```

Let’s clarify the logic:

- We check if the result contains `page_size + 1` items. If yes, we know there
  **is a next page** beyond this one. We trim the extra item off. The last item
  of the trimmed list becomes the end boundary of the current page. We generate
  a `next_cursor` from it, with `Direction::Next`.

- For the `prev` link:

- If the request had **no cursor** (first page), we obviously don’t set a `prev`
  link (there are no earlier items).

- If the request’s cursor was `Next` (meaning the client was on page 2 or beyond
  in forward direction), that implies there are items before the current page.
  The **first item of the current page** is the boundary to go back. We take
  that first item and create a cursor with `Direction::Prev` (meaning “page
  ending before this item”), and encode it for the `prev` link.

- If the request’s cursor was `Prev` (the client was going backwards in time),
  that implies there are items after the current page (newer items) –
  essentially we came from a later page. In this scenario, we ensure a `next`
  link is set pointing forward. In the code above, we set `next_cursor_str` for
  `Prev` as well using the last item (which is actually the newest in the
  current older segment). We also consider the possibility of even older pages
  (`prev` link when going backwards): if we fetched an extra item in the
  backward query, we would know older items remain. In our logic, we used
  `limit + 1` uniformly, so if `users_page.len() > page_size` before trimming
  in a backward query, that would indicate a further `prev` (older) page
  exists. We should handle that similarly by setting `prev_cursor_str` (even in
  backward mode) using the first item of the list as boundary. (The snippet
  above hints at this, but we should add it explicitly.)

To summarize:

- **`next_cursor_str`** (for `next` link) is derived from the last item of the
  current page if we know there are more items ahead (newer in ascending sort).

- **`prev_cursor_str`** (for `prev` link) is derived from the first item of the
  current page if there are items before it (older in ascending sort). We infer
  that either from the presence of an `after` cursor in the request or, in case
  of backward pagination, from retrieving an extra item.

After this, we have at most `page_size` items in `users_page`, and the
appropriate cursor strings (or None).

- **Generate Hypermedia Links:** Using the cursor strings and current request
  info, build the `PaginationLinks`. We need the **self URL**, which is the URL
  of the current request. Actix’s `HttpRequest` can be used to get the path and
  query. For simplicity, we can reconstruct it:

```rust
    let base_path = "/api/users"; // or derive from request route name
    // Reconstruct self link: current path plus original cursor if any
    let self_link = if let Some(cur_str) = &cursor {
        format!("{}?cursor={}&limit={}", base_path, cur_str, page_size)
    } else {
        // first page might include limit if non-default
        if limit.is_some() {
            format!("{}?limit={}", base_path, page_size)
        } else {
            base_path.to_string()
        }
    };
    let links = PaginationLinks {
        self_: self_link,
        next: next_cursor_str.as_ref().map(|c| format!("{}?cursor={}&limit={}", base_path, c, page_size)),
        prev: prev_cursor_str.as_ref().map(|c| format!("{}?cursor={}&limit={}", base_path, c, page_size)),
    };
```

A few details:

- We ensure the `limit` parameter is included in links to maintain the same page
  size if the client explicitly set one.

- The `base_path` might be obtained programmatically (hardcoding is fine for
  now, but in a real implementation we might use `req.url_for()` with route
  naming, or combine `req.path()` and query).

- We use the raw cursor strings in the URLs as query params. They are already
  URL-safe (base64url ensures no `+`/`/` and no padding `=`), so they should
  not need special encoding beyond normal URL encoding. We should confirm that
  `base64::URL_SAFE_NO_PAD` produces URL-legal characters (it does: uses `-`
  and `_`).

- **Return Response:** Finally, package the data and metadata into our
  `Paginated` struct and return as JSON:

```rust
    let response_body = Paginated {
        data: users_page,
        limit: page_size,
        links,
    };
    Ok(HttpResponse::Ok().json(response_body))
}
```

The JSON output will look like:

```json
{
  "data": [ { /* user1 */ }, { /* user2 */ }, ... ],
  "limit": 20,
  "links": {
    "self": "/api/users?limit=20", 
    "next": "/api/users?cursor=eyJkaXIiOiJOZXh0Iiwia2V5Ijp7ImNyZWF0ZWRfYXQiOiIyMDI1LTA...&limit=20",
    "prev": null
  }
}
```

On a subsequent page, `prev` would be a URL and `self` would include the
`cursor` used.

**OpenAPI updates:** We will update the OpenAPI spec to reflect these changes:

- **Query parameters:** Document the `cursor` (string, base64url token) and
  `limit` (integer, 1-100) query parameters for the endpoint, following the
  expectations in issue #52 client expectations[^issue-52-client-defaults].

- **Components:** Add `PaginationLinks` and a paginated response schema for each
  resource (or a generic one parameterised by item type). In our case,
  `PaginatedUsers` is defined as above, matching the outline from issue #52
  link schema[^issue-52-links]. If using **utoipa**, we can implement
  `ToSchema` for `PaginationLinks` and `Paginated<T>` and use them in the
  endpoint documentation, for example:

```rust
/// Response for a paginated users list.
#[derive(utoipa::ToSchema)]
struct PaginatedUsersResponse {
    data: Vec<User>,   // assuming User has ToSchema
    limit: u32,
    links: PaginationLinks,
}
```

We ensure the schema’s `maxItems` constraint for `data` (Max 100) is captured —
utoipa allows using attributes like `#[schema(max_items = 100)]` on the field
if needed, or we enforce via validation logic. The OpenAPI description should
note that `next`/`prev` may be omitted or null if no such page.

- **Backward Compatibility:** The new response is not a plain array but an
  object. This is a breaking change for clients, but since it’s needed for
  correct pagination, we document it and bump the API version or clearly
  communicate it. (If this is still MVP and no external clients yet, it’s fine.)

## Diesel Query Efficiency and Examples

Using Diesel with keyset pagination requires careful use of indices and query
construction:

**Index Alignment:** Ensure that the database has an index matching the sort
key. For example, on the `users` table:

```sql
CREATE INDEX idx_users_created_at_id ON users (created_at, id);
```

This index allows the query with
`created_at > X OR (created_at = X AND id > Y)` to use an index range scan,
rather than a full table scan. Similarly, that index covers the
`ORDER BY created_at, id` so the results are already sorted from the index.

**Composite Filter in Diesel:** Diesel does not have built-in syntax for
composite comparisons, but the OR condition shown above is equivalent. Another
approach is to use Diesel’s `SqlLiteral` or custom expression to directly
inject `(users.created_at, users.id) > ($1, $2)` if one wanted to rely on SQL
tuple comparison. However, the OR approach is fine and portable. Diesel’s query
builder ensures the values are parameterized to prevent SQL injection.

**Reverse Ordering for Prev Page:** In our design, we chose to keep a
consistent ascending sort in the query and adjust the filter for prev/next. An
alternative implementation for `Prev` could be:

- Query in **reverse order** (descending) for a `Prev` request to fetch the
  preceding page more directly, then reverse the results in memory. For
  example, for a `Prev` cursor, do:

```rust
query = query.order_by(users::created_at.desc()).then_order_by(users::id.desc())
             .limit(page_size + 1);
// filter: created_at < c_ts OR (created_at = c_ts AND id < c_id)
```

This would retrieve one extra older item beyond the page. You would then trim
the extra and **reverse** the list before returning, to still present ascending
order to the client. This approach is more complex in code (because you have to
branch the sorting and remember to reverse output), so our design sticks to a
single sort order (asc) and handles prev via logic. Both approaches are valid;
the chosen method favors simplicity of maintaining one ordering code path.

**Usage Example for Another Endpoint:** To demonstrate reuse, imagine an
endpoint `/api/points` that lists points of interest sorted by `name`
(alphabetically) then `id` to break ties. We could define:

```rust
struct PointCursorKey { name: String, id: Uuid }
```

and similar logic: filter with
`name > last_name OR (name = last_name AND id > last_id)` etc. As long as
there’s an index on `(name, id)`, the performance will be good. The crate’s
`Cursor` type and encoding work the same way for `PointCursorKey`. We’d just
integrate it in the handler for points.

**Connection Pool (bb8) Compatibility:** Our design is agnostic to the pooling
mechanism. We get an `AsyncPgConnection` from the pool (`bb8` or `deadpool` or
Diesel’s own connection manager). The Diesel async API (via
`diesel_async::RunQueryDsl`) operates on `&mut AsyncPgConnection`, which is
exactly what pools provide. For example, using bb8:

```rust
let conn = pool.get().await?; // pool is bb8::Pool<AsyncDieselConnectionManager<AsyncPgConnection>>
let results = query.load(&mut *conn).await?;
```

This is essentially identical to deadpool’s usage (deadpool’s `Object`
dereferences to `AsyncPgConnection` via `DerefMut`). The pagination crate does
not need to know which pool is used; it only deals with the connection or query
builder. If needed, we could provide a utility in the crate like:

```rust
async fn fetch_page<Q, C, I, K>(
    query: Q,
    conn: &mut C,
    cursor: Option<Cursor<K>>,
    page_size: u32,
) -> Result<Paginated<I>>
```

Here `Q` is a Diesel query and `C` is an `AsyncConnection`, but in practice it
is often clearer to write the few lines of logic in the handler as we did above.

By following the example integration for `/api/users`, developers can easily
adapt the pattern:

- Define the key type for the new endpoint.

- Use `Cursor::decode` and the appropriate filter logic.

- Ensure ordering by the key fields.

- Use the crate’s `Cursor` to generate `next`/`prev` cursors from query results.

- Wrap in `Paginated<T>` and return with links.

## OpenAPI Schema and Documentation Updates

As mentioned, we will update the OpenAPI documentation to include the new
pagination format:

- New **components** `PaginationLinks` and e.g. `PaginatedUsers` as shown above.

- Modify the `/api/users` endpoint spec:

- Add `cursor` and `limit` as query parameters (with descriptions like *"Opaque
  cursor for pagination (base64 encoded)"*, and *"Page size (max 100, default
  20)"*).

- Change the response schema to `$ref: '#/components/schemas/PaginatedUsers'`
  instead of an array of `User`.

- Provide an example in the docs showing a sample response with `links` and
  `data`.

If using code-first documentation (utoipa), ensure the handler function or its
context is annotated accordingly, e.g.:

```rust
/// List users (paginated).
#[utoipa::path(
    get, path = "/api/users", security=[("bearerAuth" = [])],
    params(
        ("cursor" = Option<String>, Query, description = "Pagination cursor"),
        ("limit" = Option<u32>, Query, description = "Page size (1-100, default 20)")
    ),
    responses(
        (status = 200, description = "Successful list of users", body = PaginatedUsersResponse)
    )
)]
```

The above would reference the `PaginatedUsersResponse` schema we defined.

We also update any Postman collections or documentation pages to reflect that
the response is now an object with `data` and `links`, etc., and that clients
should use the provided `next/prev` URLs rather than manipulating offsets.

## Future Enhancements and Conclusion

This pagination crate lays the groundwork for a consistent, efficient paging
mechanism across the Wildside API:

- It ensures **performance** by leveraging indexed queries and avoiding large
  offsets or full scans.

- It preserves **consistency** even if new records are inserted during paging –
  the stable ordering by `(created_at, id)` means a newly inserted user will
  either appear on a future `next` page (if it’s chronologically after the
  current page’s end) or not affect already retrieved pages. (If real-time
  consistency across pages is needed, we might include a snapshot identifier in
  the cursor, but that’s out of scope for now.)

- The approach is **client-friendly**: consumers of the API simply follow `next`
  and `prev` links, without needing to know the underlying keys or craft
  complex queries. This mirrors the client-flow guidance in issue #52 rollout
  notes[^issue-52-rollout].

- We intentionally avoid exposing internal details, and the opaque cursor can be
  extended or secured in the future without breaking clients.

In summary, the new `pagination` crate will provide the Wildside backend with a
generic way to do keyset pagination. The `/api/users` endpoint will serve as
the first implementation, returning a `PaginatedUsers` response that includes
up to 100 users per page, along with easy navigation links. This design can
then be rolled out to other listing endpoints to improve the API’s scalability
and usability, as highlighted in issue #52 rollout notes[^issue-52-rollout].

[^issue-52-client-defaults]: [GitHub issue #52 – client defaults](https://github.com/leynos/wildside/issues/52#L70-L74)
[^issue-52-envelope]: [GitHub issue #52 – envelope outline](https://github.com/leynos/wildside/issues/52#L34-L43)
[^issue-52-limit]: [GitHub issue #52 – page limit guidance](https://github.com/leynos/wildside/issues/52#L54-L61)
[^issue-52-links]: [GitHub issue #52 – link schema](https://github.com/leynos/wildside/issues/52#L48-L57)
[^issue-52-requirements]: [GitHub issue #52 – requirements](https://github.com/leynos/wildside/issues/52#L19-L24)
[^issue-52-rollout]: [GitHub issue #52 – rollout notes](https://github.com/leynos/wildside/issues/52#L75-L80)
