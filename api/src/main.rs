use std::process::ExitCode;

use tasklist_infrastructure::AppConfig;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    // `tasklist-api openapi` prints the OpenAPI spec and exits. Used by the CI drift
    // check to regenerate the committed spec + TypeScript client.
    if std::env::args().nth(1).as_deref() == Some("openapi") {
        println!("{}", tasklist_api::openapi_pretty_json());
        return ExitCode::SUCCESS;
    }

    init_tracing();

    let config = AppConfig::from_env();
    let state = match tasklist_api::build_state(&config).await {
        Ok(state) => state,
        Err(error) => {
            tracing::error!(%error, persistence = ?config.persistence, "failed to build application state");
            return ExitCode::FAILURE;
        }
    };
    let app = tasklist_api::build_router(state, &config.web_dist_dir);

    let listener = match tokio::net::TcpListener::bind(&config.bind_address).await {
        Ok(listener) => listener,
        Err(error) => {
            tracing::error!(%error, address = %config.bind_address, "failed to bind listener");
            return ExitCode::FAILURE;
        }
    };

    tracing::info!(
        address = %config.bind_address,
        environment = ?config.environment,
        "task-list.rs api listening"
    );

    if let Err(error) = axum::serve(listener, app).await {
        tracing::error!(%error, "server error");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
