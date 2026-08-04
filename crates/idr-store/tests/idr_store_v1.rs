use idr_protocol::human_centered::*;
use idr_protocol::{ProductionAuthorityContextV1, ProductionReferenceV1};
use idr_store::{
    FileHumanCenteredContractStoreV1, HumanCenteredCommitOutcomeV1, HumanCenteredContractSnapshotV1,
};
use std::path::PathBuf;

fn pref(value: &str) -> ProductionReferenceV1 {
    ProductionReferenceV1::new(value).unwrap()
}

fn context() -> ProductionAuthorityContextV1 {
    ProductionAuthorityContextV1::new(
        pref("subject:user-001"),
        pref("actor:user-001"),
        pref("caller:interaction-runtime"),
        pref("tenant:test"),
        pref("scope:test"),
        pref("purpose:test"),
        pref("operation:test"),
    )
}

fn other_context() -> ProductionAuthorityContextV1 {
    ProductionAuthorityContextV1::new(
        pref("subject:user-002"),
        pref("actor:user-002"),
        pref("caller:interaction-runtime"),
        pref("tenant:other"),
        pref("scope:test"),
        pref("purpose:test"),
        pref("operation:test"),
    )
}

fn metadata(
    run_id: HumanCenteredRunIdV1,
    turn_id: HumanCenteredTurnIdV1,
    dependencies: Vec<HumanCenteredContractRefV1>,
) -> HumanCenteredContractMetadataV1 {
    HumanCenteredContractMetadataV1::initial(
        HumanCenteredContractKindV1::Action,
        run_id,
        turn_id,
        context(),
        vec![pref("evidence:test")],
        dependencies,
        pref("policy:human-centered:v1"),
        10_000,
    )
    .unwrap()
}

fn action(
    metadata: HumanCenteredContractMetadataV1,
    operation: &str,
    idempotency_key: &str,
) -> ActionContractV1 {
    ActionContractV1::issue(
        metadata,
        None,
        pref(operation),
        serde_json::json!({"resource_id": "resource-1"}),
        Vec::new(),
        Vec::new(),
        ActionAuthorizationStateV1::NotRequired,
        ImpactLevelV1::Low,
        true,
        idempotency_key,
        vec!["result schema validated".to_string()],
        vec![pref("operation:restore-test-state")],
    )
    .unwrap()
}

fn intent(metadata: HumanCenteredContractMetadataV1) -> IntentContractV1 {
    let hypothesis = RankedIntentHypothesisV1::new(
        IntentHypothesisV1::new(
            pref("intent:test"),
            "test intent",
            UnitIntervalBasisPointsV1::FULL,
            vec![pref("evidence:test")],
            Vec::new(),
        )
        .unwrap(),
        1,
    )
    .unwrap();
    IntentContractV1::issue(
        metadata,
        pref("intent:user-stated"),
        vec![hypothesis.clone()],
        vec![hypothesis],
        pref("intent:test"),
        UnitIntervalBasisPointsV1::FULL,
        false,
        false,
        Vec::new(),
        IntentResolutionMethodV1::FullPath,
    )
    .unwrap()
}

fn force_action_contract_id(
    value: ActionContractV1,
    contract_id: HumanCenteredContractIdV1,
) -> ActionContractV1 {
    use sha2::{Digest, Sha256};

    let mut wire = serde_json::to_value(value).unwrap();
    let compute_digest = |wire: &serde_json::Value| {
        let metadata: HumanCenteredContractMetadataV1 =
            serde_json::from_value(wire["metadata"].clone()).unwrap();
        let decision_ref: Option<HumanCenteredContractRefV1> =
            serde_json::from_value(wire["decision_ref"].clone()).unwrap();
        let operation_ref: ProductionReferenceV1 =
            serde_json::from_value(wire["operation_ref"].clone()).unwrap();
        let parameters = wire["parameters"].clone();
        let parameter_digest: String =
            serde_json::from_value(wire["parameter_digest"].clone()).unwrap();
        let preconditions: Vec<String> =
            serde_json::from_value(wire["preconditions"].clone()).unwrap();
        let required_authority_refs: Vec<ProductionReferenceV1> =
            serde_json::from_value(wire["required_authority_refs"].clone()).unwrap();
        let authorization_state: ActionAuthorizationStateV1 =
            serde_json::from_value(wire["authorization_state"].clone()).unwrap();
        let impact_level: ImpactLevelV1 =
            serde_json::from_value(wire["impact_level"].clone()).unwrap();
        let reversible: bool = serde_json::from_value(wire["reversible"].clone()).unwrap();
        let idempotency_key: String =
            serde_json::from_value(wire["idempotency_key"].clone()).unwrap();
        let verification_requirements: Vec<String> =
            serde_json::from_value(wire["verification_requirements"].clone()).unwrap();
        let compensation_operations: Vec<ProductionReferenceV1> =
            serde_json::from_value(wire["compensation_operations"].clone()).unwrap();
        let digest_input = (
            (
                &metadata,
                &decision_ref,
                &operation_ref,
                &parameters,
                &parameter_digest,
                &preconditions,
            ),
            (
                &required_authority_refs,
                authorization_state,
                impact_level,
                reversible,
                &idempotency_key,
                &verification_requirements,
                &compensation_operations,
            ),
        );
        let bytes = serde_json::to_vec(&("action-contract-v1", digest_input)).unwrap();
        format!("{:x}", Sha256::digest(bytes))
    };
    assert_eq!(
        compute_digest(&wire),
        wire["record_digest"].as_str().unwrap()
    );
    wire["metadata"]["contract_id"] = serde_json::json!(contract_id.to_string());
    wire["record_digest"] = serde_json::json!(compute_digest(&wire));
    serde_json::from_value(wire).unwrap()
}

fn temp_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "aegis-human-centered-store-{}-{}.json",
        std::process::id(),
        uuid::Uuid::new_v4()
    ))
}

fn remove_store_files(path: &PathBuf) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(format!("{}.lock", path.display()));
    let _ = std::fs::remove_file(format!("{}.anchor", path.display()));
}

#[derive(Debug)]
struct TestTrustedClockV1(std::sync::atomic::AtomicU64);

impl TestTrustedClockV1 {
    fn new(now: u64) -> Self {
        Self(std::sync::atomic::AtomicU64::new(now))
    }

    fn set(&self, now: u64) {
        self.0.store(now, std::sync::atomic::Ordering::SeqCst);
    }

    fn current(&self) -> u64 {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl idr_store::TrustedClockV1 for TestTrustedClockV1 {
    fn now(&self) -> Result<u64, idr_store::HumanCenteredStoreErrorV1> {
        Ok(self.0.load(std::sync::atomic::Ordering::SeqCst))
    }
}

fn wall_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn permit_action(
    run_id: HumanCenteredRunIdV1,
    turn_id: HumanCenteredTurnIdV1,
    now: u64,
    operation: &str,
    idempotency_key: &str,
) -> ActionContractV1 {
    action(
        HumanCenteredContractMetadataV1::initial(
            HumanCenteredContractKindV1::Action,
            run_id,
            turn_id,
            context(),
            vec![pref("evidence:test")],
            Vec::new(),
            pref("policy:human-centered:v1"),
            now + 600,
        )
        .unwrap(),
        operation,
        idempotency_key,
    )
}

fn exact_approval(
    action: &ActionContractV1,
    now: u64,
    expires_at: u64,
) -> ExactActionAuthorizationV1 {
    ExactActionAuthorizationV1::issue(
        action,
        pref("actor:user-001"),
        AuthorizationDecisionV1::Approve,
        pref("scope:test"),
        now - 10,
        expires_at,
    )
    .unwrap()
}

fn verified_permit_request(
    action: &ActionContractV1,
    authorization: &ExactActionAuthorizationV1,
    attempt: u32,
    now: u64,
    lease_until: u64,
    nonce_byte: char,
    signing_seed: u8,
) -> VerifiedExecutionPermitRequestV1 {
    use ed25519_dalek::{Signer, SigningKey};
    use std::collections::BTreeSet;

    let nonce = nonce_byte.to_string().repeat(64);
    let request = ExecutionPermitRequestV1::new(
        action,
        authorization,
        pref("provider:test"),
        pref("owner:executor-1"),
        attempt,
        now,
        lease_until,
        nonce,
    )
    .unwrap();
    let signing_key = SigningKey::from_bytes(&[signing_seed; 32]);
    let issuer_ref = pref("issuer:execution-authority");
    let key_id = pref(&format!("key:execution-authority:{signing_seed}"));
    let claims = ProofClaimsV1::new(
        ProofKindV1::ExecutionPermit,
        issuer_ref.clone(),
        request.request_ref().clone(),
        request.request_digest(),
        pref("tenant:test"),
        pref("scope:test"),
        pref("purpose:test"),
        pref("policy:human-centered:v1"),
        now,
        lease_until,
        request.dispatch_nonce(),
    )
    .unwrap();
    let signature = signing_key.sign(&claims.canonical_signing_bytes().unwrap());
    let signature_hex: String = signature
        .to_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let envelope = SignedProofEnvelopeV1::new(claims, key_id.clone(), signature_hex).unwrap();
    let public_key_hex: String = signing_key
        .verifying_key()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let trust_root = ProofTrustRootV1::new(vec![IssuerKeyPolicyV1::new(
        issuer_ref.clone(),
        key_id,
        &public_key_hex,
        BTreeSet::from([ProofKindV1::ExecutionPermit]),
        pref("tenant:test"),
        now - 20,
        now + 600,
        None,
    )
    .unwrap()])
    .unwrap();
    let expected = request.expected_proof_binding(issuer_ref).unwrap();
    let verified = trust_root.verify_at(&envelope, &expected, now).unwrap();
    verify_execution_permit_request(verified, request, action, authorization).unwrap()
}

fn verified_provider_receipt(
    receipt: ExecutionReceiptV1,
    now: u64,
    signing_seed: u8,
) -> VerifiedExecutionReceiptV1 {
    use ed25519_dalek::{Signer, SigningKey};
    use std::collections::BTreeSet;

    let signing_key = SigningKey::from_bytes(&[signing_seed; 32]);
    let claims = ProofClaimsV1::new(
        ProofKindV1::ProviderExecutionReceipt,
        receipt.provider_ref().clone(),
        receipt.proof_subject_ref().unwrap(),
        receipt.record_ref().unwrap().record_digest(),
        pref("tenant:test"),
        pref("scope:test"),
        pref("purpose:test"),
        pref("policy:human-centered:v1"),
        now,
        now + 60,
        receipt.dispatch_nonce(),
    )
    .unwrap();
    let signature = signing_key.sign(&claims.canonical_signing_bytes().unwrap());
    let signature_hex: String = signature
        .to_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let key_id = pref(&format!("key:provider:{signing_seed}"));
    let envelope = SignedProofEnvelopeV1::new(claims, key_id.clone(), signature_hex).unwrap();
    let public_key_hex: String = signing_key
        .verifying_key()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let trust_root = ProofTrustRootV1::new(vec![IssuerKeyPolicyV1::new(
        receipt.provider_ref().clone(),
        key_id,
        &public_key_hex,
        BTreeSet::from([ProofKindV1::ProviderExecutionReceipt]),
        pref("tenant:test"),
        now - 20,
        now + 600,
        None,
    )
    .unwrap()])
    .unwrap();
    let expected = receipt.expected_provider_proof_binding().unwrap();
    let verified = trust_root.verify_at(&envelope, &expected, now).unwrap();
    verify_execution_receipt(verified, receipt).unwrap()
}

#[test]
fn append_only_store_replays_and_invalidates_transitive_dependents() {
    let path = temp_path();
    let run_id = HumanCenteredRunIdV1::mint();
    let turn_id = HumanCenteredTurnIdV1::mint();
    let store = FileHumanCenteredContractStoreV1::open(&path).unwrap();

    let source_v1 = action(
        metadata(run_id, turn_id, Vec::new()),
        "operation:source-read",
        "source-read:v1",
    );
    let source_v1_ref = source_v1.record_ref().unwrap();
    assert!(matches!(
        store
            .commit(HumanCenteredContractSnapshotV1::Action(source_v1.clone()))
            .unwrap(),
        HumanCenteredCommitOutcomeV1::Committed(_)
    ));

    let dependent = action(
        metadata(run_id, turn_id, vec![source_v1_ref.clone()]),
        "operation:dependent-analysis",
        "dependent-analysis:v1",
    );
    let dependent_ref = dependent.record_ref().unwrap();
    store
        .commit(HumanCenteredContractSnapshotV1::Action(dependent.clone()))
        .unwrap();
    assert!(store
        .load_current(
            HumanCenteredContractKindV1::Action,
            dependent_ref.contract_id()
        )
        .unwrap()
        .is_some());

    let transitive_dependent = action(
        metadata(run_id, turn_id, vec![dependent_ref.clone()]),
        "operation:dependent-plan",
        "dependent-plan:v1",
    );
    let transitive_dependent_ref = transitive_dependent.record_ref().unwrap();
    store
        .commit(HumanCenteredContractSnapshotV1::Action(
            transitive_dependent,
        ))
        .unwrap();

    let source_v2_metadata = HumanCenteredContractMetadataV1::successor(
        HumanCenteredContractKindV1::Action,
        source_v1_ref,
        run_id,
        turn_id,
        context(),
        vec![pref("evidence:test"), pref("evidence:new-fact")],
        Vec::new(),
        pref("policy:human-centered:v1"),
        10_000,
    )
    .unwrap();
    let source_v2 = action(
        source_v2_metadata,
        "operation:source-read",
        "source-read:v2",
    );
    store
        .commit(HumanCenteredContractSnapshotV1::Action(source_v2))
        .unwrap();

    assert!(store.is_invalidated(&dependent_ref).unwrap());
    assert!(store.is_invalidated(&transitive_dependent_ref).unwrap());
    assert!(store
        .load_current(
            HumanCenteredContractKindV1::Action,
            dependent_ref.contract_id()
        )
        .unwrap()
        .is_none());
    assert!(store
        .load_current(
            HumanCenteredContractKindV1::Action,
            transitive_dependent_ref.contract_id()
        )
        .unwrap()
        .is_none());
    assert_eq!(store.revision_count().unwrap(), 4);

    let reopened = FileHumanCenteredContractStoreV1::open(&path).unwrap();
    assert!(reopened.is_invalidated(&dependent_ref).unwrap());
    assert!(reopened.is_invalidated(&transitive_dependent_ref).unwrap());
    assert_eq!(reopened.revision_count().unwrap(), 4);

    let mut persisted: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    persisted["invalidated_refs"]
        .as_array_mut()
        .unwrap()
        .clear();
    std::fs::write(&path, serde_json::to_vec(&persisted).unwrap()).unwrap();
    assert!(matches!(
        FileHumanCenteredContractStoreV1::open(&path),
        Err(idr_store::HumanCenteredStoreErrorV1::CorruptSnapshot)
    ));
    remove_store_files(&path);
}

#[test]
fn superseding_a_contract_invalidates_cross_kind_dependents_with_the_same_uuid() {
    let path = temp_path();
    let run_id = HumanCenteredRunIdV1::mint();
    let turn_id = HumanCenteredTurnIdV1::mint();
    let store = FileHumanCenteredContractStoreV1::open(&path).unwrap();

    let source_v1 = intent(
        HumanCenteredContractMetadataV1::initial(
            HumanCenteredContractKindV1::Intent,
            run_id,
            turn_id,
            context(),
            vec![pref("evidence:test")],
            Vec::new(),
            pref("policy:human-centered:v1"),
            10_000,
        )
        .unwrap(),
    );
    let source_v1_ref = source_v1.record_ref().unwrap();
    store
        .commit(HumanCenteredContractSnapshotV1::Intent(source_v1.clone()))
        .unwrap();

    let dependent_v1 = force_action_contract_id(
        action(
            metadata(run_id, turn_id, Vec::new()),
            "operation:cross-kind-dependent",
            "cross-kind-dependent:v1",
        ),
        source_v1_ref.contract_id(),
    );
    let dependent_v1_ref = dependent_v1.record_ref().unwrap();
    store
        .commit(HumanCenteredContractSnapshotV1::Action(dependent_v1))
        .unwrap();
    let dependent_v2 = action(
        HumanCenteredContractMetadataV1::successor(
            HumanCenteredContractKindV1::Action,
            dependent_v1_ref,
            run_id,
            turn_id,
            context(),
            vec![pref("evidence:test")],
            vec![source_v1_ref.clone()],
            pref("policy:human-centered:v1"),
            10_000,
        )
        .unwrap(),
        "operation:cross-kind-dependent",
        "cross-kind-dependent:v2",
    );
    let dependent_ref = dependent_v2.record_ref().unwrap();
    assert_ne!(dependent_ref.kind(), source_v1_ref.kind());
    assert_eq!(dependent_ref.contract_id(), source_v1_ref.contract_id());
    store
        .commit(HumanCenteredContractSnapshotV1::Action(dependent_v2))
        .unwrap();

    let source_v2 = intent(
        HumanCenteredContractMetadataV1::successor(
            HumanCenteredContractKindV1::Intent,
            source_v1_ref,
            run_id,
            turn_id,
            context(),
            vec![pref("evidence:test"), pref("evidence:new-fact")],
            Vec::new(),
            pref("policy:human-centered:v1"),
            10_000,
        )
        .unwrap(),
    );
    store
        .commit(HumanCenteredContractSnapshotV1::Intent(source_v2))
        .unwrap();

    assert!(store.is_invalidated(&dependent_ref).unwrap());
    let reopened = FileHumanCenteredContractStoreV1::open(&path).unwrap();
    assert!(reopened.is_invalidated(&dependent_ref).unwrap());
    remove_store_files(&path);
}

#[test]
fn existing_empty_store_file_is_corruption_not_a_new_store() {
    let path = temp_path();
    std::fs::write(&path, b"").unwrap();
    assert!(matches!(
        FileHumanCenteredContractStoreV1::open(&path),
        Err(idr_store::HumanCenteredStoreErrorV1::CorruptSnapshot)
    ));
    remove_store_files(&path);
}

#[test]
fn old_snapshot_rollback_is_detected_by_the_monotonic_anchor() {
    let path = temp_path();
    let store = FileHumanCenteredContractStoreV1::open(&path).unwrap();
    store
        .commit(HumanCenteredContractSnapshotV1::Action(action(
            metadata(
                HumanCenteredRunIdV1::mint(),
                HumanCenteredTurnIdV1::mint(),
                Vec::new(),
            ),
            "operation:rollback-a",
            "rollback-a:v1",
        )))
        .unwrap();
    let old_snapshot = std::fs::read(&path).unwrap();
    store
        .commit(HumanCenteredContractSnapshotV1::Action(action(
            metadata(
                HumanCenteredRunIdV1::mint(),
                HumanCenteredTurnIdV1::mint(),
                Vec::new(),
            ),
            "operation:rollback-b",
            "rollback-b:v1",
        )))
        .unwrap();

    std::fs::write(&path, old_snapshot).unwrap();
    assert!(matches!(
        FileHumanCenteredContractStoreV1::open(&path),
        Err(idr_store::HumanCenteredStoreErrorV1::RollbackDetected)
    ));
    remove_store_files(&path);
}

#[test]
fn deleting_a_non_initial_anchor_is_detected() {
    let path = temp_path();
    let store = FileHumanCenteredContractStoreV1::open(&path).unwrap();
    for suffix in ["a", "b"] {
        store
            .commit(HumanCenteredContractSnapshotV1::Action(action(
                metadata(
                    HumanCenteredRunIdV1::mint(),
                    HumanCenteredTurnIdV1::mint(),
                    Vec::new(),
                ),
                &format!("operation:anchor-{suffix}"),
                &format!("anchor-{suffix}:v1"),
            )))
            .unwrap();
    }
    std::fs::remove_file(format!("{}.anchor", path.display())).unwrap();
    assert!(matches!(
        FileHumanCenteredContractStoreV1::open(&path),
        Err(idr_store::HumanCenteredStoreErrorV1::RollbackDetected)
    ));
    remove_store_files(&path);
}

#[test]
fn incomplete_final_anchor_record_is_recovered_from_the_sealed_snapshot() {
    let path = temp_path();
    let store = FileHumanCenteredContractStoreV1::open(&path).unwrap();
    store
        .commit(HumanCenteredContractSnapshotV1::Action(action(
            metadata(
                HumanCenteredRunIdV1::mint(),
                HumanCenteredTurnIdV1::mint(),
                Vec::new(),
            ),
            "operation:anchor-recovery-a",
            "anchor-recovery-a:v1",
        )))
        .unwrap();
    let anchor_path = format!("{}.anchor", path.display());
    let first_anchor = std::fs::read(&anchor_path).unwrap();

    store
        .commit(HumanCenteredContractSnapshotV1::Action(action(
            metadata(
                HumanCenteredRunIdV1::mint(),
                HumanCenteredTurnIdV1::mint(),
                Vec::new(),
            ),
            "operation:anchor-recovery-b",
            "anchor-recovery-b:v1",
        )))
        .unwrap();
    let complete_anchor = std::fs::read(&anchor_path).unwrap();
    let second_record_len = complete_anchor.len() - first_anchor.len();
    std::fs::write(
        &anchor_path,
        &complete_anchor[..first_anchor.len() + second_record_len / 2],
    )
    .unwrap();

    let reopened = FileHumanCenteredContractStoreV1::open(&path).unwrap();
    assert_eq!(reopened.revision_count().unwrap(), 2);
    let recovered = std::fs::read_to_string(anchor_path).unwrap();
    assert!(recovered.ends_with('\n'));
    assert_eq!(recovered.lines().count(), 2);
    remove_store_files(&path);
}

#[test]
fn malformed_committed_anchor_record_is_not_silently_truncated() {
    let path = temp_path();
    let store = FileHumanCenteredContractStoreV1::open(&path).unwrap();
    store
        .commit(HumanCenteredContractSnapshotV1::Action(action(
            metadata(
                HumanCenteredRunIdV1::mint(),
                HumanCenteredTurnIdV1::mint(),
                Vec::new(),
            ),
            "operation:anchor-corrupt",
            "anchor-corrupt:v1",
        )))
        .unwrap();
    let anchor_path = format!("{}.anchor", path.display());
    let mut bytes = std::fs::read(&anchor_path).unwrap();
    bytes.extend_from_slice(b"not-json\n");
    std::fs::write(anchor_path, bytes).unwrap();

    assert!(matches!(
        FileHumanCenteredContractStoreV1::open(&path),
        Err(idr_store::HumanCenteredStoreErrorV1::CorruptAnchor)
    ));
    remove_store_files(&path);
}

#[test]
fn reopen_rejects_same_revision_forks() {
    let path = temp_path();
    let shared_metadata = metadata(
        HumanCenteredRunIdV1::mint(),
        HumanCenteredTurnIdV1::mint(),
        Vec::new(),
    );
    let first = action(shared_metadata.clone(), "operation:fork-a", "fork-a:v1");
    let fork = action(shared_metadata, "operation:fork-b", "fork-b:v1");
    let store = FileHumanCenteredContractStoreV1::open(&path).unwrap();
    store
        .commit(HumanCenteredContractSnapshotV1::Action(first))
        .unwrap();

    let mut persisted: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    persisted["records"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::to_value(HumanCenteredContractSnapshotV1::Action(fork)).unwrap());
    std::fs::write(&path, serde_json::to_vec(&persisted).unwrap()).unwrap();
    assert!(matches!(
        FileHumanCenteredContractStoreV1::open(&path),
        Err(idr_store::HumanCenteredStoreErrorV1::CorruptSnapshot)
    ));
    remove_store_files(&path);
}

#[test]
fn successor_cannot_cross_run_turn_or_authority_context() {
    let path = temp_path();
    let run_id = HumanCenteredRunIdV1::mint();
    let turn_id = HumanCenteredTurnIdV1::mint();
    let store = FileHumanCenteredContractStoreV1::open(&path).unwrap();
    let first = action(
        metadata(run_id, turn_id, Vec::new()),
        "operation:lineage",
        "lineage:v1",
    );
    let first_ref = first.record_ref().unwrap();
    store
        .commit(HumanCenteredContractSnapshotV1::Action(first))
        .unwrap();
    let successor = action(
        HumanCenteredContractMetadataV1::successor(
            HumanCenteredContractKindV1::Action,
            first_ref,
            run_id,
            turn_id,
            other_context(),
            vec![pref("evidence:test")],
            Vec::new(),
            pref("policy:human-centered:v1"),
            10_000,
        )
        .unwrap(),
        "operation:lineage",
        "lineage:v2",
    );
    assert!(matches!(
        store.commit(HumanCenteredContractSnapshotV1::Action(successor)),
        Err(idr_store::HumanCenteredStoreErrorV1::CrossObjectMismatch)
    ));
    remove_store_files(&path);
}

#[test]
fn duplicate_commit_is_idempotent() {
    let path = temp_path();
    let store = FileHumanCenteredContractStoreV1::open(&path).unwrap();
    let value = action(
        metadata(
            HumanCenteredRunIdV1::mint(),
            HumanCenteredTurnIdV1::mint(),
            Vec::new(),
        ),
        "operation:read",
        "read:v1",
    );
    store
        .commit(HumanCenteredContractSnapshotV1::Action(value.clone()))
        .unwrap();
    assert!(matches!(
        store
            .commit(HumanCenteredContractSnapshotV1::Action(value))
            .unwrap(),
        HumanCenteredCommitOutcomeV1::AlreadyCommitted(_)
    ));
    assert_eq!(store.revision_count().unwrap(), 1);
    remove_store_files(&path);
}

#[test]
fn reopen_rejects_reordered_causal_history() {
    let path = temp_path();
    let run_id = HumanCenteredRunIdV1::mint();
    let turn_id = HumanCenteredTurnIdV1::mint();
    let store = FileHumanCenteredContractStoreV1::open(&path).unwrap();
    let source = action(
        metadata(run_id, turn_id, Vec::new()),
        "operation:source",
        "source:v1",
    );
    let source_ref = source.record_ref().unwrap();
    store
        .commit(HumanCenteredContractSnapshotV1::Action(source))
        .unwrap();
    store
        .commit(HumanCenteredContractSnapshotV1::Action(action(
            metadata(run_id, turn_id, vec![source_ref]),
            "operation:dependent",
            "dependent:v1",
        )))
        .unwrap();

    let mut persisted: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    persisted["records"].as_array_mut().unwrap().reverse();
    std::fs::write(&path, serde_json::to_vec(&persisted).unwrap()).unwrap();

    assert!(matches!(
        FileHumanCenteredContractStoreV1::open(&path),
        Err(idr_store::HumanCenteredStoreErrorV1::CorruptSnapshot)
    ));
    remove_store_files(&path);
}

#[test]
fn execution_permit_proof_and_nonce_are_consumed_atomically_and_durably() {
    use ed25519_dalek::{Signer, SigningKey};
    use std::collections::BTreeSet;
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let path = temp_path();
    let store = FileHumanCenteredContractStoreV1::open(&path).unwrap();
    let value = action(
        HumanCenteredContractMetadataV1::initial(
            HumanCenteredContractKindV1::Action,
            HumanCenteredRunIdV1::mint(),
            HumanCenteredTurnIdV1::mint(),
            context(),
            vec![pref("evidence:test")],
            Vec::new(),
            pref("policy:human-centered:v1"),
            now + 300,
        )
        .unwrap(),
        "operation:permit-once",
        "permit-once:v1",
    );
    store
        .commit(HumanCenteredContractSnapshotV1::Action(value.clone()))
        .unwrap();
    let authorization = ExactActionAuthorizationV1::issue(
        &value,
        pref("actor:user-001"),
        AuthorizationDecisionV1::Approve,
        pref("scope:test"),
        now - 10,
        now + 180,
    )
    .unwrap();
    let request = ExecutionPermitRequestV1::new(
        &value,
        &authorization,
        pref("provider:test"),
        pref("owner:executor-1"),
        1,
        now,
        now + 60,
        "d".repeat(64),
    )
    .unwrap();
    let signing_key = SigningKey::from_bytes(&[11_u8; 32]);
    let claims = ProofClaimsV1::new(
        ProofKindV1::ExecutionPermit,
        pref("issuer:execution-authority"),
        request.request_ref().clone(),
        request.request_digest(),
        pref("tenant:test"),
        pref("scope:test"),
        pref("purpose:test"),
        pref("policy:human-centered:v1"),
        now,
        now + 60,
        request.dispatch_nonce(),
    )
    .unwrap();
    let signature = signing_key.sign(&claims.canonical_signing_bytes().unwrap());
    let signature_hex: String = signature
        .to_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let envelope =
        SignedProofEnvelopeV1::new(claims, pref("key:execution-authority:v1"), signature_hex)
            .unwrap();
    let public_key_hex: String = signing_key
        .verifying_key()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let trust_root = ProofTrustRootV1::new(vec![IssuerKeyPolicyV1::new(
        pref("issuer:execution-authority"),
        pref("key:execution-authority:v1"),
        &public_key_hex,
        BTreeSet::from([ProofKindV1::ExecutionPermit]),
        pref("tenant:test"),
        now - 20,
        now + 300,
        None,
    )
    .unwrap()])
    .unwrap();
    let expected = request
        .expected_proof_binding(pref("issuer:execution-authority"))
        .unwrap();
    let verified = trust_root.verify_now(&envelope, &expected).unwrap();
    let verified_request =
        verify_execution_permit_request(verified, request.clone(), &value, &authorization).unwrap();
    let permit = store.issue_execution_permit(verified_request).unwrap();
    assert_eq!(permit.request().action_ref(), &value.record_ref().unwrap());
    assert_eq!(permit.request().provider_ref(), &pref("provider:test"));

    let verified_again = trust_root.verify_now(&envelope, &expected).unwrap();
    let replayed_request =
        verify_execution_permit_request(verified_again, request, &value, &authorization).unwrap();
    let reopened = FileHumanCenteredContractStoreV1::open(&path).unwrap();
    assert!(matches!(
        reopened.issue_execution_permit(replayed_request),
        Err(idr_store::HumanCenteredStoreErrorV1::ProofReplayDetected)
    ));
    remove_store_files(&path);
}

#[test]
fn two_authorizations_cannot_mint_parallel_permits_for_one_action() {
    let now = wall_now();
    let path = temp_path();
    let clock = std::sync::Arc::new(TestTrustedClockV1::new(now));
    let store = FileHumanCenteredContractStoreV1::open_with_clock(&path, clock.clone()).unwrap();
    let value = permit_action(
        HumanCenteredRunIdV1::mint(),
        HumanCenteredTurnIdV1::mint(),
        now,
        "operation:single-reservation",
        "single-reservation:v1",
    );
    store
        .commit(HumanCenteredContractSnapshotV1::Action(value.clone()))
        .unwrap();
    let first_authorization = exact_approval(&value, now, now + 180);
    let second_authorization = exact_approval(&value, now, now + 180);
    let first = verified_permit_request(&value, &first_authorization, 1, now, now + 60, 'a', 21);
    let second = verified_permit_request(&value, &second_authorization, 1, now, now + 60, 'b', 22);
    clock.set(first.verified_at().max(second.verified_at()));

    store.issue_execution_permit(first).unwrap();
    assert!(matches!(
        store.issue_execution_permit(second),
        Err(idr_store::HumanCenteredStoreErrorV1::ExecutionAlreadyReserved)
    ));
    remove_store_files(&path);
}

#[test]
fn concurrent_store_instances_create_only_one_action_reservation() {
    let now = wall_now();
    let path = temp_path();
    let clock = std::sync::Arc::new(TestTrustedClockV1::new(now));
    let first = FileHumanCenteredContractStoreV1::open_with_clock(&path, clock.clone()).unwrap();
    let value = permit_action(
        HumanCenteredRunIdV1::mint(),
        HumanCenteredTurnIdV1::mint(),
        now,
        "operation:concurrent-reservation",
        "concurrent-reservation:v1",
    );
    first
        .commit(HumanCenteredContractSnapshotV1::Action(value.clone()))
        .unwrap();
    let second = FileHumanCenteredContractStoreV1::open_with_clock(&path, clock.clone()).unwrap();
    let first_authorization = exact_approval(&value, now, now + 180);
    let second_authorization = exact_approval(&value, now, now + 180);
    let first_request =
        verified_permit_request(&value, &first_authorization, 1, now, now + 60, '7', 27);
    let second_request =
        verified_permit_request(&value, &second_authorization, 1, now, now + 60, '8', 28);
    clock.set(
        first_request
            .verified_at()
            .max(second_request.verified_at()),
    );
    let first_handle =
        std::thread::spawn(move || first.issue_execution_permit(first_request).is_ok());
    let second_handle =
        std::thread::spawn(move || second.issue_execution_permit(second_request).is_ok());
    assert_eq!(
        [first_handle.join().unwrap(), second_handle.join().unwrap()]
            .into_iter()
            .filter(|issued| *issued)
            .count(),
        1
    );
    remove_store_files(&path);
}

#[test]
fn trusted_store_clock_rejects_verify_then_delay_after_lease_expiry() {
    let now = wall_now();
    let path = temp_path();
    let clock = std::sync::Arc::new(TestTrustedClockV1::new(now));
    let store = FileHumanCenteredContractStoreV1::open_with_clock(&path, clock.clone()).unwrap();
    let value = permit_action(
        HumanCenteredRunIdV1::mint(),
        HumanCenteredTurnIdV1::mint(),
        now,
        "operation:clock-checked",
        "clock-checked:v1",
    );
    store
        .commit(HumanCenteredContractSnapshotV1::Action(value.clone()))
        .unwrap();
    let authorization = exact_approval(&value, now, now + 180);
    let verified = verified_permit_request(&value, &authorization, 1, now, now + 60, 'c', 23);

    clock.set(now + 61);
    assert!(matches!(
        store.issue_execution_permit(verified),
        Err(idr_store::HumanCenteredStoreErrorV1::Protocol(
            HumanCenteredProtocolError::ProofVerificationFailed
        ))
    ));
    remove_store_files(&path);
}

#[test]
fn persisted_permit_is_recoverable_until_dispatch_but_not_after() {
    let now = wall_now();
    let path = temp_path();
    let clock = std::sync::Arc::new(TestTrustedClockV1::new(now));
    let store = FileHumanCenteredContractStoreV1::open_with_clock(&path, clock.clone()).unwrap();
    let value = permit_action(
        HumanCenteredRunIdV1::mint(),
        HumanCenteredTurnIdV1::mint(),
        now,
        "operation:recoverable",
        "recoverable:v1",
    );
    let action_ref = value.record_ref().unwrap();
    store
        .commit(HumanCenteredContractSnapshotV1::Action(value.clone()))
        .unwrap();
    let authorization = exact_approval(&value, now, now + 180);
    let verified = verified_permit_request(&value, &authorization, 1, now, now + 60, 'd', 24);
    clock.set(verified.verified_at());
    store.issue_execution_permit(verified).unwrap();

    let reopened = FileHumanCenteredContractStoreV1::open_with_clock(&path, clock.clone()).unwrap();
    let recovered = reopened
        .recover_execution_permit(&action_ref, &pref("owner:executor-1"))
        .unwrap();
    reopened
        .mark_execution_permit_delivered(&recovered)
        .unwrap();
    let recovered_after_delivery = reopened
        .recover_execution_permit(&action_ref, &pref("owner:executor-1"))
        .unwrap();
    reopened
        .mark_execution_dispatched(&recovered_after_delivery)
        .unwrap();
    assert!(matches!(
        reopened.recover_execution_permit(&action_ref, &pref("owner:executor-1")),
        Err(idr_store::HumanCenteredStoreErrorV1::ExecutionRequiresReconciliation)
    ));
    remove_store_files(&path);
}

#[test]
fn invalidated_action_cancels_undispatched_permit_and_blocks_dispatch() {
    let now = wall_now();
    let path = temp_path();
    let clock = std::sync::Arc::new(TestTrustedClockV1::new(now));
    let store = FileHumanCenteredContractStoreV1::open_with_clock(&path, clock.clone()).unwrap();
    let run_id = HumanCenteredRunIdV1::mint();
    let turn_id = HumanCenteredTurnIdV1::mint();

    let source = action(
        HumanCenteredContractMetadataV1::initial(
            HumanCenteredContractKindV1::Action,
            run_id,
            turn_id,
            context(),
            vec![pref("evidence:source")],
            Vec::new(),
            pref("policy:human-centered:v1"),
            now + 600,
        )
        .unwrap(),
        "operation:source",
        "source:round6:v1",
    );
    let source_ref = source.record_ref().unwrap();
    store
        .commit(HumanCenteredContractSnapshotV1::Action(source.clone()))
        .unwrap();

    let dependent = action(
        HumanCenteredContractMetadataV1::initial(
            HumanCenteredContractKindV1::Action,
            run_id,
            turn_id,
            context(),
            vec![pref("evidence:dependent")],
            vec![source_ref.clone()],
            pref("policy:human-centered:v1"),
            now + 600,
        )
        .unwrap(),
        "operation:dependent",
        "dependent:round6:v1",
    );
    let dependent_ref = dependent.record_ref().unwrap();
    store
        .commit(HumanCenteredContractSnapshotV1::Action(dependent.clone()))
        .unwrap();
    let authorization = exact_approval(&dependent, now, now + 180);
    let verified = verified_permit_request(&dependent, &authorization, 1, now, now + 60, '9', 39);
    let permit = store.issue_execution_permit(verified).unwrap();

    let successor = action(
        HumanCenteredContractMetadataV1::successor(
            HumanCenteredContractKindV1::Action,
            source_ref,
            run_id,
            turn_id,
            context(),
            vec![pref("evidence:source-revised")],
            Vec::new(),
            pref("policy:human-centered:v1"),
            now + 600,
        )
        .unwrap(),
        "operation:source",
        "source:round6:v2",
    );
    store
        .commit(HumanCenteredContractSnapshotV1::Action(successor))
        .unwrap();

    assert!(store.is_invalidated(&dependent_ref).unwrap());
    assert!(matches!(
        store.mark_execution_permit_delivered(&permit),
        Err(idr_store::HumanCenteredStoreErrorV1::ExecutionActionInvalidated)
    ));
    assert!(matches!(
        store.mark_execution_dispatched(&permit),
        Err(idr_store::HumanCenteredStoreErrorV1::ExecutionActionInvalidated)
            | Err(idr_store::HumanCenteredStoreErrorV1::InvalidExecutionTransition)
    ));
    remove_store_files(&path);
}

#[test]
fn expired_lost_permit_is_persisted_as_expired_and_next_attempt_can_issue() {
    let now = wall_now();
    let path = temp_path();
    let clock = std::sync::Arc::new(TestTrustedClockV1::new(now));
    let store = FileHumanCenteredContractStoreV1::open_with_clock(&path, clock.clone()).unwrap();
    let value = permit_action(
        HumanCenteredRunIdV1::mint(),
        HumanCenteredTurnIdV1::mint(),
        now,
        "operation:expired-recovery",
        "expired-recovery:v1",
    );
    let action_ref = value.record_ref().unwrap();
    store
        .commit(HumanCenteredContractSnapshotV1::Action(value.clone()))
        .unwrap();
    let first_authorization = exact_approval(&value, now, now + 180);
    let first = verified_permit_request(&value, &first_authorization, 1, now, now + 60, '6', 36);
    store.issue_execution_permit(first).unwrap();

    clock.set(now + 61);
    assert!(matches!(
        store.recover_execution_permit(&action_ref, &pref("owner:executor-1")),
        Err(idr_store::HumanCenteredStoreErrorV1::ExecutionLeaseExpired)
    ));

    let second_authorization = exact_approval(&value, now + 61, now + 240);
    let second = verified_permit_request(
        &value,
        &second_authorization,
        2,
        now + 61,
        now + 120,
        '5',
        35,
    );
    let permit = store.issue_execution_permit(second).unwrap();
    assert_eq!(permit.request().attempt(), 2);
    remove_store_files(&path);
}

#[test]
fn receipt_must_match_dispatched_permit_and_failed_attempt_can_retry() {
    let now = wall_now();
    let path = temp_path();
    let clock = std::sync::Arc::new(TestTrustedClockV1::new(now));
    let store = FileHumanCenteredContractStoreV1::open_with_clock(&path, clock.clone()).unwrap();
    let run_id = HumanCenteredRunIdV1::mint();
    let turn_id = HumanCenteredTurnIdV1::mint();
    let value = permit_action(
        run_id,
        turn_id,
        now,
        "operation:receipt-bound",
        "receipt-bound:v1",
    );
    let action_ref = value.record_ref().unwrap();
    store
        .commit(HumanCenteredContractSnapshotV1::Action(value.clone()))
        .unwrap();
    let first_authorization = exact_approval(&value, now, now + 180);
    let first_verified =
        verified_permit_request(&value, &first_authorization, 1, now, now + 60, 'e', 25);
    clock.set(first_verified.verified_at());
    let first_permit = store.issue_execution_permit(first_verified).unwrap();
    store
        .mark_execution_permit_delivered(&first_permit)
        .unwrap();
    store.mark_execution_dispatched(&first_permit).unwrap();
    let receipt_time = wall_now().max(clock.current());

    let receipt_metadata = || {
        HumanCenteredContractMetadataV1::initial(
            HumanCenteredContractKindV1::ExecutionReceipt,
            run_id,
            turn_id,
            context(),
            vec![pref("evidence:provider-response")],
            vec![action_ref.clone()],
            pref("policy:human-centered:v1"),
            now + 600,
        )
        .unwrap()
    };
    let mismatched = ExecutionReceiptV1::issue(
        receipt_metadata(),
        &value,
        first_permit.permit_id(),
        first_permit.request().authorization_id(),
        pref("provider:attacker"),
        first_permit.request().owner_ref().clone(),
        first_permit.request().dispatch_nonce(),
        Some(pref("execution:test-1")),
        1,
        receipt_time,
        receipt_time,
        ExecutionStateV1::Failed,
        None,
        Vec::new(),
        Vec::new(),
        vec!["provider_error".to_string()],
    )
    .unwrap();
    let mismatched = verified_provider_receipt(mismatched, receipt_time, 31);
    clock.set(mismatched.verified_at());
    assert!(matches!(
        store.commit_execution_receipt(mismatched),
        Err(idr_store::HumanCenteredStoreErrorV1::CrossObjectMismatch)
    ));

    let failed = ExecutionReceiptV1::issue(
        receipt_metadata(),
        &value,
        first_permit.permit_id(),
        first_permit.request().authorization_id(),
        first_permit.request().provider_ref().clone(),
        first_permit.request().owner_ref().clone(),
        first_permit.request().dispatch_nonce(),
        Some(pref("execution:test-1")),
        1,
        receipt_time,
        receipt_time,
        ExecutionStateV1::Failed,
        None,
        Vec::new(),
        Vec::new(),
        vec!["provider_error".to_string()],
    )
    .unwrap();
    let failed = verified_provider_receipt(failed, receipt_time, 32);
    clock.set(failed.verified_at());
    store.commit_execution_receipt(failed).unwrap();

    let retry_issued_at = wall_now();
    let second_authorization = exact_approval(&value, retry_issued_at, retry_issued_at + 180);
    let second_verified = verified_permit_request(
        &value,
        &second_authorization,
        2,
        retry_issued_at,
        retry_issued_at + 60,
        'f',
        26,
    );
    clock.set(second_verified.verified_at());
    let retry = store.issue_execution_permit(second_verified).unwrap();
    assert_eq!(retry.request().attempt(), 2);
    assert_eq!(retry.reservation_id(), first_permit.reservation_id());
    remove_store_files(&path);
}

#[test]
fn store_enforces_decision_authorization_boundary_on_actions() {
    let path = temp_path();
    let run_id = HumanCenteredRunIdV1::mint();
    let turn_id = HumanCenteredTurnIdV1::mint();
    let store = FileHumanCenteredContractStoreV1::open(&path).unwrap();
    let hypothesis = RankedIntentHypothesisV1::new(
        IntentHypothesisV1::new(
            pref("intent:change-resource"),
            "change resource",
            UnitIntervalBasisPointsV1::FULL,
            vec![pref("evidence:user-request")],
            Vec::new(),
        )
        .unwrap(),
        1,
    )
    .unwrap();
    let intent = IntentContractV1::issue(
        HumanCenteredContractMetadataV1::initial(
            HumanCenteredContractKindV1::Intent,
            run_id,
            turn_id,
            context(),
            vec![pref("evidence:user-request")],
            Vec::new(),
            pref("policy:human-centered:v1"),
            10_000,
        )
        .unwrap(),
        pref("intent:change-resource"),
        vec![hypothesis.clone()],
        vec![hypothesis],
        pref("intent:change-resource"),
        UnitIntervalBasisPointsV1::FULL,
        false,
        false,
        Vec::new(),
        IntentResolutionMethodV1::FullPath,
    )
    .unwrap();
    let intent_ref = intent.record_ref().unwrap();
    store
        .commit(HumanCenteredContractSnapshotV1::Intent(intent))
        .unwrap();

    let option = DecisionOptionV1::new(
        pref("operation:update-resource"),
        "Update resource",
        "Apply the requested change",
        vec![pref("evidence:user-request")],
        true,
    )
    .unwrap();
    let decision = DecisionContractV1::issue(
        HumanCenteredContractMetadataV1::initial(
            HumanCenteredContractKindV1::Decision,
            run_id,
            turn_id,
            context(),
            vec![pref("evidence:user-request")],
            vec![intent_ref.clone()],
            pref("policy:human-centered:v1"),
            10_000,
        )
        .unwrap(),
        intent_ref,
        "resource change",
        DecisionStageV1::RecommendationReady,
        "choose whether to update",
        Vec::new(),
        Vec::new(),
        vec![option],
        vec![pref("evidence:user-request")],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Some(DecisionRecommendationV1 {
            option_ref: pref("operation:update-resource"),
            confidence: UnitIntervalBasisPointsV1::FULL,
            conditions: Vec::new(),
            evidence_refs: vec![pref("evidence:user-request")],
        }),
        None,
        Vec::new(),
        DecisionAuthorizationBoundaryV1 {
            maximum_automatic_impact: ImpactLevelV1::Low,
            user_confirmation_required: true,
            required_authority_refs: Vec::new(),
            prohibited_automatic_operations: Vec::new(),
        },
    )
    .unwrap();
    let decision_ref = decision.record_ref().unwrap();
    store
        .commit(HumanCenteredContractSnapshotV1::Decision(decision))
        .unwrap();

    let unauthorized_action = ActionContractV1::issue(
        HumanCenteredContractMetadataV1::initial(
            HumanCenteredContractKindV1::Action,
            run_id,
            turn_id,
            context(),
            vec![pref("evidence:user-request")],
            vec![decision_ref.clone()],
            pref("policy:human-centered:v1"),
            10_000,
        )
        .unwrap(),
        Some(decision_ref),
        pref("operation:update-resource"),
        serde_json::json!({"resource_id": "resource-1"}),
        Vec::new(),
        Vec::new(),
        ActionAuthorizationStateV1::NotRequired,
        ImpactLevelV1::Low,
        false,
        "update-resource:without-required-confirmation",
        vec!["result schema validated".to_string()],
        Vec::new(),
    )
    .unwrap();
    assert!(matches!(
        store.commit(HumanCenteredContractSnapshotV1::Action(unauthorized_action)),
        Err(idr_store::HumanCenteredStoreErrorV1::CrossObjectMismatch)
    ));
    remove_store_files(&path);
}
