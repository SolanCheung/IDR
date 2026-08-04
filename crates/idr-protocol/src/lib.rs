//! Independent protocol and deterministic policy layer.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct ReferenceV1(String);

impl ReferenceV1 {
    pub fn new(value: impl Into<String>) -> Result<Self, ReferenceValidationErrorV1> {
        let value = value.into();
        if value.trim().is_empty()
            || value.trim() != value
            || value.len() > 512
            || value.chars().any(char::is_control)
        {
            return Err(ReferenceValidationErrorV1);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("reference is invalid")]
pub struct ReferenceValidationErrorV1;

impl<'de> Deserialize<'de> for ReferenceV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

impl fmt::Display for ReferenceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityContextV1 {
    subject_ref: ReferenceV1,
    actor_ref: ReferenceV1,
    caller_ref: ReferenceV1,
    tenant_ref: ReferenceV1,
    scope_ref: ReferenceV1,
    purpose_ref: ReferenceV1,
    operation_ref: ReferenceV1,
}

impl AuthorityContextV1 {
    pub fn new(
        subject_ref: ReferenceV1,
        actor_ref: ReferenceV1,
        caller_ref: ReferenceV1,
        tenant_ref: ReferenceV1,
        scope_ref: ReferenceV1,
        purpose_ref: ReferenceV1,
        operation_ref: ReferenceV1,
    ) -> Self {
        Self {
            subject_ref,
            actor_ref,
            caller_ref,
            tenant_ref,
            scope_ref,
            purpose_ref,
            operation_ref,
        }
    }

    pub fn subject_ref(&self) -> &ReferenceV1 {
        &self.subject_ref
    }

    pub fn actor_ref(&self) -> &ReferenceV1 {
        &self.actor_ref
    }

    pub fn caller_ref(&self) -> &ReferenceV1 {
        &self.caller_ref
    }

    pub fn tenant_ref(&self) -> &ReferenceV1 {
        &self.tenant_ref
    }

    pub fn scope_ref(&self) -> &ReferenceV1 {
        &self.scope_ref
    }

    pub fn purpose_ref(&self) -> &ReferenceV1 {
        &self.purpose_ref
    }

    pub fn operation_ref(&self) -> &ReferenceV1 {
        &self.operation_ref
    }

    pub fn validate(&self) -> Result<(), ReferenceValidationErrorV1> {
        for reference in [
            &self.subject_ref,
            &self.actor_ref,
            &self.caller_ref,
            &self.tenant_ref,
            &self.scope_ref,
            &self.purpose_ref,
            &self.operation_ref,
        ] {
            ReferenceV1::new(reference.as_str())?;
        }
        Ok(())
    }
}

// Compatibility aliases keep the V1 wire model stable while the independent
// names above remain the public integration vocabulary.
pub type ProductionReferenceV1 = ReferenceV1;
pub type ProductionAuthorityContextV1 = AuthorityContextV1;

#[cfg(feature = "legacy-shadow-api")]
pub mod human_centered;
pub mod production;
#[cfg(feature = "legacy-shadow-api")]
pub use human_centered::*;
pub use production::*;
