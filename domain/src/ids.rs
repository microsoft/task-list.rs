use std::fmt;

use uuid::Uuid;

use crate::DomainError;

/// Time-ordered task identifier (uuidv7). Time-ordering makes it a good Cosmos item id
/// and lets a client mint ids offline (a later task) without collisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(Uuid);

impl TaskId {
    /// Mint a fresh, time-ordered id.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Wrap an existing uuid (e.g. when rehydrating from persistence).
    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    /// The underlying uuid.
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The owner of a task — the Auth0 subject (`sub`), used as the Cosmos partition key.
/// Must be non-blank.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserId(String);

impl UserId {
    /// Parse a raw subject into a `UserId`, rejecting blank input.
    pub fn parse(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        if raw.trim().is_empty() {
            return Err(DomainError::EmptyUserId);
        }
        Ok(Self(raw))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A task title: 1..=200 characters after trimming, non-blank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Title(String);

impl Title {
    const MAX_CHARS: usize = 200;

    /// Parse a raw title, trimming surrounding whitespace and enforcing length bounds.
    pub fn parse(raw: impl Into<String>) -> Result<Self, DomainError> {
        let trimmed = raw.into().trim().to_owned();
        let len = trimmed.chars().count();
        if len == 0 || len > Self::MAX_CHARS {
            return Err(DomainError::InvalidTitle);
        }
        Ok(Self(trimmed))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Title {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Optimistic-concurrency token for a persisted task — mirrors the Cosmos server-managed
/// `_etag`. It is an opaque server-issued string (the client only echoes it back on
/// update), so it has no structural invariant beyond being non-empty; a task that has not
/// been persisted yet has no `ETag` (see [`Task::version`](crate::Task::version)).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ETag(String);

impl ETag {
    /// Wrap a server-issued etag string (e.g. when rehydrating from persistence).
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ETag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_ids_are_unique_and_time_ordered() {
        let a = TaskId::new();
        let b = TaskId::new();
        assert_ne!(a, b);
        // uuidv7 is time-ordered, so a later id sorts after an earlier one.
        assert!(a.as_uuid() < b.as_uuid());
    }

    #[test]
    fn user_id_rejects_blank() {
        assert_eq!(UserId::parse("   "), Err(DomainError::EmptyUserId));
        assert_eq!(UserId::parse(""), Err(DomainError::EmptyUserId));
    }

    #[test]
    fn user_id_accepts_subject() {
        let u = UserId::parse("auth0|abc123").unwrap();
        assert_eq!(u.as_str(), "auth0|abc123");
    }

    #[test]
    fn title_trims_and_accepts_valid() {
        let t = Title::parse("  Buy milk  ").unwrap();
        assert_eq!(t.as_str(), "Buy milk");
    }

    #[test]
    fn title_rejects_empty() {
        assert_eq!(Title::parse("   "), Err(DomainError::InvalidTitle));
    }

    #[test]
    fn title_rejects_over_200_chars() {
        let long = "a".repeat(201);
        assert_eq!(Title::parse(long), Err(DomainError::InvalidTitle));
    }

    #[test]
    fn title_accepts_exactly_200_chars() {
        let max = "a".repeat(200);
        assert!(Title::parse(max).is_ok());
    }
}
