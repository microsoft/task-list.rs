//! API layer: axum host, routers, OpenAPI, RFC7807 error mapping, SPA serving and the
//! composition root. Depends on `application` + `infrastructure` + `domain` and wires
//! ports to adapters via `AppState`.

mod error;
mod extract;
mod openapi;
mod routes;
mod spa;
mod state;

use std::sync::Arc;

use axum::routing::get;
use axum::Router;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::{Config, SwaggerUi};

use tasklist_application::ports::{Clock, TaskRepository};
use tasklist_application::ApplicationError;
use tasklist_domain::{Task, TaskId, TaskStatus, Title, UserId};
use tasklist_infrastructure::{
    AppConfig, CosmosSettings, CosmosTaskRepository, InMemoryTaskRepository, Persistence,
    SystemClock,
};

pub use error::ApiError;
pub use openapi::ApiDoc;
pub use state::AppState;

/// Build the full application router: `/api/*` (health, tasks, openapi.json) with an
/// RFC7807 fallback, the Swagger UI at `/swagger`, and the SPA served with an
/// `index.html` fallback for everything else.
pub fn build_router(state: AppState, web_dist_dir: &str) -> Router {
    let api = Router::new()
        .route("/health", get(routes::health::health))
        .route(
            "/tasks",
            get(routes::tasks::list_tasks).post(routes::tasks::create_task),
        )
        .route("/openapi.json", get(routes::openapi_json))
        .fallback(routes::not_found)
        .with_state(state);

    Router::new()
        .nest("/api", api)
        // Swagger UI renders the spec fetched from our own `/api/openapi.json` handler,
        // so the canonical spec URL is served in exactly one place.
        .merge(SwaggerUi::new("/swagger").config(Config::new(["/api/openapi.json"])))
        .fallback_service(spa::spa_service(web_dist_dir))
        .layer(TraceLayer::new_for_http())
}

/// Compose the walking-skeleton [`AppState`]: an in-memory repository seeded with a demo
/// task and a system clock. The demo owner stands in until Auth lands in a later task.
/// Used by tests and as the default (`TASKLIST_PERSISTENCE=memory`) path.
pub fn seed_demo_state() -> AppState {
    let (demo_owner, demo_task) = demo_owner_and_task();
    AppState {
        repo: Arc::new(InMemoryTaskRepository::seeded(vec![demo_task])),
        clock: Arc::new(SystemClock),
        demo_owner,
    }
}

/// Compose the [`AppState`] for the selected persistence adapter. With
/// `TASKLIST_PERSISTENCE=memory` (default) this is the in-memory skeleton path; with
/// `cosmos` it connects the real Cosmos adapter and seeds the demo task once, so the same
/// `GET /api/tasks` handler round-trips a real Cosmos document — with no change to
/// `domain`/`application`. Returns a startup error string on failure.
pub async fn build_state(config: &AppConfig) -> Result<AppState, String> {
    let (demo_owner, demo_task) = demo_owner_and_task();

    let repo: Arc<dyn TaskRepository> = match config.persistence {
        Persistence::Memory => Arc::new(InMemoryTaskRepository::seeded(vec![demo_task])),
        Persistence::Cosmos => {
            let settings = CosmosSettings::from_env();
            let repo = CosmosTaskRepository::connect(&settings, config.environment)
                .await
                .map_err(|e| e.to_string())?;
            // Seed the demo task once so the skeleton path has data to round-trip; ignore
            // a Conflict if a previous run already created it.
            let existing = repo
                .list_for_owner(&demo_owner)
                .await
                .map_err(|e| e.to_string())?;
            if existing.is_empty() {
                match repo.create(&demo_task).await {
                    Ok(_) | Err(ApplicationError::Conflict) => {}
                    Err(e) => return Err(e.to_string()),
                }
            }
            Arc::new(repo)
        }
    };

    Ok(AppState {
        repo,
        clock: Arc::new(SystemClock),
        demo_owner,
    })
}

/// The demo owner and its single seed task, shared by both composition paths.
fn demo_owner_and_task() -> (UserId, Task) {
    let demo_owner = UserId::parse("demo|user").expect("demo owner is valid");
    let demo_task = Task::new(
        TaskId::new(),
        demo_owner.clone(),
        Title::parse("Welcome to task-list.rs").expect("demo title is valid"),
        TaskStatus::Todo,
        SystemClock.now(),
    );
    (demo_owner, demo_task)
}

/// The OpenAPI document as pretty JSON. Emitted by the `openapi` CLI subcommand and
/// consumed by the CI drift check (openapi-typescript regeneration).
pub fn openapi_pretty_json() -> String {
    ApiDoc::openapi()
        .to_pretty_json()
        .expect("OpenAPI serializes to JSON")
}
