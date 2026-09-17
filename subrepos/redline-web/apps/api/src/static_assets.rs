//! Embedded single-page-application assets.
//!
//! The built frontend (`web/dist`) is embedded into the binary via
//! [`rust_embed`]. Unknown, non-`/api` GET routes fall back to `index.html` so
//! client-side routing works; matched asset paths are served with a
//! content-type guessed from their extension.

use axum::body::Body;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

/// Embedded contents of the built frontend.
#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../web/dist"]
pub struct Assets;

/// Axum fallback handler: serve a matching embedded asset, otherwise the SPA
/// shell (`index.html`). Returns 404 only for `/api` paths and when no
/// `index.html` is embedded.
pub async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // API routes must never be swallowed by the SPA fallback.
    if path.starts_with("api/") || path == "api" {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }

    if !path.is_empty()
        && let Some(asset) = Assets::get(path)
    {
        return serve(path, asset);
    }

    match Assets::get("index.html") {
        Some(asset) => serve("index.html", asset),
        None => (StatusCode::NOT_FOUND, "index.html not embedded").into_response(),
    }
}

fn serve(path: &str, asset: rust_embed::EmbeddedFile) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    (
        [(header::CONTENT_TYPE, mime.as_ref().to_string())],
        Body::from(asset.data.into_owned()),
    )
        .into_response()
}
