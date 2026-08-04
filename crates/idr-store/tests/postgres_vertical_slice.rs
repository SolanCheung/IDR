use ed25519_dalek::{Signer, SigningKey};
use idr_protocol::production::{
    canonical_digest_v1, CandidateKindV1, CandidateSubmissionV1, ProductionIssuerKeyV1,
    ProductionProofClaimsV1, ProductionProofEnvelopeV1, ProductionProofKindV1, TrustDomainV1,
    TrustRootSnapshotV1,
};
use idr_protocol::ReferenceV1;
use idr_runtime::{AuthoritativeRecordRefV1, IdrCommandEnvelopeV1, IdrCommandV1};
use idr_store::{
    HumanModelQueryV1, InMemoryAuditCheckpointAnchorV1, PostgresIdrOrchestratorV1,
    PostgresSecurityContextV1, RotatingTrustRootProviderV1,
};
use sha2::{Digest, Sha256};
use sqlx::{Executor, PgPool};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use uuid::Uuid;

static NONCE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn reference(value: &str) -> ReferenceV1 {
    ReferenceV1::new(value).unwrap()
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

async fn drop_shadow_schema_and_roles(admin: &PgPool, schema: &str) {
    let role_digest = &digest(schema.as_bytes())[..32];
    admin
        .execute(format!("DROP SCHEMA {schema} CASCADE").as_str())
        .await
        .unwrap();
    for prefix in ["idr_runtime_", "idr_auditor_"] {
        admin
            .execute(format!("DROP ROLE IF EXISTS {prefix}{role_digest}").as_str())
            .await
            .unwrap();
    }
    admin
        .execute(format!("DROP OWNED BY idr_owner_{role_digest}").as_str())
        .await
        .unwrap();
    admin
        .execute(format!("DROP ROLE IF EXISTS idr_owner_{role_digest}").as_str())
        .await
        .unwrap();
}

fn all_proof_kinds() -> BTreeSet<ProductionProofKindV1> {
    use ProductionProofKindV1::*;
    BTreeSet::from([
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
    ])
}

fn candidate(
    kind: CandidateKindV1,
    run_id: Uuid,
    turn_id: Uuid,
    payload: serde_json::Value,
) -> CandidateSubmissionV1 {
    CandidateSubmissionV1::new(
        kind,
        run_id,
        turn_id,
        reference("subject:user-vertical"),
        reference("tenant:vertical"),
        reference("scope:interaction"),
        reference("purpose:vertical-test"),
        reference("policy:idr:v1"),
        1,
        now() + 3_600,
        payload,
    )
    .unwrap()
}

fn command(run_id: Uuid, expected_version: u64, command: IdrCommandV1) -> IdrCommandEnvelopeV1 {
    command_as(
        run_id,
        expected_version,
        reference("caller:vertical-test"),
        command,
    )
}

fn command_as(
    run_id: Uuid,
    expected_version: u64,
    caller_ref: ReferenceV1,
    command: IdrCommandV1,
) -> IdrCommandEnvelopeV1 {
    IdrCommandEnvelopeV1::new(
        TrustDomainV1::Shadow,
        reference("environment:idr:shadow:vertical"),
        run_id,
        expected_version,
        reference("actor:user-vertical"),
        caller_ref,
        reference("tenant:vertical"),
        reference("scope:interaction"),
        reference("purpose:vertical-test"),
        reference("policy:idr:v1"),
        reference("correlation:vertical"),
        reference(&format!("causation:version-{expected_version}")),
        command,
        vec![],
    )
    .unwrap()
}

fn attach_proofs(
    command: IdrCommandEnvelopeV1,
    proof_kinds: &[ProductionProofKindV1],
    signing_key: &SigningKey,
    issued_at: u64,
) -> IdrCommandEnvelopeV1 {
    attach_proofs_with_authorization_decision(
        command,
        proof_kinds,
        signing_key,
        issued_at,
        "approve",
    )
}

fn attach_proofs_with_authorization_decision(
    command: IdrCommandEnvelopeV1,
    proof_kinds: &[ProductionProofKindV1],
    signing_key: &SigningKey,
    issued_at: u64,
    authorization_decision: &str,
) -> IdrCommandEnvelopeV1 {
    attach_proofs_with_optional_policy_signer(
        command,
        proof_kinds,
        signing_key,
        "key:vertical:v1",
        None,
        issued_at,
        authorization_decision,
    )
}

fn attach_proofs_with_policy_signer(
    command: IdrCommandEnvelopeV1,
    proof_kinds: &[ProductionProofKindV1],
    signing_key: &SigningKey,
    policy_signing_key: &SigningKey,
    issued_at: u64,
) -> IdrCommandEnvelopeV1 {
    attach_proofs_with_optional_policy_signer(
        command,
        proof_kinds,
        signing_key,
        "key:vertical:v1",
        Some(policy_signing_key),
        issued_at,
        "approve",
    )
}

fn attach_proofs_with_optional_policy_signer(
    command: IdrCommandEnvelopeV1,
    proof_kinds: &[ProductionProofKindV1],
    signing_key: &SigningKey,
    primary_key_ref: &str,
    policy_signing_key: Option<&SigningKey>,
    issued_at: u64,
    authorization_decision: &str,
) -> IdrCommandEnvelopeV1 {
    let (subject_ref, subject_digest) = command.proof_subject().unwrap();
    let mut required: BTreeSet<_> = proof_kinds.iter().copied().collect();
    required.insert(ProductionProofKindV1::CallerAuthentication);
    let proofs = required
        .iter()
        .map(|proof_kind| {
            let uses_policy_signer =
                *proof_kind == ProductionProofKindV1::Policy && policy_signing_key.is_some();
            let proof_signing_key = if uses_policy_signer {
                policy_signing_key.unwrap()
            } else {
                signing_key
            };
            let sequence = NONCE_SEQUENCE.fetch_add(1, Ordering::SeqCst);
            let mut claims = ProductionProofClaimsV1::new(
                TrustDomainV1::Shadow,
                reference("environment:idr:shadow:vertical"),
                *proof_kind,
                reference("issuer:vertical-trust"),
                subject_ref.clone(),
                subject_digest.clone(),
                reference("audience:idr:shadow:vertical"),
                reference("tenant:vertical"),
                reference("scope:interaction"),
                reference("purpose:vertical-test"),
                reference("policy:idr:v1"),
                issued_at,
                issued_at,
                issued_at + 600,
                format!("{sequence:064x}"),
            )
            .unwrap()
            .with_principal_binding(command.actor_ref(), command.caller_ref())
            .unwrap();
            if *proof_kind == ProductionProofKindV1::ExactAuthorization {
                claims = claims
                    .with_assertion(serde_json::json!({"decision": authorization_decision}))
                    .unwrap();
            }
            let signature = proof_signing_key.sign(&claims.canonical_signing_bytes().unwrap());
            ProductionProofEnvelopeV1::new(
                claims,
                if uses_policy_signer {
                    reference("key:vertical:policy-v1")
                } else {
                    reference(primary_key_ref)
                },
                hex(&signature.to_bytes()),
            )
            .unwrap()
        })
        .collect();
    command.with_proofs(proofs).unwrap()
}

async fn handle(
    orchestrator: &PostgresIdrOrchestratorV1,
    run_id: Uuid,
    expected_version: u64,
    command_value: IdrCommandV1,
    proof_kinds: &[ProductionProofKindV1],
    signing_key: &SigningKey,
    issued_at: u64,
) -> Option<AuthoritativeRecordRefV1> {
    handle_as(
        orchestrator,
        run_id,
        expected_version,
        reference("caller:vertical-test"),
        command_value,
        proof_kinds,
        signing_key,
        issued_at,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn handle_as(
    orchestrator: &PostgresIdrOrchestratorV1,
    run_id: Uuid,
    expected_version: u64,
    caller_ref: ReferenceV1,
    command_value: IdrCommandV1,
    proof_kinds: &[ProductionProofKindV1],
    signing_key: &SigningKey,
    issued_at: u64,
) -> Option<AuthoritativeRecordRefV1> {
    orchestrator
        .handle(attach_proofs(
            command_as(run_id, expected_version, caller_ref, command_value),
            proof_kinds,
            signing_key,
            issued_at,
        ))
        .await
        .unwrap()
        .record_ref
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn full_postgres_trust_chain_vertical_slice_and_effective_human_model_query() {
    use ProductionProofKindV1::*;

    let database_url = std::env::var("IDR_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql:///postgres".to_string());
    let admin = match PgPool::connect(&database_url).await {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!("PostgreSQL test skipped because the database is unavailable: {error}");
            return;
        }
    };
    let schema = format!("idr_shadow_vertical_{}", Uuid::new_v4().simple());
    let role_digest = &digest(schema.as_bytes())[..32];
    let runtime_role = format!("idr_runtime_{role_digest}");
    let auditor_role = format!("idr_auditor_{role_digest}");
    admin
        .execute(format!("CREATE SCHEMA {schema}").as_str())
        .await
        .unwrap();

    let signing_key = SigningKey::from_bytes(&[53_u8; 32]);
    let policy_signing_key = SigningKey::from_bytes(&[54_u8; 32]);
    let recovery_signing_key = SigningKey::from_bytes(&[55_u8; 32]);
    let issued_at = now();
    let kinds = all_proof_kinds();
    let trust_root = TrustRootSnapshotV1::new(
        TrustDomainV1::Shadow,
        reference("environment:idr:shadow:vertical"),
        1,
        issued_at,
        vec![
            ProductionIssuerKeyV1::new(
                TrustDomainV1::Shadow,
                reference("environment:idr:shadow:vertical"),
                reference("issuer:vertical-trust"),
                reference("key:vertical:v1"),
                &hex(signing_key.verifying_key().as_bytes()),
                reference("audience:idr:shadow:vertical"),
                reference("tenant:vertical"),
                kinds.clone(),
                issued_at - 10,
                issued_at + 1_200,
                None,
            )
            .unwrap(),
            ProductionIssuerKeyV1::new(
                TrustDomainV1::Shadow,
                reference("environment:idr:shadow:vertical"),
                reference("issuer:vertical-trust"),
                reference("key:vertical:recovery-v2"),
                &hex(recovery_signing_key.verifying_key().as_bytes()),
                reference("audience:idr:shadow:vertical"),
                reference("tenant:vertical"),
                BTreeSet::from([
                    ProductionProofKindV1::CallerAuthentication,
                    ProductionProofKindV1::ExecutionPermit,
                ]),
                issued_at - 10,
                issued_at + 1_200,
                None,
            )
            .unwrap(),
            ProductionIssuerKeyV1::new(
                TrustDomainV1::Shadow,
                reference("environment:idr:shadow:vertical"),
                reference("issuer:vertical-trust"),
                reference("key:vertical:policy-v1"),
                &hex(policy_signing_key.verifying_key().as_bytes()),
                reference("audience:idr:shadow:vertical"),
                reference("tenant:vertical"),
                BTreeSet::from([ProductionProofKindV1::Policy]),
                issued_at - 10,
                issued_at + 1_200,
                None,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let expected_issuers: BTreeMap<_, _> = kinds
        .iter()
        .map(|kind| (*kind, reference("issuer:vertical-trust")))
        .collect();
    let trust_root_provider = Arc::new(RotatingTrustRootProviderV1::new(trust_root));
    let security = PostgresSecurityContextV1::new(
        TrustDomainV1::Shadow,
        reference("environment:idr:shadow:vertical"),
        trust_root_provider.clone(),
        Arc::new(InMemoryAuditCheckpointAnchorV1::default()),
        reference("audience:idr:shadow:vertical"),
        expected_issuers,
    )
    .unwrap();
    let store = PostgresIdrOrchestratorV1::connect_shadow(&database_url, &schema, security)
        .await
        .unwrap();
    let orchestrator = store.clone();
    let run_id = Uuid::new_v4();
    let turn_id = Uuid::new_v4();

    let input_candidate = candidate(
        CandidateKindV1::CanonicalInput,
        run_id,
        turn_id,
        serde_json::json!({"content_digest": "31".repeat(32), "source": "user"}),
    );
    let impersonated_command = command(
        run_id,
        0,
        IdrCommandV1::StartRun {
            input: input_candidate.clone(),
        },
    );
    let (subject_ref, subject_digest) = impersonated_command.proof_subject().unwrap();
    let impersonated_claims = ProductionProofClaimsV1::new(
        TrustDomainV1::Shadow,
        reference("environment:idr:shadow:vertical"),
        InputAdmission,
        reference("issuer:vertical-trust"),
        subject_ref,
        subject_digest,
        reference("audience:idr:shadow:vertical"),
        reference("tenant:vertical"),
        reference("scope:interaction"),
        reference("purpose:vertical-test"),
        reference("policy:idr:v1"),
        issued_at,
        issued_at,
        issued_at + 600,
        "29".repeat(32),
    )
    .unwrap()
    .with_principal_binding(
        &reference("actor:impersonator"),
        impersonated_command.caller_ref(),
    )
    .unwrap();
    let impersonated_signature =
        signing_key.sign(&impersonated_claims.canonical_signing_bytes().unwrap());
    let impersonated_command = impersonated_command
        .with_proofs(vec![ProductionProofEnvelopeV1::new(
            impersonated_claims,
            reference("key:vertical:v1"),
            hex(&impersonated_signature.to_bytes()),
        )
        .unwrap()])
        .unwrap();
    assert!(orchestrator.handle(impersonated_command).await.is_err());
    assert!(orchestrator
        .inspect_for_test(run_id)
        .await
        .unwrap()
        .is_none());

    handle(
        &orchestrator,
        run_id,
        0,
        IdrCommandV1::StartRun {
            input: input_candidate,
        },
        &[InputAdmission],
        &signing_key,
        issued_at,
    )
    .await;

    // A proof for one exact command cannot be redirected to another Run or
    // revision/correlation envelope.
    let second_run_id = Uuid::new_v4();
    let second_turn_id = Uuid::new_v4();
    handle(
        &orchestrator,
        second_run_id,
        0,
        IdrCommandV1::StartRun {
            input: candidate(
                CandidateKindV1::CanonicalInput,
                second_run_id,
                second_turn_id,
                serde_json::json!({"content_digest": "41".repeat(32), "source": "user"}),
            ),
        },
        &[InputAdmission],
        &signing_key,
        issued_at,
    )
    .await;
    let signed_cancel = attach_proofs(
        command(
            run_id,
            1,
            IdrCommandV1::CancelRun {
                reason_ref: reference("reason:proof-binding-test"),
            },
        ),
        &[Authority],
        &signing_key,
        issued_at,
    );
    for mutation in [
        (
            "run_id",
            serde_json::Value::String(second_run_id.to_string()),
        ),
        ("expected_aggregate_version", serde_json::Value::from(2)),
        (
            "correlation_ref",
            serde_json::Value::String("correlation:redirected".to_string()),
        ),
        (
            "causation_ref",
            serde_json::Value::String("causation:redirected".to_string()),
        ),
    ] {
        let mut redirected = serde_json::to_value(&signed_cancel).unwrap();
        redirected[mutation.0] = mutation.1;
        let redirected: IdrCommandEnvelopeV1 = serde_json::from_value(redirected).unwrap();
        assert!(matches!(
            orchestrator.handle(redirected).await.unwrap_err(),
            idr_runtime::IdrRuntimeErrorV1::Protocol(_)
        ));
    }

    handle(
        &orchestrator,
        run_id,
        1,
        IdrCommandV1::RecordContext {
            context: candidate(
                CandidateKindV1::ContextSnapshot,
                run_id,
                turn_id,
                serde_json::json!({
                    "input_refs": ["input:vertical"],
                    "snapshot_digest": "32".repeat(32)
                }),
            ),
        },
        &[ContextSnapshot],
        &signing_key,
        issued_at,
    )
    .await;
    handle(
        &orchestrator,
        run_id,
        2,
        IdrCommandV1::RecordIntent {
            intent: candidate(
                CandidateKindV1::Intent,
                run_id,
                turn_id,
                serde_json::json!({
                    "fast_path_outcome": "unknown",
                    "resolution_method": "full_path"
                }),
            ),
        },
        &[IntentFastPathAdmission],
        &signing_key,
        issued_at,
    )
    .await;
    handle(
        &orchestrator,
        run_id,
        3,
        IdrCommandV1::RecordDecision {
            decision: candidate(
                CandidateKindV1::Decision,
                run_id,
                turn_id,
                serde_json::json!({
                    "necessity_outcome": "required",
                    "selected_option_ref": "option:approve",
                    "selected_action_operation_ref": "operation:vertical",
                    "selected_action_parameter_digest": "34".repeat(32)
                }),
            ),
        },
        &[DecisionNecessityAdmission],
        &signing_key,
        issued_at,
    )
    .await;
    handle(
        &orchestrator,
        run_id,
        4,
        IdrCommandV1::RecordTurn {
            turn: candidate(
                CandidateKindV1::TurnCoordination,
                run_id,
                turn_id,
                serde_json::json!({
                    "mode": "respond_then_act",
                    "nodes": ["response", "action"],
                    "edges": [["response", "action"]]
                }),
            ),
        },
        &[TurnCoordinationAdmission],
        &signing_key,
        issued_at,
    )
    .await;

    let rendered_bytes = b"Approved; execution will begin.".to_vec();
    let response_ref = handle(
        &orchestrator,
        run_id,
        5,
        IdrCommandV1::RecordResponse {
            response: candidate(
                CandidateKindV1::Response,
                run_id,
                turn_id,
                serde_json::json!({
                    "rendered_content_digest": digest(&rendered_bytes),
                    "channel_ref": "channel:test",
                    "audience_ref": "audience:user"
                }),
            ),
        },
        &[],
        &signing_key,
        issued_at,
    )
    .await
    .unwrap();
    let tampered_send = attach_proofs(
        command(
            run_id,
            6,
            IdrCommandV1::ConsumeResponseSend {
                response_ref: response_ref.clone(),
                rendered_bytes: b"tampered rendered bytes".to_vec(),
                channel_ref: reference("channel:test"),
                audience_ref: reference("audience:user"),
                send_nonce: "39".repeat(32),
            },
        ),
        &[ResponsePolicy, ResponseAdmission],
        &signing_key,
        issued_at,
    );
    assert!(orchestrator.handle(tampered_send).await.is_err());
    assert_eq!(
        orchestrator
            .inspect_for_test(run_id)
            .await
            .unwrap()
            .unwrap()
            .aggregate_version,
        6
    );
    handle(
        &orchestrator,
        run_id,
        6,
        IdrCommandV1::ConsumeResponseSend {
            response_ref: response_ref.clone(),
            rendered_bytes: rendered_bytes.clone(),
            channel_ref: reference("channel:test"),
            audience_ref: reference("audience:user"),
            send_nonce: "33".repeat(32),
        },
        &[ResponsePolicy, ResponseAdmission],
        &signing_key,
        issued_at,
    )
    .await;
    let replayed_send = attach_proofs(
        command(
            run_id,
            7,
            IdrCommandV1::ConsumeResponseSend {
                response_ref: response_ref.clone(),
                rendered_bytes,
                channel_ref: reference("channel:test"),
                audience_ref: reference("audience:user"),
                send_nonce: "33".repeat(32),
            },
        ),
        &[ResponsePolicy, ResponseAdmission],
        &signing_key,
        issued_at,
    );
    assert!(orchestrator.handle(replayed_send).await.is_err());

    let parameter_digest = "34".repeat(32);
    let mismatched_action = attach_proofs(
        command(
            run_id,
            7,
            IdrCommandV1::RecordAction {
                action: candidate(
                    CandidateKindV1::Action,
                    run_id,
                    turn_id,
                    serde_json::json!({
                        "operation_ref": "operation:vertical",
                        "parameter_digest": parameter_digest,
                        "selected_option_ref": "option:substituted"
                    }),
                ),
            },
        ),
        &[],
        &signing_key,
        issued_at,
    );
    assert!(matches!(
        orchestrator.handle(mismatched_action).await.unwrap_err(),
        idr_runtime::IdrRuntimeErrorV1::ActionDerivationMismatch
    ));
    let action_ref = handle(
        &orchestrator,
        run_id,
        7,
        IdrCommandV1::RecordAction {
            action: candidate(
                CandidateKindV1::Action,
                run_id,
                turn_id,
                serde_json::json!({
                    "operation_ref": "operation:vertical",
                    "parameter_digest": parameter_digest,
                    "selected_option_ref": "option:approve"
                }),
            ),
        },
        &[],
        &signing_key,
        issued_at,
    )
    .await
    .unwrap();
    let request_digest = canonical_digest_v1(
        "idr-execution-request-v1",
        &(
            &action_ref,
            &reference("operation:vertical"),
            &parameter_digest,
        ),
    )
    .unwrap();
    let denied_admission = attach_proofs_with_authorization_decision(
        command(
            run_id,
            8,
            IdrCommandV1::AdmitAction {
                action_ref: action_ref.clone(),
                action_digest: action_ref.record_digest().to_string(),
                provider_ref: reference("provider:vertical"),
                owner_ref: reference("caller:vertical-test"),
                idempotency_key: "vertical-execution".to_string(),
            },
        ),
        &[
            Capability,
            Authority,
            Policy,
            ExactAuthorization,
            ActionAdmission,
        ],
        &signing_key,
        issued_at,
        "deny",
    );
    assert!(orchestrator.handle(denied_admission).await.is_err());
    assert_eq!(
        orchestrator
            .inspect_for_test(run_id)
            .await
            .unwrap()
            .unwrap()
            .aggregate_version,
        8
    );
    let admission_ref = orchestrator
        .handle(attach_proofs_with_policy_signer(
            command(
                run_id,
                8,
                IdrCommandV1::AdmitAction {
                    action_ref: action_ref.clone(),
                    action_digest: action_ref.record_digest().to_string(),
                    provider_ref: reference("provider:vertical"),
                    owner_ref: reference("caller:vertical-test"),
                    idempotency_key: "vertical-execution".to_string(),
                },
            ),
            &[
                Capability,
                Authority,
                Policy,
                ExactAuthorization,
                ActionAdmission,
            ],
            &signing_key,
            &policy_signing_key,
            issued_at,
        ))
        .await
        .unwrap()
        .record_ref
        .unwrap();

    let revoked_policy_root = TrustRootSnapshotV1::new(
        TrustDomainV1::Shadow,
        reference("environment:idr:shadow:vertical"),
        2,
        issued_at,
        vec![
            ProductionIssuerKeyV1::new(
                TrustDomainV1::Shadow,
                reference("environment:idr:shadow:vertical"),
                reference("issuer:vertical-trust"),
                reference("key:vertical:v1"),
                &hex(signing_key.verifying_key().as_bytes()),
                reference("audience:idr:shadow:vertical"),
                reference("tenant:vertical"),
                kinds.clone(),
                issued_at - 10,
                issued_at + 1_200,
                None,
            )
            .unwrap(),
            ProductionIssuerKeyV1::new(
                TrustDomainV1::Shadow,
                reference("environment:idr:shadow:vertical"),
                reference("issuer:vertical-trust"),
                reference("key:vertical:policy-v1"),
                &hex(policy_signing_key.verifying_key().as_bytes()),
                reference("audience:idr:shadow:vertical"),
                reference("tenant:vertical"),
                BTreeSet::from([ProductionProofKindV1::Policy]),
                issued_at - 10,
                issued_at + 1_200,
                Some(issued_at),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    trust_root_provider.rotate(revoked_policy_root).unwrap();
    // Attack regression: revoked_policy_after_admission_blocks_reserve.
    let revoked_policy_reservation = attach_proofs(
        command(
            run_id,
            9,
            IdrCommandV1::ReserveExecution {
                action_admission_ref: admission_ref.clone(),
                action_ref: action_ref.clone(),
                action_digest: action_ref.record_digest().to_string(),
                provider_ref: reference("provider:vertical"),
                owner_ref: reference("caller:vertical-test"),
                idempotency_key: "vertical-execution".to_string(),
                attempt: 1,
                lease_until: issued_at + 300,
                dispatch_nonce: "37".repeat(32),
            },
        ),
        &[ExecutionPermit],
        &signing_key,
        issued_at,
    );
    let governed_revocation = orchestrator
        .handle(revoked_policy_reservation)
        .await
        .unwrap();
    assert_eq!(
        governed_revocation.state,
        idr_runtime::IdrRunStateV1::WaitingAuthorization,
        "a pre-dispatch Admission proof revocation must persist a governed cancellation"
    );
    assert_eq!(
        orchestrator
            .inspect_for_test(run_id)
            .await
            .unwrap()
            .unwrap()
            .aggregate_version,
        10
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*)
               FROM idr_contract_invalidations
              WHERE record_id = $1 AND revision = $2"
        )
        .bind(admission_ref.record_id())
        .bind(i64::try_from(admission_ref.revision()).unwrap())
        .fetch_one(store.pool_for_test())
        .await
        .unwrap(),
        1,
        "the failed Action Admission Contract must be persistently invalidated"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT event_type
               FROM idr_run_events
              WHERE run_id = $1
              ORDER BY global_sequence DESC
              LIMIT 1"
        )
        .bind(run_id)
        .fetch_one(store.pool_for_test())
        .await
        .unwrap(),
        "action_admission_proof_revoked_cancelled"
    );

    let admission_ref = handle(
        &orchestrator,
        run_id,
        10,
        IdrCommandV1::AdmitAction {
            action_ref: action_ref.clone(),
            action_digest: action_ref.record_digest().to_string(),
            provider_ref: reference("provider:vertical"),
            owner_ref: reference("caller:vertical-test"),
            idempotency_key: "vertical-execution".to_string(),
        },
        &[
            Capability,
            Authority,
            Policy,
            ExactAuthorization,
            ActionAdmission,
        ],
        &signing_key,
        issued_at,
    )
    .await
    .unwrap();
    let admission_binding_mismatch = attach_proofs(
        command(
            run_id,
            11,
            IdrCommandV1::ReserveExecution {
                action_admission_ref: admission_ref.clone(),
                action_ref: action_ref.clone(),
                action_digest: action_ref.record_digest().to_string(),
                provider_ref: reference("provider:substituted"),
                owner_ref: reference("caller:vertical-test"),
                idempotency_key: "vertical-execution".to_string(),
                attempt: 1,
                lease_until: issued_at + 300,
                dispatch_nonce: "36".repeat(32),
            },
        ),
        &[ExecutionPermit],
        &signing_key,
        issued_at,
    );
    assert!(orchestrator
        .handle(admission_binding_mismatch)
        .await
        .is_err());
    let bypass_reservation = command(
        run_id,
        11,
        IdrCommandV1::ReserveExecution {
            action_admission_ref: action_ref.clone(),
            action_ref: action_ref.clone(),
            action_digest: action_ref.record_digest().to_string(),
            provider_ref: reference("provider:vertical"),
            owner_ref: reference("caller:vertical-test"),
            idempotency_key: "vertical-execution".to_string(),
            attempt: 1,
            lease_until: issued_at + 300,
            dispatch_nonce: "30".repeat(32),
        },
    );
    assert!(orchestrator.handle(bypass_reservation).await.is_err());
    let reservation_command = attach_proofs(
        command(
            run_id,
            11,
            IdrCommandV1::ReserveExecution {
                action_admission_ref: admission_ref,
                action_ref: action_ref.clone(),
                action_digest: action_ref.record_digest().to_string(),
                provider_ref: reference("provider:vertical"),
                owner_ref: reference("caller:vertical-test"),
                idempotency_key: "vertical-execution".to_string(),
                attempt: 1,
                lease_until: issued_at + 300,
                dispatch_nonce: "35".repeat(32),
            },
        ),
        &[ExecutionPermit],
        &signing_key,
        issued_at,
    );
    let lost_response_replay = reservation_command.clone();
    let reservation_receipt = orchestrator.handle(reservation_command).await.unwrap();
    assert!(!reservation_receipt.idempotent_replay);
    assert!(
        orchestrator
            .handle(lost_response_replay)
            .await
            .unwrap()
            .idempotent_replay
    );
    let execution = orchestrator
        .inspect_for_test(run_id)
        .await
        .unwrap()
        .unwrap()
        .execution
        .unwrap();
    let unauthorized_delivery = attach_proofs(
        command(
            run_id,
            12,
            IdrCommandV1::DeliverPermit {
                reservation_id: execution.reservation_id,
                permit_id: execution.permit_id,
            },
        ),
        &[ExecutionPermit],
        &signing_key,
        issued_at,
    );
    assert!(matches!(
        orchestrator
            .handle(unauthorized_delivery)
            .await
            .unwrap_err(),
        idr_runtime::IdrRuntimeErrorV1::PrincipalMismatch
    ));
    handle_as(
        &orchestrator,
        run_id,
        12,
        reference("provider:vertical"),
        IdrCommandV1::DeliverPermit {
            reservation_id: execution.reservation_id,
            permit_id: execution.permit_id,
        },
        &[ExecutionPermit],
        &signing_key,
        issued_at,
    )
    .await;
    handle_as(
        &orchestrator,
        run_id,
        13,
        reference("provider:vertical"),
        IdrCommandV1::StartDispatch {
            reservation_id: execution.reservation_id,
            permit_id: execution.permit_id,
        },
        &[ExecutionPermit],
        &signing_key,
        issued_at,
    )
    .await;

    handle(
        &orchestrator,
        run_id,
        14,
        IdrCommandV1::InvalidateRecord {
            record_ref: action_ref.clone(),
            reason_ref: reference("reason:new-policy-fact"),
        },
        &[Policy],
        &signing_key,
        issued_at,
    )
    .await;
    assert_eq!(
        orchestrator
            .inspect_for_test(run_id)
            .await
            .unwrap()
            .unwrap()
            .execution
            .unwrap()
            .state,
        idr_runtime::ExecutionAggregateStateV1::ReconciliationRequired
    );

    let unsupported_terminal_reconciliation = attach_proofs(
        command_as(
            run_id,
            15,
            reference("provider:vertical"),
            IdrCommandV1::ReconcileExecution {
                reservation_id: execution.reservation_id,
                resolution: idr_runtime::ReconciliationResolutionV1::Failed,
            },
        ),
        &[ProviderReceipt],
        &signing_key,
        issued_at,
    );
    assert!(matches!(
        orchestrator
            .handle(unsupported_terminal_reconciliation)
            .await
            .unwrap_err(),
        idr_runtime::IdrRuntimeErrorV1::BindingMismatch
    ));

    let mismatched_receipt = attach_proofs(
        command_as(
            run_id,
            15,
            reference("provider:vertical"),
            IdrCommandV1::CommitExecutionReceipt {
                receipt: candidate(
                    CandidateKindV1::ExecutionReceipt,
                    run_id,
                    turn_id,
                    serde_json::json!({
                        "state": "succeeded",
                        "permit_id": execution.permit_id,
                        "dispatch_nonce": execution.dispatch_nonce,
                        "provider_ref": "provider:vertical",
                        "attempt": 1,
                        "request_digest": parameter_digest,
                        "result_digest": "36".repeat(32),
                        "action_was_invalidated": true
                    }),
                ),
                reservation_id: execution.reservation_id,
                permit_id: execution.permit_id,
                provider_ref: reference("provider:vertical"),
                dispatch_nonce: execution.dispatch_nonce.clone(),
                attempt: 1,
            },
        ),
        &[ProviderReceipt],
        &signing_key,
        issued_at,
    );
    assert!(matches!(
        orchestrator.handle(mismatched_receipt).await.unwrap_err(),
        idr_runtime::IdrRuntimeErrorV1::BindingMismatch
    ));
    let receipt_ref = handle_as(
        &orchestrator,
        run_id,
        15,
        reference("provider:vertical"),
        IdrCommandV1::CommitExecutionReceipt {
            receipt: candidate(
                CandidateKindV1::ExecutionReceipt,
                run_id,
                turn_id,
                serde_json::json!({
                    "state": "succeeded",
                    "permit_id": execution.permit_id,
                    "dispatch_nonce": execution.dispatch_nonce,
                    "provider_ref": "provider:vertical",
                    "attempt": 1,
                    "request_digest": request_digest,
                    "result_digest": "36".repeat(32),
                    "action_was_invalidated": true
                }),
            ),
            reservation_id: execution.reservation_id,
            permit_id: execution.permit_id,
            provider_ref: reference("provider:vertical"),
            dispatch_nonce: execution.dispatch_nonce,
            attempt: 1,
        },
        &[ProviderReceipt],
        &signing_key,
        issued_at,
    )
    .await
    .unwrap();
    assert_eq!(
        orchestrator
            .inspect_for_test(run_id)
            .await
            .unwrap()
            .unwrap()
            .state,
        idr_runtime::IdrRunStateV1::Succeeded
    );
    let outcome_ref = handle(
        &orchestrator,
        run_id,
        16,
        IdrCommandV1::RecordOutcome {
            outcome: candidate(
                CandidateKindV1::Outcome,
                run_id,
                turn_id,
                serde_json::json!({
                    "receipt_record_digest": receipt_ref.record_digest(),
                    "observation": "goal_satisfied",
                    "observed_value_digest": "37".repeat(32)
                }),
            ),
        },
        &[OutcomeObservation],
        &signing_key,
        issued_at,
    )
    .await
    .unwrap();

    let mismatched_outcome_candidate = attach_proofs(
        command(
            run_id,
            17,
            IdrCommandV1::ProposeHumanModelCandidate {
                candidate: candidate(
                    CandidateKindV1::HumanModelCandidate,
                    run_id,
                    turn_id,
                    serde_json::json!({
                        "predicate": "prefers_explicit_confirmation",
                        "value_digest": "38".repeat(32),
                        "scope_ref": "scope:interaction",
                        "evidence_refs": ["evidence:one", "evidence:two"],
                        "allowed_purposes": ["purpose:vertical-test"],
                        "maximum_impact_basis_points": 100,
                        "outcome_record_id": outcome_ref.record_id(),
                        "outcome_revision": outcome_ref.revision(),
                        "outcome_record_digest": "00".repeat(32)
                    }),
                ),
            },
        ),
        &[OutcomeObservation],
        &signing_key,
        issued_at,
    );
    assert!(matches!(
        orchestrator
            .handle(mismatched_outcome_candidate)
            .await
            .unwrap_err(),
        idr_runtime::IdrRuntimeErrorV1::BindingMismatch
    ));
    let human_candidate_ref = handle(
        &orchestrator,
        run_id,
        17,
        IdrCommandV1::ProposeHumanModelCandidate {
            candidate: candidate(
                CandidateKindV1::HumanModelCandidate,
                run_id,
                turn_id,
                serde_json::json!({
                    "predicate": "prefers_explicit_confirmation",
                    "value_digest": "38".repeat(32),
                    "scope_ref": "scope:interaction",
                    "evidence_refs": ["evidence:one", "evidence:two"],
                    "allowed_purposes": ["purpose:vertical-test"],
                    "maximum_impact_basis_points": 100,
                    "outcome_record_id": outcome_ref.record_id(),
                    "outcome_revision": outcome_ref.revision(),
                    "outcome_record_digest": outcome_ref.record_digest()
                }),
            ),
        },
        &[OutcomeObservation],
        &signing_key,
        issued_at,
    )
    .await
    .unwrap();
    let promotion_bypass = command(
        run_id,
        18,
        IdrCommandV1::PromoteHumanModelAssertion {
            assertion: candidate(
                CandidateKindV1::HumanModelAssertion,
                run_id,
                turn_id,
                serde_json::json!({
                    "source_candidate_digest": human_candidate_ref.record_digest(),
                    "predicate": "prefers_explicit_confirmation",
                    "value_digest": "38".repeat(32),
                    "scope_ref": "scope:interaction",
                    "evidence_refs": ["evidence:one", "evidence:two"],
                    "allowed_purposes": ["purpose:vertical-test"],
                    "lifecycle_state": "provisional",
                    "maximum_impact_basis_points": 100
                }),
            ),
            promotion_decision_ref: human_candidate_ref.clone(),
        },
    );
    assert!(orchestrator.handle(promotion_bypass).await.is_err());
    let promotion_ref = handle(
        &orchestrator,
        run_id,
        18,
        IdrCommandV1::RecordHumanModelPromotion {
            decision: candidate(
                CandidateKindV1::HumanModelPromotionDecision,
                run_id,
                turn_id,
                serde_json::json!({
                    "candidate_digest": human_candidate_ref.record_digest(),
                    "outcome": "promote_provisional",
                    "independent_evidence_count": 2,
                    "outcome_record_id": outcome_ref.record_id(),
                    "outcome_revision": outcome_ref.revision(),
                    "outcome_record_digest": outcome_ref.record_digest()
                }),
            ),
            source_candidate_ref: human_candidate_ref.clone(),
        },
        &[HumanModelPromotion],
        &signing_key,
        issued_at,
    )
    .await
    .unwrap();
    let substituted_assertion = attach_proofs(
        command(
            run_id,
            19,
            IdrCommandV1::PromoteHumanModelAssertion {
                assertion: candidate(
                    CandidateKindV1::HumanModelAssertion,
                    run_id,
                    turn_id,
                    serde_json::json!({
                        "source_candidate_digest": human_candidate_ref.record_digest(),
                        "predicate": "substituted_predicate"
                    }),
                ),
                promotion_decision_ref: promotion_ref.clone(),
            },
        ),
        &[HumanModelPromotion],
        &signing_key,
        issued_at,
    );
    assert!(matches!(
        orchestrator
            .handle(substituted_assertion)
            .await
            .unwrap_err(),
        idr_runtime::IdrRuntimeErrorV1::BindingMismatch
    ));
    let escalated_assertion = attach_proofs(
        command(
            run_id,
            19,
            IdrCommandV1::PromoteHumanModelAssertion {
                assertion: candidate(
                    CandidateKindV1::HumanModelAssertion,
                    run_id,
                    turn_id,
                    serde_json::json!({
                        "source_candidate_digest": human_candidate_ref.record_digest(),
                        "lifecycle_state": "user_confirmed",
                        "maximum_impact_basis_points": 10000
                    }),
                ),
                promotion_decision_ref: promotion_ref.clone(),
            },
        ),
        &[HumanModelPromotion],
        &signing_key,
        issued_at,
    );
    assert!(matches!(
        orchestrator.handle(escalated_assertion).await.unwrap_err(),
        idr_runtime::IdrRuntimeErrorV1::BindingMismatch
    ));
    let assertion_ref = handle(
        &orchestrator,
        run_id,
        19,
        IdrCommandV1::PromoteHumanModelAssertion {
            assertion: candidate(
                CandidateKindV1::HumanModelAssertion,
                run_id,
                turn_id,
                serde_json::json!({
                    "source_candidate_digest": human_candidate_ref.record_digest()
                }),
            ),
            promotion_decision_ref: promotion_ref.clone(),
        },
        &[HumanModelPromotion],
        &signing_key,
        issued_at,
    )
    .await
    .unwrap();

    let effective = store
        .query_effective_human_model_assertions_for_test(
            &HumanModelQueryV1::new(
                reference("tenant:vertical"),
                reference("subject:user-vertical"),
                reference("scope:interaction"),
                reference("purpose:vertical-test"),
                reference("policy:idr:v1"),
                100,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(effective.len(), 1);
    assert_eq!(effective[0].predicate, "prefers_explicit_confirmation");
    assert_eq!(effective[0].lifecycle_state, "provisional");
    assert_eq!(effective[0].maximum_impact_basis_points, 100);
    let materialized_assertion: serde_json::Value = sqlx::query_scalar(
        "SELECT assertion #> '{payload}'
         FROM idr_human_model_assertions
         WHERE assertion_record_id = $1 AND assertion_revision = $2",
    )
    .bind(assertion_ref.record_id())
    .bind(i64::try_from(assertion_ref.revision()).unwrap())
    .fetch_one(store.pool_for_test())
    .await
    .unwrap();
    assert_eq!(
        materialized_assertion["promotion_decision_record_id"],
        promotion_ref.record_id().to_string()
    );
    assert_eq!(
        materialized_assertion["promotion_decision_revision"],
        promotion_ref.revision()
    );
    assert_eq!(
        materialized_assertion["promotion_decision_record_digest"],
        promotion_ref.record_digest()
    );

    handle(
        &orchestrator,
        run_id,
        20,
        IdrCommandV1::ProposeHumanModelCandidate {
            candidate: candidate(
                CandidateKindV1::HumanModelCandidate,
                run_id,
                turn_id,
                serde_json::json!({
                    "predicate": "prefers_explicit_confirmation",
                    "value_digest": "40".repeat(32),
                    "scope_ref": "scope:interaction",
                    "evidence_refs": ["evidence:successor"],
                    "allowed_purposes": ["purpose:vertical-test"],
                    "maximum_impact_basis_points": 50,
                    "outcome_record_id": outcome_ref.record_id(),
                    "outcome_revision": outcome_ref.revision(),
                    "outcome_record_digest": outcome_ref.record_digest()
                }),
            ),
        },
        &[OutcomeObservation],
        &signing_key,
        issued_at,
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT operation_ref FROM idr_execution_reservations WHERE reservation_id = $1"
        )
        .bind(execution.reservation_id)
        .fetch_one(store.pool_for_test())
        .await
        .unwrap(),
        "operation:vertical",
        "operation-level idempotency must use the selected real operation"
    );
    // Attack regressions: execution_identity_update_is_rejected and
    // operation_idempotency_slot_cannot_be_released_by_sql_update.
    assert!(
        sqlx::query(
            "UPDATE idr_execution_reservations
             SET operation_ref = 'operation:tampered',
                 idempotency_key = 'tampered-key'
             WHERE reservation_id = $1",
        )
        .bind(execution.reservation_id)
        .execute(store.pool_for_test())
        .await
        .is_err(),
        "Execution identity trigger must prevent releasing the exactly-once slot"
    );
    assert!(
        sqlx::query(
            "UPDATE idr_execution_attempts
             SET dispatch_nonce = $2
             WHERE reservation_id = $1 AND attempt = 1",
        )
        .bind(execution.reservation_id)
        .bind("9".repeat(64))
        .execute(store.pool_for_test())
        .await
        .is_err(),
        "Execution attempt Permit identity must be immutable"
    );
    assert!(
        sqlx::query("DELETE FROM idr_execution_reservations WHERE reservation_id = $1")
            .bind(execution.reservation_id)
            .execute(store.pool_for_test())
            .await
            .is_err(),
        "Execution reservation identity must not be deletable"
    );
    assert!(
        sqlx::query(
            "UPDATE idr_operation_idempotency_fences
             SET idempotency_key = 'released-by-update'
             WHERE fence_id = $1",
        )
        .bind(execution.reservation_id)
        .execute(store.pool_for_test())
        .await
        .is_err(),
        "the append-only idempotency fence must never release its original tuple"
    );

    // Attack regression: a stolen Runtime login has view DML but cannot open
    // an HMAC-attested Orchestrator transition or mutate any authority row.
    let mut restricted_lifecycle_update = store.pool_for_test().begin().await.unwrap();
    sqlx::query(&format!("SET LOCAL ROLE {runtime_role}"))
        .execute(&mut *restricted_lifecycle_update)
        .await
        .unwrap();
    let raw_runtime_update_error = sqlx::query(
        "UPDATE idr_execution_reservations
             SET state = state,
                 aggregate_version = aggregate_version + 1,
                 updated_at = clock_timestamp()
             WHERE reservation_id = $1",
    )
    .bind(execution.reservation_id)
    .execute(&mut *restricted_lifecycle_update)
    .await
    .unwrap_err();
    assert!(
        raw_runtime_update_error
            .to_string()
            .contains("Orchestrator attestation"),
        "a raw Runtime login must not mutate authority state: {raw_runtime_update_error}"
    );
    drop(restricted_lifecycle_update);

    let mut restricted_transaction = store.pool_for_test().begin().await.unwrap();
    sqlx::query(&format!("SET LOCAL ROLE {runtime_role}"))
        .execute(&mut *restricted_transaction)
        .await
        .unwrap();
    let alter_error = sqlx::query(
        "ALTER TABLE idr_execution_reservations_authority_v7
         DISABLE TRIGGER idr_execution_reservation_update_guard_v5",
    )
    .execute(&mut *restricted_transaction)
    .await
    .unwrap_err();
    assert_eq!(
        alter_error
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref(),
        Some("42501"),
        "the Runtime role must be denied table-owner/DDL authority by PostgreSQL"
    );
    drop(restricted_transaction);
    let mut restricted_update = store.pool_for_test().begin().await.unwrap();
    sqlx::query(&format!("SET LOCAL ROLE {runtime_role}"))
        .execute(&mut *restricted_update)
        .await
        .unwrap();
    let identity_update_error = sqlx::query(
        "UPDATE idr_execution_reservations
         SET idempotency_key = 'runtime-role-release-attempt'
         WHERE reservation_id = $1",
    )
    .bind(execution.reservation_id)
    .execute(&mut *restricted_update)
    .await
    .unwrap_err();
    assert!(
        identity_update_error
            .to_string()
            .contains("Orchestrator attestation"),
        "the Runtime role must not mutate identity columns without an attestation: \
         {identity_update_error}"
    );
    drop(restricted_update);

    // Attack regressions: stolen_runtime_login_has_zero_base_table_dml and
    // raw_runtime_login_cannot_forge_authority_rows.
    let zero_base_table_dml: bool = sqlx::query_scalar(
        "SELECT NOT EXISTS (
            SELECT 1
              FROM pg_class relation
              JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
             WHERE namespace.nspname = current_schema()
               AND relation.relname LIKE '%\\_authority\\_v7' ESCAPE '\\'
               AND (
                   has_table_privilege($1, relation.oid, 'INSERT')
                   OR has_table_privilege($1, relation.oid, 'UPDATE')
                   OR has_table_privilege($1, relation.oid, 'DELETE')
                   OR has_table_privilege($1, relation.oid, 'TRUNCATE')
                   OR has_table_privilege($1, relation.oid, 'REFERENCES')
                   OR has_table_privilege($1, relation.oid, 'TRIGGER')
               )
        )",
    )
    .bind(&runtime_role)
    .fetch_one(store.pool_for_test())
    .await
    .unwrap();
    assert!(
        zero_base_table_dml,
        "the Runtime role must have zero DML privilege on every authority base table"
    );

    // Attack regression: temporary_schema_shadow_cannot_replace_attestation_authority.
    // The fixture deliberately grants PUBLIC TEMP in the shared Shadow database,
    // so this proves the pinned function path independently of the Production
    // startup rule that rejects TEMP-capable logins.
    let mut temporary_shadow_transaction = store.pool_for_test().begin().await.unwrap();
    let (attack_txid, attack_pid): (i64, i32) =
        sqlx::query_as("SELECT txid_current()::bigint, pg_backend_pid()")
            .fetch_one(&mut *temporary_shadow_transaction)
            .await
            .unwrap();
    let attack_attestation_id = Uuid::new_v4();
    let attack_command_id = Uuid::new_v4();
    let attack_nonce = Uuid::new_v4();
    let attack_command_digest = "a".repeat(64);
    let attack_transition_digest = "b".repeat(64);
    let attacker_key = vec![0x41_u8; 32];
    let attacker_mac: String = sqlx::query_scalar(
        "SELECT encode(
            idr_hmac_sha256_v7(
                $1,
                idr_attestation_material_v7(
                    'shadow', 'environment:idr:shadow:vertical',
                    'command_transition', $2, $3, $4,
                    'tenant:vertical', 0, 1, $5, $6, $7, $8, $9, 1
                )
            ),
            'hex'
         )",
    )
    .bind(&attacker_key)
    .bind(attack_attestation_id)
    .bind(attack_command_id)
    .bind(run_id)
    .bind(&attack_command_digest)
    .bind(&attack_transition_digest)
    .bind(attack_txid)
    .bind(attack_pid)
    .bind(attack_nonce)
    .fetch_one(&mut *temporary_shadow_transaction)
    .await
    .unwrap();
    sqlx::query(&format!("SET LOCAL ROLE {runtime_role}"))
        .execute(&mut *temporary_shadow_transaction)
        .await
        .unwrap();
    let runtime_has_temp: bool = sqlx::query_scalar(
        "SELECT has_database_privilege(
            current_user, current_database(), 'TEMPORARY'
         )",
    )
    .fetch_one(&mut *temporary_shadow_transaction)
    .await
    .unwrap();
    assert!(
        runtime_has_temp,
        "the Shadow exploit fixture must exercise an identity that can create TEMP tables"
    );
    sqlx::query(
        "CREATE TEMP TABLE idr_orchestrator_attestation_secrets_v7 (
            key_version bigint PRIMARY KEY,
            key_digest text NOT NULL,
            secret bytea NOT NULL,
            activated_at timestamptz NOT NULL,
            retired_at timestamptz
         ) ON COMMIT DROP",
    )
    .execute(&mut *temporary_shadow_transaction)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO pg_temp.idr_orchestrator_attestation_secrets_v7
            (key_version, key_digest, secret, activated_at, retired_at)
         VALUES (1, encode(sha256($1), 'hex'), $1, clock_timestamp(), NULL)",
    )
    .bind(&attacker_key)
    .execute(&mut *temporary_shadow_transaction)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TEMP TABLE idr_transition_attestations_v7 (
            attestation_id uuid PRIMARY KEY,
            trust_domain text NOT NULL,
            environment_ref text NOT NULL,
            purpose text NOT NULL,
            command_id uuid NOT NULL,
            run_id uuid NOT NULL,
            tenant_ref text NOT NULL,
            pre_aggregate_version bigint NOT NULL,
            post_aggregate_version bigint NOT NULL,
            command_digest text NOT NULL,
            transition_digest text NOT NULL,
            database_txid bigint NOT NULL,
            backend_pid integer NOT NULL,
            nonce uuid NOT NULL,
            key_version bigint NOT NULL,
            mac text NOT NULL,
            attested_at timestamptz NOT NULL
         ) ON COMMIT DROP",
    )
    .execute(&mut *temporary_shadow_transaction)
    .await
    .unwrap();
    let legacy_opener_available: bool = sqlx::query_scalar(
        "SELECT has_function_privilege(
            current_user,
            'idr_open_attested_transition_v7(text,text,text,uuid,uuid,uuid,text,bigint,bigint,text,text,bigint,integer,uuid,bigint,text)',
            'EXECUTE'
         )",
    )
    .fetch_one(&mut *temporary_shadow_transaction)
    .await
    .unwrap();
    assert!(
        !legacy_opener_available,
        "the Runtime role must not retain the legacy text-comparison opener"
    );
    let temporary_shadow_error = sqlx::query_scalar::<_, Uuid>(
        "SELECT idr_open_attested_transition_v8(
            'shadow', 'environment:idr:shadow:vertical',
            'command_transition', $1, $2, $3, 'tenant:vertical',
            0, 1, $4, $5, $6, $7, $8, 1, $9
         )",
    )
    .bind(attack_attestation_id)
    .bind(attack_command_id)
    .bind(run_id)
    .bind(&attack_command_digest)
    .bind(&attack_transition_digest)
    .bind(attack_txid)
    .bind(attack_pid)
    .bind(attack_nonce)
    .bind(attacker_mac)
    .fetch_one(&mut *temporary_shadow_transaction)
    .await
    .unwrap_err();
    assert!(
        temporary_shadow_error
            .to_string()
            .contains("attestation authentication failed"),
        "attacker-owned TEMP key material must never authenticate: {temporary_shadow_error}"
    );
    drop(temporary_shadow_transaction);

    let pinned_definer_paths: bool = sqlx::query_scalar(
        "SELECT bool_and(
            array_position(
                procedure.proconfig,
                'search_path=' || current_schema() || ', pg_catalog, pg_temp'
            ) IS NOT NULL
         )
           FROM pg_proc procedure
           JOIN pg_namespace namespace ON namespace.oid = procedure.pronamespace
          WHERE namespace.nspname = current_schema()
            AND procedure.prosecdef",
    )
    .fetch_one(store.pool_for_test())
    .await
    .unwrap();
    assert!(
        pinned_definer_paths,
        "all SECURITY DEFINER functions must pin pg_temp after the trusted schema"
    );

    let mut forged_receipt_transaction = store.pool_for_test().begin().await.unwrap();
    sqlx::query(&format!("SET LOCAL ROLE {runtime_role}"))
        .execute(&mut *forged_receipt_transaction)
        .await
        .unwrap();
    let forged_receipt_error = sqlx::query(
        "INSERT INTO idr_command_receipts (
            command_id, trust_domain, environment_ref, run_id, aggregate_version,
            event_sequence, receipt, tenant_ref, actor_ref, caller_ref, command_digest
         )
         SELECT $1, trust_domain, environment_ref, run_id, aggregate_version,
                event_sequence, receipt, tenant_ref, actor_ref, caller_ref,
                repeat('f', 64)
           FROM idr_command_receipts
          LIMIT 1",
    )
    .bind(Uuid::new_v4())
    .execute(&mut *forged_receipt_transaction)
    .await
    .unwrap_err();
    assert!(
        forged_receipt_error
            .to_string()
            .contains("Orchestrator attestation"),
        "a raw Runtime login must not forge a command Receipt: {forged_receipt_error}"
    );
    drop(forged_receipt_transaction);

    let mut forged_fence_transaction = store.pool_for_test().begin().await.unwrap();
    sqlx::query(&format!("SET LOCAL ROLE {runtime_role}"))
        .execute(&mut *forged_fence_transaction)
        .await
        .unwrap();
    let forged_fence_error = sqlx::query(
        "INSERT INTO idr_operation_idempotency_fences (
            fence_id, trust_domain, environment_ref, tenant_ref,
            operation_ref, idempotency_key, first_run_id,
            first_action_record_id, first_action_revision,
            first_action_digest, fence_digest
         )
         SELECT $2, trust_domain, environment_ref, tenant_ref,
                operation_ref, 'runtime-forged-fence', first_run_id,
                first_action_record_id, first_action_revision,
                first_action_digest, fence_digest
           FROM idr_operation_idempotency_fences
          WHERE fence_id = $1",
    )
    .bind(execution.reservation_id)
    .bind(Uuid::new_v4())
    .execute(&mut *forged_fence_transaction)
    .await
    .unwrap_err();
    assert!(
        forged_fence_error
            .to_string()
            .contains("Orchestrator attestation"),
        "a raw Runtime login must not forge an idempotency Fence: {forged_fence_error}"
    );
    drop(forged_fence_transaction);

    let mut forged_contract_transaction = store.pool_for_test().begin().await.unwrap();
    sqlx::query(&format!("SET LOCAL ROLE {runtime_role}"))
        .execute(&mut *forged_contract_transaction)
        .await
        .unwrap();
    let forged_contract_error = sqlx::query(
        "INSERT INTO idr_contract_records (
            record_id, revision, candidate_kind, trust_domain, environment_ref,
            run_id, tenant_ref, subject_ref, record_digest, record,
            issued_by_command_id, issued_at, valid_from, valid_until
         )
         SELECT $1, 1, candidate_kind, trust_domain, environment_ref,
                run_id, tenant_ref, subject_ref, repeat('e', 64), record,
                $2, issued_at, valid_from, valid_until
           FROM idr_contract_records
          LIMIT 1",
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&mut *forged_contract_transaction)
    .await
    .unwrap_err();
    assert!(
        forged_contract_error
            .to_string()
            .contains("Orchestrator attestation"),
        "a raw Runtime login must not forge a Contract: {forged_contract_error}"
    );
    drop(forged_contract_transaction);

    let mut forged_attestation_transaction = store.pool_for_test().begin().await.unwrap();
    sqlx::query(&format!("SET LOCAL ROLE {runtime_role}"))
        .execute(&mut *forged_attestation_transaction)
        .await
        .unwrap();
    let forged_attestation_error =
        sqlx::query("INSERT INTO idr_transition_attestations_v7 DEFAULT VALUES")
            .execute(&mut *forged_attestation_transaction)
            .await
            .unwrap_err();
    assert_eq!(
        forged_attestation_error
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref(),
        Some("42501"),
        "the Runtime role must not insert its own authority attestation"
    );
    drop(forged_attestation_transaction);

    let mut restricted_delete = store.pool_for_test().begin().await.unwrap();
    sqlx::query(&format!("SET LOCAL ROLE {runtime_role}"))
        .execute(&mut *restricted_delete)
        .await
        .unwrap();
    let fence_delete_error =
        sqlx::query("DELETE FROM idr_operation_idempotency_fences WHERE fence_id = $1")
            .bind(execution.reservation_id)
            .execute(&mut *restricted_delete)
            .await
            .unwrap_err();
    assert_eq!(
        fence_delete_error
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref(),
        Some("42501"),
        "the Runtime role must be denied deletion of the permanent fence"
    );
    drop(restricted_delete);

    let mut auditor_transaction = store.pool_for_test().begin().await.unwrap();
    sqlx::query(&format!("SET LOCAL ROLE {auditor_role}"))
        .execute(&mut *auditor_transaction)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM idr_operation_idempotency_fences")
            .fetch_one(&mut *auditor_transaction)
            .await
            .unwrap(),
        1,
        "the independent auditor role must retain read access"
    );
    let auditor_write_error = sqlx::query(
        "UPDATE idr_execution_reservations
         SET state = state
         WHERE reservation_id = $1",
    )
    .bind(execution.reservation_id)
    .execute(&mut *auditor_transaction)
    .await
    .unwrap_err();
    assert_eq!(
        auditor_write_error
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref(),
        Some("42501"),
        "the auditor role must be read-only"
    );
    drop(auditor_transaction);

    // Attack regressions: execution_index_tamper_detected_on_startup and
    // pre_restart_duplicate_reservation_is_blocked_by_append_only_fence.
    sqlx::query(
        "ALTER TABLE idr_execution_reservations_authority_v7
         DISABLE TRIGGER idr_execution_reservation_update_guard_v5",
    )
    .execute(store.pool_for_test())
    .await
    .unwrap();
    sqlx::query(
        "ALTER TABLE idr_execution_reservations_authority_v7
         DISABLE TRIGGER idr_execution_reservation_fence_guard_v6",
    )
    .execute(store.pool_for_test())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE idr_execution_reservations_authority_v7
         SET operation_ref = 'operation:privileged-tamper',
             idempotency_key = 'privileged-tamper'
         WHERE reservation_id = $1",
    )
    .bind(execution.reservation_id)
    .execute(store.pool_for_test())
    .await
    .unwrap();
    assert!(
        sqlx::query(
            "INSERT INTO idr_operation_idempotency_fences (
                fence_id, trust_domain, environment_ref, tenant_ref,
                operation_ref, idempotency_key, first_run_id,
                first_action_record_id, first_action_revision,
                first_action_digest, fence_digest
             )
             SELECT $2, trust_domain, environment_ref, tenant_ref,
                    'operation:vertical', 'vertical-execution', first_run_id,
                    first_action_record_id, first_action_revision,
                    first_action_digest, fence_digest
             FROM idr_operation_idempotency_fences WHERE fence_id = $1",
        )
        .bind(execution.reservation_id)
        .bind(Uuid::new_v4())
        .execute(store.pool_for_test())
        .await
        .is_err(),
        "tampering with the Reservation cannot release the append-only operation/idempotency fence"
    );
    assert!(matches!(
        store.verify_external_anchor_for_test().await.unwrap_err(),
        idr_runtime::IdrRuntimeErrorV1::IntegrityViolation
    ));
    sqlx::query(
        "UPDATE idr_execution_reservations_authority_v7
         SET operation_ref = 'operation:vertical',
             idempotency_key = 'vertical-execution'
         WHERE reservation_id = $1",
    )
    .bind(execution.reservation_id)
    .execute(store.pool_for_test())
    .await
    .unwrap();
    sqlx::query(
        "ALTER TABLE idr_execution_reservations_authority_v7
         ENABLE TRIGGER idr_execution_reservation_update_guard_v5",
    )
    .execute(store.pool_for_test())
    .await
    .unwrap();
    sqlx::query(
        "ALTER TABLE idr_execution_reservations_authority_v7
         ENABLE TRIGGER idr_execution_reservation_fence_guard_v6",
    )
    .execute(store.pool_for_test())
    .await
    .unwrap();
    store.verify_external_anchor_for_test().await.unwrap();

    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM idr_contract_invalidations
             WHERE (record_id, revision) IN (($1,$2),($3,$4),($5,$6))"
        )
        .bind(human_candidate_ref.record_id())
        .bind(i64::try_from(human_candidate_ref.revision()).unwrap())
        .bind(promotion_ref.record_id())
        .bind(i64::try_from(promotion_ref.revision()).unwrap())
        .bind(assertion_ref.record_id())
        .bind(i64::try_from(assertion_ref.revision()).unwrap())
        .fetch_one(store.pool_for_test())
        .await
        .unwrap(),
        3,
        "a successor must recursively invalidate its dependent promotion and assertion"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM idr_run_events WHERE run_id = $1")
            .bind(run_id)
            .fetch_one(store.pool_for_test())
            .await
            .unwrap(),
        21
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM idr_outbox_events WHERE run_id = $1")
            .bind(run_id)
            .fetch_one(store.pool_for_test())
            .await
            .unwrap(),
        21
    );
    let first_claim = store
        .claim_outbox_events_for_test(&reference("worker:first"), 1, 60)
        .await
        .unwrap();
    assert_eq!(first_claim.len(), 1);
    assert_eq!(first_claim[0].delivery_attempts, 1);
    sqlx::query(
        "UPDATE idr_outbox_events_authority_v7
         SET claimed_at = clock_timestamp() - interval '2 minutes',
             claim_until = clock_timestamp() - interval '1 minute'
         WHERE outbox_id = $1",
    )
    .bind(first_claim[0].outbox_id)
    .execute(store.pool_for_test())
    .await
    .unwrap();
    let recovered_claim = store
        .claim_outbox_events_for_test(&reference("worker:recovery"), 1, 60)
        .await
        .unwrap();
    assert_eq!(recovered_claim[0].outbox_id, first_claim[0].outbox_id);
    assert_eq!(recovered_claim[0].delivery_attempts, 2);
    assert!(store
        .acknowledge_outbox_event_for_test(&reference("worker:first"), first_claim[0].outbox_id)
        .await
        .is_err());
    store
        .acknowledge_outbox_event_for_test(
            &reference("worker:recovery"),
            recovered_claim[0].outbox_id,
        )
        .await
        .unwrap();

    assert!(
        sqlx::query(
            "UPDATE idr_contract_records
             SET record = jsonb_set(
                 record, '{payload,value_digest}', to_jsonb(repeat('f', 64))
             )
             WHERE record_id = $1 AND revision = $2",
        )
        .bind(assertion_ref.record_id())
        .bind(i64::try_from(assertion_ref.revision()).unwrap())
        .execute(store.pool_for_test())
        .await
        .is_err(),
        "authoritative Human Model Contract must be append-only"
    );
    assert!(
        sqlx::query(
            "UPDATE idr_human_model_assertions
             SET assertion = jsonb_set(
                 assertion, '{payload,value_digest}', to_jsonb(repeat('f', 64))
             )
             WHERE assertion_record_id = $1 AND assertion_revision = $2",
        )
        .bind(assertion_ref.record_id())
        .bind(i64::try_from(assertion_ref.revision()).unwrap())
        .execute(store.pool_for_test())
        .await
        .is_err(),
        "specialized Human Model projection must be append-only"
    );
    store.verify_external_anchor_for_test().await.unwrap();

    drop_shadow_schema_and_roles(&admin, &schema).await;
}
