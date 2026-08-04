use ed25519_dalek::{Signer, SigningKey};
use idr_protocol::production::{
    CandidateKindV1, CandidateSubmissionV1, ProductionIssuerKeyV1, ProductionProofClaimsV1,
    ProductionProofEnvelopeV1, ProductionProofKindV1, TrustDomainV1, TrustRootSnapshotV1,
};
use idr_protocol::ReferenceV1;
use idr_runtime::{IdrCommandEnvelopeV1, IdrCommandV1, IdrRuntimeErrorV1};
use idr_store::{
    InMemoryAuditCheckpointAnchorV1, PostgresIdrOrchestratorV1, PostgresSecurityContextV1,
    RotatingTrustRootProviderV1,
};
use sqlx::{Executor, PgPool};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use uuid::Uuid;

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

async fn drop_shadow_schema_and_roles(admin: &PgPool, schema: &str) {
    let role_digest: String =
        sqlx::query_scalar("SELECT substr(encode(sha256(convert_to($1, 'UTF8')), 'hex'), 1, 32)")
            .bind(schema)
            .fetch_one(admin)
            .await
            .unwrap();
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

fn candidate(kind: CandidateKindV1, run_id: Uuid, turn_id: Uuid) -> CandidateSubmissionV1 {
    CandidateSubmissionV1::new(
        kind,
        run_id,
        turn_id,
        reference("subject:user-001"),
        reference("tenant:test"),
        reference("scope:interaction"),
        reference("purpose:supplier-review"),
        reference("policy:idr:v1"),
        1,
        now() + 3_600,
        serde_json::json!({
            "kind": format!("{kind:?}"),
            "evidence_refs": ["evidence:test"]
        }),
    )
    .unwrap()
}

fn signed_command_proof(
    kind: ProductionProofKindV1,
    issuer: &str,
    key_id: &str,
    command: &IdrCommandEnvelopeV1,
    signing_key: &SigningKey,
    issued_at: u64,
    nonce: char,
) -> ProductionProofEnvelopeV1 {
    let (subject_ref, subject_digest) = command.proof_subject().unwrap();
    let claims = ProductionProofClaimsV1::new(
        TrustDomainV1::Shadow,
        reference("environment:idr:shadow:test"),
        kind,
        reference(issuer),
        subject_ref,
        subject_digest,
        reference("audience:idr:shadow:test"),
        reference("tenant:test"),
        reference("scope:interaction"),
        reference("purpose:supplier-review"),
        reference("policy:idr:v1"),
        issued_at,
        issued_at,
        issued_at + 300,
        nonce.to_string().repeat(64),
    )
    .unwrap()
    .with_principal_binding(command.actor_ref(), command.caller_ref())
    .unwrap();
    let signature = signing_key.sign(&claims.canonical_signing_bytes().unwrap());
    ProductionProofEnvelopeV1::new(claims, reference(key_id), hex(&signature.to_bytes())).unwrap()
}

#[allow(clippy::too_many_arguments)]
fn authenticated_command_proofs(
    kind: ProductionProofKindV1,
    issuer: &str,
    key_id: &str,
    command: &IdrCommandEnvelopeV1,
    signing_key: &SigningKey,
    issued_at: u64,
    semantic_nonce: char,
    authentication_nonce: char,
) -> Vec<ProductionProofEnvelopeV1> {
    vec![
        signed_command_proof(
            kind,
            issuer,
            key_id,
            command,
            signing_key,
            issued_at,
            semantic_nonce,
        ),
        signed_command_proof(
            ProductionProofKindV1::CallerAuthentication,
            "issuer:gateway",
            "key:gateway:v1",
            command,
            signing_key,
            issued_at,
            authentication_nonce,
        ),
    ]
}

async fn test_store(
    database_url: &str,
    schema: &str,
    trust_root: TrustRootSnapshotV1,
    anchor: Arc<InMemoryAuditCheckpointAnchorV1>,
) -> (PostgresIdrOrchestratorV1, RotatingTrustRootProviderV1) {
    let provider = RotatingTrustRootProviderV1::new(trust_root);
    let expected_issuers = BTreeMap::from([
        (
            ProductionProofKindV1::InputAdmission,
            reference("issuer:gateway"),
        ),
        (
            ProductionProofKindV1::ContextSnapshot,
            reference("issuer:context"),
        ),
        (
            ProductionProofKindV1::CallerAuthentication,
            reference("issuer:gateway"),
        ),
    ]);
    let security = PostgresSecurityContextV1::new(
        TrustDomainV1::Shadow,
        reference("environment:idr:shadow:test"),
        Arc::new(provider.clone()),
        anchor,
        reference("audience:idr:shadow:test"),
        expected_issuers,
    )
    .unwrap();
    (
        PostgresIdrOrchestratorV1::connect_shadow(database_url, schema, security)
            .await
            .unwrap(),
        provider,
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn postgres_command_is_atomic_idempotent_and_externally_anchored() {
    let database_url = std::env::var("IDR_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql:///postgres".to_string());
    let admin = match PgPool::connect(&database_url).await {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!("PostgreSQL test skipped because the database is unavailable: {error}");
            return;
        }
    };
    let schema = format!("idr_shadow_test_{}", Uuid::new_v4().simple());
    admin
        .execute(format!("CREATE SCHEMA {schema}").as_str())
        .await
        .unwrap();

    let signing_key = SigningKey::from_bytes(&[41_u8; 32]);
    let issued_at = now();
    let trust_root = TrustRootSnapshotV1::new(
        TrustDomainV1::Shadow,
        reference("environment:idr:shadow:test"),
        1,
        issued_at,
        vec![
            ProductionIssuerKeyV1::new(
                TrustDomainV1::Shadow,
                reference("environment:idr:shadow:test"),
                reference("issuer:gateway"),
                reference("key:gateway:v1"),
                &hex(signing_key.verifying_key().as_bytes()),
                reference("audience:idr:shadow:test"),
                reference("tenant:test"),
                BTreeSet::from([
                    ProductionProofKindV1::InputAdmission,
                    ProductionProofKindV1::CallerAuthentication,
                ]),
                issued_at - 10,
                issued_at + 600,
                None,
            )
            .unwrap(),
            ProductionIssuerKeyV1::new(
                TrustDomainV1::Shadow,
                reference("environment:idr:shadow:test"),
                reference("issuer:context"),
                reference("key:context:v1"),
                &hex(signing_key.verifying_key().as_bytes()),
                reference("audience:idr:shadow:test"),
                reference("tenant:test"),
                BTreeSet::from([ProductionProofKindV1::ContextSnapshot]),
                issued_at - 10,
                issued_at + 600,
                None,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let anchor = Arc::new(InMemoryAuditCheckpointAnchorV1::default());
    let (store, provider) = test_store(&database_url, &schema, trust_root, anchor.clone()).await;
    let orchestrator = store.clone();

    let run_id = Uuid::new_v4();
    let turn_id = Uuid::new_v4();
    let input = candidate(CandidateKindV1::CanonicalInput, run_id, turn_id);
    let start = IdrCommandEnvelopeV1::new(
        TrustDomainV1::Shadow,
        reference("environment:idr:shadow:test"),
        run_id,
        0,
        reference("actor:user-001"),
        reference("caller:gateway"),
        reference("tenant:test"),
        reference("scope:interaction"),
        reference("purpose:supplier-review"),
        reference("policy:idr:v1"),
        reference("correlation:test"),
        reference("causation:input"),
        IdrCommandV1::StartRun { input },
        vec![],
    )
    .unwrap();
    let start_proofs = authenticated_command_proofs(
        ProductionProofKindV1::InputAdmission,
        "issuer:gateway",
        "key:gateway:v1",
        &start,
        &signing_key,
        issued_at,
        'a',
        '0',
    );
    let start = start.with_proofs(start_proofs).unwrap();
    let replay = start.clone();
    let receipt = orchestrator.handle(start).await.unwrap();
    assert_eq!(receipt.aggregate_version, 1);
    let replay_receipt = orchestrator.handle(replay.clone()).await.unwrap();
    assert!(replay_receipt.idempotent_replay);
    assert_eq!(replay_receipt.event_sequence, receipt.event_sequence);

    let mut conflicting_value = serde_json::to_value(&replay).unwrap();
    conflicting_value["causation_ref"] =
        serde_json::Value::String("causation:conflicting-command-content".to_string());
    let conflicting_command: IdrCommandEnvelopeV1 =
        serde_json::from_value(conflicting_value).unwrap();
    assert!(matches!(
        orchestrator.handle(conflicting_command).await.unwrap_err(),
        IdrRuntimeErrorV1::IdempotencyConflict
    ));

    let switched_subject = CandidateSubmissionV1::new(
        CandidateKindV1::ContextSnapshot,
        run_id,
        turn_id,
        reference("subject:attacker"),
        reference("tenant:test"),
        reference("scope:interaction"),
        reference("purpose:supplier-review"),
        reference("policy:idr:v1"),
        1,
        now() + 3_600,
        serde_json::json!({"kind": "ContextSnapshot", "evidence_refs": ["evidence:test"]}),
    )
    .unwrap();
    let switched_subject_command = IdrCommandEnvelopeV1::new(
        TrustDomainV1::Shadow,
        reference("environment:idr:shadow:test"),
        run_id,
        1,
        reference("actor:user-001"),
        reference("caller:context-runtime"),
        reference("tenant:test"),
        reference("scope:interaction"),
        reference("purpose:supplier-review"),
        reference("policy:idr:v1"),
        reference("correlation:test"),
        reference("causation:switched-subject"),
        IdrCommandV1::RecordContext {
            context: switched_subject,
        },
        vec![],
    )
    .unwrap();
    let switched_subject_proofs = authenticated_command_proofs(
        ProductionProofKindV1::ContextSnapshot,
        "issuer:context",
        "key:context:v1",
        &switched_subject_command,
        &signing_key,
        issued_at,
        '8',
        '1',
    );
    let switched_subject_command = switched_subject_command
        .with_proofs(switched_subject_proofs)
        .unwrap();
    assert!(matches!(
        orchestrator
            .handle(switched_subject_command)
            .await
            .unwrap_err(),
        IdrRuntimeErrorV1::LineageMismatch
    ));

    let switched_turn = candidate(CandidateKindV1::ContextSnapshot, run_id, Uuid::new_v4());
    let switched_turn_command = IdrCommandEnvelopeV1::new(
        TrustDomainV1::Shadow,
        reference("environment:idr:shadow:test"),
        run_id,
        1,
        reference("actor:user-001"),
        reference("caller:context-runtime"),
        reference("tenant:test"),
        reference("scope:interaction"),
        reference("purpose:supplier-review"),
        reference("policy:idr:v1"),
        reference("correlation:test"),
        reference("causation:switched-turn"),
        IdrCommandV1::RecordContext {
            context: switched_turn,
        },
        vec![],
    )
    .unwrap();
    let switched_turn_proofs = authenticated_command_proofs(
        ProductionProofKindV1::ContextSnapshot,
        "issuer:context",
        "key:context:v1",
        &switched_turn_command,
        &signing_key,
        issued_at,
        '9',
        '2',
    );
    let switched_turn_command = switched_turn_command
        .with_proofs(switched_turn_proofs)
        .unwrap();
    assert!(matches!(
        orchestrator
            .handle(switched_turn_command)
            .await
            .unwrap_err(),
        IdrRuntimeErrorV1::LineageMismatch
    ));

    let context = candidate(CandidateKindV1::ContextSnapshot, run_id, turn_id);
    let context_command = IdrCommandEnvelopeV1::new(
        TrustDomainV1::Shadow,
        reference("environment:idr:shadow:test"),
        run_id,
        1,
        reference("actor:user-001"),
        reference("caller:context-runtime"),
        reference("tenant:test"),
        reference("scope:interaction"),
        reference("purpose:supplier-review"),
        reference("policy:idr:v1"),
        reference("correlation:test"),
        reference("causation:context"),
        IdrCommandV1::RecordContext { context },
        vec![],
    )
    .unwrap();
    let context_proofs = authenticated_command_proofs(
        ProductionProofKindV1::ContextSnapshot,
        "issuer:context",
        "key:context:v1",
        &context_command,
        &signing_key,
        issued_at,
        'b',
        '3',
    );
    let context_command = context_command.with_proofs(context_proofs).unwrap();
    assert_eq!(
        orchestrator
            .handle(context_command)
            .await
            .unwrap()
            .aggregate_version,
        2
    );

    let projection = orchestrator
        .inspect_for_test(run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(projection.aggregate_version, 2);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM idr_outbox_events")
            .fetch_one(store.pool_for_test())
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM idr_proof_consumptions")
            .fetch_one(store.pool_for_test())
            .await
            .unwrap(),
        4,
        "each command consumes both current caller authentication and semantic admission proof"
    );

    assert!(
        sqlx::query(
            "UPDATE idr_audit_checkpoints
             SET execution_state_root = repeat('0', 64)
             WHERE tenant_ref = 'tenant:test'",
        )
        .execute(store.pool_for_test())
        .await
        .is_err(),
        "signed checkpoint roots must be immutable after insertion"
    );

    assert!(
        sqlx::query(
            "UPDATE idr_audit_events SET actor_ref = 'actor:forged'
             WHERE tenant_ref = 'tenant:test'
               AND sequence = (
                   SELECT max(sequence) FROM idr_audit_events WHERE tenant_ref = 'tenant:test'
               )",
        )
        .execute(store.pool_for_test())
        .await
        .is_err(),
        "append-only audit trigger must reject in-place mutation"
    );

    assert!(
        sqlx::query(
            "UPDATE idr_contract_records_authority_v7
             SET record = jsonb_set(record, '{payload,kind}', '\"forged\"'::jsonb)
             WHERE run_id = $1 AND candidate_kind = 'canonical_input'",
        )
        .bind(run_id)
        .execute(store.pool_for_test())
        .await
        .is_err(),
        "append-only Contract trigger must reject in-place mutation"
    );
    let original_record: serde_json::Value = sqlx::query_scalar(
        "SELECT record FROM idr_contract_records
         WHERE run_id = $1 AND candidate_kind = 'canonical_input'",
    )
    .bind(run_id)
    .fetch_one(store.pool_for_test())
    .await
    .unwrap();
    sqlx::query(
        "ALTER TABLE idr_contract_records_authority_v7
         DISABLE TRIGGER idr_contract_records_append_only_v4",
    )
    .execute(store.pool_for_test())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE idr_contract_records_authority_v7
         SET record = jsonb_set(record, '{payload,kind}', '\"privileged-forgery\"'::jsonb)
         WHERE run_id = $1 AND candidate_kind = 'canonical_input'",
    )
    .bind(run_id)
    .execute(store.pool_for_test())
    .await
    .unwrap();
    assert!(matches!(
        store.verify_external_anchor_for_test().await.unwrap_err(),
        IdrRuntimeErrorV1::IntegrityViolation
    ));
    sqlx::query(
        "UPDATE idr_contract_records_authority_v7 SET record = $1
         WHERE run_id = $2 AND candidate_kind = 'canonical_input'",
    )
    .bind(&original_record)
    .bind(run_id)
    .execute(store.pool_for_test())
    .await
    .unwrap();
    sqlx::query(
        "ALTER TABLE idr_contract_records_authority_v7
         ENABLE TRIGGER idr_contract_records_append_only_v4",
    )
    .execute(store.pool_for_test())
    .await
    .unwrap();
    store.verify_external_anchor_for_test().await.unwrap();

    // Attack regression: command_receipt_payload_requires_its_exact_hmac.
    let (receipt_command_id, original_receipt): (Uuid, serde_json::Value) = sqlx::query_as(
        "SELECT command_id, receipt
               FROM idr_command_receipts
              WHERE run_id = $1
              ORDER BY created_at
              LIMIT 1",
    )
    .bind(run_id)
    .fetch_one(store.pool_for_test())
    .await
    .unwrap();
    sqlx::query(
        "ALTER TABLE idr_command_receipts_authority_v7
         DISABLE TRIGGER idr_command_receipts_append_only_v4",
    )
    .execute(store.pool_for_test())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE idr_command_receipts_authority_v7
            SET receipt = jsonb_set(receipt, '{state}', '\"failed\"'::jsonb)
          WHERE command_id = $1",
    )
    .bind(receipt_command_id)
    .execute(store.pool_for_test())
    .await
    .unwrap();
    assert!(matches!(
        store.verify_external_anchor_for_test().await.unwrap_err(),
        IdrRuntimeErrorV1::IntegrityViolation
    ));
    sqlx::query(
        "UPDATE idr_command_receipts_authority_v7
            SET receipt = $1
          WHERE command_id = $2",
    )
    .bind(&original_receipt)
    .bind(receipt_command_id)
    .execute(store.pool_for_test())
    .await
    .unwrap();
    sqlx::query(
        "ALTER TABLE idr_command_receipts_authority_v7
         ENABLE TRIGGER idr_command_receipts_append_only_v4",
    )
    .execute(store.pool_for_test())
    .await
    .unwrap();
    store.verify_external_anchor_for_test().await.unwrap();

    assert!(
        sqlx::query(
            "UPDATE idr_runs_authority_v7
             SET trust_domain = 'production' WHERE run_id = $1",
        )
        .bind(run_id)
        .execute(store.pool_for_test())
        .await
        .is_err(),
        "database trigger must reject a Shadow-to-Production domain rewrite"
    );

    sqlx::query(
        "UPDATE idr_runs_authority_v7
         SET projection = jsonb_set(projection, '{aggregate_version}', '999'::jsonb)
         WHERE run_id = $1",
    )
    .bind(run_id)
    .execute(store.pool_for_test())
    .await
    .unwrap();
    assert!(matches!(
        store.verify_external_anchor_for_test().await.unwrap_err(),
        IdrRuntimeErrorV1::IntegrityViolation
    ));

    drop_shadow_schema_and_roles(&admin, &schema).await;
    drop(provider);
}

#[tokio::test]
async fn revoked_key_is_rechecked_at_transaction_consumption_time_on_idempotent_replay() {
    let database_url = std::env::var("IDR_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql:///postgres".to_string());
    let admin = match PgPool::connect(&database_url).await {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!("PostgreSQL test skipped because the database is unavailable: {error}");
            return;
        }
    };
    let schema = format!("idr_shadow_revoke_{}", Uuid::new_v4().simple());
    admin
        .execute(format!("CREATE SCHEMA {schema}").as_str())
        .await
        .unwrap();
    let signing_key = SigningKey::from_bytes(&[42_u8; 32]);
    let issued_at = now();
    let policy = |revoked_at| {
        ProductionIssuerKeyV1::new(
            TrustDomainV1::Shadow,
            reference("environment:idr:shadow:test"),
            reference("issuer:gateway"),
            reference("key:gateway:v1"),
            &hex(signing_key.verifying_key().as_bytes()),
            reference("audience:idr:shadow:test"),
            reference("tenant:test"),
            BTreeSet::from([
                ProductionProofKindV1::InputAdmission,
                ProductionProofKindV1::CallerAuthentication,
            ]),
            issued_at - 10,
            issued_at + 600,
            revoked_at,
        )
        .unwrap()
    };
    let trust_root = TrustRootSnapshotV1::new(
        TrustDomainV1::Shadow,
        reference("environment:idr:shadow:test"),
        1,
        issued_at,
        vec![policy(None)],
    )
    .unwrap();
    let anchor = Arc::new(InMemoryAuditCheckpointAnchorV1::default());
    let (store, provider) = test_store(&database_url, &schema, trust_root, anchor).await;
    let run_id = Uuid::new_v4();
    let turn_id = Uuid::new_v4();
    let input = candidate(CandidateKindV1::CanonicalInput, run_id, turn_id);
    let command = IdrCommandEnvelopeV1::new(
        TrustDomainV1::Shadow,
        reference("environment:idr:shadow:test"),
        run_id,
        0,
        reference("actor:user-001"),
        reference("caller:gateway"),
        reference("tenant:test"),
        reference("scope:interaction"),
        reference("purpose:supplier-review"),
        reference("policy:idr:v1"),
        reference("correlation:test"),
        reference("causation:input"),
        IdrCommandV1::StartRun { input },
        vec![],
    )
    .unwrap();
    let proofs = authenticated_command_proofs(
        ProductionProofKindV1::InputAdmission,
        "issuer:gateway",
        "key:gateway:v1",
        &command,
        &signing_key,
        issued_at,
        'c',
        '4',
    );
    let command = command.with_proofs(proofs).unwrap();
    let replay = command.clone();

    store.handle(command).await.unwrap();

    provider
        .rotate(
            TrustRootSnapshotV1::new(
                TrustDomainV1::Shadow,
                reference("environment:idr:shadow:test"),
                2,
                issued_at,
                vec![policy(Some(issued_at))],
            )
            .unwrap(),
        )
        .unwrap();
    let error = store.handle(replay).await.unwrap_err();
    assert!(matches!(error, IdrRuntimeErrorV1::Protocol(_)));

    drop_shadow_schema_and_roles(&admin, &schema).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_expected_version_has_exactly_one_winner() {
    let database_url = std::env::var("IDR_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql:///postgres".to_string());
    let admin = match PgPool::connect(&database_url).await {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!("PostgreSQL test skipped because the database is unavailable: {error}");
            return;
        }
    };
    let schema = format!("idr_shadow_cas_{}", Uuid::new_v4().simple());
    admin
        .execute(format!("CREATE SCHEMA {schema}").as_str())
        .await
        .unwrap();

    let signing_key = SigningKey::from_bytes(&[43_u8; 32]);
    let issued_at = now();
    let trust_root = TrustRootSnapshotV1::new(
        TrustDomainV1::Shadow,
        reference("environment:idr:shadow:test"),
        1,
        issued_at,
        vec![
            ProductionIssuerKeyV1::new(
                TrustDomainV1::Shadow,
                reference("environment:idr:shadow:test"),
                reference("issuer:gateway"),
                reference("key:gateway:v1"),
                &hex(signing_key.verifying_key().as_bytes()),
                reference("audience:idr:shadow:test"),
                reference("tenant:test"),
                BTreeSet::from([
                    ProductionProofKindV1::InputAdmission,
                    ProductionProofKindV1::CallerAuthentication,
                ]),
                issued_at - 10,
                issued_at + 600,
                None,
            )
            .unwrap(),
            ProductionIssuerKeyV1::new(
                TrustDomainV1::Shadow,
                reference("environment:idr:shadow:test"),
                reference("issuer:context"),
                reference("key:context:v1"),
                &hex(signing_key.verifying_key().as_bytes()),
                reference("audience:idr:shadow:test"),
                reference("tenant:test"),
                BTreeSet::from([ProductionProofKindV1::ContextSnapshot]),
                issued_at - 10,
                issued_at + 600,
                None,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let anchor = Arc::new(InMemoryAuditCheckpointAnchorV1::default());
    let (store, _provider) = test_store(&database_url, &schema, trust_root, anchor).await;
    let run_id = Uuid::new_v4();
    let turn_id = Uuid::new_v4();
    let input = candidate(CandidateKindV1::CanonicalInput, run_id, turn_id);
    let input_command = IdrCommandEnvelopeV1::new(
        TrustDomainV1::Shadow,
        reference("environment:idr:shadow:test"),
        run_id,
        0,
        reference("actor:user-001"),
        reference("caller:gateway"),
        reference("tenant:test"),
        reference("scope:interaction"),
        reference("purpose:supplier-review"),
        reference("policy:idr:v1"),
        reference("correlation:cas"),
        reference("causation:input"),
        IdrCommandV1::StartRun { input },
        vec![],
    )
    .unwrap();
    let input_proofs = authenticated_command_proofs(
        ProductionProofKindV1::InputAdmission,
        "issuer:gateway",
        "key:gateway:v1",
        &input_command,
        &signing_key,
        issued_at,
        'd',
        '5',
    );
    store
        .clone()
        .handle(input_command.with_proofs(input_proofs).unwrap())
        .await
        .unwrap();

    let context_a = candidate(CandidateKindV1::ContextSnapshot, run_id, turn_id);
    let context_b = candidate(CandidateKindV1::ContextSnapshot, run_id, turn_id);
    let command = |context: CandidateSubmissionV1, nonce, authentication_nonce, causation: &str| {
        let command = IdrCommandEnvelopeV1::new(
            TrustDomainV1::Shadow,
            reference("environment:idr:shadow:test"),
            run_id,
            1,
            reference("actor:user-001"),
            reference("caller:context-runtime"),
            reference("tenant:test"),
            reference("scope:interaction"),
            reference("purpose:supplier-review"),
            reference("policy:idr:v1"),
            reference("correlation:cas"),
            reference(causation),
            IdrCommandV1::RecordContext { context },
            vec![],
        )
        .unwrap();
        let proofs = authenticated_command_proofs(
            ProductionProofKindV1::ContextSnapshot,
            "issuer:context",
            "key:context:v1",
            &command,
            &signing_key,
            issued_at,
            nonce,
            authentication_nonce,
        );
        command.with_proofs(proofs).unwrap()
    };
    let first = store.clone();
    let second = store.clone();
    let (left, right) = tokio::join!(
        first.handle(command(context_a, 'e', '6', "causation:context-a")),
        second.handle(command(context_b, 'f', 'a', "causation:context-b"))
    );
    assert_eq!(
        usize::from(left.is_ok()) + usize::from(right.is_ok()),
        1,
        "SERIALIZABLE/CAS must produce exactly one winner: left={left:?} right={right:?}"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM idr_contract_records WHERE candidate_kind = 'context_snapshot'"
        )
        .fetch_one(store.pool_for_test())
        .await
        .unwrap(),
        1
    );

    drop_shadow_schema_and_roles(&admin, &schema).await;
}

#[tokio::test]
async fn external_anchor_detects_database_audit_rollback() {
    let database_url = std::env::var("IDR_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql:///postgres".to_string());
    let admin = match PgPool::connect(&database_url).await {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!("PostgreSQL test skipped because the database is unavailable: {error}");
            return;
        }
    };
    let schema = format!("idr_shadow_anchor_{}", Uuid::new_v4().simple());
    admin
        .execute(format!("CREATE SCHEMA {schema}").as_str())
        .await
        .unwrap();
    let signing_key = SigningKey::from_bytes(&[44_u8; 32]);
    let issued_at = now();
    let trust_root = TrustRootSnapshotV1::new(
        TrustDomainV1::Shadow,
        reference("environment:idr:shadow:test"),
        1,
        issued_at,
        vec![
            ProductionIssuerKeyV1::new(
                TrustDomainV1::Shadow,
                reference("environment:idr:shadow:test"),
                reference("issuer:gateway"),
                reference("key:gateway:v1"),
                &hex(signing_key.verifying_key().as_bytes()),
                reference("audience:idr:shadow:test"),
                reference("tenant:test"),
                BTreeSet::from([
                    ProductionProofKindV1::InputAdmission,
                    ProductionProofKindV1::CallerAuthentication,
                ]),
                issued_at - 10,
                issued_at + 600,
                None,
            )
            .unwrap(),
            ProductionIssuerKeyV1::new(
                TrustDomainV1::Shadow,
                reference("environment:idr:shadow:test"),
                reference("issuer:context"),
                reference("key:context:v1"),
                &hex(signing_key.verifying_key().as_bytes()),
                reference("audience:idr:shadow:test"),
                reference("tenant:test"),
                BTreeSet::from([ProductionProofKindV1::ContextSnapshot]),
                issued_at - 10,
                issued_at + 600,
                None,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let anchor = Arc::new(InMemoryAuditCheckpointAnchorV1::default());
    let (store, _provider) = test_store(&database_url, &schema, trust_root, anchor).await;
    let run_id = Uuid::new_v4();
    let turn_id = Uuid::new_v4();
    let input = candidate(CandidateKindV1::CanonicalInput, run_id, turn_id);
    let command = IdrCommandEnvelopeV1::new(
        TrustDomainV1::Shadow,
        reference("environment:idr:shadow:test"),
        run_id,
        0,
        reference("actor:user-001"),
        reference("caller:gateway"),
        reference("tenant:test"),
        reference("scope:interaction"),
        reference("purpose:supplier-review"),
        reference("policy:idr:v1"),
        reference("correlation:anchor"),
        reference("causation:input"),
        IdrCommandV1::StartRun { input },
        vec![],
    )
    .unwrap();
    let proofs = authenticated_command_proofs(
        ProductionProofKindV1::InputAdmission,
        "issuer:gateway",
        "key:gateway:v1",
        &command,
        &signing_key,
        issued_at,
        '7',
        'b',
    );
    store
        .clone()
        .handle(command.with_proofs(proofs).unwrap())
        .await
        .unwrap();

    assert!(
        sqlx::query("DELETE FROM idr_audit_events WHERE tenant_ref = 'tenant:test'")
            .execute(store.pool_for_test())
            .await
            .is_err(),
        "append-only audit trigger must reject rollback"
    );
    store.verify_external_anchor_for_test().await.unwrap();

    drop_shadow_schema_and_roles(&admin, &schema).await;
}
