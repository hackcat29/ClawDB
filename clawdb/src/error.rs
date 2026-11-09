//! Error types for the `clawdb` wrapper.

use claw_guard::error::GuardError;

/// Result alias for the wrapper crate.
pub type ClawDBResult<T> = Result<T, ClawDBError>;

/// Unified error type for the wrapper.
#[derive(Debug, thiserror::Error)]
pub enum ClawDBError {
    /// Storage error from claw-core.
    #[error("storage error: {0}")]
    Core(#[from] claw_core::ClawError),
    /// Vector error from claw-vector.
    #[error("vector error: {0}")]
    Vector(#[from] claw_vector::VectorError),
    /// Branch error from claw-branch.
    #[error("branch error: {0}")]
    Branch(#[from] claw_branch::BranchError),
    /// Sync error from claw-sync.
    #[error("sync error: {0}")]
    Sync(#[from] claw_sync::SyncError),
    /// Guard error from claw-guard.
    #[error("guard error: {0}")]
    Guard(#[from] GuardError),
    /// Reflect error from claw-reflect-client.
    #[error("reflect error: {0}")]
    Reflect(#[from] claw_reflect_client::ReflectError),
    /// Permission denied.
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    /// Component disabled.
    #[error("component disabled: {0}")]
    ComponentDisabled(&'static str),
    /// Component initialization error.
    #[error("component init failed ({0}): {1}")]
    ComponentInit(&'static str, String),
    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),
    /// Session invalid or expired.
    #[error("session expired or invalid")]
    SessionInvalid,

    /// Sending memories to the reflect service failed.
    #[error("reflect ingest failed: {0}")]
    ReflectIngest(String),

    /// A referenced record or branch does not exist.
    #[error("not found: {0}")]
    NotFound(String),
    /// Transaction error.
    #[error("transaction error: {0}")]
    Transaction(String),
    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    /// I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Errors caused by the request rather than the server. Servers map these to
/// non-retryable statuses instead of INTERNAL, which clients retry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientErrorKind {
    /// The request itself is malformed (e.g. an invalid branch name).
    InvalidArgument,
    /// The thing being created already exists.
    AlreadyExists,
    /// The request is valid but the current state doesn't allow it.
    FailedPrecondition,
}

impl ClawDBError {
    /// Classifies errors caused by the caller; `None` for server faults.
    pub fn client_error_kind(&self) -> Option<ClientErrorKind> {
        use claw_branch::BranchError as B;
        match self {
            Self::Branch(B::InvalidBranchName(_) | B::NamingError(_)) => {
                Some(ClientErrorKind::InvalidArgument)
            }
            Self::Branch(B::BranchAlreadyExists(_)) => Some(ClientErrorKind::AlreadyExists),
            Self::Branch(
                B::BranchNotActive { .. }
                | B::BranchLimitExceeded
                | B::MergeConflictUnresolved { .. }
                | B::MergeIncompatibleBase { .. },
            ) => Some(ClientErrorKind::FailedPrecondition),
            _ => None,
        }
    }

    /// True when the error means the referenced thing doesn't exist, from any
    /// component, so servers can answer NOT_FOUND instead of INTERNAL.
    pub fn is_not_found(&self) -> bool {
        match self {
            Self::NotFound(_) => true,
            Self::Core(claw_core::ClawError::NotFound { .. }) => true,
            Self::Branch(error) => error.is_not_found(),
            _ => false,
        }
    }
}
