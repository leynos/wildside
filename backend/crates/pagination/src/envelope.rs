//! Paginated response envelopes and hypermedia links.

use serde::Serialize;
use url::{Url, form_urlencoded};

use crate::PageParams;

/// Hypermedia links for one paginated response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PaginationLinks {
    /// Canonical link for the current page.
    #[serde(rename = "self")]
    pub self_: String,
    /// Link for the next page, if a following page exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    /// Link for the previous page, if an earlier page exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev: Option<String>,
}

impl PaginationLinks {
    /// Construct a set of links directly from concrete values.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pagination::PaginationLinks;
    ///
    /// let links = PaginationLinks::new(
    ///     "https://example.test/api/v1/users?limit=25&cursor=current-token".to_owned(),
    ///     Some("https://example.test/api/v1/users?limit=25&cursor=next-token".to_owned()),
    ///     None,
    /// );
    ///
    /// assert_eq!(
    ///     links.next.as_deref(),
    ///     Some("https://example.test/api/v1/users?limit=25&cursor=next-token")
    /// );
    /// assert_eq!(links.prev, None);
    /// ```
    #[must_use]
    pub const fn new(self_: String, next: Option<String>, prev: Option<String>) -> Self {
        Self { self_, next, prev }
    }

    /// Build links for the current request URL and page parameters.
    ///
    /// Existing non-pagination query parameters are preserved.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pagination::{PageParams, PaginationLinks};
    /// use url::Url;
    ///
    /// let request_url = Url::parse(
    ///     "https://example.test/api/v1/users?role=admin&limit=1&cursor=stale",
    /// )
    /// .expect("request url should parse");
    /// let params = PageParams::new(Some("current-token".to_owned()), Some(25))
    ///     .expect("page params should be valid");
    ///
    /// let links = PaginationLinks::from_request(
    ///     &request_url,
    ///     &params,
    ///     Some("next-token"),
    ///     Some("prev-token"),
    /// );
    ///
    /// assert_eq!(
    ///     links.self_,
    ///     "https://example.test/api/v1/users?role=admin&limit=25&cursor=current-token"
    /// );
    /// assert_eq!(
    ///     links.next.as_deref(),
    ///     Some("https://example.test/api/v1/users?role=admin&limit=25&cursor=next-token")
    /// );
    /// assert_eq!(
    ///     links.prev.as_deref(),
    ///     Some("https://example.test/api/v1/users?role=admin&limit=25&cursor=prev-token")
    /// );
    /// ```
    #[must_use]
    pub fn from_request(
        request_url: &Url,
        params: &PageParams,
        next_cursor: Option<&str>,
        prev_cursor: Option<&str>,
    ) -> Self {
        Self {
            self_: build_page_link(request_url, params.limit(), params.cursor()),
            next: next_cursor
                .map(|cursor| build_page_link(request_url, params.limit(), Some(cursor))),
            prev: prev_cursor
                .map(|cursor| build_page_link(request_url, params.limit(), Some(cursor))),
        }
    }
}

/// Generic paginated response envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Paginated<T> {
    /// Page items in stable ordering.
    pub data: Vec<T>,
    /// Effective page size used for this response.
    pub limit: usize,
    /// Hypermedia links for traversing the result set.
    pub links: PaginationLinks,
}

impl<T> Paginated<T> {
    /// Construct an envelope from explicit link values.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pagination::{Paginated, PaginationLinks};
    ///
    /// let page = Paginated::new(
    ///     vec![1, 2, 3],
    ///     3,
    ///     PaginationLinks::new(
    ///         "https://example.test/api/v1/numbers?limit=3".to_owned(),
    ///         Some("https://example.test/api/v1/numbers?limit=3&cursor=next".to_owned()),
    ///         None,
    ///     ),
    /// );
    ///
    /// assert_eq!(page.data, vec![1, 2, 3]);
    /// assert_eq!(page.limit, 3);
    /// assert_eq!(
    ///     page.links.next.as_deref(),
    ///     Some("https://example.test/api/v1/numbers?limit=3&cursor=next")
    /// );
    /// ```
    #[must_use]
    pub const fn new(data: Vec<T>, limit: usize, links: PaginationLinks) -> Self {
        Self { data, limit, links }
    }
}

fn build_page_link(request_url: &Url, limit: usize, cursor: Option<&str>) -> String {
    let mut url = request_url.clone();
    let retained_pairs = request_url
        .query_pairs()
        .filter(|(key, _)| key != "limit" && key != "cursor")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();

    let mut serializer = form_urlencoded::Serializer::new(String::new());
    for (key, value) in retained_pairs {
        serializer.append_pair(&key, &value);
    }
    serializer.append_pair("limit", &limit.to_string());
    if let Some(cursor_token) = cursor {
        serializer.append_pair("cursor", cursor_token);
    }

    url.set_query(Some(&serializer.finish()));
    url.into()
}

#[cfg(test)]
mod tests {
    //! Unit tests for pagination link generation.

    use url::Url;

    use crate::{PageParams, Paginated, PaginationLinks};

    #[test]
    fn paginated_links_preserve_non_pagination_query_parameters() {
        let params =
            PageParams::new(Some("current".to_owned()), Some(50)).expect("params should be valid");
        let request_url =
            Url::parse("https://example.test/api/v1/users?role=admin&cursor=stale&limit=1")
                .expect("request url should be valid");

        let page = Paginated::new(
            vec!["Ada"],
            params.limit(),
            PaginationLinks::from_request(
                &request_url,
                &params,
                Some("next-token"),
                Some("prev-token"),
            ),
        );

        assert_eq!(
            page.links.self_,
            "https://example.test/api/v1/users?role=admin&limit=50&cursor=current"
        );
        assert_eq!(
            page.links.next.as_deref(),
            Some("https://example.test/api/v1/users?role=admin&limit=50&cursor=next-token")
        );
        assert_eq!(
            page.links.prev.as_deref(),
            Some("https://example.test/api/v1/users?role=admin&limit=50&cursor=prev-token")
        );
    }
}
