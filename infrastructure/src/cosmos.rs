//! Cosmos DB adapter for the [`TaskRepository`] port.
//!
//! This is the *only* place Azure/Cosmos SDK types appear — the mapping to and from the
//! preview `azure_data_cosmos` crate is fully contained here, so `domain`/`application`
//! stay free of any crate names or types (the Dependency-Inversion proof). A [`Task`] is
//! stored as a [`TaskDocument`] (`id`, `ownerId`, the task fields, and the server-managed
//! `_etag`); reads/writes are scoped to a single partition (`/ownerId`); and updates use
//! `If-Match` optimistic concurrency, surfacing a 412 as [`ApplicationError::Conflict`].

use std::borrow::Cow;
use std::sync::Arc;

use async_trait::async_trait;
use azure_core::credentials::{Secret, TokenCredential};
use azure_core::http::Etag;
use azure_data_cosmos::clients::ContainerClient;
use azure_data_cosmos::feed::FeedScope;
use azure_data_cosmos::models::{ContainerProperties, PartitionKeyDefinition};
use azure_data_cosmos::options::{
    ConnectionPoolOptions, ItemWriteOptions, Precondition, Region, ServerCertificateValidation,
};
use azure_data_cosmos::{
    AccountEndpoint, AccountReference, CosmosClient, CosmosError, CosmosRuntime, Query,
    RoutingStrategy,
};
use azure_identity::DeveloperToolsCredential;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use tasklist_application::error::ApplicationError;
use tasklist_application::ports::TaskRepository;
use tasklist_domain::{ETag, Task, TaskId, TaskStatus, Title, UserId};

use crate::config::{CosmosSettings, Environment};

/// Well-known Cosmos DB emulator authentication key. This is a fixed, publicly documented
/// value baked into every emulator image (not a secret), used only when `TASKLIST_ENV=local`
/// and no explicit key is configured.
const EMULATOR_KEY: &str =
    "C2y6yDjf5/R+ob0N8A7Cgv30VRDJIWEHLM+4QDU5DE2nQ9nDuVTqobD4b8mGGyPMbIZnqyMsEcaGQy67XIw/Jw==";

/// Failure while establishing the Cosmos connection (or provisioning the emulator). Kept
/// as a plain-string infrastructure error so no Azure/Cosmos type leaks out of this crate.
#[derive(Debug, thiserror::Error)]
pub enum CosmosStartupError {
    #[error("invalid Cosmos configuration: {0}")]
    Config(String),
    #[error("failed to connect to Cosmos: {0}")]
    Connect(String),
    #[error("failed to provision Cosmos database/container: {0}")]
    Provision(String),
}

/// The serde shape of a task as stored in a Cosmos container. Field names use the Cosmos
/// conventions (`id`, camelCase, `_etag`); `_etag` is server-managed, so it is read back
/// but never written.
#[derive(Debug, Serialize, Deserialize)]
struct TaskDocument {
    id: String,
    #[serde(rename = "ownerId")]
    owner_id: String,
    title: String,
    status: String,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
    /// Server-managed optimistic-concurrency token. Present on reads/queries; skipped on
    /// writes (Cosmos owns it).
    #[serde(rename = "_etag", default, skip_serializing)]
    etag: Option<String>,
}

impl TaskDocument {
    fn from_task(task: &Task) -> Result<Self, ApplicationError> {
        Ok(Self {
            id: task.id.to_string(),
            owner_id: task.owner.as_str().to_owned(),
            title: task.title.as_str().to_owned(),
            status: status_to_str(task.status).to_owned(),
            created_at: task
                .created_at
                .format(&Rfc3339)
                .map_err(|e| ApplicationError::Repository(e.to_string()))?,
            updated_at: task
                .updated_at
                .format(&Rfc3339)
                .map_err(|e| ApplicationError::Repository(e.to_string()))?,
            etag: None,
        })
    }

    /// Rehydrate a domain [`Task`], preferring `etag_override` (captured from the response
    /// header on a point op) over the body `_etag` (available on list queries).
    fn into_task(self, etag_override: Option<ETag>) -> Result<Task, ApplicationError> {
        let bad =
            |what: &str| ApplicationError::Repository(format!("corrupt task document: {what}"));

        let uuid = uuid::Uuid::parse_str(&self.id).map_err(|_| bad("id"))?;
        let owner = UserId::parse(self.owner_id).map_err(|_| bad("ownerId"))?;
        let title = Title::parse(self.title).map_err(|_| bad("title"))?;
        let status = status_from_str(&self.status).ok_or_else(|| bad("status"))?;
        let created_at =
            OffsetDateTime::parse(&self.created_at, &Rfc3339).map_err(|_| bad("createdAt"))?;
        let updated_at =
            OffsetDateTime::parse(&self.updated_at, &Rfc3339).map_err(|_| bad("updatedAt"))?;
        let version = etag_override.or_else(|| self.etag.map(ETag::new));

        Ok(Task {
            id: TaskId::from_uuid(uuid),
            owner,
            title,
            status,
            created_at,
            updated_at,
            version,
        })
    }
}

fn status_to_str(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Todo => "todo",
        TaskStatus::InProgress => "in_progress",
        TaskStatus::Done => "done",
    }
}

fn status_from_str(raw: &str) -> Option<TaskStatus> {
    match raw {
        "todo" => Some(TaskStatus::Todo),
        "in_progress" => Some(TaskStatus::InProgress),
        "done" => Some(TaskStatus::Done),
        _ => None,
    }
}

/// Map a Cosmos SDK error onto the application-level error contract:
/// 404 → [`NotFound`](ApplicationError::NotFound); 412/409 → [`Conflict`](ApplicationError::Conflict);
/// everything else → [`Repository`](ApplicationError::Repository) (opaque 500).
fn map_cosmos_error(error: CosmosError) -> ApplicationError {
    let status = error.status();
    if status.is_not_found() {
        ApplicationError::NotFound
    } else if status.is_precondition_failed() || status.is_conflict() {
        ApplicationError::Conflict
    } else {
        ApplicationError::Repository(error.to_string())
    }
}

/// `TaskRepository` backed by an Azure Cosmos DB container. Selected by the composition
/// root when `TASKLIST_PERSISTENCE=cosmos`.
pub struct CosmosTaskRepository {
    container: ContainerClient,
}

impl CosmosTaskRepository {
    /// Connect to Cosmos using `settings`. When `environment` is [`Environment::Local`],
    /// the client trusts the emulator's self-signed certificate and the database/container
    /// are provisioned on the fly; for [`Environment::Cloud`] the resources are assumed to
    /// be created by infrastructure-as-code and a token credential is used.
    pub async fn connect(
        settings: &CosmosSettings,
        environment: Environment,
    ) -> Result<Self, CosmosStartupError> {
        let client = build_client(settings, environment).await?;

        if environment == Environment::Local {
            provision(&client, settings).await?;
        }

        let container = client
            .database_client(&settings.database)
            .container_client(&settings.container)
            .await
            .map_err(|e| CosmosStartupError::Connect(e.to_string()))?;

        Ok(Self { container })
    }
}

/// Build a [`CosmosClient`] for the selected environment.
async fn build_client(
    settings: &CosmosSettings,
    environment: Environment,
) -> Result<CosmosClient, CosmosStartupError> {
    let endpoint: AccountEndpoint = settings
        .endpoint
        .parse()
        .map_err(|e: CosmosError| CosmosStartupError::Config(e.to_string()))?;

    // A routing region is required by the SDK; it is irrelevant against the single-region
    // emulator and is overridden by IaC/region config in the cloud (a later task).
    let strategy = RoutingStrategy::ProximityTo(Region::WEST_US);
    let mut builder = CosmosClient::builder();

    let account = match environment {
        Environment::Local => {
            // The emulator presents a self-signed certificate; a runtime configured with
            // `RequiredUnlessEmulator` trusts it while keeping full validation elsewhere.
            let pool = ConnectionPoolOptions::builder()
                .with_server_certificate_validation(
                    ServerCertificateValidation::RequiredUnlessEmulator,
                )
                .build()
                .map_err(|e| CosmosStartupError::Config(e.to_string()))?;
            let runtime = CosmosRuntime::builder()
                .with_connection_pool(pool)
                .build()
                .await
                .map_err(|e| CosmosStartupError::Config(e.to_string()))?;
            builder = builder.with_runtime(runtime);

            let key = settings
                .key
                .clone()
                .unwrap_or_else(|| EMULATOR_KEY.to_owned());
            AccountReference::with_authentication_key(endpoint, Secret::from(key))
        }
        Environment::Cloud => {
            // Dev/az-login today; Container Apps managed identity swaps in at Task 7.
            let credential: Arc<dyn TokenCredential> = DeveloperToolsCredential::new(None)
                .map_err(|e| CosmosStartupError::Config(e.to_string()))?;
            AccountReference::with_credential(endpoint, credential)
        }
    };

    builder
        .build(account, strategy)
        .await
        .map_err(|e| CosmosStartupError::Connect(e.to_string()))
}

/// Create the configured database and container if they don't already exist (emulator
/// only). Cosmos has no native "create if not exists", so a 409 Conflict is treated as
/// "already provisioned".
async fn provision(
    client: &CosmosClient,
    settings: &CosmosSettings,
) -> Result<(), CosmosStartupError> {
    let ignore_conflict = |error: CosmosError| -> Result<(), CosmosStartupError> {
        if error.status().is_conflict() {
            Ok(())
        } else {
            Err(CosmosStartupError::Provision(error.to_string()))
        }
    };

    if let Err(error) = client.create_database(&settings.database, None).await {
        ignore_conflict(error)?;
    }

    let properties = ContainerProperties::new(
        settings.container.clone(),
        PartitionKeyDefinition::new(vec![Cow::Borrowed("/ownerId")]),
    );
    if let Err(error) = client
        .database_client(&settings.database)
        .create_container(properties, None)
        .await
    {
        ignore_conflict(error)?;
    }

    Ok(())
}

/// Build a Cosmos partition-key argument from an owner. The SDK's `PartitionKey` accepts
/// an owned `String`, so we hand it one scoped to this call.
fn partition_key(owner: &UserId) -> String {
    owner.as_str().to_owned()
}

#[async_trait]
impl TaskRepository for CosmosTaskRepository {
    async fn list_for_owner(&self, owner: &UserId) -> Result<Vec<Task>, ApplicationError> {
        let query = Query::from("SELECT * FROM c");
        let mut iter = self
            .container
            .query_items::<TaskDocument>(query, FeedScope::partition(partition_key(owner)), None)
            .await
            .map_err(map_cosmos_error)?;

        let mut tasks = Vec::new();
        while let Some(doc) = iter.try_next().await.map_err(map_cosmos_error)? {
            // On the list path the etag lives in the document body (`_etag`).
            tasks.push(doc.into_task(None)?);
        }
        Ok(tasks)
    }

    async fn get(&self, owner: &UserId, id: &TaskId) -> Result<Task, ApplicationError> {
        let response = self
            .container
            .read_item(partition_key(owner), &id.to_string(), None)
            .await
            .map_err(map_cosmos_error)?;

        // Capture the header etag before `into_model` consumes the response.
        let etag = response.headers().etag().map(|e| ETag::new(e.as_ref()));
        let doc: TaskDocument = response.into_model().map_err(map_cosmos_error)?;
        doc.into_task(etag)
    }

    async fn create(&self, task: &Task) -> Result<Task, ApplicationError> {
        let doc = TaskDocument::from_task(task)?;
        let response = self
            .container
            .create_item(partition_key(&task.owner), &doc.id, &doc, None)
            .await
            .map_err(map_cosmos_error)?;

        let etag = response.headers().etag().map(|e| ETag::new(e.as_ref()));
        let mut created = task.clone();
        created.version = etag;
        Ok(created)
    }

    async fn update(&self, task: &Task) -> Result<Task, ApplicationError> {
        let doc = TaskDocument::from_task(task)?;

        // Optimistic concurrency: guard the replace with the caller's etag via `If-Match`.
        // A stale etag makes Cosmos return 412, which `map_cosmos_error` maps to Conflict.
        let options = task.version.as_ref().map(|version| {
            ItemWriteOptions::default().with_precondition(Precondition::IfMatch(Etag::from(
                version.as_str().to_owned(),
            )))
        });

        let response = self
            .container
            .replace_item(partition_key(&task.owner), &doc.id, &doc, options)
            .await
            .map_err(map_cosmos_error)?;

        let etag = response.headers().etag().map(|e| ETag::new(e.as_ref()));
        let mut updated = task.clone();
        updated.version = etag;
        Ok(updated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_round_trips_through_strings() {
        for status in [TaskStatus::Todo, TaskStatus::InProgress, TaskStatus::Done] {
            assert_eq!(status_from_str(status_to_str(status)), Some(status));
        }
        assert_eq!(status_from_str("bogus"), None);
    }

    #[test]
    fn document_maps_to_task_and_back() {
        let owner = UserId::parse("auth0|abc").unwrap();
        let task = Task::new(
            TaskId::new(),
            owner,
            Title::parse("Write the adapter").unwrap(),
            TaskStatus::InProgress,
            OffsetDateTime::UNIX_EPOCH,
        );

        let doc = TaskDocument::from_task(&task).unwrap();
        assert_eq!(doc.owner_id, "auth0|abc");
        assert_eq!(doc.status, "in_progress");
        assert!(doc.etag.is_none());

        let rehydrated = doc.into_task(Some(ETag::new("etag-from-server"))).unwrap();
        assert_eq!(rehydrated.id, task.id);
        assert_eq!(rehydrated.title, task.title);
        assert_eq!(rehydrated.status, TaskStatus::InProgress);
        assert_eq!(
            rehydrated.version,
            Some(ETag::new("etag-from-server")),
            "header etag override wins"
        );
    }

    #[test]
    fn body_etag_used_when_no_override() {
        let doc = TaskDocument {
            id: TaskId::new().to_string(),
            owner_id: "auth0|abc".to_owned(),
            title: "t".to_owned(),
            status: "done".to_owned(),
            created_at: "1970-01-01T00:00:00Z".to_owned(),
            updated_at: "1970-01-01T00:00:00Z".to_owned(),
            etag: Some("body-etag".to_owned()),
        };
        let task = doc.into_task(None).unwrap();
        assert_eq!(task.version, Some(ETag::new("body-etag")));
    }

    #[test]
    fn corrupt_document_is_a_repository_error() {
        let doc = TaskDocument {
            id: "not-a-uuid".to_owned(),
            owner_id: "auth0|abc".to_owned(),
            title: "t".to_owned(),
            status: "todo".to_owned(),
            created_at: "1970-01-01T00:00:00Z".to_owned(),
            updated_at: "1970-01-01T00:00:00Z".to_owned(),
            etag: None,
        };
        let err = doc.into_task(None).unwrap_err();
        assert!(matches!(err, ApplicationError::Repository(_)));
    }
}
