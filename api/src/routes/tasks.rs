use axum::extract::State;
use axum::http::StatusCode;

use tasklist_application::dto::{CreateTaskRequest, TaskDto};

use crate::error::ApiError;
use crate::extract::Json;
use crate::state::AppState;

#[utoipa::path(
    get,
    path = "/api/tasks",
    tag = "tasks",
    responses((status = 200, description = "The caller's tasks", body = [TaskDto]))
)]
pub async fn list_tasks(State(state): State<AppState>) -> Result<Json<Vec<TaskDto>>, ApiError> {
    // handler -> use-case -> TaskRepository port -> adapter: the dependency-inversion path.
    let tasks = tasklist_application::tasks::list(state.repo.as_ref(), &state.demo_owner).await?;
    Ok(Json(tasks))
}

#[utoipa::path(
    post,
    path = "/api/tasks",
    tag = "tasks",
    request_body = CreateTaskRequest,
    responses(
        (status = 201, description = "The created task", body = TaskDto),
        (
            status = 400,
            description = "Invalid or malformed request body (bad title, malformed JSON, or schema mismatch)",
            content_type = "application/problem+json"
        ),
        (
            status = 415,
            description = "Unsupported media type (expected application/json)",
            content_type = "application/problem+json"
        )
    )
)]
pub async fn create_task(
    State(state): State<AppState>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<TaskDto>), ApiError> {
    let task = tasklist_application::tasks::create(
        state.repo.as_ref(),
        state.clock.as_ref(),
        &state.demo_owner,
        req,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(task)))
}
