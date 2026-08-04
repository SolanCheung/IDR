use super::{
    valid_digest, HumanCenteredProofIdV1, HumanCenteredProtocolError,
    HUMAN_CENTERED_MAX_SAFE_WIRE_INTEGER, HUMAN_CENTERED_SCHEMA_VERSION,
};
use crate::ProductionReferenceV1;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{SystemTime, UNIX_EPOCH};

const PROOF_SIGNATURE_DOMAIN: &[u8] = b"idr-proof-v1\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofKindV1 {
    InputAdmission,
    ContextSnapshot,
    IntentFastPathFacts,
    DecisionNecessityFacts,
    TurnCoordinationFacts,
    AuthorityGrant,
    PolicyEvaluation,
    Capability,
    HumanConfirmation,
    DependencySnapshot,
    OutcomeObservation,
    ExecutionPermit,
    ProviderExecutionReceipt,
}

impl ProofKindV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::InputAdmission => 1,
            Self::ContextSnapshot => 2,
            Self::IntentFastPathFacts => 3,
            Self::DecisionNecessityFacts => 4,
            Self::TurnCoordinationFacts => 5,
            Self::AuthorityGrant => 6,
            Self::PolicyEvaluation => 7,
            Self::Capability => 8,
            Self::HumanConfirmation => 9,
            Self::DependencySnapshot => 10,
            Self::OutcomeObservation => 11,
            Self::ExecutionPermit => 12,
            Self::ProviderExecutionReceipt => 13,
        }
    }
}

/// Compile-time release posture for V1.3. These flags are intentionally not
/// configurable from wire input. Enabling any production capability requires
/// a later protocol revision and an explicit trust-root implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductionFeatureGatesV1 {
    pub production_release_authorized: bool,
    pub production_authorization_enabled: bool,
    pub production_execution_enabled: bool,
    pub long_term_human_model_write_enabled: bool,
    pub aegis_production_adapter_enabled: bool,
}

impl ProductionFeatureGatesV1 {
    pub const BLOCKED: Self = Self {
        production_release_authorized: false,
        production_authorization_enabled: false,
        production_execution_enabled: false,
        long_term_human_model_write_enabled: false,
        aegis_production_adapter_enabled: false,
    };

    pub const fn production_trust_root_is_blocked(self) -> bool {
        !self.production_release_authorized
            && !self.production_authorization_enabled
            && !self.production_execution_enabled
            && !self.long_term_human_model_write_enabled
            && !self.aegis_production_adapter_enabled
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofClaimsV1 {
    proof_id: HumanCenteredProofIdV1,
    proof_kind: ProofKindV1,
    issuer_ref: ProductionReferenceV1,
    subject_ref: ProductionReferenceV1,
    subject_digest: String,
    tenant_ref: ProductionReferenceV1,
    scope_ref: ProductionReferenceV1,
    purpose_ref: ProductionReferenceV1,
    policy_revision_ref: ProductionReferenceV1,
    issued_at: u64,
    expires_at: u64,
    nonce: String,
    schema_version: u16,
}

impl ProofClaimsV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        proof_kind: ProofKindV1,
        issuer_ref: ProductionReferenceV1,
        subject_ref: ProductionReferenceV1,
        subject_digest: impl Into<String>,
        tenant_ref: ProductionReferenceV1,
        scope_ref: ProductionReferenceV1,
        purpose_ref: ProductionReferenceV1,
        policy_revision_ref: ProductionReferenceV1,
        issued_at: u64,
        expires_at: u64,
        nonce: impl Into<String>,
    ) -> Result<Self, HumanCenteredProtocolError> {
        let value = Self {
            proof_id: HumanCenteredProofIdV1::mint(),
            proof_kind,
            issuer_ref,
            subject_ref,
            subject_digest: subject_digest.into(),
            tenant_ref,
            scope_ref,
            purpose_ref,
            policy_revision_ref,
            issued_at,
            expires_at,
            nonce: nonce.into(),
            schema_version: HUMAN_CENTERED_SCHEMA_VERSION,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), HumanCenteredProtocolError> {
        let references = [
            &self.issuer_ref,
            &self.subject_ref,
            &self.tenant_ref,
            &self.scope_ref,
            &self.purpose_ref,
            &self.policy_revision_ref,
        ];
        if self.proof_id.is_empty()
            || references
                .iter()
                .any(|reference| reference.as_str().trim().is_empty())
            || !valid_digest(&self.subject_digest)
            || self.issued_at == 0
            || self.expires_at <= self.issued_at
            || self.expires_at > HUMAN_CENTERED_MAX_SAFE_WIRE_INTEGER
            || self.nonce.len() != 64
            || !valid_digest(&self.nonce)
            || self.schema_version != HUMAN_CENTERED_SCHEMA_VERSION
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        Ok(())
    }

    /// Canonical, language-neutral bytes signed by the issuer.
    ///
    /// This encoding is domain-separated and length-prefixes every string; it
    /// does not depend on JSON object order or a language's serializer.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, HumanCenteredProtocolError> {
        self.validate()?;
        let mut bytes = Vec::with_capacity(512);
        bytes.extend_from_slice(PROOF_SIGNATURE_DOMAIN);
        bytes.extend_from_slice(self.proof_id.to_string().as_bytes());
        bytes.push(self.proof_kind.tag());
        append_text(&mut bytes, self.issuer_ref.as_str())?;
        append_text(&mut bytes, self.subject_ref.as_str())?;
        append_text(&mut bytes, &self.subject_digest)?;
        append_text(&mut bytes, self.tenant_ref.as_str())?;
        append_text(&mut bytes, self.scope_ref.as_str())?;
        append_text(&mut bytes, self.purpose_ref.as_str())?;
        append_text(&mut bytes, self.policy_revision_ref.as_str())?;
        bytes.extend_from_slice(&self.issued_at.to_be_bytes());
        bytes.extend_from_slice(&self.expires_at.to_be_bytes());
        append_text(&mut bytes, &self.nonce)?;
        bytes.extend_from_slice(&self.schema_version.to_be_bytes());
        Ok(bytes)
    }

    pub fn proof_kind(&self) -> ProofKindV1 {
        self.proof_kind
    }

    pub fn proof_id(&self) -> HumanCenteredProofIdV1 {
        self.proof_id
    }

    pub fn issuer_ref(&self) -> &ProductionReferenceV1 {
        &self.issuer_ref
    }

    pub fn subject_ref(&self) -> &ProductionReferenceV1 {
        &self.subject_ref
    }

    pub fn subject_digest(&self) -> &str {
        &self.subject_digest
    }

    pub fn tenant_ref(&self) -> &ProductionReferenceV1 {
        &self.tenant_ref
    }

    pub fn scope_ref(&self) -> &ProductionReferenceV1 {
        &self.scope_ref
    }

    pub fn purpose_ref(&self) -> &ProductionReferenceV1 {
        &self.purpose_ref
    }

    pub fn policy_revision_ref(&self) -> &ProductionReferenceV1 {
        &self.policy_revision_ref
    }

    pub fn issued_at(&self) -> u64 {
        self.issued_at
    }

    pub fn expires_at(&self) -> u64 {
        self.expires_at
    }

    pub fn nonce(&self) -> &str {
        &self.nonce
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedProofEnvelopeV1 {
    claims: ProofClaimsV1,
    key_id: ProductionReferenceV1,
    signature: String,
}

impl SignedProofEnvelopeV1 {
    pub fn new(
        claims: ProofClaimsV1,
        key_id: ProductionReferenceV1,
        signature: impl Into<String>,
    ) -> Result<Self, HumanCenteredProtocolError> {
        let value = Self {
            claims,
            key_id,
            signature: signature.into(),
        };
        value.validate_shape()?;
        Ok(value)
    }

    pub fn validate_shape(&self) -> Result<(), HumanCenteredProtocolError> {
        self.claims.validate()?;
        if self.key_id.as_str().trim().is_empty()
            || self.signature.len() != 128
            || decode_hex::<64>(&self.signature).is_none()
        {
            return Err(HumanCenteredProtocolError::ProofVerificationFailed);
        }
        Ok(())
    }

    pub fn claims(&self) -> &ProofClaimsV1 {
        &self.claims
    }

    pub fn key_id(&self) -> &ProductionReferenceV1 {
        &self.key_id
    }

    pub fn signature(&self) -> &str {
        &self.signature
    }
}

#[derive(Debug, Clone)]
pub struct IssuerKeyPolicyV1 {
    issuer_ref: ProductionReferenceV1,
    key_id: ProductionReferenceV1,
    verifying_key: VerifyingKey,
    allowed_proof_kinds: BTreeSet<ProofKindV1>,
    tenant_ref: ProductionReferenceV1,
    valid_from: u64,
    valid_until: u64,
    revoked_at: Option<u64>,
}

impl IssuerKeyPolicyV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        issuer_ref: ProductionReferenceV1,
        key_id: ProductionReferenceV1,
        public_key_hex: &str,
        allowed_proof_kinds: BTreeSet<ProofKindV1>,
        tenant_ref: ProductionReferenceV1,
        valid_from: u64,
        valid_until: u64,
        revoked_at: Option<u64>,
    ) -> Result<Self, HumanCenteredProtocolError> {
        let key_bytes = decode_hex::<32>(public_key_hex)
            .ok_or(HumanCenteredProtocolError::ProofVerificationFailed)?;
        let verifying_key = VerifyingKey::from_bytes(&key_bytes)
            .map_err(|_| HumanCenteredProtocolError::ProofVerificationFailed)?;
        if allowed_proof_kinds.is_empty()
            || valid_from == 0
            || valid_until <= valid_from
            || valid_until > HUMAN_CENTERED_MAX_SAFE_WIRE_INTEGER
            || revoked_at.is_some_and(|value| value < valid_from || value > valid_until)
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        Ok(Self {
            issuer_ref,
            key_id,
            verifying_key,
            allowed_proof_kinds,
            tenant_ref,
            valid_from,
            valid_until,
            revoked_at,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ProofTrustRootV1 {
    keys: BTreeMap<String, IssuerKeyPolicyV1>,
}

/// Complete caller-side expectation for a signed proof.
///
/// Verification never treats signed-but-unexpected scope, purpose, subject, or
/// policy values as sufficient. The caller must supply every security-relevant
/// binding and the trust root compares them before returning an opaque proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedProofBindingV1 {
    proof_kind: ProofKindV1,
    issuer_ref: ProductionReferenceV1,
    subject_ref: ProductionReferenceV1,
    subject_digest: String,
    tenant_ref: ProductionReferenceV1,
    scope_ref: ProductionReferenceV1,
    purpose_ref: ProductionReferenceV1,
    policy_revision_ref: ProductionReferenceV1,
}

impl ExpectedProofBindingV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        proof_kind: ProofKindV1,
        issuer_ref: ProductionReferenceV1,
        subject_ref: ProductionReferenceV1,
        subject_digest: impl Into<String>,
        tenant_ref: ProductionReferenceV1,
        scope_ref: ProductionReferenceV1,
        purpose_ref: ProductionReferenceV1,
        policy_revision_ref: ProductionReferenceV1,
    ) -> Result<Self, HumanCenteredProtocolError> {
        let value = Self {
            proof_kind,
            issuer_ref,
            subject_ref,
            subject_digest: subject_digest.into(),
            tenant_ref,
            scope_ref,
            purpose_ref,
            policy_revision_ref,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), HumanCenteredProtocolError> {
        if !valid_digest(&self.subject_digest)
            || [
                &self.issuer_ref,
                &self.subject_ref,
                &self.tenant_ref,
                &self.scope_ref,
                &self.purpose_ref,
                &self.policy_revision_ref,
            ]
            .iter()
            .any(|reference| reference.as_str().trim().is_empty())
        {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        Ok(())
    }

    pub fn tenant_ref(&self) -> &ProductionReferenceV1 {
        &self.tenant_ref
    }

    pub fn scope_ref(&self) -> &ProductionReferenceV1 {
        &self.scope_ref
    }

    pub fn purpose_ref(&self) -> &ProductionReferenceV1 {
        &self.purpose_ref
    }

    pub fn policy_revision_ref(&self) -> &ProductionReferenceV1 {
        &self.policy_revision_ref
    }
}

impl ProofTrustRootV1 {
    pub fn new(policies: Vec<IssuerKeyPolicyV1>) -> Result<Self, HumanCenteredProtocolError> {
        let mut keys = BTreeMap::new();
        for policy in policies {
            let key_id = policy.key_id.as_str().to_string();
            if keys.insert(key_id, policy).is_some() {
                return Err(HumanCenteredProtocolError::InvalidContract);
            }
        }
        if keys.is_empty() {
            return Err(HumanCenteredProtocolError::InvalidContract);
        }
        Ok(Self { keys })
    }

    pub fn verify_now(
        &self,
        envelope: &SignedProofEnvelopeV1,
        expected: &ExpectedProofBindingV1,
    ) -> Result<VerifiedProofV1, HumanCenteredProtocolError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| HumanCenteredProtocolError::ProofVerificationFailed)?
            .as_secs();
        self.verify_at(envelope, expected, now)
    }

    pub fn verify_at(
        &self,
        envelope: &SignedProofEnvelopeV1,
        expected: &ExpectedProofBindingV1,
        valid_at: u64,
    ) -> Result<VerifiedProofV1, HumanCenteredProtocolError> {
        envelope.validate_shape()?;
        expected.validate()?;
        let claims = &envelope.claims;
        let policy = self
            .keys
            .get(envelope.key_id.as_str())
            .ok_or(HumanCenteredProtocolError::ProofVerificationFailed)?;
        if claims.proof_kind != expected.proof_kind
            || claims.issuer_ref != expected.issuer_ref
            || claims.subject_ref != expected.subject_ref
            || claims.subject_digest != expected.subject_digest
            || claims.tenant_ref != expected.tenant_ref
            || claims.scope_ref != expected.scope_ref
            || claims.purpose_ref != expected.purpose_ref
            || claims.policy_revision_ref != expected.policy_revision_ref
            || claims.issuer_ref != policy.issuer_ref
            || claims.tenant_ref != policy.tenant_ref
            || !policy.allowed_proof_kinds.contains(&claims.proof_kind)
            || valid_at < claims.issued_at
            || valid_at >= claims.expires_at
            || claims.issued_at < policy.valid_from
            || valid_at >= policy.valid_until
            || policy
                .revoked_at
                .is_some_and(|revoked_at| valid_at >= revoked_at)
        {
            return Err(HumanCenteredProtocolError::ProofVerificationFailed);
        }
        let signature_bytes = decode_hex::<64>(&envelope.signature)
            .ok_or(HumanCenteredProtocolError::ProofVerificationFailed)?;
        let signature = Signature::from_bytes(&signature_bytes);
        policy
            .verifying_key
            .verify(&claims.canonical_signing_bytes()?, &signature)
            .map_err(|_| HumanCenteredProtocolError::ProofVerificationFailed)?;
        Ok(VerifiedProofV1 {
            envelope: envelope.clone(),
            verified_at: valid_at,
        })
    }
}

/// Opaque proof accepted only after signature, issuer policy, purpose binding,
/// tenant binding, time window and revocation checks succeed.
#[derive(Debug, PartialEq, Eq)]
pub struct VerifiedProofV1 {
    envelope: SignedProofEnvelopeV1,
    verified_at: u64,
}

impl VerifiedProofV1 {
    pub fn claims(&self) -> &ProofClaimsV1 {
        self.envelope.claims()
    }

    pub fn key_id(&self) -> &ProductionReferenceV1 {
        self.envelope.key_id()
    }

    pub fn signed_envelope(&self) -> &SignedProofEnvelopeV1 {
        &self.envelope
    }

    pub fn verified_at(&self) -> u64 {
        self.verified_at
    }
}

fn append_text(bytes: &mut Vec<u8>, value: &str) -> Result<(), HumanCenteredProtocolError> {
    let length = u32::try_from(value.len()).map_err(|_| HumanCenteredProtocolError::InvalidText)?;
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
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

const fn decode_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn reference(value: &str) -> ProductionReferenceV1 {
        ProductionReferenceV1::new(value).unwrap()
    }

    fn encode_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn proof_verification_binds_signature_kind_tenant_time_and_revocation() {
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let claims = ProofClaimsV1::new(
            ProofKindV1::InputAdmission,
            reference("issuer:gateway"),
            reference("input:event-1"),
            "a".repeat(64),
            reference("tenant:test"),
            reference("scope:interaction"),
            reference("purpose:input-admission"),
            reference("policy:gateway:v1"),
            100,
            200,
            "b".repeat(64),
        )
        .unwrap();
        let signature = signing_key.sign(&claims.canonical_signing_bytes().unwrap());
        let envelope = SignedProofEnvelopeV1::new(
            claims,
            reference("key:gateway:v1"),
            encode_hex(&signature.to_bytes()),
        )
        .unwrap();
        let policy = IssuerKeyPolicyV1::new(
            reference("issuer:gateway"),
            reference("key:gateway:v1"),
            &encode_hex(signing_key.verifying_key().as_bytes()),
            BTreeSet::from([ProofKindV1::InputAdmission]),
            reference("tenant:test"),
            50,
            300,
            None,
        )
        .unwrap();
        let trust_root = ProofTrustRootV1::new(vec![policy]).unwrap();
        let expected = ExpectedProofBindingV1::new(
            ProofKindV1::InputAdmission,
            reference("issuer:gateway"),
            reference("input:event-1"),
            "a".repeat(64),
            reference("tenant:test"),
            reference("scope:interaction"),
            reference("purpose:input-admission"),
            reference("policy:gateway:v1"),
        )
        .unwrap();
        assert!(trust_root.verify_at(&envelope, &expected, 150).is_ok());
        let wrong_purpose = ExpectedProofBindingV1::new(
            ProofKindV1::InputAdmission,
            reference("issuer:gateway"),
            reference("input:event-1"),
            "a".repeat(64),
            reference("tenant:test"),
            reference("scope:interaction"),
            reference("purpose:other"),
            reference("policy:gateway:v1"),
        )
        .unwrap();
        assert!(trust_root
            .verify_at(&envelope, &wrong_purpose, 150)
            .is_err());
        let wrong_kind = ExpectedProofBindingV1::new(
            ProofKindV1::InputAdmission,
            reference("issuer:gateway"),
            reference("input:event-1"),
            "a".repeat(64),
            reference("tenant:test"),
            reference("scope:interaction"),
            reference("purpose:input-admission"),
            reference("policy:gateway:v1"),
        )
        .unwrap();
        let mut wrong_kind = wrong_kind;
        wrong_kind.proof_kind = ProofKindV1::PolicyEvaluation;
        assert!(trust_root.verify_at(&envelope, &wrong_kind, 150).is_err());

        let revoked_policy = IssuerKeyPolicyV1::new(
            reference("issuer:gateway"),
            reference("key:gateway:v1"),
            &encode_hex(signing_key.verifying_key().as_bytes()),
            BTreeSet::from([ProofKindV1::InputAdmission]),
            reference("tenant:test"),
            50,
            300,
            Some(140),
        )
        .unwrap();
        let revoked_root = ProofTrustRootV1::new(vec![revoked_policy]).unwrap();
        assert!(revoked_root.verify_at(&envelope, &expected, 150).is_err());
        assert!(ProductionFeatureGatesV1::BLOCKED.production_trust_root_is_blocked());
    }
}
