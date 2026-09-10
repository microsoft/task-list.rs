use utoipa::OpenApi;

use tasklist_application::dto::{CreateTaskRequest, TaskDto, TaskStatusDto};

use crate::routes::health::HealthResponse;

/// The OpenAPI document. `utoipa` derives it from the annotated handlers and DTOs; the
/// web build regenerates `web/src/ApiClient.generated.ts` from it via openapi-typescript.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "task-list.rs API",
        version = "0.1.0",
        description = "Walking-skeleton API for the task-list.rs clean-architecture starter."
    ),
    paths(
        crate::routes::health::health,
        crate::routes::tasks::list_tasks,
        crate::routes::tasks::create_task
    ),
    components(schemas(HealthResponse, TaskDto, TaskStatusDto, CreateTaskRequest)),
    tags(
        (name = "health", description = "Liveness"),
        (name = "tasks", description = "Task queries")
    )
)]
pub struct ApiDoc;
