use crate::{ProductionAuthorityContextV1, ProductionReferenceV1};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;
use uuid::Uuid;

pub const HUMAN_CENTERED_SCHEMA_VERSION: u16 = 1;
pub const HUMAN_CENTERED_RULE_VERSION: u64 = 1;
pub const HUMAN_CENTERED_MAX_SAFE_WIRE_INTEGER: u64 = 9_007_199_254_740_991;

macro_rules! uuid_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub fn mint() -> Self {
                Self(Uuid::new_v4())
            }

            pub fn from_uuid(value: Uuid) -> Result<Self, HumanCenteredProtocolError> {
                let version = value.get_version_num();
                if value.is_nil()
                    || value.get_variant() != uuid::Variant::RFC4122
                    || !(1..=8).contains(&version)
                {
                    return Err(HumanCenteredProtocolError::EmptyIdentifier);
                }
                Ok(Self(value))
            }

            pub fn as_uuid(self) -> Uuid {
                self.0
            }

            pub fn is_empty(self) -> bool {
                self.0.is_nil()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}", self.0.hyphenated())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = Uuid::deserialize(deserializer)?;
                Self::from_uuid(value).map_err(D::Error::custom)
            }
        }
    };
}

uuid_id!(HumanCenteredRunIdV1);
uuid_id!(HumanCenteredTurnIdV1);
uuid_id!(HumanCenteredContractIdV1);
uuid_id!(HumanModelAssertionIdV1);
uuid_id!(HumanCenteredInputEventIdV1);
uuid_id!(HumanCenteredProofIdV1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HumanCenteredContractKindV1 {
    Intent,
    Decision,
    TurnCoordination,
    Response,
    Action,
    ExecutionReceipt,
    Outcome,
    HumanModelAssertion,
    HumanModelUpdateCandidate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct UnitIntervalBasisPointsV1(u16);

impl UnitIntervalBasisPointsV1 {
    pub const ZERO: Self = Self(0);
    pub const FULL: Self = Self(10_000);

    pub fn new(value: u16) -> Result<Self, HumanCenteredProtocolError> {
        if value > 10_000 {
            return Err(HumanCenteredProtocolError::InvalidBasisPoints(value));
        }
        Ok(Self(value))
    }

    pub fn value(self) -> u16 {
        self.0
    }
}

impl<'de> Deserialize<'de> for UnitIntervalBasisPointsV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u16::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactLevelV1 {
    Low,
    Medium,
    High,
    Critical,
}

impl ImpactLevelV1 {
    pub const fn rank(self) -> u8 {
        match self {
            Self::Low => 0,
            Self::Medium => 1,
            Self::High => 2,
            Self::Critical => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HumanCenteredContractRefV1 {
    kind: HumanCenteredContractKindV1,
    contract_id: HumanCenteredContractIdV1,
    revision: u64,
    record_digest: String,
}

impl HumanCenteredContractRefV1 {
    pub fn new(
        kind: HumanCenteredContractKindV1,
        contract_id: HumanCenteredContractIdV1,
        revision: u64,
        record_digest: impl Into<String>,
    ) -> Result<Self, HumanCenteredProtocolError> {
        let value = Self {
            kind,
            contract_id,
            revision,
            record_digest: record_digest.into(),
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), HumanCenteredProtocolError> {
        if self.contract_id.is_empty()
            || self.revision == 0
            || self.revision > HUMAN_CENTERED_MAX_SAFE_WIRE_INTEGER
            || !valid_digest(&self.record_digest)
        {
            return Err(HumanCenteredProtocolError::InvalidContractReference);
        }
        Ok(())
    }

    pub fn kind(&self) -> HumanCenteredContractKindV1 {
        self.kind
    }

    pub fn contract_id(&self) -> HumanCenteredContractIdV1 {
        self.contract_id
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn record_digest(&self) -> &str {
        &self.record_digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HumanCenteredContractMetadataV1 {
    contract_kind: HumanCenteredContractKindV1,
    contract_id: HumanCenteredContractIdV1,
    run_id: HumanCenteredRunIdV1,
    turn_id: HumanCenteredTurnIdV1,
    revision: u64,
    predecessor_ref: Option<HumanCenteredContractRefV1>,
    authority_context: ProductionAuthorityContextV1,
    evidence_refs: Vec<ProductionReferenceV1>,
    dependency_refs: Vec<HumanCenteredContractRefV1>,
    policy_revision_ref: ProductionReferenceV1,
    valid_until: u64,
    schema_version: u16,
}

impl HumanCenteredContractMetadataV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn initial(
        contract_kind: HumanCenteredContractKindV1,
        run_id: HumanCenteredRunIdV1,
        turn_id: HumanCenteredTurnIdV1,
        authority_context: ProductionAuthorityContextV1,
        evidence_refs: Vec<ProductionReferenceV1>,
        dependency_refs: Vec<HumanCenteredContractRefV1>,
        policy_revision_ref: ProductionReferenceV1,
        valid_until: u64,
    ) -> Result<Self, HumanCenteredProtocolError> {
        Self::build(
            contract_kind,
            HumanCenteredContractIdV1::mint(),
            run_id,
            turn_id,
            1,
            None,
            authority_context,
            evidence_refs,
            dependency_refs,
            policy_revision_ref,
            valid_until,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn successor(
        contract_kind: HumanCenteredContractKindV1,
        previous_ref: HumanCenteredContractRefV1,
        run_id: HumanCenteredRunIdV1,
        turn_id: HumanCenteredTurnIdV1,
        authority_context: ProductionAuthorityContextV1,
        evidence_refs: Vec<ProductionReferenceV1>,
        dependency_refs: Vec<HumanCenteredContractRefV1>,
        policy_revision_ref: ProductionReferenceV1,
        valid_until: u64,
    ) -> Result<Self, HumanCenteredProtocolError> {
        previous_ref.validate()?;
        if previous_ref.kind() != contract_kind {
            return Err(HumanCenteredProtocolError::ContractKindMismatch);
        }
        let revision = previous_ref
            .revision()
            .checked_add(1)
            .ok_or(HumanCenteredProtocolError::RevisionOverflow)?;
        Self::build(
            contract_kind,
            previous_ref.contract_id(),
            run_id,
            turn_id,
            revision,
            Some(previous_ref),
            authority_context,
            evidence_refs,
            dependency_refs,
            policy_revision_ref,
            valid_until,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        contract_kind: HumanCenteredContractKindV1,
        contract_id: HumanCenteredContractIdV1,
        run_id: HumanCenteredRunIdV1,
        turn_id: HumanCenteredTurnIdV1,
        revision: u64,
        predecessor_ref: Option<HumanCenteredContractRefV1>,
        authority_context: ProductionAuthorityContextV1,
        evidence_refs: Vec<ProductionReferenceV1>,
        dependency_refs: Vec<HumanCenteredContractRefV1>,
        policy_revision_ref: ProductionReferenceV1,
        valid_until: u64,
    ) -> Result<Self, HumanCenteredProtocolError> {
        let value = Self {
            contract_kind,
            contract_id,
            run_id,
            turn_id,
            revision,
            predecessor_ref,
            authority_context,
            evidence_refs: canonical_references(evidence_refs)?,
            dependency_refs: canonical_contract_references(dependency_refs)?,
            policy_revision_ref,
            valid_until,
            schema_version: HUMAN_CENTERED_SCHEMA_VERSION,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), HumanCenteredProtocolError> {
        if self.contract_id.is_empty()
            || self.run_id.is_empty()
            || self.turn_id.is_empty()
            || self.revision == 0
            || self.revision > HUMAN_CENTERED_MAX_SAFE_WIRE_INTEGER
            || self.valid_until == 0
            || self.valid_until > HUMAN_CENTERED_MAX_SAFE_WIRE_INTEGER
            || self.policy_revision_ref.as_str().trim().is_empty()
            || self.schema_version != HUMAN_CENTERED_SCHEMA_VERSION
        {
            return Err(HumanCenteredProtocolError::InvalidMetadata);
        }
        self.authority_context.validate()?;
        match (&self.predecessor_ref, self.revision) {
            (None, 1) => {}
            (Some(previous), revision)
                if previous.kind() == self.contract_kind
                    && previous.contract_id() == self.contract_id
                    && previous.revision().checked_add(1) == Some(revision) =>
            {
                previous.validate()?;
            }
            _ => return Err(HumanCenteredProtocolError::InvalidRevisionLineage),
        }
        for reference in &self.dependency_refs {
            reference.validate()?;
            if reference.contract_id() == self.contract_id && reference.revision() >= self.revision
            {
                return Err(HumanCenteredProtocolError::InvalidDependency);
            }
        }
        Ok(())
    }

    pub fn contract_kind(&self) -> HumanCenteredContractKindV1 {
        self.contract_kind
    }

    pub fn contract_id(&self) -> HumanCenteredContractIdV1 {
        self.contract_id
    }

    pub fn run_id(&self) -> HumanCenteredRunIdV1 {
        self.run_id
    }

    pub fn turn_id(&self) -> HumanCenteredTurnIdV1 {
        self.turn_id
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn authority_context(&self) -> &ProductionAuthorityContextV1 {
        &self.authority_context
    }

    pub fn evidence_refs(&self) -> &[ProductionReferenceV1] {
        &self.evidence_refs
    }

    pub fn dependency_refs(&self) -> &[HumanCenteredContractRefV1] {
        &self.dependency_refs
    }

    pub fn predecessor_ref(&self) -> Option<&HumanCenteredContractRefV1> {
        self.predecessor_ref.as_ref()
    }

    pub fn valid_until(&self) -> u64 {
        self.valid_until
    }

    pub fn policy_revision_ref(&self) -> &ProductionReferenceV1 {
        &self.policy_revision_ref
    }
}

pub(crate) fn contract_digest<T: Serialize + ?Sized>(
    domain: &str,
    value: &T,
) -> Result<String, HumanCenteredProtocolError> {
    let bytes = serde_json::to_vec(&(domain, value))
        .map_err(|_| HumanCenteredProtocolError::Serialization)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn validate_required_text(value: &str) -> Result<(), HumanCenteredProtocolError> {
    if value.trim().is_empty() || value.len() > 4_096 || value.chars().any(char::is_control) {
        return Err(HumanCenteredProtocolError::InvalidText);
    }
    Ok(())
}

pub(crate) fn validate_text_collection(
    values: &[String],
) -> Result<(), HumanCenteredProtocolError> {
    if values.len() > 256 {
        return Err(HumanCenteredProtocolError::CollectionLimitExceeded);
    }
    for value in values {
        validate_required_text(value)?;
    }
    Ok(())
}

pub(crate) fn validate_json_payload(value: &Value) -> Result<(), HumanCenteredProtocolError> {
    const MAX_BYTES: usize = 1_048_576;
    const MAX_DEPTH: usize = 32;
    const MAX_NODES: usize = 10_000;

    if serde_json::to_vec(value)
        .map_err(|_| HumanCenteredProtocolError::Serialization)?
        .len()
        > MAX_BYTES
    {
        return Err(HumanCenteredProtocolError::PayloadLimitExceeded);
    }
    let mut stack = vec![(value, 1_usize)];
    let mut nodes = 0_usize;
    while let Some((node, depth)) = stack.pop() {
        nodes += 1;
        if depth > MAX_DEPTH || nodes > MAX_NODES {
            return Err(HumanCenteredProtocolError::PayloadLimitExceeded);
        }
        match node {
            Value::Array(values) => {
                stack.extend(values.iter().map(|child| (child, depth + 1)));
            }
            Value::Object(values) => {
                stack.extend(values.values().map(|child| (child, depth + 1)));
            }
            _ => {}
        }
    }
    Ok(())
}

pub(crate) fn canonical_references(
    mut values: Vec<ProductionReferenceV1>,
) -> Result<Vec<ProductionReferenceV1>, HumanCenteredProtocolError> {
    if values.len() > 256 {
        return Err(HumanCenteredProtocolError::CollectionLimitExceeded);
    }
    values.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    values.dedup();
    if values.iter().any(|value| value.as_str().trim().is_empty()) {
        return Err(HumanCenteredProtocolError::InvalidReference);
    }
    Ok(values)
}

fn canonical_contract_references(
    mut values: Vec<HumanCenteredContractRefV1>,
) -> Result<Vec<HumanCenteredContractRefV1>, HumanCenteredProtocolError> {
    if values.len() > 256 {
        return Err(HumanCenteredProtocolError::CollectionLimitExceeded);
    }
    for value in &values {
        value.validate()?;
    }
    values.sort_by_key(|value| {
        (
            value.kind(),
            value.contract_id(),
            value.revision(),
            value.record_digest().to_string(),
        )
    });
    values.dedup();
    Ok(values)
}

pub(crate) fn unique_strings(values: &[String]) -> bool {
    let set: BTreeSet<_> = values.iter().collect();
    set.len() == values.len()
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HumanCenteredProtocolError {
    #[error("identifier must not be empty")]
    EmptyIdentifier,
    #[error("basis points must be within 0..=10000, found {0}")]
    InvalidBasisPoints(u16),
    #[error("required text is empty, too large, or contains control characters")]
    InvalidText,
    #[error("reference is invalid")]
    InvalidReference,
    #[error("contract reference is invalid")]
    InvalidContractReference,
    #[error("contract metadata is invalid")]
    InvalidMetadata,
    #[error("contract kind does not match its metadata or revision lineage")]
    ContractKindMismatch,
    #[error("contract revision lineage is invalid")]
    InvalidRevisionLineage,
    #[error("contract dependency is invalid")]
    InvalidDependency,
    #[error("contract collection exceeds its V1 limit")]
    CollectionLimitExceeded,
    #[error("JSON payload exceeds the V1 byte, depth, or node limit")]
    PayloadLimitExceeded,
    #[error("contract state or shape is invalid")]
    InvalidContract,
    #[error("contract state transition is invalid")]
    InvalidTransition,
    #[error("contract digest does not match its content")]
    DigestMismatch,
    #[error("signed proof could not be verified against the pinned trust root")]
    ProofVerificationFailed,
    #[error("revision overflow")]
    RevisionOverflow,
    #[error("serialization failed")]
    Serialization,
}

impl From<crate::ReferenceValidationErrorV1> for HumanCenteredProtocolError {
    fn from(_: crate::ReferenceValidationErrorV1) -> Self {
        Self::InvalidReference
    }
}
