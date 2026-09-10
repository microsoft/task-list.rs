//! HTTP-level tests for the api crate, exercised via axum's `tower::ServiceExt::oneshot`
//! against the composed router (no network).

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use tasklist_api::{build_router, seed_demo_state};

fn app() -> axum::Router {
    // The SPA dir need not exist for /api tests; those requests never reach the fallback.
    build_router(seed_demo_state(), "web/dist")
}

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// A `POST /api/tasks` request with a JSON body.
fn post_task(body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/api/tasks")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// A `POST /api/tasks` request with a raw (possibly malformed) body and an optional
/// `Content-Type` — used to exercise the body-extractor rejection paths.
fn post_task_raw(body: &str, content_type: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().method("POST").uri("/api/tasks");
    if let Some(ct) = content_type {
        builder = builder.header(header::CONTENT_TYPE, ct);
    }
    builder.body(Body::from(body.to_owned())).unwrap()
}

/// Assert an RFC7807 `application/problem+json` response with the given status.
async fn assert_problem_json(response: axum::response::Response, status: StatusCode) {
    assert_eq!(response.status(), status);
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert_eq!(content_type, "application/problem+json");
    let json = body_json(response).await;
    assert_eq!(json["status"], status.as_u16());
    assert!(json["title"].as_str().is_some_and(|t| !t.is_empty()));
    assert!(json["detail"].as_str().is_some_and(|d| !d.is_empty()));
}

#[tokio::test]
async fn health_returns_ok_status() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["status"], "ok");
    assert!(json["time"].as_str().is_some_and(|t| !t.is_empty()));
}

#[tokio::test]
async fn tasks_returns_seeded_task_through_the_port() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/tasks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    let tasks = json.as_array().expect("array of tasks");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["title"], "Welcome to task-list.rs");
    assert_eq!(tasks[0]["owner"], "demo|user");
    assert_eq!(tasks[0]["status"], "todo");
}

#[tokio::test]
async fn openapi_json_is_served() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(
        json["openapi"].as_str().unwrap().split('.').next(),
        Some("3")
    );
    assert!(json["paths"]["/api/health"].is_object());
}

#[tokio::test]
async fn unknown_api_route_is_rfc7807_not_found() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/nope")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert_eq!(content_type, "application/problem+json");
    let json = body_json(response).await;
    assert_eq!(json["status"], 404);
    assert_eq!(json["title"], "Not Found");
}

#[tokio::test]
async fn create_task_returns_201_with_the_created_task() {
    let response = app()
        .oneshot(post_task(json!({ "title": "  Buy milk  " })))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let json = body_json(response).await;
    // Title is trimmed by the domain; status defaults to Todo; the id is server-minted.
    assert_eq!(json["title"], "Buy milk");
    assert_eq!(json["status"], "todo");
    assert_eq!(json["owner"], "demo|user");
    assert!(json["id"].as_str().is_some_and(|id| !id.is_empty()));
}

#[tokio::test]
async fn create_task_with_invalid_title_is_rfc7807_bad_request() {
    let response = app()
        .oneshot(post_task(json!({ "title": "   " })))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert_eq!(content_type, "application/problem+json");
    let json = body_json(response).await;
    assert_eq!(json["status"], 400);
    assert_eq!(json["title"], "Bad Request");
    assert!(json["detail"].as_str().is_some_and(|d| !d.is_empty()));
}

#[tokio::test]
async fn created_task_appears_in_subsequent_list() {
    let app = app();

    let created = app
        .clone()
        .oneshot(post_task(json!({ "title": "Ship it" })))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_id = body_json(created).await["id"].as_str().unwrap().to_owned();

    let listed = app
        .oneshot(
            Request::builder()
                .uri("/api/tasks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let tasks = body_json(listed).await;
    let tasks = tasks.as_array().expect("array of tasks");
    // The demo seed task plus the one we just created.
    assert_eq!(tasks.len(), 2);
    assert!(tasks
        .iter()
        .any(|t| t["id"] == created_id && t["title"] == "Ship it"));
}

#[tokio::test]
async fn create_task_missing_title_field_is_rfc7807_bad_request() {
    // Well-formed JSON that fails the schema (missing `title`) is folded into 400
    // problem+json by the shared extractor — never a bare 422 text/plain.
    let response = app().oneshot(post_task(json!({}))).await.unwrap();
    assert_problem_json(response, StatusCode::BAD_REQUEST).await;
}

#[tokio::test]
async fn create_task_malformed_json_is_rfc7807_bad_request() {
    let response = app()
        .oneshot(post_task_raw("{ not valid json", Some("application/json")))
        .await
        .unwrap();
    assert_problem_json(response, StatusCode::BAD_REQUEST).await;
}

#[tokio::test]
async fn create_task_missing_content_type_is_rfc7807_unsupported_media_type() {
    let response = app()
        .oneshot(post_task_raw(r#"{"title":"hi"}"#, None))
        .await
        .unwrap();
    assert_problem_json(response, StatusCode::UNSUPPORTED_MEDIA_TYPE).await;
}
