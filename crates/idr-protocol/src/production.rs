//! Production wire boundary for IDR Trust-Chain Closure.
//!
//! Everything in this module is either an untrusted submission, a signed proof
//! envelope, or an immutable reference. Authoritative records are deliberately
//! owned by `idr-runtime` and cannot be deserialized from this wire surface.

use crate::ReferenceV1;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub const IDR_WIRE_SCHEMA_VERSION_V1: u16 = 1;
pub const IDR_MINIMUM_READER_VERSION_V1: u16 = 1;
pub const IDR_PROOF_ALGORITHM_ED25519_V1: &str = "ed25519";
pub const IDR_MAX_SAFE_WIRE_INTEGER_V1: u64 = 9_007_199_254_740_991;
/// Year 9999 UTC. This is portable across PostgreSQL and the supported
/// TypeScript/Python wire implementations.
pub const IDR_MAX_AUTHORITY_EPOCH_SECONDS_V1: u64 = 253_402_300_799;

/// Cryptographic trust boundary. It is part of every command/proof/root
/// identity and must never be inferred from a database schema at verification
/// time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustDomainV1 {
    Shadow,
    Production,
}

/// Compile-time release latch. It remains blocked until the complete master
/// specification and both required independent audits have passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductionReleaseGatesV1 {
    trust_chain_closure_complete: bool,
    production_trust_root_blocked: bool,
    production_authorization_enabled: bool,
    production_execution_enabled: bool,
    human_model_long_term_write_enabled: bool,
    aegis_production_adapter_enabled: bool,
}

impl ProductionReleaseGatesV1 {
    pub const BLOCKED: Self = Self {
        trust_chain_closure_complete: false,
        production_trust_root_blocked: true,
        production_authorization_enabled: false,
        production_execution_enabled: false,
        human_model_long_term_write_enabled: false,
        aegis_production_adapter_enabled: false,
    };

    pub const fn trust_chain_closure_is_complete(self) -> bool {
        self.trust_chain_closure_complete
    }

    pub const fn production_trust_root_is_blocked(self) -> bool {
        self.production_trust_root_blocked
    }

    pub const fn production_authorization_is_enabled(self) -> bool {
        self.production_authorization_enabled
    }

    pub const fn production_execution_is_enabled(self) -> bool {
        self.production_execution_enabled
    }

    pub const fn human_model_long_term_write_is_enabled(self) -> bool {
        self.human_model_long_term_write_enabled
    }

    pub const fn aegis_production_adapter_is_enabled(self) -> bool {
        self.aegis_production_adapter_enabled
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductionProofKindV1 {
    CallerAuthentication,
    InputAdmission,
    ContextSnapshot,
    IntentFastPathAdmission,
    DecisionNecessityAdmission,
    TurnCoordinationAdmission,
    ResponsePolicy,
    ResponseAdmission,
    Authority,
    Capability,
    Policy,
    ExactAuthorization,
    ActionAdmission,
    ExecutionPermit,
    ProviderReceipt,
    OutcomeObservation,
    HumanModelPromotion,
    HumanModelUserConfirmation,
    HumanModelCorrection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateKindV1 {
    CanonicalInput,
    ContextSnapshot,
    Intent,
    Decision,
    TurnCoordination,
    Response,
    Action,
    ActionAdmissionDecision,
    ExecutionReceipt,
    Outcome,
    HumanModelCandidate,
    HumanModelPromotionDecision,
    HumanModelAssertion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireCompatibilityV1 {
    schema_version: u16,
    minimum_reader_version: u16,
    critical_feature_flags: BTreeSet<String>,
}

impl WireCompatibilityV1 {
    pub fn v1() -> Self {
        Self {
            schema_version: IDR_WIRE_SCHEMA_VERSION_V1,
            minimum_reader_version: IDR_MINIMUM_READER_VERSION_V1,
            critical_feature_flags: BTreeSet::new(),
        }
    }

    pub fn validate(&self) -> Result<(), ProductionProtocolErrorV1> {
        if self.schema_version != IDR_WIRE_SCHEMA_VERSION_V1
            || self.minimum_reader_version > IDR_WIRE_SCHEMA_VERSION_V1
            || self.critical_feature_flags.iter().any(|flag| {
                flag.is_empty() || flag.len() > 128 || flag.chars().any(char::is_control)
            })
        {
            return Err(ProductionProtocolErrorV1::UnsupportedWireVersion);
        }
        if !self.critical_feature_flags.is_empty() {
            return Err(ProductionProtocolErrorV1::UnsupportedCriticalFeature);
        }
        Ok(())
    }
}

/// Public, untrusted input. A Candidate has no authority and cannot be stored
/// as an authoritative record without an Orchestrator command and proof set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateSubmissionV1 {
    compatibility: WireCompatibilityV1,
    candidate_id: Uuid,
    candidate_kind: CandidateKindV1,
    run_id: Uuid,
    turn_id: Uuid,
    subject_ref: ReferenceV1,
    tenant_ref: ReferenceV1,
    scope_ref: ReferenceV1,
    purpose_ref: ReferenceV1,
    policy_revision_ref: ReferenceV1,
    valid_from: u64,
    valid_until: u64,
    payload: Value,
}

impl CandidateSubmissionV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        candidate_kind: CandidateKindV1,
        run_id: Uuid,
        turn_id: Uuid,
        subject_ref: ReferenceV1,
        tenant_ref: ReferenceV1,
        scope_ref: ReferenceV1,
        purpose_ref: ReferenceV1,
        policy_revision_ref: ReferenceV1,
        valid_from: u64,
        valid_until: u64,
        payload: Value,
    ) -> Result<Self, ProductionProtocolErrorV1> {
        let candidate = Self {
            compatibility: WireCompatibilityV1::v1(),
            candidate_id: Uuid::new_v4(),
            candidate_kind,
            run_id,
            turn_id,
            subject_ref,
            tenant_ref,
            scope_ref,
            purpose_ref,
            policy_revision_ref,
            valid_from,
            valid_until,
            payload,
        };
        candidate.validate()?;
        Ok(candidate)
    }

    pub fn validate(&self) -> Result<(), ProductionProtocolErrorV1> {
        self.compatibility.validate()?;
        if self.candidate_id.is_nil()
            || !valid_wire_uuid(self.run_id)
            || !valid_wire_uuid(self.turn_id)
            || self.valid_from == 0
            || self.valid_until <= self.valid_from
            || self.valid_until > IDR_MAX_AUTHORITY_EPOCH_SECONDS_V1
            || canonical_json_bytes_v1(&self.payload)?.len() > 1_048_576
        {
            return Err(ProductionProtocolErrorV1::InvalidCandidate);
        }
        validate_json_limits(&self.payload)?;
        Ok(())
    }

    pub fn candidate_id(&self) -> Uuid {
        self.candidate_id
    }

    pub fn candidate_kind(&self) -> CandidateKindV1 {
        self.candidate_kind
    }

    pub fn run_id(&self) -> Uuid {
        self.run_id
    }

    pub fn turn_id(&self) -> Uuid {
        self.turn_id
    }

    pub fn subject_ref(&self) -> &ReferenceV1 {
        &self.subject_ref
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

    pub fn policy_revision_ref(&self) -> &ReferenceV1 {
        &self.policy_revision_ref
    }

    pub fn valid_from(&self) -> u64 {
        self.valid_from
    }

    pub fn valid_until(&self) -> u64 {
        self.valid_until
    }

    pub fn payload(&self) -> &Value {
        &self.payload
    }

    pub fn canonical_digest(&self) -> Result<String, ProductionProtocolErrorV1> {
        canonical_digest_v1("idr-candidate-v1", self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionProofClaimsV1 {
    compatibility: WireCompatibilityV1,
    proof_id: Uuid,
    trust_domain: TrustDomainV1,
    environment_ref: ReferenceV1,
    proof_kind: ProductionProofKindV1,
    issuer_ref: ReferenceV1,
    subject_ref: ReferenceV1,
    subject_digest: String,
    audience_ref: ReferenceV1,
    tenant_ref: ReferenceV1,
    scope_ref: ReferenceV1,
    purpose_ref: ReferenceV1,
    policy_revision_ref: ReferenceV1,
    issued_at: u64,
    not_before: u64,
    expires_at: u64,
    nonce: String,
    principal_digest: String,
    assertion: Value,
}

impl ProductionProofClaimsV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        trust_domain: TrustDomainV1,
        environment_ref: ReferenceV1,
        proof_kind: ProductionProofKindV1,
        issuer_ref: ReferenceV1,
        subject_ref: ReferenceV1,
        subject_digest: impl Into<String>,
        audience_ref: ReferenceV1,
        tenant_ref: ReferenceV1,
        scope_ref: ReferenceV1,
        purpose_ref: ReferenceV1,
        policy_revision_ref: ReferenceV1,
        issued_at: u64,
        not_before: u64,
        expires_at: u64,
        nonce: impl Into<String>,
    ) -> Result<Self, ProductionProtocolErrorV1> {
        let claims = Self {
            compatibility: WireCompatibilityV1::v1(),
            proof_id: Uuid::new_v4(),
            trust_domain,
            environment_ref,
            proof_kind,
            issuer_ref,
            subject_ref,
            subject_digest: subject_digest.into(),
            audience_ref,
            tenant_ref,
            scope_ref,
            purpose_ref,
            policy_revision_ref,
            issued_at,
            not_before,
            expires_at,
            nonce: nonce.into(),
            principal_digest: "0".repeat(64),
            assertion: default_proof_assertion(proof_kind),
        };
        claims.validate()?;
        Ok(claims)
    }

    pub fn validate(&self) -> Result<(), ProductionProtocolErrorV1> {
        self.compatibility.validate()?;
        if self.proof_id.is_nil()
            || !valid_digest(&self.subject_digest)
            || self.issued_at == 0
            || self.not_before < self.issued_at
            || self.expires_at <= self.not_before
            || self.expires_at > IDR_MAX_AUTHORITY_EPOCH_SECONDS_V1
            || !valid_digest(&self.nonce)
            || !valid_digest(&self.principal_digest)
        {
            return Err(ProductionProtocolErrorV1::InvalidProof);
        }
        validate_json_limits(&self.assertion)?;
        Ok(())
    }

    pub fn with_assertion(mut self, assertion: Value) -> Result<Self, ProductionProtocolErrorV1> {
        validate_json_limits(&assertion)?;
        self.assertion = assertion;
        self.validate()?;
        Ok(self)
    }

    pub fn with_principal_binding(
        mut self,
        actor_ref: &ReferenceV1,
        caller_ref: &ReferenceV1,
    ) -> Result<Self, ProductionProtocolErrorV1> {
        self.principal_digest =
            canonical_digest_v1("idr-command-principal-v1", &(actor_ref, caller_ref))?;
        self.validate()?;
        Ok(self)
    }

    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, ProductionProtocolErrorV1> {
        canonical_json_bytes_v1(&("idr-production-proof-v1", self))
    }

    pub fn proof_id(&self) -> Uuid {
        self.proof_id
    }

    pub fn proof_kind(&self) -> ProductionProofKindV1 {
        self.proof_kind
    }

    pub fn trust_domain(&self) -> TrustDomainV1 {
        self.trust_domain
    }

    pub fn environment_ref(&self) -> &ReferenceV1 {
        &self.environment_ref
    }

    pub fn issuer_ref(&self) -> &ReferenceV1 {
        &self.issuer_ref
    }

    pub fn subject_ref(&self) -> &ReferenceV1 {
        &self.subject_ref
    }

    pub fn subject_digest(&self) -> &str {
        &self.subject_digest
    }

    pub fn audience_ref(&self) -> &ReferenceV1 {
        &self.audience_ref
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

    pub fn policy_revision_ref(&self) -> &ReferenceV1 {
        &self.policy_revision_ref
    }

    pub fn issued_at(&self) -> u64 {
        self.issued_at
    }

    pub fn not_before(&self) -> u64 {
        self.not_before
    }

    pub fn expires_at(&self) -> u64 {
        self.expires_at
    }

    pub fn nonce(&self) -> &str {
        &self.nonce
    }

    pub fn assertion(&self) -> &Value {
        &self.assertion
    }

    pub fn principal_digest(&self) -> &str {
        &self.principal_digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionProofEnvelopeV1 {
    claims: ProductionProofClaimsV1,
    key_id: ReferenceV1,
    algorithm: String,
    signature: String,
}

impl ProductionProofEnvelopeV1 {
    pub fn new(
        claims: ProductionProofClaimsV1,
        key_id: ReferenceV1,
        signature: impl Into<String>,
    ) -> Result<Self, ProductionProtocolErrorV1> {
        let envelope = Self {
            claims,
            key_id,
            algorithm: IDR_PROOF_ALGORITHM_ED25519_V1.to_string(),
            signature: signature.into(),
        };
        envelope.validate_shape()?;
        Ok(envelope)
    }

    pub fn validate_shape(&self) -> Result<(), ProductionProtocolErrorV1> {
        self.claims.validate()?;
        if self.algorithm != IDR_PROOF_ALGORITHM_ED25519_V1
            || decode_hex::<64>(&self.signature).is_none()
        {
            return Err(ProductionProtocolErrorV1::InvalidProof);
        }
        Ok(())
    }

    pub fn claims(&self) -> &ProductionProofClaimsV1 {
        &self.claims
    }

    pub fn key_id(&self) -> &ReferenceV1 {
        &self.key_id
    }

    pub fn signature(&self) -> &str {
        &self.signature
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedProductionProofV1 {
    pub trust_domain: TrustDomainV1,
    pub environment_ref: ReferenceV1,
    pub proof_kind: ProductionProofKindV1,
    pub issuer_ref: ReferenceV1,
    pub subject_ref: ReferenceV1,
    pub subject_digest: String,
    pub audience_ref: ReferenceV1,
    pub tenant_ref: ReferenceV1,
    pub scope_ref: ReferenceV1,
    pub purpose_ref: ReferenceV1,
    pub policy_revision_ref: ReferenceV1,
}

#[derive(Debug, Clone)]
pub struct ProductionIssuerKeyV1 {
    trust_domain: TrustDomainV1,
    environment_ref: ReferenceV1,
    issuer_ref: ReferenceV1,
    key_id: ReferenceV1,
    public_key: VerifyingKey,
    audience_ref: ReferenceV1,
    tenant_ref: ReferenceV1,
    allowed_proof_kinds: BTreeSet<ProductionProofKindV1>,
    valid_from: u64,
    valid_until: u64,
    revoked_at: Option<u64>,
}

impl ProductionIssuerKeyV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        trust_domain: TrustDomainV1,
        environment_ref: ReferenceV1,
        issuer_ref: ReferenceV1,
        key_id: ReferenceV1,
        public_key_hex: &str,
        audience_ref: ReferenceV1,
        tenant_ref: ReferenceV1,
        allowed_proof_kinds: BTreeSet<ProductionProofKindV1>,
        valid_from: u64,
        valid_until: u64,
        revoked_at: Option<u64>,
    ) -> Result<Self, ProductionProtocolErrorV1> {
        let key = decode_hex::<32>(public_key_hex).ok_or(ProductionProtocolErrorV1::InvalidKey)?;
        let public_key =
            VerifyingKey::from_bytes(&key).map_err(|_| ProductionProtocolErrorV1::InvalidKey)?;
        if allowed_proof_kinds.is_empty()
            || valid_from == 0
            || valid_until <= valid_from
            || valid_until > IDR_MAX_AUTHORITY_EPOCH_SECONDS_V1
            || revoked_at.is_some_and(|revoked| revoked < valid_from || revoked > valid_until)
            || !audience_matches_environment(trust_domain, &environment_ref, &audience_ref)
        {
            return Err(ProductionProtocolErrorV1::InvalidKey);
        }
        Ok(Self {
            trust_domain,
            environment_ref,
            issuer_ref,
            key_id,
            public_key,
            audience_ref,
            tenant_ref,
            allowed_proof_kinds,
            valid_from,
            valid_until,
            revoked_at,
        })
    }
}

#[derive(Debug, Clone)]
pub struct TrustRootSnapshotV1 {
    trust_domain: TrustDomainV1,
    environment_ref: ReferenceV1,
    root_version: u64,
    root_digest: String,
    loaded_at: u64,
    keys: BTreeMap<String, ProductionIssuerKeyV1>,
}

impl TrustRootSnapshotV1 {
    pub fn new(
        trust_domain: TrustDomainV1,
        environment_ref: ReferenceV1,
        root_version: u64,
        loaded_at: u64,
        policies: Vec<ProductionIssuerKeyV1>,
    ) -> Result<Self, ProductionProtocolErrorV1> {
        let mut keys = BTreeMap::new();
        for policy in policies {
            if policy.trust_domain != trust_domain || policy.environment_ref != environment_ref {
                return Err(ProductionProtocolErrorV1::InvalidTrustRoot);
            }
            let key_id = policy.key_id.as_str().to_string();
            if keys.insert(key_id, policy).is_some() {
                return Err(ProductionProtocolErrorV1::InvalidKey);
            }
        }
        if root_version == 0 || loaded_at == 0 || keys.is_empty() {
            return Err(ProductionProtocolErrorV1::InvalidTrustRoot);
        }
        let digest_input: Vec<_> = keys
            .iter()
            .map(|(key_id, policy)| {
                (
                    key_id,
                    policy.trust_domain,
                    policy.environment_ref.as_str(),
                    policy.issuer_ref.as_str(),
                    policy.audience_ref.as_str(),
                    policy.tenant_ref.as_str(),
                    policy.valid_from,
                    policy.valid_until,
                    policy.revoked_at,
                    &policy.allowed_proof_kinds,
                    encode_hex(policy.public_key.as_bytes()),
                )
            })
            .collect();
        let root_digest = canonical_digest_v1(
            "idr-trust-root-v1",
            &(
                trust_domain,
                &environment_ref,
                root_version,
                loaded_at,
                digest_input,
            ),
        )?;
        Ok(Self {
            trust_domain,
            environment_ref,
            root_version,
            root_digest,
            loaded_at,
            keys,
        })
    }

    pub fn verify_at(
        &self,
        envelope: &ProductionProofEnvelopeV1,
        expected: &ExpectedProductionProofV1,
        trusted_now: u64,
    ) -> Result<VerifiedProductionProofV1, ProductionProtocolErrorV1> {
        envelope.validate_shape()?;
        let claims = envelope.claims();
        let key = self
            .keys
            .get(envelope.key_id().as_str())
            .ok_or(ProductionProtocolErrorV1::ProofVerificationFailed)?;
        if self.trust_domain != expected.trust_domain
            || self.environment_ref != expected.environment_ref
            || claims.trust_domain() != expected.trust_domain
            || claims.environment_ref() != &expected.environment_ref
            || key.trust_domain != expected.trust_domain
            || key.environment_ref != expected.environment_ref
            || claims.proof_kind() != expected.proof_kind
            || claims.issuer_ref() != &expected.issuer_ref
            || claims.subject_ref() != &expected.subject_ref
            || claims.subject_digest() != expected.subject_digest
            || claims.audience_ref() != &expected.audience_ref
            || claims.audience_ref() != &key.audience_ref
            || claims.tenant_ref() != &expected.tenant_ref
            || claims.scope_ref() != &expected.scope_ref
            || claims.purpose_ref() != &expected.purpose_ref
            || claims.policy_revision_ref() != &expected.policy_revision_ref
            || claims.issuer_ref() != &key.issuer_ref
            || claims.tenant_ref() != &key.tenant_ref
            || envelope.key_id() != &key.key_id
            || !key.allowed_proof_kinds.contains(&claims.proof_kind())
            || trusted_now < claims.not_before()
            || trusted_now >= claims.expires_at()
            || claims.issued_at() < key.valid_from
            || trusted_now >= key.valid_until
            || key.revoked_at.is_some_and(|revoked| trusted_now >= revoked)
        {
            return Err(ProductionProtocolErrorV1::ProofVerificationFailed);
        }
        let signature = Signature::from_bytes(
            &decode_hex::<64>(envelope.signature())
                .ok_or(ProductionProtocolErrorV1::ProofVerificationFailed)?,
        );
        key.public_key
            .verify(&claims.canonical_signing_bytes()?, &signature)
            .map_err(|_| ProductionProtocolErrorV1::ProofVerificationFailed)?;
        Ok(VerifiedProductionProofV1 {
            proof_id: claims.proof_id(),
            nonce: claims.nonce().to_string(),
            trust_domain: claims.trust_domain(),
            environment_ref: claims.environment_ref().clone(),
            proof_kind: claims.proof_kind(),
            expires_at: claims.expires_at(),
            trust_root_version: self.root_version,
            trust_root_digest: self.root_digest.clone(),
            verified_at: trusted_now,
        })
    }

    pub fn root_version(&self) -> u64 {
        self.root_version
    }

    pub fn trust_domain(&self) -> TrustDomainV1 {
        self.trust_domain
    }

    pub fn environment_ref(&self) -> &ReferenceV1 {
        &self.environment_ref
    }

    pub fn root_digest(&self) -> &str {
        &self.root_digest
    }

    pub fn loaded_at(&self) -> u64 {
        self.loaded_at
    }
}

/// Opaque result. It is intentionally neither Clone nor Deserialize.
#[derive(Debug, PartialEq, Eq)]
pub struct VerifiedProductionProofV1 {
    proof_id: Uuid,
    nonce: String,
    trust_domain: TrustDomainV1,
    environment_ref: ReferenceV1,
    proof_kind: ProductionProofKindV1,
    expires_at: u64,
    trust_root_version: u64,
    trust_root_digest: String,
    verified_at: u64,
}

impl VerifiedProductionProofV1 {
    pub fn proof_id(&self) -> Uuid {
        self.proof_id
    }

    pub fn nonce(&self) -> &str {
        &self.nonce
    }

    pub fn proof_kind(&self) -> ProductionProofKindV1 {
        self.proof_kind
    }

    pub fn trust_domain(&self) -> TrustDomainV1 {
        self.trust_domain
    }

    pub fn environment_ref(&self) -> &ReferenceV1 {
        &self.environment_ref
    }

    pub fn expires_at(&self) -> u64 {
        self.expires_at
    }

    pub fn trust_root_version(&self) -> u64 {
        self.trust_root_version
    }

    pub fn trust_root_digest(&self) -> &str {
        &self.trust_root_digest
    }

    pub fn verified_at(&self) -> u64 {
        self.verified_at
    }
}

pub fn canonical_digest_v1<T: Serialize + ?Sized>(
    domain: &str,
    value: &T,
) -> Result<String, ProductionProtocolErrorV1> {
    let bytes = canonical_json_bytes_v1(&(domain, value))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

/// RFC 8785/JCS-compatible encoder for the IDR V1 constrained JSON profile.
///
/// IDR forbids floating-point numbers at the production boundary, eliminating
/// cross-runtime ambiguity around non-integral IEEE-754 rendering.
pub fn canonical_json_bytes_v1<T: Serialize + ?Sized>(
    value: &T,
) -> Result<Vec<u8>, ProductionProtocolErrorV1> {
    let value =
        serde_json::to_value(value).map_err(|_| ProductionProtocolErrorV1::CanonicalEncoding)?;
    let mut output = Vec::new();
    write_canonical_value(&value, &mut output)?;
    Ok(output)
}

fn write_canonical_value(
    value: &Value,
    output: &mut Vec<u8>,
) -> Result<(), ProductionProtocolErrorV1> {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(value) => output.extend_from_slice(if *value { b"true" } else { b"false" }),
        Value::Number(value) => {
            validate_canonical_number(value)?;
            output.extend_from_slice(value.to_string().as_bytes());
        }
        Value::String(value) => {
            output.extend_from_slice(
                serde_json::to_string(value)
                    .map_err(|_| ProductionProtocolErrorV1::CanonicalEncoding)?
                    .as_bytes(),
            );
        }
        Value::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                write_canonical_value(value, output)?;
            }
            output.push(b']');
        }
        Value::Object(values) => {
            output.push(b'{');
            let mut keys: Vec<_> = values.keys().collect();
            // RFC 8785 sorts property names by their raw UTF-16 code units,
            // matching ECMAScript rather than Rust scalar-value ordering.
            keys.sort_by(|left, right| left.encode_utf16().cmp(right.encode_utf16()));
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                write_canonical_value(&Value::String(key.clone()), output)?;
                output.push(b':');
                write_canonical_value(&values[key], output)?;
            }
            output.push(b'}');
        }
    }
    Ok(())
}

fn validate_json_limits(value: &Value) -> Result<(), ProductionProtocolErrorV1> {
    let mut stack = vec![(value, 1_usize)];
    let mut nodes = 0_usize;
    while let Some((value, depth)) = stack.pop() {
        nodes += 1;
        if depth > 32 || nodes > 10_000 {
            return Err(ProductionProtocolErrorV1::InvalidCandidate);
        }
        match value {
            Value::Array(values) => {
                stack.extend(values.iter().map(|child| (child, depth + 1)));
            }
            Value::Object(values) => {
                stack.extend(values.values().map(|child| (child, depth + 1)));
            }
            Value::Number(number) => validate_canonical_number(number)?,
            _ => {}
        }
    }
    Ok(())
}

fn validate_canonical_number(number: &serde_json::Number) -> Result<(), ProductionProtocolErrorV1> {
    if let Some(value) = number.as_i64() {
        let max = IDR_MAX_SAFE_WIRE_INTEGER_V1 as i64;
        if !(-max..=max).contains(&value) {
            return Err(ProductionProtocolErrorV1::UnsafeInteger);
        }
        return Ok(());
    }
    if let Some(value) = number.as_u64() {
        if value > IDR_MAX_SAFE_WIRE_INTEGER_V1 {
            return Err(ProductionProtocolErrorV1::UnsafeInteger);
        }
        return Ok(());
    }
    Err(ProductionProtocolErrorV1::NonIntegralNumber)
}

fn valid_wire_uuid(value: Uuid) -> bool {
    !value.is_nil()
        && value.get_variant() == uuid::Variant::RFC4122
        && (1..=8).contains(&value.get_version_num())
}

fn default_proof_assertion(proof_kind: ProductionProofKindV1) -> Value {
    use ProductionProofKindV1::*;
    match proof_kind {
        CallerAuthentication => serde_json::json!({"outcome": "authenticate"}),
        ExactAuthorization => serde_json::json!({"decision": "approve"}),
        InputAdmission => serde_json::json!({"outcome": "admit"}),
        ContextSnapshot => serde_json::json!({"outcome": "attest"}),
        IntentFastPathAdmission | DecisionNecessityAdmission | TurnCoordinationAdmission => {
            serde_json::json!({"outcome": "admit"})
        }
        ResponsePolicy | ResponseAdmission | Authority | Capability | Policy | ActionAdmission => {
            serde_json::json!({"outcome": "allow"})
        }
        ExecutionPermit => serde_json::json!({"outcome": "issue"}),
        ProviderReceipt => serde_json::json!({"outcome": "attest"}),
        OutcomeObservation => serde_json::json!({"outcome": "observe"}),
        HumanModelPromotion => serde_json::json!({"outcome": "promote"}),
        HumanModelUserConfirmation => serde_json::json!({"outcome": "confirm"}),
        HumanModelCorrection => serde_json::json!({"outcome": "correct"}),
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn audience_matches_environment(
    trust_domain: TrustDomainV1,
    environment_ref: &ReferenceV1,
    audience_ref: &ReferenceV1,
) -> bool {
    let domain = match trust_domain {
        TrustDomainV1::Shadow => "shadow",
        TrustDomainV1::Production => "production",
    };
    environment_ref
        .as_str()
        .strip_prefix(&format!("environment:idr:{domain}:"))
        .is_some_and(|environment| {
            !environment.is_empty()
                && audience_ref.as_str() == format!("audience:idr:{domain}:{environment}")
        })
}

fn decode_hex<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != N * 2 {
        return None;
    }
    let mut output = [0_u8; N];
    for (index, slot) in output.iter_mut().enumerate() {
        let high = decode_nibble(value.as_bytes()[index * 2])?;
        let low = decode_nibble(value.as_bytes()[index * 2 + 1])?;
        *slot = (high << 4) | low;
    }
    Some(output)
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

const fn decode_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProductionProtocolErrorV1 {
    #[error("wire schema or minimum reader version is unsupported")]
    UnsupportedWireVersion,
    #[error("an unknown critical feature was requested")]
    UnsupportedCriticalFeature,
    #[error("candidate submission is invalid")]
    InvalidCandidate,
    #[error("proof envelope is invalid")]
    InvalidProof,
    #[error("issuer key is invalid")]
    InvalidKey,
    #[error("trust-root snapshot is invalid")]
    InvalidTrustRoot,
    #[error("proof verification failed")]
    ProofVerificationFailed,
    #[error("canonical JSON encoding failed")]
    CanonicalEncoding,
    #[error("non-integral JSON numbers are forbidden in the V1 canonical profile")]
    NonIntegralNumber,
    #[error("JSON integers outside the ECMAScript safe range are forbidden")]
    UnsafeInteger,
}
