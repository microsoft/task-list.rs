//! JSON request/response wrapper that keeps the error contract uniform.
//!
//! axum's built-in [`axum::Json`] extractor renders body rejections (malformed JSON,
//! schema mismatch, wrong content-type) as `text/plain`, bypassing our RFC7807 seam. This
//! thin wrapper maps those rejections to [`ApiError`] (→ `application/problem+json`) on the
//! way in and serialises responses exactly like `axum::Json` on the way out, so every
//! mutation handler inherits one consistent error contract by simply using this type.

use axum::extract::{FromRequest, Request};
use axum::response::{IntoResponse, Response};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::ApiError;

/// Drop-in replacement for [`axum::Json`]: RFC7807 request-body rejections, identical
/// response serialisation.
#[derive(Debug, Clone, Copy)]
pub struct Json<T>(pub T);

impl<T, S> FromRequest<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        // Reuse axum's parsing, then funnel any rejection through `From<JsonRejection>`.
        let axum::Json(value) = axum::Json::<T>::from_request(req, state).await?;
        Ok(Self(value))
    }
}

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}
