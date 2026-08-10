use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum IdrError {
    #[error("invalid contract: {0}")]
    InvalidContract(String),
    #[error("unknown decision: {0}")]
    UnknownDecision(String),
    #[error("host model provider failed: {0}")]
    HostModel(String),
    #[error("host capability violation: {0}")]
    CapabilityViolation(String),
    #[error("resolution failed closed: {0}")]
    Unresolved(String),
    #[error("conflicting feedback replay for decision: {0}")]
    FeedbackConflict(String),
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{message}")]
pub struct HostModelProviderError {
    pub message: String,
}

impl HostModelProviderError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
