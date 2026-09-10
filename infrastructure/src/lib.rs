//! Infrastructure layer: port adapters and runtime configuration.
//!
//! The only crate that references Azure SDKs — the Cosmos adapter and all
//! `azure_data_cosmos`/`azure_core`/`azure_identity` types are contained here. It also
//! supplies an in-memory `TaskRepository`, a `SystemClock`, and a config loader with the
//! `TASKLIST_ENV` / `TASKLIST_PERSISTENCE` seams.

mod clock;
mod config;
mod cosmos;
mod repository;

pub use clock::SystemClock;
pub use config::{AppConfig, CosmosSettings, Environment, Persistence};
pub use cosmos::{CosmosStartupError, CosmosTaskRepository};
pub use repository::InMemoryTaskRepository;
