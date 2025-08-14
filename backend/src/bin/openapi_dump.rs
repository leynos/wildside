//! Print the OpenAPI document as JSON.

use backend::doc::ApiDoc;
use utoipa::OpenApi;

fn main() {
    println!(
        "{}",
        ApiDoc::openapi()
            .to_json()
            .expect("serialising OpenAPI document"),
    );
}
