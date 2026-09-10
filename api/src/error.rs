use axum::extract::rejection::JsonRejection;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use tasklist_application::ApplicationError;

/// An API error rendered as an RFC7807 `application/problem+json` response. This is the
/// mapping seam every handler's `Result` funnels through.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    title: &'static str,
    detail: Option<String>,
}

impl ApiError {
    pub fn new(status: StatusCode, title: &'static str, detail: Option<String>) -> Self {
        Self {
            status,
            title,
            detail,
        }
    }
}

/// RFC7807 problem document.
#[derive(Debug, Serialize)]
struct ProblemDetails {
    #[serde(rename = "type")]
    type_uri: &'static str,
    title: &'static str,
    status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ProblemDetails {
            type_uri: "about:blank",
            title: self.title,
            status: self.status.as_u16(),
            detail: self.detail,
        };
        let mut response = (self.status, Json(body)).into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/problem+json"),
        );
        response
    }
}

impl From<ApplicationError> for ApiError {
    fn from(error: ApplicationError) -> Self {
        match error {
            ApplicationError::Domain(domain) => ApiError::new(
                StatusCode::BAD_REQUEST,
                "Bad Request",
                Some(domain.to_string()),
            ),
            ApplicationError::NotFound => ApiError::new(StatusCode::NOT_FOUND, "Not Found", None),
            ApplicationError::Conflict => {
                ApiError::new(StatusCode::CONFLICT, "Conflict", Some(error.to_string()))
            }
            // Never leak internal repository details to the client.
            ApplicationError::Repository(_) => ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                None,
            ),
        }
    }
}

impl From<JsonRejection> for ApiError {
    /// Map request-body extractor rejections to RFC7807 (never axum's default
    /// text/plain). A missing/incorrect `Content-Type` is 415; every other bad body
    /// (malformed JSON syntax, schema mismatch, unreadable bytes) folds into 400, so
    /// "any bad request body" is consistently 400 — matching the domain-invalid path.
    /// The rejection's own message is client-facing input feedback, safe to surface.
    fn from(rejection: JsonRejection) -> Self {
        let (status, title) = match &rejection {
            JsonRejection::MissingJsonContentType(_) => {
                (StatusCode::UNSUPPORTED_MEDIA_TYPE, "Unsupported Media Type")
            }
            _ => (StatusCode::BAD_REQUEST, "Bad Request"),
        };
        ApiError::new(status, title, Some(rejection.body_text()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_maps_to_409() {
        let api: ApiError = ApplicationError::Conflict.into();
        assert_eq!(api.status, StatusCode::CONFLICT);
        assert_eq!(api.title, "Conflict");
        assert!(api.detail.is_some());
    }

    #[test]
    fn not_found_maps_to_404_without_detail() {
        let api: ApiError = ApplicationError::NotFound.into();
        assert_eq!(api.status, StatusCode::NOT_FOUND);
        assert!(api.detail.is_none());
    }

    #[test]
    fn repository_error_maps_to_500_without_leaking_detail() {
        let api: ApiError = ApplicationError::Repository("secret db url".to_owned()).into();
        assert_eq!(api.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(api.detail.is_none(), "internal details must not leak");
    }
}
