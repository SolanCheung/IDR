//! Concrete PostgreSQL authority runtime for the command-driven IDR control plane.
//!
//! Every command is evaluated under one SERIALIZABLE transaction after
//! loading database time, the current Trust Root and the aggregate row. Proof
//! IDs/nonces, events, snapshots, audit, outbox and command receipt are
//! committed atomically.

#![cfg_attr(feature = "migration", allow(dead_code))]

use crate::{
    evaluate_authoritative_transition_v1, hash_event_v1, verify_authoritative_record_value_v1,
    AuthoritativeRecordV1, IdrCommandEnvelopeV1, IdrCommandReceiptV1, IdrCommandV1,
    IdrRunProjectionV1, IdrRuntimeErrorV1, IdrTrustDomainV1, OrchestratorAuthorityV1,
    ACTION_ADMISSION_PROOF_KINDS_V1,
};
use async_trait::async_trait;
#[cfg(feature = "production")]
use idr_protocol::production::ProductionReleaseGatesV1;
use idr_protocol::production::{
    CandidateKindV1, ExpectedProductionProofV1, ProductionProofEnvelopeV1, ProductionProofKindV1,
    TrustRootSnapshotV1, VerifiedProductionProofV1,
};
use idr_protocol::ReferenceV1;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{ConnectOptions, Executor, PgPool, Postgres, Row, Transaction};
use std::collections::{BTreeMap, BTreeSet};
#[cfg(any(feature = "production", feature = "migration"))]
use std::os::unix::fs::PermissionsExt;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

#[cfg(any(feature = "shadow-mode", feature = "migration"))]
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../idr-store/migrations");
const EXPECTED_SCHEMA_MIGRATION_VERSION_V1: i64 = 8;
const ORCHESTRATOR_ATTESTATION_KEY_VERSION_V1: i64 = 1;
const ZERO_RUN_ID_V1: Uuid = Uuid::from_u128(0);

#[derive(Clone)]
struct OrchestratorAttestationKeyV1 {
    bytes: [u8; 32],
    version: i64,
}

impl OrchestratorAttestationKeyV1 {
    #[cfg(feature = "shadow-mode")]
    fn random_shadow_key() -> Self {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let mut bytes = [0_u8; 32];
        bytes[..16].copy_from_slice(first.as_bytes());
        bytes[16..].copy_from_slice(second.as_bytes());
        Self {
            bytes,
            version: ORCHESTRATOR_ATTESTATION_KEY_VERSION_V1,
        }
    }

    #[cfg(any(feature = "production", feature = "migration"))]
    fn from_protected_environment_file() -> Result<Self, IdrRuntimeErrorV1> {
        let path = std::env::var("IDR_ORCHESTRATOR_ATTESTATION_KEY_FILE").map_err(|_| {
            IdrRuntimeErrorV1::Repository(
                "IDR_ORCHESTRATOR_ATTESTATION_KEY_FILE is required".to_string(),
            )
        })?;
        let metadata = std::fs::metadata(&path).map_err(|error| {
            IdrRuntimeErrorV1::Repository(format!(
                "cannot inspect Orchestrator attestation key file: {error}"
            ))
        })?;
        if !metadata.is_file() || metadata.permissions().mode() & 0o077 != 0 {
            return Err(IdrRuntimeErrorV1::Repository(
                "Orchestrator attestation key must be a regular mode-0600 file".to_string(),
            ));
        }
        let encoded = std::fs::read_to_string(&path).map_err(|error| {
            IdrRuntimeErrorV1::Repository(format!(
                "cannot read Orchestrator attestation key file: {error}"
            ))
        })?;
        let encoded = encoded.trim();
        if encoded.len() != 64 {
            return Err(IdrRuntimeErrorV1::Repository(
                "Orchestrator attestation key must contain exactly 64 lowercase hex characters"
                    .to_string(),
            ));
        }
        let mut bytes = [0_u8; 32];
        for (index, destination) in bytes.iter_mut().enumerate() {
            let pair = &encoded[index * 2..index * 2 + 2];
            if !pair
                .as_bytes()
                .iter()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
            {
                return Err(IdrRuntimeErrorV1::Repository(
                    "Orchestrator attestation key must use canonical lowercase hex".to_string(),
                ));
            }
            *destination = u8::from_str_radix(pair, 16).map_err(|_| {
                IdrRuntimeErrorV1::Repository(
                    "Orchestrator attestation key contains invalid hex".to_string(),
                )
            })?;
        }
        Ok(Self {
            bytes,
            version: ORCHESTRATOR_ATTESTATION_KEY_VERSION_V1,
        })
    }

    #[cfg(any(feature = "shadow-mode", feature = "migration"))]
    fn digest(&self) -> String {
        format!("{:x}", Sha256::digest(self.bytes))
    }

    fn mac(&self, payload: &[u8]) -> String {
        let mut key_block = [0_u8; 64];
        key_block[..self.bytes.len()].copy_from_slice(&self.bytes);
        let mut inner_pad = [0x36_u8; 64];
        let mut outer_pad = [0x5c_u8; 64];
        for index in 0..64 {
            inner_pad[index] ^= key_block[index];
            outer_pad[index] ^= key_block[index];
        }
        let mut inner = Sha256::new();
        inner.update(inner_pad);
        inner.update(payload);
        let inner_digest = inner.finalize();
        let mut outer = Sha256::new();
        outer.update(outer_pad);
        outer.update(inner_digest);
        format!("{:x}", outer.finalize())
    }
}

pub trait CurrentTrustRootProviderV1: Send + Sync {
    fn current_snapshot(&self) -> Result<TrustRootSnapshotV1, IdrRuntimeErrorV1>;
}

#[derive(Clone)]
pub struct RotatingTrustRootProviderV1 {
    current: Arc<RwLock<TrustRootSnapshotV1>>,
}

impl RotatingTrustRootProviderV1 {
    pub fn new(snapshot: TrustRootSnapshotV1) -> Self {
        Self {
            current: Arc::new(RwLock::new(snapshot)),
        }
    }

    pub fn rotate(&self, snapshot: TrustRootSnapshotV1) -> Result<(), IdrRuntimeErrorV1> {
        let mut current = self
            .current
            .write()
            .map_err(|_| IdrRuntimeErrorV1::Repository("Trust Root lock poisoned".to_string()))?;
        if snapshot.root_version() <= current.root_version() {
            return Err(IdrRuntimeErrorV1::Repository(
                "Trust Root version must advance monotonically".to_string(),
            ));
        }
        *current = snapshot;
        Ok(())
    }
}

impl CurrentTrustRootProviderV1 for RotatingTrustRootProviderV1 {
    fn current_snapshot(&self) -> Result<TrustRootSnapshotV1, IdrRuntimeErrorV1> {
        self.current
            .read()
            .map(|snapshot| snapshot.clone())
            .map_err(|_| IdrRuntimeErrorV1::Repository("Trust Root lock poisoned".to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditCheckpointV1 {
    pub trust_domain: IdrTrustDomainV1,
    pub environment_ref: String,
    pub tenant_ref: String,
    pub audit_sequence: i64,
    pub chain_root: String,
    pub record_set_root: String,
    pub execution_state_root: String,
    pub checkpoint_id: Uuid,
}

#[async_trait]
pub trait AuditCheckpointAnchorBackendV1: Send + Sync {
    async fn load_latest(
        &self,
        trust_domain: IdrTrustDomainV1,
        environment_ref: &str,
        tenant_ref: &str,
    ) -> Result<Option<AuditCheckpointV1>, IdrRuntimeErrorV1>;

    async fn publish(&self, checkpoint: &AuditCheckpointV1) -> Result<(), IdrRuntimeErrorV1>;

    fn is_production_durable(&self) -> bool;
}

/// Test/shadow anchor with append-only monotonic semantics. It deliberately
/// reports `is_production_durable=false`, so production startup cannot
/// accidentally rely on process memory.
#[derive(Default)]
pub struct InMemoryAuditCheckpointAnchorV1 {
    checkpoints:
        tokio::sync::RwLock<BTreeMap<(IdrTrustDomainV1, String, String), AuditCheckpointV1>>,
}

#[async_trait]
impl AuditCheckpointAnchorBackendV1 for InMemoryAuditCheckpointAnchorV1 {
    async fn load_latest(
        &self,
        trust_domain: IdrTrustDomainV1,
        environment_ref: &str,
        tenant_ref: &str,
    ) -> Result<Option<AuditCheckpointV1>, IdrRuntimeErrorV1> {
        Ok(self
            .checkpoints
            .read()
            .await
            .get(&(
                trust_domain,
                environment_ref.to_string(),
                tenant_ref.to_string(),
            ))
            .cloned())
    }

    async fn publish(&self, checkpoint: &AuditCheckpointV1) -> Result<(), IdrRuntimeErrorV1> {
        let mut checkpoints = self.checkpoints.write().await;
        let key = (
            checkpoint.trust_domain,
            checkpoint.environment_ref.clone(),
            checkpoint.tenant_ref.clone(),
        );
        if let Some(current) = checkpoints.get(&key) {
            if checkpoint.audit_sequence < current.audit_sequence
                || (checkpoint.audit_sequence == current.audit_sequence
                    && (checkpoint.chain_root != current.chain_root
                        || checkpoint.record_set_root != current.record_set_root
                        || checkpoint.execution_state_root != current.execution_state_root))
            {
                return Err(IdrRuntimeErrorV1::Repository(
                    "external audit checkpoint rollback/fork".to_string(),
                ));
            }
        }
        checkpoints.insert(key, checkpoint.clone());
        Ok(())
    }

    fn is_production_durable(&self) -> bool {
        false
    }
}

#[derive(Clone)]
pub struct PostgresSecurityContextV1 {
    trust_domain: IdrTrustDomainV1,
    environment_ref: ReferenceV1,
    trust_root_provider: Arc<dyn CurrentTrustRootProviderV1>,
    anchor_backend: Arc<dyn AuditCheckpointAnchorBackendV1>,
    audience_ref: ReferenceV1,
    expected_issuers: BTreeMap<ProductionProofKindV1, ReferenceV1>,
}

impl PostgresSecurityContextV1 {
    pub fn new(
        trust_domain: IdrTrustDomainV1,
        environment_ref: ReferenceV1,
        trust_root_provider: Arc<dyn CurrentTrustRootProviderV1>,
        anchor_backend: Arc<dyn AuditCheckpointAnchorBackendV1>,
        audience_ref: ReferenceV1,
        expected_issuers: BTreeMap<ProductionProofKindV1, ReferenceV1>,
    ) -> Result<Self, IdrRuntimeErrorV1> {
        if expected_issuers.is_empty()
            || !audience_matches_trust_domain(&audience_ref, trust_domain, &environment_ref)
        {
            return Err(IdrRuntimeErrorV1::Repository(
                "expected issuer registry is empty or audience is not domain-namespaced"
                    .to_string(),
            ));
        }
        Ok(Self {
            trust_domain,
            environment_ref,
            trust_root_provider,
            anchor_backend,
            audience_ref,
            expected_issuers,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanModelQueryV1 {
    tenant_ref: ReferenceV1,
    subject_ref: ReferenceV1,
    scope_ref: ReferenceV1,
    purpose_ref: ReferenceV1,
    policy_revision_ref: ReferenceV1,
    maximum_impact_basis_points: u16,
}

impl HumanModelQueryV1 {
    pub fn new(
        tenant_ref: ReferenceV1,
        subject_ref: ReferenceV1,
        scope_ref: ReferenceV1,
        purpose_ref: ReferenceV1,
        policy_revision_ref: ReferenceV1,
        maximum_impact_basis_points: u16,
    ) -> Result<Self, IdrRuntimeErrorV1> {
        if maximum_impact_basis_points > 10_000 {
            return Err(IdrRuntimeErrorV1::BindingMismatch);
        }
        Ok(Self {
            tenant_ref,
            subject_ref,
            scope_ref,
            purpose_ref,
            policy_revision_ref,
            maximum_impact_basis_points,
        })
    }
}

/// Policy-filtered read model. It deliberately excludes the raw stored
/// assertion and its unrestricted evidence payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectiveHumanModelAssertionV1 {
    pub assertion_record_id: Uuid,
    pub assertion_revision: u64,
    pub record_digest: String,
    pub predicate: String,
    pub value_digest: String,
    pub scope_ref: String,
    pub lifecycle_state: String,
    pub maximum_impact_basis_points: u16,
    pub expires_at_epoch_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimedOutboxEventV1 {
    pub outbox_id: Uuid,
    pub tenant_ref: String,
    pub run_id: Uuid,
    pub aggregate_version: u64,
    pub event_type: String,
    pub payload: Value,
    pub delivery_attempts: u32,
}

#[derive(Clone)]
pub struct PostgresIdrOrchestratorV1 {
    pool: PgPool,
    security: PostgresSecurityContextV1,
    trust_domain: IdrTrustDomainV1,
    authority: OrchestratorAuthorityV1,
    attestation_key: OrchestratorAttestationKeyV1,
}

/// Migration authority is compiled separately from the Production runtime.
/// Its process/credential may apply DDL but cannot be linked into a binary
/// that enables the `production` feature.
#[cfg(feature = "migration")]
pub struct PostgresIdrMigratorV1;

#[cfg(feature = "migration")]
impl PostgresIdrMigratorV1 {
    pub async fn migrate_production(
        database_url: &str,
        schema: &str,
    ) -> Result<(), IdrRuntimeErrorV1> {
        let attestation_key = OrchestratorAttestationKeyV1::from_protected_environment_file()?;
        validate_schema_name(schema)?;
        if !schema.starts_with("idr_production_") {
            return Err(IdrRuntimeErrorV1::Repository(
                "Production migration requires an idr_production_* schema".to_string(),
            ));
        }
        let pool = connect_pool_for_schema(database_url, schema).await?;
        MIGRATOR.run(&pool).await.map_err(repository_error)?;
        provision_orchestrator_attestation_key(&pool, &attestation_key).await?;
        verify_schema_version_for_pool(&pool).await?;
        pool.close().await;
        Ok(())
    }

    pub async fn migrate_production_from_environment(
        schema: &str,
    ) -> Result<(), IdrRuntimeErrorV1> {
        let database_url = std::env::var("IDR_MIGRATOR_DATABASE_URL").map_err(|_| {
            IdrRuntimeErrorV1::Repository("IDR_MIGRATOR_DATABASE_URL is required".to_string())
        })?;
        Self::migrate_production(&database_url, schema).await
    }
}

#[cfg(all(feature = "shadow-mode", feature = "test-support"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestExecutionProjectionV1 {
    pub reservation_id: Uuid,
    pub permit_id: Uuid,
    pub dispatch_nonce: String,
    pub state: crate::ExecutionAggregateStateV1,
}

#[cfg(all(feature = "shadow-mode", feature = "test-support"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestRunProjectionV1 {
    pub aggregate_version: u64,
    pub state: crate::IdrRunStateV1,
    pub execution: Option<TestExecutionProjectionV1>,
}

impl PostgresIdrOrchestratorV1 {
    /// Connects the separately compiled Shadow authority to a Shadow-only
    /// schema. Production builds do not expose this constructor.
    #[cfg(feature = "shadow-mode")]
    pub async fn connect_shadow(
        database_url: &str,
        schema: &str,
        security: PostgresSecurityContextV1,
    ) -> Result<Self, IdrRuntimeErrorV1> {
        if !schema.starts_with("idr_shadow_") {
            return Err(IdrRuntimeErrorV1::Repository(
                "Shadow authority requires an idr_shadow_* schema".to_string(),
            ));
        }
        Self::connect_for_domain(
            database_url,
            schema,
            security,
            IdrTrustDomainV1::Shadow,
            OrchestratorAttestationKeyV1::random_shadow_key(),
        )
        .await
    }

    /// Connects the separately compiled Production authority to a
    /// Production-only schema. Shadow builds do not expose this constructor.
    #[cfg(feature = "production")]
    pub async fn connect_production(
        database_url: &str,
        schema: &str,
        security: PostgresSecurityContextV1,
    ) -> Result<Self, IdrRuntimeErrorV1> {
        enforce_production_startup_release_gates()?;
        if !schema.starts_with("idr_production_") {
            return Err(IdrRuntimeErrorV1::Repository(
                "Production authority requires an idr_production_* schema".to_string(),
            ));
        }
        if !security.anchor_backend.is_production_durable() {
            return Err(IdrRuntimeErrorV1::Repository(
                "production startup requires an external durable audit anchor".to_string(),
            ));
        }
        let attestation_key = OrchestratorAttestationKeyV1::from_protected_environment_file()?;
        Self::connect_for_domain(
            database_url,
            schema,
            security,
            IdrTrustDomainV1::Production,
            attestation_key,
        )
        .await
    }

    async fn connect_for_domain(
        database_url: &str,
        schema: &str,
        security: PostgresSecurityContextV1,
        trust_domain: IdrTrustDomainV1,
        attestation_key: OrchestratorAttestationKeyV1,
    ) -> Result<Self, IdrRuntimeErrorV1> {
        validate_schema_name(schema)?;
        if security.trust_domain != trust_domain
            || security
                .trust_root_provider
                .current_snapshot()?
                .trust_domain()
                != trust_domain
            || security
                .trust_root_provider
                .current_snapshot()?
                .environment_ref()
                != &security.environment_ref
        {
            return Err(IdrRuntimeErrorV1::Repository(
                "security context or Trust Root crossed IDR trust domains".to_string(),
            ));
        }
        let pool = connect_pool_for_schema(database_url, schema).await?;
        #[cfg(feature = "shadow-mode")]
        MIGRATOR.run(&pool).await.map_err(repository_error)?;
        #[cfg(feature = "shadow-mode")]
        provision_orchestrator_attestation_key(&pool, &attestation_key).await?;
        let store = Self {
            pool,
            security,
            trust_domain,
            authority: OrchestratorAuthorityV1::new(),
            attestation_key,
        };
        store.verify_schema_version().await?;
        #[cfg(feature = "production")]
        store.verify_production_runtime_role().await?;
        store.verify_trust_domain().await?;
        store.verify_external_anchor().await?;
        Ok(store)
    }

    async fn verify_schema_version(&self) -> Result<(), IdrRuntimeErrorV1> {
        verify_schema_version_for_pool(&self.pool).await
    }

    #[cfg(feature = "production")]
    async fn verify_production_runtime_role(&self) -> Result<(), IdrRuntimeErrorV1> {
        let row = sqlx::query(
            "WITH required_role AS (
                 SELECT 'idr_runtime_' || substr(
                     encode(sha256(convert_to(current_schema(), 'UTF8')), 'hex'),
                     1,
                     32
                 ) AS role_name
             )
             SELECT actor.rolcanlogin, actor.rolsuper, actor.rolcreaterole, actor.rolcreatedb,
                    actor.rolreplication, actor.rolbypassrls, actor.rolinherit,
                    pg_has_role(current_user, required_role.role_name, 'USAGE')
                        AS has_required_role,
                    pg_has_role(current_user, required_role.role_name, 'MEMBER')
                        AS is_required_role_member,
                    pg_has_role(current_user, required_role.role_name, 'SET')
                        AS can_set_required_role,
                    (
                        SELECT NOT required.rolcanlogin
                           AND NOT required.rolsuper
                           AND NOT required.rolcreaterole
                           AND NOT required.rolcreatedb
                           AND NOT required.rolreplication
                           AND NOT required.rolbypassrls
                           AND NOT required.rolinherit
                          FROM pg_roles required
                         WHERE required.rolname = required_role.role_name
                    ) AS required_role_is_restricted,
                    EXISTS (
                        SELECT 1
                          FROM pg_roles candidate
                         WHERE candidate.rolname NOT IN (
                                   current_user,
                                   required_role.role_name
                               )
                           AND pg_has_role(current_user, candidate.oid, 'SET')
                    ) AS can_set_unexpected_role,
                    has_schema_privilege(current_user, current_schema(), 'CREATE')
                        AS can_create_in_schema,
                    has_database_privilege(current_user, current_database(), 'CREATE')
                        AS can_create_database_objects,
                    has_database_privilege(
                        current_user, current_database(), 'TEMPORARY'
                    ) AS can_create_temporary_objects,
                    EXISTS (
                        SELECT 1
                         FROM pg_class relation
                          JOIN pg_namespace namespace
                            ON namespace.oid = relation.relnamespace
                         WHERE namespace.nspname = current_schema()
                           AND relation.relkind IN ('r', 'p')
                           AND (
                               has_table_privilege(current_user, relation.oid, 'INSERT')
                               OR has_table_privilege(current_user, relation.oid, 'UPDATE')
                               OR has_table_privilege(current_user, relation.oid, 'DELETE')
                               OR has_table_privilege(current_user, relation.oid, 'TRUNCATE')
                               OR has_table_privilege(current_user, relation.oid, 'REFERENCES')
                               OR has_table_privilege(current_user, relation.oid, 'TRIGGER')
                           )
                    ) AS can_mutate_base_table,
                    EXISTS (
                        SELECT 1
                          FROM pg_class relation
                          JOIN pg_namespace namespace
                            ON namespace.oid = relation.relnamespace
                         WHERE namespace.nspname = current_schema()
                           AND relation.relkind = 'S'
                           AND (
                               has_sequence_privilege(current_user, relation.oid, 'USAGE')
                               OR has_sequence_privilege(current_user, relation.oid, 'SELECT')
                               OR has_sequence_privilege(current_user, relation.oid, 'UPDATE')
                           )
                    ) AS can_use_authority_sequence,
                    (
                        SELECT count(*) = 22
                           AND bool_and(
                               has_table_privilege(current_user, relation.oid, 'SELECT')
                               AND has_table_privilege(current_user, relation.oid, 'INSERT')
                               AND has_table_privilege(current_user, relation.oid, 'UPDATE')
                               AND NOT has_table_privilege(current_user, relation.oid, 'DELETE')
                               AND NOT has_table_privilege(current_user, relation.oid, 'TRIGGER')
                           )
                          FROM pg_class relation
                          JOIN pg_namespace namespace
                            ON namespace.oid = relation.relnamespace
                         WHERE namespace.nspname = current_schema()
                           AND relation.relkind = 'v'
                           AND relation.relname LIKE 'idr\\_%' ESCAPE '\\'
                    ) AS has_exact_view_acl,
                    has_table_privilege(
                        current_user, 'idr_transition_attestations_v7', 'SELECT'
                    ) AND NOT has_table_privilege(
                        current_user, 'idr_transition_attestations_v7', 'INSERT'
                    ) AND NOT has_table_privilege(
                        current_user, 'idr_transition_attestations_v7', 'UPDATE'
                    ) AS has_exact_attestation_acl,
                    NOT has_table_privilege(
                        current_user, 'idr_orchestrator_attestation_secrets_v7', 'SELECT'
                    ) AND NOT has_table_privilege(
                        current_user, 'idr_orchestrator_attestation_secrets_v7', 'INSERT'
                    ) AS cannot_read_attestation_secret,
                    NOT EXISTS (
                        SELECT 1
                          FROM pg_proc procedure
                          JOIN pg_namespace namespace
                            ON namespace.oid = procedure.pronamespace
                         WHERE namespace.nspname = current_schema()
                           AND has_function_privilege(
                               current_user, procedure.oid, 'EXECUTE'
                           )
                           AND procedure.proname <> 'idr_open_attested_transition_v8'
                    ) AS has_no_unexpected_function_execution,
                    EXISTS (
                        SELECT 1
                          FROM pg_proc procedure
                          JOIN pg_namespace namespace
                            ON namespace.oid = procedure.pronamespace
                         WHERE namespace.nspname = current_schema()
                           AND procedure.proname = 'idr_open_attested_transition_v8'
                           AND has_function_privilege(
                               current_user, procedure.oid, 'EXECUTE'
                           )
                    ) AS can_open_attested_transition,
                    NOT EXISTS (
                        SELECT 1
                          FROM pg_proc procedure
                          JOIN pg_namespace namespace
                            ON namespace.oid = procedure.pronamespace
                         WHERE namespace.nspname = current_schema()
                           AND procedure.prosecdef
                           AND (
                               procedure.proconfig IS NULL
                               OR array_position(
                                   procedure.proconfig,
                                   'search_path='
                                       || current_schema()
                                       || ', pg_catalog, pg_temp'
                               ) IS NULL
                           )
                    ) AS security_definer_paths_are_pinned,
                    EXISTS (
                        SELECT 1
                        FROM pg_class relation
                        JOIN pg_namespace namespace
                          ON namespace.oid = relation.relnamespace
                        WHERE namespace.nspname = current_schema()
                          AND relation.relkind IN ('r', 'p', 'S')
                          AND pg_has_role(current_user, relation.relowner, 'USAGE')
                    ) AS can_assume_object_owner
             FROM pg_roles actor
             CROSS JOIN required_role
             WHERE actor.rolname = current_user",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(repository_error)?;
        let forbidden_authority = !row
            .try_get::<bool, _>("rolcanlogin")
            .map_err(repository_error)?
            || row
                .try_get::<bool, _>("rolsuper")
                .map_err(repository_error)?
            || row
                .try_get::<bool, _>("rolcreaterole")
                .map_err(repository_error)?
            || row
                .try_get::<bool, _>("rolcreatedb")
                .map_err(repository_error)?
            || row
                .try_get::<bool, _>("rolreplication")
                .map_err(repository_error)?
            || row
                .try_get::<bool, _>("rolbypassrls")
                .map_err(repository_error)?
            || !row
                .try_get::<bool, _>("rolinherit")
                .map_err(repository_error)?
            || !row
                .try_get::<bool, _>("has_required_role")
                .map_err(repository_error)?
            || !row
                .try_get::<bool, _>("is_required_role_member")
                .map_err(repository_error)?
            || !row
                .try_get::<bool, _>("can_set_required_role")
                .map_err(repository_error)?
            || !row
                .try_get::<bool, _>("required_role_is_restricted")
                .map_err(repository_error)?
            || row
                .try_get::<bool, _>("can_set_unexpected_role")
                .map_err(repository_error)?
            || row
                .try_get::<bool, _>("can_create_in_schema")
                .map_err(repository_error)?
            || row
                .try_get::<bool, _>("can_create_database_objects")
                .map_err(repository_error)?
            || row
                .try_get::<bool, _>("can_create_temporary_objects")
                .map_err(repository_error)?
            || row
                .try_get::<bool, _>("can_mutate_base_table")
                .map_err(repository_error)?
            || row
                .try_get::<bool, _>("can_use_authority_sequence")
                .map_err(repository_error)?
            || !row
                .try_get::<bool, _>("has_exact_view_acl")
                .map_err(repository_error)?
            || !row
                .try_get::<bool, _>("has_exact_attestation_acl")
                .map_err(repository_error)?
            || !row
                .try_get::<bool, _>("cannot_read_attestation_secret")
                .map_err(repository_error)?
            || !row
                .try_get::<bool, _>("has_no_unexpected_function_execution")
                .map_err(repository_error)?
            || !row
                .try_get::<bool, _>("can_open_attested_transition")
                .map_err(repository_error)?
            || !row
                .try_get::<bool, _>("security_definer_paths_are_pinned")
                .map_err(repository_error)?
            || row
                .try_get::<bool, _>("can_assume_object_owner")
                .map_err(repository_error)?;
        if forbidden_authority {
            return Err(IdrRuntimeErrorV1::Repository(
                "Production Runtime database identity does not match the exact Round 13 ACL"
                    .to_string(),
            ));
        }
        Ok(())
    }

    #[cfg(all(feature = "shadow-mode", feature = "test-support"))]
    pub fn pool_for_test(&self) -> &PgPool {
        &self.pool
    }

    #[cfg(all(feature = "shadow-mode", feature = "test-support"))]
    pub async fn query_effective_human_model_assertions_for_test(
        &self,
        query: &HumanModelQueryV1,
    ) -> Result<Vec<EffectiveHumanModelAssertionV1>, IdrRuntimeErrorV1> {
        let rows = sqlx::query(
            "SELECT a.assertion_record_id, a.assertion_revision, c.record_digest,
                    a.lifecycle_state,
                    a.assertion #>> '{payload,predicate}' AS predicate,
                    a.assertion #>> '{payload,value_digest}' AS value_digest,
                    a.assertion #>> '{payload,scope_ref}' AS scope_ref,
                    (a.assertion #>> '{payload,maximum_impact_basis_points}')::integer
                        AS maximum_impact_basis_points,
                    EXTRACT(EPOCH FROM a.expires_at)::bigint AS expires_at
             FROM idr_human_model_assertions a
             JOIN idr_contract_records c
               ON c.record_id = a.assertion_record_id
              AND c.revision = a.assertion_revision
             JOIN idr_contract_current current_record
               ON current_record.record_id = a.assertion_record_id
              AND current_record.revision = a.assertion_revision
             WHERE c.tenant_ref = $1
               AND c.subject_ref = $2
               AND c.record #>> '{policy_revision_ref}' = $3
               AND a.assertion #>> '{payload,scope_ref}' = $4
               AND jsonb_exists(a.assertion #> '{payload,allowed_purposes}', $5)
               AND (a.assertion #>> '{payload,maximum_impact_basis_points}')::integer <= $6
               AND a.lifecycle_state IN ('provisional','user_confirmed','outcome_supported')
               AND (a.expires_at IS NULL OR a.expires_at > clock_timestamp())
               AND c.trust_domain = $7
               AND c.environment_ref = $8
               AND NOT EXISTS (
                   SELECT 1 FROM idr_contract_invalidations invalidation
                   WHERE invalidation.record_id = a.assertion_record_id
                     AND invalidation.revision = a.assertion_revision
               )
             ORDER BY a.assertion_record_id, a.assertion_revision",
        )
        .bind(query.tenant_ref.as_str())
        .bind(query.subject_ref.as_str())
        .bind(query.policy_revision_ref.as_str())
        .bind(query.scope_ref.as_str())
        .bind(query.purpose_ref.as_str())
        .bind(i32::from(query.maximum_impact_basis_points))
        .bind(trust_domain_name(self.trust_domain)?)
        .bind(self.security.environment_ref.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(repository_error)?;
        rows.into_iter()
            .map(|row| {
                let revision: i64 = row
                    .try_get("assertion_revision")
                    .map_err(repository_error)?;
                let impact: i32 = row
                    .try_get("maximum_impact_basis_points")
                    .map_err(repository_error)?;
                let expires_at: Option<i64> =
                    row.try_get("expires_at").map_err(repository_error)?;
                Ok(EffectiveHumanModelAssertionV1 {
                    assertion_record_id: row
                        .try_get("assertion_record_id")
                        .map_err(repository_error)?,
                    assertion_revision: u64::try_from(revision)
                        .map_err(|_| IdrRuntimeErrorV1::Serialization)?,
                    record_digest: row.try_get("record_digest").map_err(repository_error)?,
                    predicate: row.try_get("predicate").map_err(repository_error)?,
                    value_digest: row.try_get("value_digest").map_err(repository_error)?,
                    scope_ref: row.try_get("scope_ref").map_err(repository_error)?,
                    lifecycle_state: row.try_get("lifecycle_state").map_err(repository_error)?,
                    maximum_impact_basis_points: u16::try_from(impact)
                        .map_err(|_| IdrRuntimeErrorV1::Serialization)?,
                    expires_at_epoch_seconds: expires_at
                        .map(u64::try_from)
                        .transpose()
                        .map_err(|_| IdrRuntimeErrorV1::Serialization)?,
                })
            })
            .collect()
    }

    #[cfg(all(feature = "shadow-mode", feature = "test-support"))]
    pub async fn claim_outbox_events_for_test(
        &self,
        worker_ref: &ReferenceV1,
        limit: u16,
        lease_seconds: u32,
    ) -> Result<Vec<ClaimedOutboxEventV1>, IdrRuntimeErrorV1> {
        if limit == 0 || limit > 500 || lease_seconds == 0 || lease_seconds > 3_600 {
            return Err(IdrRuntimeErrorV1::BindingMismatch);
        }
        let command_id = Uuid::new_v4();
        let command_digest = idr_protocol::production::canonical_digest_v1(
            "idr-shadow-outbox-claim-command-v1",
            &json!({
                "command_id": command_id,
                "worker_ref": worker_ref,
                "limit": limit,
                "lease_seconds": lease_seconds,
            }),
        )
        .map_err(IdrRuntimeErrorV1::from)?;
        let transition_digest = idr_protocol::production::canonical_digest_v1(
            "idr-shadow-outbox-claim-transition-v1",
            &json!({
                "command_id": command_id,
                "worker_ref": worker_ref,
                "limit": limit,
                "lease_seconds": lease_seconds,
            }),
        )
        .map_err(IdrRuntimeErrorV1::from)?;
        let mut transaction = self.pool.begin().await.map_err(repository_error)?;
        self.open_orchestrator_attestation(
            &mut transaction,
            "outbox_delivery",
            command_id,
            ZERO_RUN_ID_V1,
            "*",
            0,
            0,
            &command_digest,
            &transition_digest,
        )
        .await?;
        let rows = sqlx::query(
            "WITH selected AS (
                SELECT outbox_id
                FROM idr_outbox_events
                WHERE delivered_at IS NULL
                  AND trust_domain = $4
                  AND environment_ref = $5
                  AND available_at <= clock_timestamp()
                  AND (claim_until IS NULL OR claim_until <= clock_timestamp())
                ORDER BY available_at, created_at, outbox_id
                FOR UPDATE SKIP LOCKED
                LIMIT $1
             )
             UPDATE idr_outbox_events event
             SET claim_owner = $2,
                 claimed_at = clock_timestamp(),
                 claim_until = clock_timestamp() + ($3::text || ' seconds')::interval,
                 delivery_attempts = delivery_attempts + 1
             FROM selected
             WHERE event.outbox_id = selected.outbox_id
             RETURNING event.outbox_id, event.tenant_ref, event.run_id,
                       event.aggregate_version, event.event_type, event.payload,
                       event.delivery_attempts",
        )
        .bind(i64::from(limit))
        .bind(worker_ref.as_str())
        .bind(i64::from(lease_seconds))
        .bind(trust_domain_name(self.trust_domain)?)
        .bind(self.security.environment_ref.as_str())
        .fetch_all(&mut *transaction)
        .await
        .map_err(repository_error)?;
        let claimed = rows
            .into_iter()
            .map(|row| {
                let version: i64 = row.try_get("aggregate_version").map_err(repository_error)?;
                let attempts: i32 = row.try_get("delivery_attempts").map_err(repository_error)?;
                Ok(ClaimedOutboxEventV1 {
                    outbox_id: row.try_get("outbox_id").map_err(repository_error)?,
                    tenant_ref: row.try_get("tenant_ref").map_err(repository_error)?,
                    run_id: row.try_get("run_id").map_err(repository_error)?,
                    aggregate_version: u64::try_from(version)
                        .map_err(|_| IdrRuntimeErrorV1::Serialization)?,
                    event_type: row.try_get("event_type").map_err(repository_error)?,
                    payload: row.try_get("payload").map_err(repository_error)?,
                    delivery_attempts: u32::try_from(attempts)
                        .map_err(|_| IdrRuntimeErrorV1::Serialization)?,
                })
            })
            .collect::<Result<Vec<_>, IdrRuntimeErrorV1>>()?;
        transaction.commit().await.map_err(repository_error)?;
        Ok(claimed)
    }

    #[cfg(all(feature = "shadow-mode", feature = "test-support"))]
    pub async fn acknowledge_outbox_event_for_test(
        &self,
        worker_ref: &ReferenceV1,
        outbox_id: Uuid,
    ) -> Result<(), IdrRuntimeErrorV1> {
        let command_id = Uuid::new_v4();
        let command_digest = idr_protocol::production::canonical_digest_v1(
            "idr-shadow-outbox-acknowledgement-command-v1",
            &json!({
                "command_id": command_id,
                "worker_ref": worker_ref,
                "outbox_id": outbox_id,
            }),
        )
        .map_err(IdrRuntimeErrorV1::from)?;
        let transition_digest = idr_protocol::production::canonical_digest_v1(
            "idr-shadow-outbox-acknowledgement-transition-v1",
            &json!({
                "command_id": command_id,
                "worker_ref": worker_ref,
                "outbox_id": outbox_id,
            }),
        )
        .map_err(IdrRuntimeErrorV1::from)?;
        let mut transaction = self.pool.begin().await.map_err(repository_error)?;
        self.open_orchestrator_attestation(
            &mut transaction,
            "outbox_delivery",
            command_id,
            ZERO_RUN_ID_V1,
            "*",
            0,
            0,
            &command_digest,
            &transition_digest,
        )
        .await?;
        let result = sqlx::query(
            "UPDATE idr_outbox_events
             SET delivered_at = clock_timestamp(),
                 claim_owner = NULL,
                 claimed_at = NULL,
                 claim_until = NULL
             WHERE outbox_id = $1
               AND claim_owner = $2
               AND trust_domain = $3
               AND environment_ref = $4
               AND delivered_at IS NULL
               AND claim_until > clock_timestamp()",
        )
        .bind(outbox_id)
        .bind(worker_ref.as_str())
        .bind(trust_domain_name(self.trust_domain)?)
        .bind(self.security.environment_ref.as_str())
        .execute(&mut *transaction)
        .await
        .map_err(repository_error)?;
        if result.rows_affected() != 1 {
            return Err(IdrRuntimeErrorV1::BindingMismatch);
        }
        transaction.commit().await.map_err(repository_error)?;
        Ok(())
    }

    async fn verify_trust_domain(&self) -> Result<(), IdrRuntimeErrorV1> {
        let expected = trust_domain_name(self.trust_domain)?;
        let environment = self.security.environment_ref.as_str();
        let incompatible: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                SELECT 1 FROM idr_runs
                 WHERE trust_domain <> $1 OR environment_ref <> $2
                UNION ALL
                SELECT 1 FROM idr_run_events
                 WHERE trust_domain <> $1 OR environment_ref <> $2
                UNION ALL
                SELECT 1 FROM idr_step_snapshots
                 WHERE trust_domain <> $1 OR environment_ref <> $2
                UNION ALL
                SELECT 1 FROM idr_contract_records
                 WHERE trust_domain <> $1 OR environment_ref <> $2
                UNION ALL
                SELECT 1 FROM idr_contract_current
                 WHERE trust_domain <> $1 OR environment_ref <> $2
                UNION ALL
                SELECT 1 FROM idr_proof_envelopes
                 WHERE trust_domain <> $1 OR environment_ref <> $2
                UNION ALL
                SELECT 1 FROM idr_proof_consumptions
                 WHERE trust_domain <> $1 OR environment_ref <> $2
                UNION ALL
                SELECT 1 FROM idr_audit_events
                 WHERE trust_domain <> $1 OR environment_ref <> $2
                UNION ALL
                SELECT 1 FROM idr_outbox_events
                 WHERE trust_domain <> $1 OR environment_ref <> $2
                UNION ALL
                SELECT 1 FROM idr_audit_checkpoints
                 WHERE trust_domain <> $1 OR environment_ref <> $2
                UNION ALL
                SELECT 1 FROM idr_command_receipts
                 WHERE trust_domain <> $1 OR environment_ref <> $2
                UNION ALL
                SELECT 1 FROM idr_contract_dependencies dependency
                JOIN idr_contract_records dependent
                  ON dependent.record_id = dependency.dependent_record_id
                 AND dependent.revision = dependency.dependent_revision
                JOIN idr_contract_records source
                  ON source.record_id = dependency.dependency_record_id
                 AND source.revision = dependency.dependency_revision
                 WHERE dependent.trust_domain <> $1 OR dependent.environment_ref <> $2
                    OR source.trust_domain <> $1 OR source.environment_ref <> $2
                UNION ALL
                SELECT 1 FROM idr_execution_reservations execution
                JOIN idr_runs run ON run.run_id = execution.run_id
                 WHERE run.trust_domain <> $1 OR run.environment_ref <> $2
                UNION ALL
                SELECT 1 FROM idr_operation_idempotency_fences
                 WHERE trust_domain <> $1 OR environment_ref <> $2
                UNION ALL
                SELECT 1 FROM idr_execution_receipts receipt
                JOIN idr_contract_records contract
                  ON contract.record_id = receipt.receipt_record_id
                 AND contract.revision = receipt.receipt_revision
                 WHERE contract.trust_domain <> $1 OR contract.environment_ref <> $2
                UNION ALL
                SELECT 1 FROM idr_outcome_records outcome
                JOIN idr_contract_records contract
                  ON contract.record_id = outcome.outcome_record_id
                 AND contract.revision = outcome.outcome_revision
                 WHERE contract.trust_domain <> $1 OR contract.environment_ref <> $2
                UNION ALL
                SELECT 1 FROM idr_human_model_candidates candidate
                JOIN idr_contract_records contract
                  ON contract.record_id = candidate.candidate_record_id
                 AND contract.revision = candidate.candidate_revision
                 WHERE contract.trust_domain <> $1 OR contract.environment_ref <> $2
                UNION ALL
                SELECT 1 FROM idr_human_model_promotion_decisions decision
                JOIN idr_contract_records contract
                  ON contract.record_id = decision.decision_record_id
                 AND contract.revision = decision.decision_revision
                 WHERE contract.trust_domain <> $1 OR contract.environment_ref <> $2
                UNION ALL
                SELECT 1 FROM idr_human_model_assertions assertion
                JOIN idr_contract_records contract
                  ON contract.record_id = assertion.assertion_record_id
                 AND contract.revision = assertion.assertion_revision
                 WHERE contract.trust_domain <> $1 OR contract.environment_ref <> $2
             )",
        )
        .bind(expected)
        .bind(environment)
        .fetch_one(&self.pool)
        .await
        .map_err(repository_error)?;
        if incompatible {
            return Err(IdrRuntimeErrorV1::Repository(
                "database contains unclassified or foreign trust identity state".to_string(),
            ));
        }
        Ok(())
    }

    async fn verify_external_anchor(&self) -> Result<(), IdrRuntimeErrorV1> {
        self.verify_database_integrity().await?;
        let rows = sqlx::query(
            "SELECT tenants.tenant_ref, latest.signed_checkpoint
             FROM (SELECT DISTINCT tenant_ref FROM idr_runs) AS tenants
             LEFT JOIN LATERAL (
                SELECT signed_checkpoint
                FROM idr_audit_checkpoints
                WHERE tenant_ref = tenants.tenant_ref
                  AND trust_domain = $1
                  AND environment_ref = $2
                ORDER BY audit_sequence DESC LIMIT 1
             ) AS latest ON true
             ORDER BY tenants.tenant_ref",
        )
        .bind(trust_domain_name(self.trust_domain)?)
        .bind(self.security.environment_ref.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(repository_error)?;
        for row in rows {
            let tenant_ref: String = row.try_get("tenant_ref").map_err(repository_error)?;
            let db_checkpoint: Option<Value> =
                row.try_get("signed_checkpoint").map_err(repository_error)?;
            let db_checkpoint = db_checkpoint
                .map(serde_json::from_value::<AuditCheckpointV1>)
                .transpose()
                .map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?;
            let mut anchor = self
                .security
                .anchor_backend
                .load_latest(
                    self.trust_domain,
                    self.security.environment_ref.as_str(),
                    &tenant_ref,
                )
                .await?;
            if let Some(checkpoint) = &db_checkpoint {
                if anchor
                    .as_ref()
                    .is_none_or(|current| current.audit_sequence < checkpoint.audit_sequence)
                {
                    self.security.anchor_backend.publish(checkpoint).await?;
                    anchor = self
                        .security
                        .anchor_backend
                        .load_latest(
                            self.trust_domain,
                            self.security.environment_ref.as_str(),
                            &tenant_ref,
                        )
                        .await?;
                }
                if anchor.as_ref() != Some(checkpoint) {
                    return Err(IdrRuntimeErrorV1::Repository(
                        "database is behind or forked from the external audit anchor".to_string(),
                    ));
                }
            } else if anchor.is_some() || self.trust_domain == IdrTrustDomainV1::Production {
                return Err(IdrRuntimeErrorV1::Repository(
                    "database and external checkpoint state are inconsistent".to_string(),
                ));
            }
        }
        Ok(())
    }

    async fn verify_database_integrity(&self) -> Result<(), IdrRuntimeErrorV1> {
        self.verify_all_orchestrator_attestations().await?;
        let audit_rows = sqlx::query(
            "SELECT trust_domain, environment_ref, tenant_ref, run_id, aggregate_version,
                    actor_ref, caller_ref, correlation_ref, causation_ref, contract_refs, proof_refs,
                    policy_revision_ref, previous_hash, event_hash, payload
             FROM idr_audit_events
             ORDER BY tenant_ref, sequence",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(repository_error)?;
        let mut tenant_roots = BTreeMap::<String, String>::new();
        for row in audit_rows {
            let tenant_ref: String = row.try_get("tenant_ref").map_err(repository_error)?;
            let row_domain: String = row.try_get("trust_domain").map_err(repository_error)?;
            let row_environment: String =
                row.try_get("environment_ref").map_err(repository_error)?;
            let previous_hash: String = row.try_get("previous_hash").map_err(repository_error)?;
            let expected_previous = tenant_roots
                .get(&tenant_ref)
                .cloned()
                .unwrap_or_else(|| "0".repeat(64));
            let payload: Value = row.try_get("payload").map_err(repository_error)?;
            let columns_match = row_domain == trust_domain_name(self.trust_domain)?
                && row_environment == self.security.environment_ref.as_str()
                && payload.get("trust_domain").and_then(Value::as_str) == Some(row_domain.as_str())
                && payload.get("environment_ref").and_then(Value::as_str)
                    == Some(row_environment.as_str())
                && previous_hash == expected_previous
                && payload.get("tenant_ref").and_then(Value::as_str) == Some(tenant_ref.as_str())
                && payload.get("run_id").and_then(Value::as_str)
                    == Some(
                        row.try_get::<Uuid, _>("run_id")
                            .map_err(repository_error)?
                            .to_string()
                            .as_str(),
                    )
                && payload.get("aggregate_version").and_then(Value::as_u64)
                    == Some(
                        u64::try_from(
                            row.try_get::<i64, _>("aggregate_version")
                                .map_err(repository_error)?,
                        )
                        .map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?,
                    )
                && payload.get("actor_ref").and_then(Value::as_str)
                    == Some(
                        row.try_get::<String, _>("actor_ref")
                            .map_err(repository_error)?
                            .as_str(),
                    )
                && payload.get("caller_ref").and_then(Value::as_str)
                    == Some(
                        row.try_get::<String, _>("caller_ref")
                            .map_err(repository_error)?
                            .as_str(),
                    )
                && payload.get("correlation_ref").and_then(Value::as_str)
                    == Some(
                        row.try_get::<String, _>("correlation_ref")
                            .map_err(repository_error)?
                            .as_str(),
                    )
                && payload.get("causation_ref").and_then(Value::as_str)
                    == Some(
                        row.try_get::<String, _>("causation_ref")
                            .map_err(repository_error)?
                            .as_str(),
                    )
                && payload.get("policy_revision_ref").and_then(Value::as_str)
                    == Some(
                        row.try_get::<String, _>("policy_revision_ref")
                            .map_err(repository_error)?
                            .as_str(),
                    )
                && payload.get("contract_refs")
                    == Some(
                        &row.try_get::<Value, _>("contract_refs")
                            .map_err(repository_error)?,
                    )
                && payload.get("proof_refs")
                    == Some(
                        &row.try_get::<Value, _>("proof_refs")
                            .map_err(repository_error)?,
                    );
            let event_hash: String = row.try_get("event_hash").map_err(repository_error)?;
            if !columns_match || hash_event_v1(&previous_hash, &payload)? != event_hash {
                return Err(IdrRuntimeErrorV1::IntegrityViolation);
            }
            tenant_roots.insert(tenant_ref, event_hash);
        }

        let run_rows = sqlx::query(
            "SELECT run.run_id, run.trust_domain, run.environment_ref, run.projection,
                    run.projection_digest,
                    audit.payload #>> '{projection_digest}' AS audited_projection_digest
             FROM idr_runs run
             LEFT JOIN LATERAL (
                SELECT payload FROM idr_audit_events
                WHERE run_id = run.run_id ORDER BY sequence DESC LIMIT 1
             ) audit ON true",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(repository_error)?;
        for row in run_rows {
            let projection: Value = row.try_get("projection").map_err(repository_error)?;
            let row_domain: String = row.try_get("trust_domain").map_err(repository_error)?;
            let row_environment: String =
                row.try_get("environment_ref").map_err(repository_error)?;
            let stored_digest: String =
                row.try_get("projection_digest").map_err(repository_error)?;
            let audited_digest: Option<String> = row
                .try_get("audited_projection_digest")
                .map_err(repository_error)?;
            if row_domain != trust_domain_name(self.trust_domain)?
                || row_environment != self.security.environment_ref.as_str()
                || projection.get("trust_domain").and_then(Value::as_str)
                    != Some(row_domain.as_str())
                || projection.get("environment_ref").and_then(Value::as_str)
                    != Some(row_environment.as_str())
                || projection_digest_v1(&projection)? != stored_digest
                || audited_digest.as_deref() != Some(stored_digest.as_str())
            {
                return Err(IdrRuntimeErrorV1::IntegrityViolation);
            }
            let projection: IdrRunProjectionV1 = serde_json::from_value(projection)
                .map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?;
            self.verify_action_admission_projection(&projection).await?;
            self.verify_current_execution_projection(&projection)
                .await?;
        }

        let contract_rows = sqlx::query(
            "SELECT record_id, revision, candidate_kind, trust_domain, environment_ref,
                    run_id, tenant_ref, subject_ref, record_digest, record,
                    EXTRACT(EPOCH FROM valid_from)::bigint AS valid_from,
                    EXTRACT(EPOCH FROM valid_until)::bigint AS valid_until
             FROM idr_contract_records
             ORDER BY record_id, revision",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(repository_error)?;
        for row in contract_rows {
            let record: Value = row.try_get("record").map_err(repository_error)?;
            let reference = verify_authoritative_record_value_v1(&record)?;
            let revision: i64 = row.try_get("revision").map_err(repository_error)?;
            let valid_from: i64 = row.try_get("valid_from").map_err(repository_error)?;
            let valid_until: i64 = row.try_get("valid_until").map_err(repository_error)?;
            if reference.record_id()
                != row
                    .try_get::<Uuid, _>("record_id")
                    .map_err(repository_error)?
                || reference.revision()
                    != u64::try_from(revision).map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?
                || enum_name(&reference.candidate_kind())?
                    != row
                        .try_get::<String, _>("candidate_kind")
                        .map_err(repository_error)?
                || reference.trust_domain() != self.trust_domain
                || trust_domain_name(reference.trust_domain())?
                    != row
                        .try_get::<String, _>("trust_domain")
                        .map_err(repository_error)?
                || reference.environment_ref() != &self.security.environment_ref
                || reference.environment_ref().as_str()
                    != row
                        .try_get::<String, _>("environment_ref")
                        .map_err(repository_error)?
                || reference.valid_from()
                    != u64::try_from(valid_from)
                        .map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?
                || reference.valid_until()
                    != u64::try_from(valid_until)
                        .map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?
                || reference.record_digest()
                    != row
                        .try_get::<String, _>("record_digest")
                        .map_err(repository_error)?
                || record.get("run_id").and_then(Value::as_str)
                    != Some(
                        row.try_get::<Uuid, _>("run_id")
                            .map_err(repository_error)?
                            .to_string()
                            .as_str(),
                    )
                || record.get("tenant_ref").and_then(Value::as_str)
                    != Some(
                        row.try_get::<String, _>("tenant_ref")
                            .map_err(repository_error)?
                            .as_str(),
                    )
                || record.get("subject_ref").and_then(Value::as_str)
                    != Some(
                        row.try_get::<String, _>("subject_ref")
                            .map_err(repository_error)?
                            .as_str(),
                    )
            {
                return Err(IdrRuntimeErrorV1::IntegrityViolation);
            }
        }

        let proof_rows = sqlx::query(
            "SELECT proof_id, trust_domain, environment_ref, proof_kind, issuer_ref,
                    key_id, tenant_ref, subject_ref, subject_digest, nonce, envelope
             FROM idr_proof_envelopes
             ORDER BY proof_id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(repository_error)?;
        for row in proof_rows {
            let envelope: ProductionProofEnvelopeV1 =
                serde_json::from_value(row.try_get("envelope").map_err(repository_error)?)
                    .map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?;
            envelope
                .validate_shape()
                .map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?;
            let claims = envelope.claims();
            if claims.proof_id()
                != row
                    .try_get::<Uuid, _>("proof_id")
                    .map_err(repository_error)?
                || trust_domain_name(claims.trust_domain())?
                    != row
                        .try_get::<String, _>("trust_domain")
                        .map_err(repository_error)?
                || claims.environment_ref().as_str()
                    != row
                        .try_get::<String, _>("environment_ref")
                        .map_err(repository_error)?
                || enum_name(&claims.proof_kind())?
                    != row
                        .try_get::<String, _>("proof_kind")
                        .map_err(repository_error)?
                || claims.issuer_ref().as_str()
                    != row
                        .try_get::<String, _>("issuer_ref")
                        .map_err(repository_error)?
                || envelope.key_id().as_str()
                    != row
                        .try_get::<String, _>("key_id")
                        .map_err(repository_error)?
                || claims.tenant_ref().as_str()
                    != row
                        .try_get::<String, _>("tenant_ref")
                        .map_err(repository_error)?
                || claims.subject_ref().as_str()
                    != row
                        .try_get::<String, _>("subject_ref")
                        .map_err(repository_error)?
                || claims.subject_digest()
                    != row
                        .try_get::<String, _>("subject_digest")
                        .map_err(repository_error)?
                || claims.nonce()
                    != row
                        .try_get::<String, _>("nonce")
                        .map_err(repository_error)?
            {
                return Err(IdrRuntimeErrorV1::IntegrityViolation);
            }
        }

        let current_rows = sqlx::query(
            "SELECT current_record.trust_domain, current_record.environment_ref,
                    current_record.candidate_kind, current_record.record_id,
                    current_record.revision, current_record.record_digest, run.projection
             FROM idr_contract_current current_record
             JOIN idr_runs run ON run.run_id = current_record.run_id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(repository_error)?;
        for row in current_rows {
            let kind: String = row.try_get("candidate_kind").map_err(repository_error)?;
            let kind: CandidateKindV1 = serde_json::from_value(Value::String(kind))
                .map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?;
            let projection: IdrRunProjectionV1 =
                serde_json::from_value(row.try_get("projection").map_err(repository_error)?)
                    .map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?;
            let expected = projection
                .records
                .get(&kind)
                .ok_or(IdrRuntimeErrorV1::IntegrityViolation)?;
            let revision: i64 = row.try_get("revision").map_err(repository_error)?;
            if row
                .try_get::<String, _>("trust_domain")
                .map_err(repository_error)?
                != trust_domain_name(self.trust_domain)?
                || row
                    .try_get::<String, _>("environment_ref")
                    .map_err(repository_error)?
                    != self.security.environment_ref.as_str()
                || expected.record_id()
                    != row
                        .try_get::<Uuid, _>("record_id")
                        .map_err(repository_error)?
                || expected.revision()
                    != u64::try_from(revision).map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?
                || expected.record_digest()
                    != row
                        .try_get::<String, _>("record_digest")
                        .map_err(repository_error)?
            {
                return Err(IdrRuntimeErrorV1::IntegrityViolation);
            }
        }

        let relational_mismatch: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                SELECT 1
                FROM idr_contract_dependencies dependency
                JOIN idr_contract_records dependent
                  ON dependent.record_id = dependency.dependent_record_id
                 AND dependent.revision = dependency.dependent_revision
                JOIN idr_contract_records source
                  ON source.record_id = dependency.dependency_record_id
                 AND source.revision = dependency.dependency_revision
                WHERE dependency.dependency_digest <> source.record_digest
                   OR dependent.trust_domain <> source.trust_domain
                   OR dependent.environment_ref <> source.environment_ref
                   OR dependent.run_id <> source.run_id
                UNION ALL
                SELECT 1
                FROM idr_runs run
                CROSS JOIN LATERAL jsonb_each(run.projection #> '{records}') projection_record
                LEFT JOIN idr_contract_current current_record
                  ON current_record.run_id = run.run_id
                 AND current_record.candidate_kind = projection_record.key
                WHERE current_record.record_id IS NULL
                   OR current_record.record_id::text
                      <> projection_record.value #>> '{record_id}'
                   OR current_record.revision::text
                      <> projection_record.value #>> '{revision}'
                   OR current_record.record_digest
                      <> projection_record.value #>> '{record_digest}'
                   OR current_record.trust_domain
                      <> projection_record.value #>> '{trust_domain}'
                   OR current_record.environment_ref
                      <> projection_record.value #>> '{environment_ref}'
                UNION ALL
                SELECT 1 FROM idr_proof_envelopes proof
                LEFT JOIN idr_proof_consumptions consumption
                  ON consumption.proof_id = proof.proof_id
                WHERE consumption.proof_id IS NULL
                   OR proof.trust_domain <> consumption.trust_domain
                   OR proof.environment_ref <> consumption.environment_ref
                   OR proof.nonce <> consumption.nonce
                UNION ALL
                SELECT 1 FROM idr_contract_records contract
                CROSS JOIN LATERAL jsonb_array_elements_text(
                    contract.record #> '{proof_ids}'
                ) proof_id
                LEFT JOIN idr_proof_envelopes proof
                  ON proof.proof_id = proof_id::text::uuid
                WHERE proof.proof_id IS NULL
                   OR proof.trust_domain <> contract.trust_domain
                   OR proof.environment_ref <> contract.environment_ref
                UNION ALL
                SELECT 1 FROM idr_execution_receipts receipt
                JOIN idr_contract_records contract
                  ON contract.record_id = receipt.receipt_record_id
                 AND contract.revision = receipt.receipt_revision
                WHERE receipt.receipt IS DISTINCT FROM contract.record
                UNION ALL
                SELECT 1 FROM idr_execution_reservations execution
                JOIN idr_contract_records action
                  ON action.record_id = execution.action_record_id
                 AND action.revision = execution.action_revision
                JOIN idr_contract_records admission
                  ON admission.record_id = execution.action_admission_record_id
                 AND admission.revision = execution.action_admission_revision
                JOIN idr_proof_envelopes auth_proof
                  ON auth_proof.proof_id = execution.exact_authorization_proof_id
                JOIN idr_operation_idempotency_fences fence
                  ON fence.fence_id = execution.fence_id
                WHERE action.record_digest <> execution.action_digest
                   OR action.valid_until <> execution.action_valid_until
                   OR admission.valid_until <> execution.admission_valid_until
                   OR auth_proof.proof_kind <> 'exact_authorization'
                   OR to_timestamp(
                        (auth_proof.envelope #>> '{claims,expires_at}')::bigint
                      ) <> execution.authorization_valid_until
                   OR execution.fence_id <> execution.reservation_id
                   OR fence.trust_domain <> action.trust_domain
                   OR fence.environment_ref <> action.environment_ref
                   OR fence.tenant_ref <> execution.tenant_ref
                   OR fence.operation_ref <> execution.operation_ref
                   OR fence.idempotency_key <> execution.idempotency_key
                   OR fence.first_run_id <> execution.run_id
                   OR fence.first_action_record_id <> execution.action_record_id
                   OR fence.first_action_revision <> execution.action_revision
                   OR fence.first_action_digest <> execution.action_digest
                UNION ALL
                SELECT 1 FROM idr_outcome_records outcome
                JOIN idr_contract_records contract
                  ON contract.record_id = outcome.outcome_record_id
                 AND contract.revision = outcome.outcome_revision
                WHERE outcome.outcome IS DISTINCT FROM contract.record
                UNION ALL
                SELECT 1 FROM idr_human_model_candidates candidate
                JOIN idr_contract_records contract
                  ON contract.record_id = candidate.candidate_record_id
                 AND contract.revision = candidate.candidate_revision
                WHERE candidate.candidate IS DISTINCT FROM contract.record
                UNION ALL
                SELECT 1 FROM idr_human_model_promotion_decisions decision
                JOIN idr_contract_records contract
                  ON contract.record_id = decision.decision_record_id
                 AND contract.revision = decision.decision_revision
                WHERE decision.decision IS DISTINCT FROM contract.record
                UNION ALL
                SELECT 1 FROM idr_human_model_assertions assertion
                JOIN idr_contract_records contract
                  ON contract.record_id = assertion.assertion_record_id
                 AND contract.revision = assertion.assertion_revision
                WHERE assertion.assertion IS DISTINCT FROM contract.record
             )",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(repository_error)?;
        if relational_mismatch {
            return Err(IdrRuntimeErrorV1::IntegrityViolation);
        }

        let checkpoint_rows = sqlx::query(
            "SELECT DISTINCT ON (tenant_ref)
                    checkpoint_id, trust_domain, environment_ref, tenant_ref,
                    audit_sequence, chain_root, record_set_root, execution_state_root,
                    signed_checkpoint
             FROM idr_audit_checkpoints
             ORDER BY tenant_ref, audit_sequence DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(repository_error)?;
        for row in checkpoint_rows {
            let checkpoint: AuditCheckpointV1 =
                serde_json::from_value(row.try_get("signed_checkpoint").map_err(repository_error)?)
                    .map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?;
            let tenant_ref: String = row.try_get("tenant_ref").map_err(repository_error)?;
            if checkpoint.checkpoint_id
                != row
                    .try_get::<Uuid, _>("checkpoint_id")
                    .map_err(repository_error)?
                || trust_domain_name(checkpoint.trust_domain)?
                    != row
                        .try_get::<String, _>("trust_domain")
                        .map_err(repository_error)?
                || checkpoint.environment_ref
                    != row
                        .try_get::<String, _>("environment_ref")
                        .map_err(repository_error)?
                || checkpoint.tenant_ref != tenant_ref
                || checkpoint.audit_sequence
                    != row
                        .try_get::<i64, _>("audit_sequence")
                        .map_err(repository_error)?
                || checkpoint.chain_root
                    != row
                        .try_get::<String, _>("chain_root")
                        .map_err(repository_error)?
                || checkpoint.record_set_root
                    != row
                        .try_get::<String, _>("record_set_root")
                        .map_err(repository_error)?
                || checkpoint.record_set_root
                    != record_set_root_for_pool(&self.pool, &tenant_ref).await?
                || checkpoint.execution_state_root
                    != row
                        .try_get::<String, _>("execution_state_root")
                        .map_err(repository_error)?
                || checkpoint.execution_state_root
                    != execution_state_root_for_pool(&self.pool, &tenant_ref).await?
            {
                return Err(IdrRuntimeErrorV1::IntegrityViolation);
            }
        }
        Ok(())
    }

    async fn verify_action_admission_projection(
        &self,
        projection: &IdrRunProjectionV1,
    ) -> Result<(), IdrRuntimeErrorV1> {
        let Some(admission) = &projection.action_admission else {
            return Ok(());
        };
        if admission.proof_bindings.len() != ACTION_ADMISSION_PROOF_KINDS_V1.len()
            || admission
                .proof_bindings
                .get(&ProductionProofKindV1::ExactAuthorization)
                .is_none_or(|binding| {
                    binding.proof_id != admission.exact_authorization_proof_id
                        || binding.valid_until != admission.authorization_valid_until
                })
        {
            return Err(IdrRuntimeErrorV1::IntegrityViolation);
        }
        let record: Value = sqlx::query_scalar(
            "SELECT record FROM idr_contract_records
             WHERE record_id = $1 AND revision = $2",
        )
        .bind(admission.admission_ref.record_id())
        .bind(i64_from_u64(admission.admission_ref.revision())?)
        .fetch_optional(&self.pool)
        .await
        .map_err(repository_error)?
        .ok_or(IdrRuntimeErrorV1::IntegrityViolation)?;
        let record_proof_ids = record
            .get("proof_ids")
            .and_then(Value::as_array)
            .ok_or(IdrRuntimeErrorV1::IntegrityViolation)?
            .iter()
            .map(|proof_id| {
                proof_id
                    .as_str()
                    .and_then(|proof_id| Uuid::parse_str(proof_id).ok())
                    .ok_or(IdrRuntimeErrorV1::IntegrityViolation)
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        let projected_proof_ids: BTreeSet<_> = admission
            .proof_bindings
            .values()
            .map(|binding| binding.proof_id)
            .collect();
        if projected_proof_ids.len() != ACTION_ADMISSION_PROOF_KINDS_V1.len()
            || record_proof_ids != projected_proof_ids
            || record.pointer("/payload/proof_bindings")
                != Some(
                    &serde_json::to_value(&admission.proof_bindings)
                        .map_err(|_| IdrRuntimeErrorV1::Serialization)?,
                )
        {
            return Err(IdrRuntimeErrorV1::IntegrityViolation);
        }
        Ok(())
    }

    async fn verify_current_execution_projection(
        &self,
        projection: &IdrRunProjectionV1,
    ) -> Result<(), IdrRuntimeErrorV1> {
        let Some(execution) = &projection.execution else {
            return Ok(());
        };
        let row = sqlx::query(
            "SELECT reservation.reservation_id, reservation.fence_id,
                    reservation.run_id, reservation.tenant_ref,
                    reservation.action_record_id, reservation.action_revision,
                    reservation.action_admission_record_id,
                    reservation.action_admission_revision,
                    reservation.operation_ref, reservation.idempotency_key,
                    reservation.provider_ref, reservation.owner_ref,
                    reservation.state, reservation.aggregate_version,
                    reservation.action_digest, reservation.parameter_digest,
                    reservation.request_digest,
                    reservation.exact_authorization_proof_id,
                    EXTRACT(EPOCH FROM reservation.action_valid_until)::bigint
                        AS action_valid_until,
                    EXTRACT(EPOCH FROM reservation.admission_valid_until)::bigint
                        AS admission_valid_until,
                    EXTRACT(EPOCH FROM reservation.authorization_valid_until)::bigint
                        AS authorization_valid_until
                    ,fence.trust_domain AS fence_trust_domain
                    ,fence.environment_ref AS fence_environment_ref
                    ,fence.tenant_ref AS fence_tenant_ref
                    ,fence.operation_ref AS fence_operation_ref
                    ,fence.idempotency_key AS fence_idempotency_key
                    ,fence.first_run_id
                    ,fence.first_action_record_id
                    ,fence.first_action_revision
                    ,fence.first_action_digest
                    ,fence.fence_digest
             FROM idr_execution_reservations reservation
             JOIN idr_operation_idempotency_fences fence
               ON fence.fence_id = reservation.fence_id
             WHERE reservation.reservation_id = $1",
        )
        .bind(execution.reservation_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(repository_error)?
        .ok_or(IdrRuntimeErrorV1::IntegrityViolation)?;
        let reservation_matches = row
            .try_get::<Uuid, _>("reservation_id")
            .map_err(repository_error)?
            == execution.reservation_id
            && row
                .try_get::<Uuid, _>("fence_id")
                .map_err(repository_error)?
                == execution.reservation_id
            && row.try_get::<Uuid, _>("run_id").map_err(repository_error)? == projection.run_id
            && row
                .try_get::<String, _>("tenant_ref")
                .map_err(repository_error)?
                == projection.tenant_ref.as_str()
            && row
                .try_get::<Uuid, _>("action_record_id")
                .map_err(repository_error)?
                == execution.action_ref.record_id()
            && u64::try_from(
                row.try_get::<i64, _>("action_revision")
                    .map_err(repository_error)?,
            )
            .map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?
                == execution.action_ref.revision()
            && row
                .try_get::<Uuid, _>("action_admission_record_id")
                .map_err(repository_error)?
                == execution.action_admission_ref.record_id()
            && u64::try_from(
                row.try_get::<i64, _>("action_admission_revision")
                    .map_err(repository_error)?,
            )
            .map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?
                == execution.action_admission_ref.revision()
            && row
                .try_get::<String, _>("operation_ref")
                .map_err(repository_error)?
                == execution.operation_ref.as_str()
            && row
                .try_get::<String, _>("idempotency_key")
                .map_err(repository_error)?
                == execution.idempotency_key
            && row
                .try_get::<String, _>("provider_ref")
                .map_err(repository_error)?
                == execution.provider_ref.as_str()
            && row
                .try_get::<String, _>("owner_ref")
                .map_err(repository_error)?
                == execution.owner_ref.as_str()
            && row
                .try_get::<String, _>("state")
                .map_err(repository_error)?
                == enum_name(&execution.state)?
            && u64::try_from(
                row.try_get::<i64, _>("aggregate_version")
                    .map_err(repository_error)?,
            )
            .map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?
                == projection.aggregate_version
            && row
                .try_get::<String, _>("action_digest")
                .map_err(repository_error)?
                == execution.action_ref.record_digest()
            && row
                .try_get::<String, _>("parameter_digest")
                .map_err(repository_error)?
                == execution.parameter_digest
            && row
                .try_get::<String, _>("request_digest")
                .map_err(repository_error)?
                == execution.request_digest
            && row
                .try_get::<Uuid, _>("exact_authorization_proof_id")
                .map_err(repository_error)?
                == execution.exact_authorization_proof_id
            && u64::try_from(
                row.try_get::<i64, _>("action_valid_until")
                    .map_err(repository_error)?,
            )
            .map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?
                == execution.action_valid_until
            && u64::try_from(
                row.try_get::<i64, _>("admission_valid_until")
                    .map_err(repository_error)?,
            )
            .map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?
                == execution.admission_valid_until
            && u64::try_from(
                row.try_get::<i64, _>("authorization_valid_until")
                    .map_err(repository_error)?,
            )
            .map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?
                == execution.authorization_valid_until
            && row
                .try_get::<String, _>("fence_trust_domain")
                .map_err(repository_error)?
                == trust_domain_name(projection.trust_domain)?
            && row
                .try_get::<String, _>("fence_environment_ref")
                .map_err(repository_error)?
                == projection.environment_ref.as_str()
            && row
                .try_get::<String, _>("fence_tenant_ref")
                .map_err(repository_error)?
                == projection.tenant_ref.as_str()
            && row
                .try_get::<String, _>("fence_operation_ref")
                .map_err(repository_error)?
                == execution.operation_ref.as_str()
            && row
                .try_get::<String, _>("fence_idempotency_key")
                .map_err(repository_error)?
                == execution.idempotency_key
            && row
                .try_get::<Uuid, _>("first_run_id")
                .map_err(repository_error)?
                == projection.run_id
            && row
                .try_get::<Uuid, _>("first_action_record_id")
                .map_err(repository_error)?
                == execution.action_ref.record_id()
            && u64::try_from(
                row.try_get::<i64, _>("first_action_revision")
                    .map_err(repository_error)?,
            )
            .map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?
                == execution.action_ref.revision()
            && row
                .try_get::<String, _>("first_action_digest")
                .map_err(repository_error)?
                == execution.action_ref.record_digest()
            && row
                .try_get::<String, _>("fence_digest")
                .map_err(repository_error)?
                == idempotency_fence_digest_v1(projection, execution)?;
        if !reservation_matches {
            return Err(IdrRuntimeErrorV1::IntegrityViolation);
        }

        let attempt = sqlx::query(
            "SELECT reservation_id, attempt, permit_id, dispatch_nonce,
                    EXTRACT(EPOCH FROM lease_until)::bigint AS lease_until,
                    EXTRACT(EPOCH FROM permit_valid_until)::bigint AS permit_valid_until,
                    state, provider_ref, owner_ref
             FROM idr_execution_attempts
             WHERE reservation_id = $1 AND attempt = $2",
        )
        .bind(execution.reservation_id)
        .bind(i32::try_from(execution.attempt).map_err(|_| IdrRuntimeErrorV1::Serialization)?)
        .fetch_optional(&self.pool)
        .await
        .map_err(repository_error)?
        .ok_or(IdrRuntimeErrorV1::IntegrityViolation)?;
        let attempt_matches = attempt
            .try_get::<Uuid, _>("reservation_id")
            .map_err(repository_error)?
            == execution.reservation_id
            && u32::try_from(
                attempt
                    .try_get::<i32, _>("attempt")
                    .map_err(repository_error)?,
            )
            .map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?
                == execution.attempt
            && attempt
                .try_get::<Uuid, _>("permit_id")
                .map_err(repository_error)?
                == execution.permit_id
            && attempt
                .try_get::<String, _>("dispatch_nonce")
                .map_err(repository_error)?
                == execution.dispatch_nonce
            && u64::try_from(
                attempt
                    .try_get::<i64, _>("lease_until")
                    .map_err(repository_error)?,
            )
            .map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?
                == execution.lease_until
            && u64::try_from(
                attempt
                    .try_get::<i64, _>("permit_valid_until")
                    .map_err(repository_error)?,
            )
            .map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?
                == execution.permit_valid_until
            && attempt
                .try_get::<String, _>("state")
                .map_err(repository_error)?
                == enum_name(&execution.state)?
            && attempt
                .try_get::<String, _>("provider_ref")
                .map_err(repository_error)?
                == execution.provider_ref.as_str()
            && attempt
                .try_get::<String, _>("owner_ref")
                .map_err(repository_error)?
                == execution.owner_ref.as_str();
        if !attempt_matches {
            return Err(IdrRuntimeErrorV1::IntegrityViolation);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn open_orchestrator_attestation(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        purpose: &str,
        command_id: Uuid,
        run_id: Uuid,
        tenant_ref: &str,
        pre_aggregate_version: u64,
        post_aggregate_version: u64,
        command_digest: &str,
        transition_digest: &str,
    ) -> Result<Uuid, IdrRuntimeErrorV1> {
        let (database_txid, backend_pid): (i64, i32) =
            sqlx::query_as("SELECT txid_current()::bigint, pg_backend_pid()")
                .fetch_one(&mut **transaction)
                .await
                .map_err(repository_error)?;
        let attestation_id = Uuid::new_v4();
        let nonce = Uuid::new_v4();
        let material = orchestrator_attestation_material_v1(
            &trust_domain_name(self.trust_domain)?,
            self.security.environment_ref.as_str(),
            purpose,
            attestation_id,
            command_id,
            run_id,
            tenant_ref,
            pre_aggregate_version,
            post_aggregate_version,
            command_digest,
            transition_digest,
            database_txid,
            backend_pid,
            nonce,
            self.attestation_key.version,
        )?;
        let mac = self.attestation_key.mac(&material);
        let opened: Uuid = sqlx::query_scalar(
            "SELECT idr_open_attested_transition_v8(
                $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16
             )",
        )
        .bind(trust_domain_name(self.trust_domain)?)
        .bind(self.security.environment_ref.as_str())
        .bind(purpose)
        .bind(attestation_id)
        .bind(command_id)
        .bind(run_id)
        .bind(tenant_ref)
        .bind(i64_from_u64(pre_aggregate_version)?)
        .bind(i64_from_u64(post_aggregate_version)?)
        .bind(command_digest)
        .bind(transition_digest)
        .bind(database_txid)
        .bind(backend_pid)
        .bind(nonce)
        .bind(self.attestation_key.version)
        .bind(&mac)
        .fetch_one(&mut **transaction)
        .await
        .map_err(repository_error)?;
        if opened != attestation_id {
            return Err(IdrRuntimeErrorV1::IntegrityViolation);
        }
        Ok(attestation_id)
    }

    fn verify_attestation_row(
        &self,
        row: &sqlx::postgres::PgRow,
    ) -> Result<Uuid, IdrRuntimeErrorV1> {
        let attestation_id: Uuid = row.try_get("attestation_id").map_err(repository_error)?;
        let row_domain: String = row.try_get("trust_domain").map_err(repository_error)?;
        let environment_ref: String = row.try_get("environment_ref").map_err(repository_error)?;
        let purpose: String = row.try_get("purpose").map_err(repository_error)?;
        let command_id: Uuid = row.try_get("command_id").map_err(repository_error)?;
        let run_id: Uuid = row.try_get("run_id").map_err(repository_error)?;
        let tenant_ref: String = row.try_get("tenant_ref").map_err(repository_error)?;
        let pre_version: i64 = row
            .try_get("pre_aggregate_version")
            .map_err(repository_error)?;
        let post_version: i64 = row
            .try_get("post_aggregate_version")
            .map_err(repository_error)?;
        let command_digest: String = row.try_get("command_digest").map_err(repository_error)?;
        let transition_digest: String =
            row.try_get("transition_digest").map_err(repository_error)?;
        let database_txid: i64 = row.try_get("database_txid").map_err(repository_error)?;
        let backend_pid: i32 = row.try_get("backend_pid").map_err(repository_error)?;
        let nonce: Uuid = row.try_get("nonce").map_err(repository_error)?;
        let key_version: i64 = row.try_get("key_version").map_err(repository_error)?;
        let stored_mac: String = row.try_get("mac").map_err(repository_error)?;
        let pre_version =
            u64::try_from(pre_version).map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?;
        let post_version =
            u64::try_from(post_version).map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?;
        if row_domain != trust_domain_name(self.trust_domain)?
            || environment_ref != self.security.environment_ref.as_str()
            || key_version != self.attestation_key.version
            || post_version < pre_version
        {
            return Err(IdrRuntimeErrorV1::IntegrityViolation);
        }
        let material = orchestrator_attestation_material_v1(
            &row_domain,
            &environment_ref,
            &purpose,
            attestation_id,
            command_id,
            run_id,
            &tenant_ref,
            pre_version,
            post_version,
            &command_digest,
            &transition_digest,
            database_txid,
            backend_pid,
            nonce,
            key_version,
        )?;
        if self.attestation_key.mac(&material) != stored_mac {
            return Err(IdrRuntimeErrorV1::IntegrityViolation);
        }
        Ok(attestation_id)
    }

    fn command_receipt_mac(
        &self,
        receipt: &IdrCommandReceiptV1,
    ) -> Result<String, IdrRuntimeErrorV1> {
        if receipt.idempotent_replay {
            return Err(IdrRuntimeErrorV1::IntegrityViolation);
        }
        let receipt_digest =
            idr_protocol::production::canonical_digest_v1("idr-command-receipt-v1", receipt)
                .map_err(IdrRuntimeErrorV1::from)?;
        Ok(self.attestation_key.mac(receipt_digest.as_bytes()))
    }

    async fn verify_all_orchestrator_attestations(&self) -> Result<(), IdrRuntimeErrorV1> {
        let rows = sqlx::query(
            "SELECT attestation_id, trust_domain, environment_ref, purpose, command_id,
                    run_id, tenant_ref, pre_aggregate_version, post_aggregate_version,
                    command_digest, transition_digest, database_txid, backend_pid,
                    nonce, key_version, mac
             FROM idr_transition_attestations_v7
             ORDER BY attested_at, attestation_id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(repository_error)?;
        for row in &rows {
            self.verify_attestation_row(row)?;
        }
        let receipt_rows = sqlx::query(
            "SELECT receipt, receipt_mac, idr_attestation_id
               FROM idr_command_receipts
              ORDER BY command_id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(repository_error)?;
        for row in receipt_rows {
            let receipt: IdrCommandReceiptV1 =
                serde_json::from_value(row.try_get("receipt").map_err(repository_error)?)
                    .map_err(|_| IdrRuntimeErrorV1::Serialization)?;
            let stored_mac: String = row.try_get("receipt_mac").map_err(repository_error)?;
            let attestation_id: Uuid = row
                .try_get("idr_attestation_id")
                .map_err(repository_error)?;
            if receipt.attestation_id != attestation_id
                || self.command_receipt_mac(&receipt)? != stored_mac
            {
                return Err(IdrRuntimeErrorV1::IntegrityViolation);
            }
        }
        let forged_receipt: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                SELECT 1
                  FROM idr_command_receipts receipt
                  JOIN idr_transition_attestations_v7 attestation
                    ON attestation.attestation_id = receipt.idr_attestation_id
                  LEFT JOIN idr_run_events run_event
                    ON run_event.global_sequence = receipt.event_sequence
                   AND run_event.run_id = receipt.run_id
                  LEFT JOIN idr_audit_events audit_event
                    ON audit_event.event_id = run_event.event_id
                   AND audit_event.run_id = receipt.run_id
                  LEFT JOIN idr_audit_checkpoints checkpoint
                    ON checkpoint.audit_sequence = audit_event.sequence
                   AND checkpoint.tenant_ref = receipt.tenant_ref
                 WHERE receipt.command_id <> attestation.command_id
                    OR receipt.run_id <> attestation.run_id
                    OR receipt.tenant_ref <> attestation.tenant_ref
                    OR receipt.command_digest <> attestation.command_digest
                    OR receipt.receipt #>> '{attestation_id}'
                         IS DISTINCT FROM receipt.idr_attestation_id::text
                    OR run_event.event_id IS NULL
                    OR run_event.aggregate_version <> receipt.aggregate_version
                    OR run_event.idr_attestation_id <> receipt.idr_attestation_id
                    OR audit_event.event_id IS NULL
                    OR audit_event.idr_attestation_id <> receipt.idr_attestation_id
                    OR audit_event.payload #>> '{command_id}'
                         IS DISTINCT FROM receipt.command_id::text
                    OR audit_event.payload #>> '{command_digest}'
                         IS DISTINCT FROM receipt.command_digest
                    OR checkpoint.checkpoint_id IS NULL
             )",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(repository_error)?;
        if forged_receipt {
            return Err(IdrRuntimeErrorV1::IntegrityViolation);
        }
        Ok(())
    }

    async fn transact_inner(
        &self,
        authority: &OrchestratorAuthorityV1,
        command: &IdrCommandEnvelopeV1,
    ) -> Result<(IdrCommandReceiptV1, AuditCheckpointV1), IdrRuntimeErrorV1> {
        let mut transaction = self.pool.begin().await.map_err(repository_error)?;
        transaction
            .execute("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
            .await
            .map_err(repository_error)?;

        let trusted_now: i64 =
            sqlx::query_scalar("SELECT EXTRACT(EPOCH FROM clock_timestamp())::bigint")
                .fetch_one(&mut *transaction)
                .await
                .map_err(repository_error)?;
        let trusted_now = u64::try_from(trusted_now).map_err(|_| {
            IdrRuntimeErrorV1::Repository("database trusted time is invalid".to_string())
        })?;
        let trust_root = self.security.trust_root_provider.current_snapshot()?;
        if trust_root.trust_domain() != self.trust_domain
            || trust_root.environment_ref() != &self.security.environment_ref
        {
            return Err(IdrRuntimeErrorV1::BindingMismatch);
        }
        let command_digest = command.canonical_digest()?;
        if let Some(row) = sqlx::query(
            "SELECT trust_domain, environment_ref, tenant_ref, run_id, actor_ref, caller_ref,
                    command_digest, receipt, receipt_mac, idr_attestation_id
                 FROM idr_command_receipts WHERE command_id = $1",
        )
        .bind(command.command_id())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(repository_error)?
        {
            let same_command = row
                .try_get::<String, _>("trust_domain")
                .map_err(repository_error)?
                == trust_domain_name(self.trust_domain)?
                && row
                    .try_get::<String, _>("environment_ref")
                    .map_err(repository_error)?
                    == self.security.environment_ref.as_str()
                && row
                    .try_get::<String, _>("tenant_ref")
                    .map_err(repository_error)?
                    == command.tenant_ref().as_str()
                && row.try_get::<Uuid, _>("run_id").map_err(repository_error)? == command.run_id()
                && row
                    .try_get::<String, _>("actor_ref")
                    .map_err(repository_error)?
                    == command.actor_ref().as_str()
                && row
                    .try_get::<String, _>("caller_ref")
                    .map_err(repository_error)?
                    == command.caller_ref().as_str()
                && row
                    .try_get::<String, _>("command_digest")
                    .map_err(repository_error)?
                    == command_digest;
            if !same_command {
                return Err(IdrRuntimeErrorV1::IdempotencyConflict);
            }
            // A matching idempotent Receipt replay is an authenticated read,
            // not a bypass. After rejecting command-ID conflicts, revalidate
            // the original exact proof set against current database time and
            // the current Trust Root before returning the stored Receipt.
            self.verify_proofs_inside_transaction(command, trusted_now, &trust_root)?;
            let mut receipt: IdrCommandReceiptV1 =
                serde_json::from_value(row.try_get("receipt").map_err(repository_error)?)
                    .map_err(|_| IdrRuntimeErrorV1::Serialization)?;
            let stored_attestation_id: Uuid = row
                .try_get("idr_attestation_id")
                .map_err(repository_error)?;
            if receipt.attestation_id != stored_attestation_id {
                return Err(IdrRuntimeErrorV1::IntegrityViolation);
            }
            let stored_receipt_mac: String =
                row.try_get("receipt_mac").map_err(repository_error)?;
            if self.command_receipt_mac(&receipt)? != stored_receipt_mac {
                return Err(IdrRuntimeErrorV1::IntegrityViolation);
            }
            let attestation_row = sqlx::query(
                "SELECT attestation_id, trust_domain, environment_ref, purpose, command_id,
                        run_id, tenant_ref, pre_aggregate_version, post_aggregate_version,
                        command_digest, transition_digest, database_txid, backend_pid,
                        nonce, key_version, mac
                   FROM idr_transition_attestations_v7
                  WHERE attestation_id = $1",
            )
            .bind(stored_attestation_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(repository_error)?;
            if self.verify_attestation_row(&attestation_row)? != stored_attestation_id {
                return Err(IdrRuntimeErrorV1::IntegrityViolation);
            }
            receipt.idempotent_replay = true;
            let checkpoint = latest_checkpoint_for_tenant(
                &mut transaction,
                self.trust_domain,
                self.security.environment_ref.as_str(),
                command.tenant_ref().as_str(),
            )
            .await?;
            transaction.commit().await.map_err(repository_error)?;
            return Ok((receipt, checkpoint));
        }

        let verified_proofs =
            self.verify_proofs_inside_transaction(command, trusted_now, &trust_root)?;
        let projection_row = sqlx::query(
            "SELECT trust_domain, environment_ref, projection, projection_digest
             FROM idr_runs WHERE run_id = $1 FOR UPDATE",
        )
        .bind(command.run_id())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(repository_error)?;
        let projection = match projection_row {
            Some(row) => {
                let row_domain: String = row.try_get("trust_domain").map_err(repository_error)?;
                let row_environment: String =
                    row.try_get("environment_ref").map_err(repository_error)?;
                let projection_value: Value =
                    row.try_get("projection").map_err(repository_error)?;
                let stored_digest: String =
                    row.try_get("projection_digest").map_err(repository_error)?;
                let computed_digest = projection_digest_v1(&projection_value)?;
                let audited_digest: Option<String> = sqlx::query_scalar(
                    "SELECT payload #>> '{projection_digest}'
                     FROM idr_audit_events
                     WHERE run_id = $1 ORDER BY sequence DESC LIMIT 1",
                )
                .bind(command.run_id())
                .fetch_optional(&mut *transaction)
                .await
                .map_err(repository_error)?;
                if row_domain != trust_domain_name(self.trust_domain)?
                    || row_environment != self.security.environment_ref.as_str()
                    || projection_value.get("trust_domain").and_then(Value::as_str)
                        != Some(row_domain.as_str())
                    || projection_value
                        .get("environment_ref")
                        .and_then(Value::as_str)
                        != Some(row_environment.as_str())
                    || stored_digest != computed_digest
                    || audited_digest.as_deref() != Some(stored_digest.as_str())
                {
                    return Err(IdrRuntimeErrorV1::IntegrityViolation);
                }
                serde_json::from_value(projection_value)
                    .map_err(|_| IdrRuntimeErrorV1::Serialization)?
            }
            None if matches!(command.command(), IdrCommandV1::StartRun { .. })
                && command.expected_aggregate_version() == 0 =>
            {
                IdrRunProjectionV1::pending(
                    self.trust_domain,
                    self.security.environment_ref.clone(),
                    command.run_id(),
                    command.tenant_ref().clone(),
                )
            }
            None => return Err(IdrRuntimeErrorV1::MissingDependency),
        };
        let pre_aggregate_version = projection.aggregate_version;

        let bound_admission_result = self
            .reverify_bound_action_admission_proofs(
                &mut transaction,
                &projection,
                command,
                trusted_now,
                &trust_root,
            )
            .await;
        check_proof_replay(
            &mut transaction,
            command,
            &verified_proofs,
            self.trust_domain,
        )
        .await?;
        let transition = match bound_admission_result {
            Ok(()) => {
                let superseded_record = command
                    .command()
                    .expected_candidate_kind()
                    .and_then(|kind| projection.records.get(&kind).cloned());
                let mut transition = evaluate_authoritative_transition_v1(
                    authority,
                    projection,
                    command,
                    trusted_now,
                    &verified_proofs,
                )?;
                if let IdrCommandV1::InvalidateRecord { record_ref, .. } = command.command() {
                    let closure =
                        load_recursive_invalidation_closure(&mut transaction, record_ref).await?;
                    transition.apply_repository_invalidation_closure(authority, closure, true);
                } else if let Some(previous_record) = superseded_record {
                    if transition.authoritative_record().is_some_and(|record| {
                        record.record_ref().candidate_kind() == previous_record.candidate_kind()
                            && record.record_ref() != &previous_record
                    }) {
                        let closure =
                            load_recursive_invalidation_closure(&mut transaction, &previous_record)
                                .await?;
                        transition.apply_repository_invalidation_closure(authority, closure, false);
                    }
                }
                transition
            }
            Err(IdrRuntimeErrorV1::ActionNotAdmitted) => {
                let failed_admission_ref = projection
                    .action_admission
                    .as_ref()
                    .map(|admission| admission.admission_ref.clone())
                    .ok_or(IdrRuntimeErrorV1::ActionNotAdmitted)?;
                let closure =
                    load_recursive_invalidation_closure(&mut transaction, &failed_admission_ref)
                        .await?;
                let mut transition = crate::IdrTransitionV1::action_admission_proof_failure(
                    authority,
                    projection,
                    command,
                    &verified_proofs,
                )?;
                transition.apply_repository_invalidation_closure(authority, closure, false);
                transition
            }
            Err(error) => return Err(error),
        };

        let projection_digest = projection_digest_v1(transition.projection())?;
        let transition_digest = idr_protocol::production::canonical_digest_v1(
            "idr-orchestrator-transition-attestation-v1",
            &json!({
                "trust_domain": trust_domain_name(self.trust_domain)?,
                "environment_ref": self.security.environment_ref.as_str(),
                "command_id": command.command_id(),
                "command_digest": command_digest,
                "run_id": command.run_id(),
                "tenant_ref": command.tenant_ref(),
                "pre_aggregate_version": pre_aggregate_version,
                "post_aggregate_version": transition.projection().aggregate_version,
                "projection_digest": projection_digest,
                "event_type": transition.event_type(),
                "record_ref": transition
                    .authoritative_record()
                    .map(AuthoritativeRecordV1::record_ref),
                "consumed_proof_ids": transition.consumed_proof_ids(),
            }),
        )
        .map_err(IdrRuntimeErrorV1::from)?;
        let attestation_purpose = if transition
            .event_type()
            .starts_with("action_admission_proof_revoked_")
        {
            "governance_proof_revocation"
        } else {
            "command_transition"
        };
        let attestation_id = self
            .open_orchestrator_attestation(
                &mut transaction,
                attestation_purpose,
                command.command_id(),
                command.run_id(),
                command.tenant_ref().as_str(),
                pre_aggregate_version,
                transition.projection().aggregate_version,
                &command_digest,
                &transition_digest,
            )
            .await?;
        upsert_run(
            &mut transaction,
            transition.projection(),
            &projection_digest,
        )
        .await?;
        persist_step_snapshot(&mut transaction, command, &transition).await?;
        persist_authoritative_record(
            &mut transaction,
            transition.authoritative_record(),
            transition.projection(),
        )
        .await?;
        persist_contract_dependencies(&mut transaction, command, &transition).await?;
        persist_proof_consumptions(
            &mut transaction,
            command,
            &verified_proofs,
            trust_root.root_version(),
            trust_root.root_digest(),
            trusted_now,
            self.trust_domain,
        )
        .await?;
        persist_specialized_projection(&mut transaction, command, &transition).await?;

        let previous_hash: String = sqlx::query_scalar(
            "SELECT event_hash FROM idr_audit_events
             WHERE tenant_ref = $1 ORDER BY sequence DESC LIMIT 1 FOR UPDATE",
        )
        .bind(command.tenant_ref().as_str())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(repository_error)?
        .unwrap_or_else(|| "0".repeat(64));
        let contract_refs = transition
            .authoritative_record()
            .map(|record| json!([record.record_ref()]))
            .unwrap_or_else(|| json!([]));
        let proof_refs = serde_json::to_value(transition.consumed_proof_ids())
            .map_err(|_| IdrRuntimeErrorV1::Serialization)?;
        let audit_payload = json!({
            "trust_domain": trust_domain_name(self.trust_domain)?,
            "environment_ref": self.security.environment_ref.as_str(),
            "event_type": transition.event_type(),
            "command_id": command.command_id(),
            "command_digest": command_digest,
            "run_id": command.run_id(),
            "tenant_ref": command.tenant_ref(),
            "aggregate_version": transition.projection().aggregate_version,
            "actor_ref": command.actor_ref(),
            "caller_ref": command.caller_ref(),
            "correlation_ref": command.correlation_ref(),
            "causation_ref": command.causation_ref(),
            "policy_revision_ref": command.policy_revision_ref(),
            "contract_refs": contract_refs,
            "proof_refs": proof_refs,
            "projection_digest": projection_digest,
            "event_payload": transition.event_payload(),
        });
        let event_hash = hash_event_v1(&previous_hash, &audit_payload)?;
        let event_id = Uuid::new_v4();
        let global_sequence: i64 = sqlx::query_scalar(
            "INSERT INTO idr_run_events (
                event_id, trust_domain, environment_ref, run_id, tenant_ref, run_event_sequence,
                aggregate_version, event_type, correlation_ref, causation_ref, actor_ref,
                caller_ref, event_payload, previous_hash, event_hash
             ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
             RETURNING global_sequence",
        )
        .bind(event_id)
        .bind(trust_domain_name(self.trust_domain)?)
        .bind(self.security.environment_ref.as_str())
        .bind(command.run_id())
        .bind(command.tenant_ref().as_str())
        .bind(i64_from_u64(transition.projection().last_event_sequence)?)
        .bind(i64_from_u64(transition.projection().aggregate_version)?)
        .bind(transition.event_type())
        .bind(command.correlation_ref().as_str())
        .bind(command.causation_ref().as_str())
        .bind(command.actor_ref().as_str())
        .bind(command.caller_ref().as_str())
        .bind(transition.event_payload())
        .bind(&previous_hash)
        .bind(&event_hash)
        .fetch_one(&mut *transaction)
        .await
        .map_err(repository_error)?;

        let audit_sequence: i64 = sqlx::query_scalar(
            "INSERT INTO idr_audit_events (
                event_id, trust_domain, environment_ref, tenant_ref, run_id, aggregate_version,
                actor_ref, caller_ref, correlation_ref, causation_ref, contract_refs, proof_refs,
                policy_revision_ref, previous_hash, event_hash, payload
             ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)
             RETURNING sequence",
        )
        .bind(event_id)
        .bind(trust_domain_name(self.trust_domain)?)
        .bind(self.security.environment_ref.as_str())
        .bind(command.tenant_ref().as_str())
        .bind(command.run_id())
        .bind(i64_from_u64(transition.projection().aggregate_version)?)
        .bind(command.actor_ref().as_str())
        .bind(command.caller_ref().as_str())
        .bind(command.correlation_ref().as_str())
        .bind(command.causation_ref().as_str())
        .bind(&audit_payload["contract_refs"])
        .bind(&audit_payload["proof_refs"])
        .bind(command.policy_revision_ref().as_str())
        .bind(&previous_hash)
        .bind(&event_hash)
        .bind(&audit_payload)
        .fetch_one(&mut *transaction)
        .await
        .map_err(repository_error)?;

        sqlx::query(
            "INSERT INTO idr_outbox_events (
                outbox_id, trust_domain, environment_ref, tenant_ref, run_id, aggregate_version,
                event_type, payload
             ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        )
        .bind(Uuid::new_v4())
        .bind(trust_domain_name(self.trust_domain)?)
        .bind(self.security.environment_ref.as_str())
        .bind(command.tenant_ref().as_str())
        .bind(command.run_id())
        .bind(i64_from_u64(transition.projection().aggregate_version)?)
        .bind(transition.event_type())
        .bind(&audit_payload)
        .execute(&mut *transaction)
        .await
        .map_err(repository_error)?;

        let receipt = IdrCommandReceiptV1 {
            command_id: command.command_id(),
            attestation_id,
            trust_domain: self.trust_domain,
            environment_ref: self.security.environment_ref.clone(),
            run_id: command.run_id(),
            aggregate_version: transition.projection().aggregate_version,
            event_sequence: u64::try_from(global_sequence)
                .map_err(|_| IdrRuntimeErrorV1::Serialization)?,
            state: transition.projection().state,
            record_ref: transition
                .authoritative_record()
                .map(|record| record.record_ref().clone()),
            idempotent_replay: false,
        };
        let receipt_mac = self.command_receipt_mac(&receipt)?;
        sqlx::query(
            "INSERT INTO idr_command_receipts (
                command_id, trust_domain, environment_ref, run_id, aggregate_version,
                event_sequence, receipt, receipt_mac, tenant_ref, actor_ref, caller_ref,
                command_digest
             ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
        )
        .bind(command.command_id())
        .bind(trust_domain_name(self.trust_domain)?)
        .bind(self.security.environment_ref.as_str())
        .bind(command.run_id())
        .bind(i64_from_u64(receipt.aggregate_version)?)
        .bind(global_sequence)
        .bind(serde_json::to_value(&receipt).map_err(|_| IdrRuntimeErrorV1::Serialization)?)
        .bind(receipt_mac)
        .bind(command.tenant_ref().as_str())
        .bind(command.actor_ref().as_str())
        .bind(command.caller_ref().as_str())
        .bind(&command_digest)
        .execute(&mut *transaction)
        .await
        .map_err(repository_error)?;

        let record_set_root =
            record_set_root_for_tenant(&mut transaction, command.tenant_ref().as_str()).await?;
        let execution_state_root =
            execution_state_root_for_tenant(&mut transaction, command.tenant_ref().as_str())
                .await?;
        let checkpoint = AuditCheckpointV1 {
            trust_domain: self.trust_domain,
            environment_ref: self.security.environment_ref.as_str().to_string(),
            tenant_ref: command.tenant_ref().as_str().to_string(),
            audit_sequence,
            chain_root: event_hash,
            record_set_root,
            execution_state_root,
            checkpoint_id: Uuid::new_v4(),
        };
        sqlx::query(
            "INSERT INTO idr_audit_checkpoints (
                checkpoint_id, trust_domain, environment_ref, tenant_ref, audit_sequence,
                chain_root, record_set_root, execution_state_root, signed_checkpoint
             ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(checkpoint.checkpoint_id)
        .bind(trust_domain_name(self.trust_domain)?)
        .bind(&checkpoint.environment_ref)
        .bind(&checkpoint.tenant_ref)
        .bind(checkpoint.audit_sequence)
        .bind(&checkpoint.chain_root)
        .bind(&checkpoint.record_set_root)
        .bind(&checkpoint.execution_state_root)
        .bind(serde_json::to_value(&checkpoint).map_err(|_| IdrRuntimeErrorV1::Serialization)?)
        .execute(&mut *transaction)
        .await
        .map_err(repository_error)?;

        transaction.commit().await.map_err(repository_error)?;
        Ok((receipt, checkpoint))
    }

    fn verify_proofs_inside_transaction(
        &self,
        command: &IdrCommandEnvelopeV1,
        trusted_now: u64,
        trust_root: &TrustRootSnapshotV1,
    ) -> Result<Vec<VerifiedProductionProofV1>, IdrRuntimeErrorV1> {
        let required = command.required_proof_kinds();
        let found: BTreeSet<_> = command
            .proofs()
            .iter()
            .map(|proof| proof.claims().proof_kind())
            .collect();
        if found != required || command.proofs().len() != required.len() {
            return Err(IdrRuntimeErrorV1::MissingRequiredProof);
        }
        let principal_digest = expected_principal_digest(command)?;
        if command.proofs().iter().any(|proof| {
            !proof_assertion_allows(proof.claims())
                || proof.claims().principal_digest() != principal_digest
        }) {
            return Err(IdrRuntimeErrorV1::BindingMismatch);
        }
        let (subject_ref, subject_digest) = command.proof_subject()?;
        command
            .proofs()
            .iter()
            .map(|proof| {
                let kind = proof.claims().proof_kind();
                let issuer_ref = self
                    .security
                    .expected_issuers
                    .get(&kind)
                    .cloned()
                    .ok_or(IdrRuntimeErrorV1::MissingRequiredProof)?;
                let expected = ExpectedProductionProofV1 {
                    trust_domain: self.trust_domain,
                    environment_ref: self.security.environment_ref.clone(),
                    proof_kind: kind,
                    issuer_ref,
                    subject_ref: subject_ref.clone(),
                    subject_digest: subject_digest.clone(),
                    audience_ref: self.security.audience_ref.clone(),
                    tenant_ref: command.tenant_ref().clone(),
                    scope_ref: command.scope_ref().clone(),
                    purpose_ref: command.purpose_ref().clone(),
                    policy_revision_ref: command.policy_revision_ref().clone(),
                };
                trust_root
                    .verify_at(proof, &expected, trusted_now)
                    .map_err(IdrRuntimeErrorV1::from)
            })
            .collect()
    }

    async fn reverify_bound_action_admission_proofs(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        projection: &IdrRunProjectionV1,
        command: &IdrCommandEnvelopeV1,
        trusted_now: u64,
        trust_root: &TrustRootSnapshotV1,
    ) -> Result<(), IdrRuntimeErrorV1> {
        if !matches!(
            command.command(),
            IdrCommandV1::ReserveExecution { .. }
                | IdrCommandV1::RecoverPermit { .. }
                | IdrCommandV1::DeliverPermit { .. }
                | IdrCommandV1::StartDispatch { .. }
        ) {
            return Ok(());
        }
        let admission = projection
            .action_admission
            .as_ref()
            .ok_or(IdrRuntimeErrorV1::ActionNotAdmitted)?;
        if admission.proof_bindings.len() != ACTION_ADMISSION_PROOF_KINDS_V1.len()
            || admission
                .proof_bindings
                .get(&ProductionProofKindV1::ExactAuthorization)
                .is_none_or(|binding| {
                    binding.proof_id != admission.exact_authorization_proof_id
                        || binding.valid_until != admission.authorization_valid_until
                })
        {
            return Err(IdrRuntimeErrorV1::ActionNotAdmitted);
        }
        for proof_kind in ACTION_ADMISSION_PROOF_KINDS_V1 {
            let binding = admission
                .proof_bindings
                .get(&proof_kind)
                .ok_or(IdrRuntimeErrorV1::ActionNotAdmitted)?;
            let envelope_value: Value = sqlx::query_scalar(
                "SELECT envelope FROM idr_proof_envelopes
                 WHERE proof_id = $1 AND trust_domain = $2 AND environment_ref = $3",
            )
            .bind(binding.proof_id)
            .bind(trust_domain_name(self.trust_domain)?)
            .bind(self.security.environment_ref.as_str())
            .fetch_optional(&mut **transaction)
            .await
            .map_err(repository_error)?
            .ok_or(IdrRuntimeErrorV1::ActionNotAdmitted)?;
            let envelope: ProductionProofEnvelopeV1 = serde_json::from_value(envelope_value)
                .map_err(|_| IdrRuntimeErrorV1::IntegrityViolation)?;
            let claims = envelope.claims();
            let issuer_ref = self
                .security
                .expected_issuers
                .get(&proof_kind)
                .cloned()
                .ok_or(IdrRuntimeErrorV1::MissingRequiredProof)?;
            if claims.proof_id() != binding.proof_id
                || claims.proof_kind() != proof_kind
                || claims.expires_at() != binding.valid_until
                || claims.tenant_ref() != command.tenant_ref()
                || claims.scope_ref() != command.scope_ref()
                || claims.purpose_ref() != command.purpose_ref()
                || claims.policy_revision_ref() != command.policy_revision_ref()
                || !proof_assertion_allows(claims)
            {
                return Err(IdrRuntimeErrorV1::ActionNotAdmitted);
            }
            trust_root
                .verify_at(
                    &envelope,
                    &ExpectedProductionProofV1 {
                        trust_domain: self.trust_domain,
                        environment_ref: self.security.environment_ref.clone(),
                        proof_kind,
                        issuer_ref,
                        subject_ref: claims.subject_ref().clone(),
                        subject_digest: claims.subject_digest().to_string(),
                        audience_ref: self.security.audience_ref.clone(),
                        tenant_ref: command.tenant_ref().clone(),
                        scope_ref: command.scope_ref().clone(),
                        purpose_ref: command.purpose_ref().clone(),
                        policy_revision_ref: command.policy_revision_ref().clone(),
                    },
                    trusted_now,
                )
                .map_err(|_| IdrRuntimeErrorV1::ActionNotAdmitted)?;
        }
        Ok(())
    }
}

impl PostgresIdrOrchestratorV1 {
    /// The only production mutation entry point. The PostgreSQL transaction,
    /// DB clock, current Trust Root, proof consumption, transition engine,
    /// audit and outbox are inseparable behind this concrete type.
    pub async fn handle(
        &self,
        command: IdrCommandEnvelopeV1,
    ) -> Result<IdrCommandReceiptV1, IdrRuntimeErrorV1> {
        #[cfg(feature = "production")]
        {
            if self.trust_domain == IdrTrustDomainV1::Production {
                enforce_production_command_release_gate(command.command())?;
            }
        }
        if is_human_model_write(command.command()) && !cfg!(feature = "human-model-write") {
            return Err(IdrRuntimeErrorV1::Repository(
                "HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED".to_string(),
            ));
        }
        command.validate()?;
        if command.trust_domain() != self.trust_domain {
            return Err(IdrRuntimeErrorV1::BindingMismatch);
        }
        if command.environment_ref() != &self.security.environment_ref {
            return Err(IdrRuntimeErrorV1::BindingMismatch);
        }
        let (receipt, checkpoint) = self.transact_inner(&self.authority, &command).await?;
        if receipt.idempotent_replay {
            return Ok(receipt);
        }
        self.security.anchor_backend.publish(&checkpoint).await?;
        let publication_command_digest = idr_protocol::production::canonical_digest_v1(
            "idr-checkpoint-publication-command-v1",
            &json!({
                "command_id": receipt.command_id,
                "attestation_id": receipt.attestation_id,
                "checkpoint": checkpoint,
            }),
        )
        .map_err(IdrRuntimeErrorV1::from)?;
        let publication_transition_digest = idr_protocol::production::canonical_digest_v1(
            "idr-checkpoint-publication-transition-v1",
            &json!({
                "checkpoint_id": checkpoint.checkpoint_id,
                "audit_sequence": checkpoint.audit_sequence,
                "chain_root": checkpoint.chain_root,
                "record_set_root": checkpoint.record_set_root,
                "execution_state_root": checkpoint.execution_state_root,
            }),
        )
        .map_err(IdrRuntimeErrorV1::from)?;
        let mut publication_transaction = self.pool.begin().await.map_err(repository_error)?;
        publication_transaction
            .execute("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
            .await
            .map_err(repository_error)?;
        self.open_orchestrator_attestation(
            &mut publication_transaction,
            "checkpoint_publication",
            receipt.command_id,
            ZERO_RUN_ID_V1,
            &checkpoint.tenant_ref,
            receipt.aggregate_version,
            receipt.aggregate_version,
            &publication_command_digest,
            &publication_transition_digest,
        )
        .await?;
        sqlx::query(
            "UPDATE idr_audit_checkpoints
             SET published_at = clock_timestamp()
             WHERE checkpoint_id = $1",
        )
        .bind(checkpoint.checkpoint_id)
        .execute(&mut *publication_transaction)
        .await
        .map_err(repository_error)?;
        publication_transaction
            .commit()
            .await
            .map_err(repository_error)?;
        Ok(receipt)
    }

    #[cfg(all(feature = "shadow-mode", feature = "test-support"))]
    pub async fn inspect_for_test(
        &self,
        run_id: Uuid,
    ) -> Result<Option<TestRunProjectionV1>, IdrRuntimeErrorV1> {
        let value: Option<Value> =
            sqlx::query_scalar("SELECT projection FROM idr_runs WHERE run_id = $1")
                .bind(run_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(repository_error)?;
        value
            .map(|value| {
                let projection: IdrRunProjectionV1 =
                    serde_json::from_value(value).map_err(|_| IdrRuntimeErrorV1::Serialization)?;
                Ok(TestRunProjectionV1 {
                    aggregate_version: projection.aggregate_version,
                    state: projection.state,
                    execution: projection
                        .execution
                        .map(|execution| TestExecutionProjectionV1 {
                            reservation_id: execution.reservation_id,
                            permit_id: execution.permit_id,
                            dispatch_nonce: execution.dispatch_nonce,
                            state: execution.state,
                        }),
                })
            })
            .transpose()
    }

    #[cfg(all(feature = "shadow-mode", feature = "test-support"))]
    pub async fn verify_external_anchor_for_test(&self) -> Result<(), IdrRuntimeErrorV1> {
        self.verify_external_anchor().await
    }
}

fn proof_assertion_allows(claims: &idr_protocol::production::ProductionProofClaimsV1) -> bool {
    use ProductionProofKindV1::*;
    let expected = match claims.proof_kind() {
        CallerAuthentication => ("outcome", "authenticate"),
        ExactAuthorization => ("decision", "approve"),
        InputAdmission => ("outcome", "admit"),
        ContextSnapshot => ("outcome", "attest"),
        IntentFastPathAdmission | DecisionNecessityAdmission | TurnCoordinationAdmission => {
            ("outcome", "admit")
        }
        ResponsePolicy | ResponseAdmission | Authority | Capability | Policy | ActionAdmission => {
            ("outcome", "allow")
        }
        ExecutionPermit => ("outcome", "issue"),
        ProviderReceipt => ("outcome", "attest"),
        OutcomeObservation => ("outcome", "observe"),
        HumanModelPromotion => ("outcome", "promote"),
        HumanModelUserConfirmation => ("outcome", "confirm"),
        HumanModelCorrection => ("outcome", "correct"),
    };
    claims.assertion().get(expected.0).and_then(Value::as_str) == Some(expected.1)
}

fn is_human_model_write(command: &IdrCommandV1) -> bool {
    matches!(
        command,
        IdrCommandV1::ProposeHumanModelCandidate { .. }
            | IdrCommandV1::RecordHumanModelPromotion { .. }
            | IdrCommandV1::PromoteHumanModelAssertion { .. }
            | IdrCommandV1::CorrectHumanModelAssertion { .. }
    )
}

#[cfg(feature = "production")]
fn enforce_production_startup_release_gates() -> Result<(), IdrRuntimeErrorV1> {
    let gates = ProductionReleaseGatesV1::BLOCKED;
    let mut blocked = Vec::new();
    if !gates.trust_chain_closure_is_complete() {
        blocked.push("TRUST CHAIN CLOSURE = INCOMPLETE");
    }
    if gates.production_trust_root_is_blocked() {
        blocked.push("PRODUCTION TRUST ROOT = BLOCKED");
    }
    if !gates.production_authorization_is_enabled() {
        blocked.push("PRODUCTION AUTHORIZATION = NOT AUTHORIZED");
    }
    if !gates.production_execution_is_enabled() {
        blocked.push("PRODUCTION EXECUTION = NOT AUTHORIZED");
    }
    if !gates.human_model_long_term_write_is_enabled() {
        blocked.push("HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED");
    }
    if blocked.is_empty() {
        Ok(())
    } else {
        Err(IdrRuntimeErrorV1::Repository(blocked.join("; ")))
    }
}

#[cfg(feature = "production")]
fn enforce_production_command_release_gate(
    command: &IdrCommandV1,
) -> Result<(), IdrRuntimeErrorV1> {
    let gates = ProductionReleaseGatesV1::BLOCKED;
    if matches!(command, IdrCommandV1::AdmitAction { .. })
        && !gates.production_authorization_is_enabled()
    {
        return Err(IdrRuntimeErrorV1::Repository(
            "PRODUCTION AUTHORIZATION = NOT AUTHORIZED".to_string(),
        ));
    }
    if matches!(
        command,
        IdrCommandV1::ReserveExecution { .. }
            | IdrCommandV1::RecoverPermit { .. }
            | IdrCommandV1::DeliverPermit { .. }
            | IdrCommandV1::StartDispatch { .. }
            | IdrCommandV1::CommitExecutionReceipt { .. }
            | IdrCommandV1::ReconcileExecution { .. }
            | IdrCommandV1::ExpireExecutionLease { .. }
    ) && !gates.production_execution_is_enabled()
    {
        return Err(IdrRuntimeErrorV1::Repository(
            "PRODUCTION EXECUTION = NOT AUTHORIZED".to_string(),
        ));
    }
    if is_human_model_write(command) && !gates.human_model_long_term_write_is_enabled() {
        return Err(IdrRuntimeErrorV1::Repository(
            "HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED".to_string(),
        ));
    }
    Ok(())
}

fn expected_principal_digest(command: &IdrCommandEnvelopeV1) -> Result<String, IdrRuntimeErrorV1> {
    idr_protocol::production::canonical_digest_v1(
        "idr-command-principal-v1",
        &(command.actor_ref(), command.caller_ref()),
    )
    .map_err(IdrRuntimeErrorV1::from)
}

fn projection_digest_v1<T: Serialize + ?Sized>(
    projection: &T,
) -> Result<String, IdrRuntimeErrorV1> {
    idr_protocol::production::canonical_digest_v1("idr-run-projection-v1", projection)
        .map_err(Into::into)
}

fn trust_domain_name(domain: IdrTrustDomainV1) -> Result<String, IdrRuntimeErrorV1> {
    enum_name(&domain)
}

async fn check_proof_replay(
    transaction: &mut Transaction<'_, Postgres>,
    command: &IdrCommandEnvelopeV1,
    proofs: &[VerifiedProductionProofV1],
    trust_domain: IdrTrustDomainV1,
) -> Result<(), IdrRuntimeErrorV1> {
    for proof in proofs {
        let replay: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                SELECT 1 FROM idr_proof_consumptions
                WHERE trust_domain = $4 AND environment_ref = $5
                  AND (proof_id = $1 OR (tenant_ref = $2 AND nonce = $3))
             )",
        )
        .bind(proof.proof_id())
        .bind(command.tenant_ref().as_str())
        .bind(proof.nonce())
        .bind(trust_domain_name(trust_domain)?)
        .bind(command.environment_ref().as_str())
        .fetch_one(&mut **transaction)
        .await
        .map_err(repository_error)?;
        if replay {
            return Err(IdrRuntimeErrorV1::ProofReplay);
        }
    }
    Ok(())
}

async fn upsert_run(
    transaction: &mut Transaction<'_, Postgres>,
    projection: &IdrRunProjectionV1,
    projection_digest: &str,
) -> Result<(), IdrRuntimeErrorV1> {
    let projection_json =
        serde_json::to_value(projection).map_err(|_| IdrRuntimeErrorV1::Serialization)?;
    let result = sqlx::query(
        "INSERT INTO idr_runs (
            run_id, trust_domain, environment_ref, tenant_ref, aggregate_version, state,
            projection, projection_digest, last_event_sequence
         ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
    )
    .bind(projection.run_id)
    .bind(trust_domain_name(projection.trust_domain)?)
    .bind(projection.environment_ref.as_str())
    .bind(projection.tenant_ref.as_str())
    .bind(i64_from_u64(projection.aggregate_version)?)
    .bind(enum_name(&projection.state)?)
    .bind(projection_json)
    .bind(projection_digest)
    .bind(i64_from_u64(projection.last_event_sequence)?)
    .execute(&mut **transaction)
    .await
    .map_err(repository_error)?;
    if result.rows_affected() != 1 {
        return Err(IdrRuntimeErrorV1::AggregateVersionConflict);
    }
    Ok(())
}

async fn persist_step_snapshot(
    transaction: &mut Transaction<'_, Postgres>,
    command: &IdrCommandEnvelopeV1,
    transition: &crate::IdrTransitionV1,
) -> Result<(), IdrRuntimeErrorV1> {
    let step_state = match transition.projection().state {
        crate::IdrRunStateV1::WaitingInput
        | crate::IdrRunStateV1::WaitingAuthorization
        | crate::IdrRunStateV1::WaitingDependency
        | crate::IdrRunStateV1::ReconciliationRequired => "waiting",
        crate::IdrRunStateV1::Failed
        | crate::IdrRunStateV1::Rejected
        | crate::IdrRunStateV1::TimedOut => "failed",
        crate::IdrRunStateV1::Cancelled | crate::IdrRunStateV1::Invalidated => "cancelled",
        crate::IdrRunStateV1::Pending
        | crate::IdrRunStateV1::Running
        | crate::IdrRunStateV1::Succeeded => {
            if matches!(
                transition.event_type(),
                "execution_permit_issued"
                    | "execution_permit_delivered"
                    | "execution_dispatch_started"
            ) {
                "waiting"
            } else {
                "completed"
            }
        }
    };
    sqlx::query(
        "INSERT INTO idr_step_snapshots (
            run_id, trust_domain, environment_ref, step_id, step_kind, step_state,
            aggregate_version, snapshot
         ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
    )
    .bind(command.run_id())
    .bind(trust_domain_name(transition.projection().trust_domain)?)
    .bind(transition.projection().environment_ref.as_str())
    .bind(command.command_id())
    .bind(transition.event_type())
    .bind(step_state)
    .bind(i64_from_u64(transition.projection().aggregate_version)?)
    .bind(
        serde_json::to_value(transition.projection())
            .map_err(|_| IdrRuntimeErrorV1::Serialization)?,
    )
    .execute(&mut **transaction)
    .await
    .map_err(repository_error)?;
    Ok(())
}

async fn persist_authoritative_record(
    transaction: &mut Transaction<'_, Postgres>,
    record: Option<&AuthoritativeRecordV1>,
    projection: &IdrRunProjectionV1,
) -> Result<(), IdrRuntimeErrorV1> {
    let Some(record) = record else {
        return Ok(());
    };
    let record_json = serde_json::to_value(record).map_err(|_| IdrRuntimeErrorV1::Serialization)?;
    let issued_by_command_id = record_json
        .get("issued_by_command_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or(IdrRuntimeErrorV1::Serialization)?;
    let issued_at = record_json
        .get("issued_at")
        .and_then(Value::as_u64)
        .ok_or(IdrRuntimeErrorV1::Serialization)?;
    sqlx::query(
        "INSERT INTO idr_contract_records (
            record_id, revision, candidate_kind, trust_domain, environment_ref, run_id,
            tenant_ref, subject_ref, record_digest, record, issued_by_command_id,
            issued_at, valid_from, valid_until
         ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,to_timestamp($12),
                   to_timestamp($13),to_timestamp($14))",
    )
    .bind(record.record_ref().record_id())
    .bind(i64_from_u64(record.record_ref().revision())?)
    .bind(enum_name(&record.record_ref().candidate_kind())?)
    .bind(trust_domain_name(record.trust_domain())?)
    .bind(record.environment_ref().as_str())
    .bind(record.run_id())
    .bind(record.tenant_ref().as_str())
    .bind(record.subject_ref().as_str())
    .bind(record.record_ref().record_digest())
    .bind(&record_json)
    .bind(issued_by_command_id)
    .bind(i64_from_u64(issued_at)?)
    .bind(i64_from_u64(record.valid_from())?)
    .bind(i64_from_u64(record.valid_until())?)
    .execute(&mut **transaction)
    .await
    .map_err(repository_error)?;
    let result = sqlx::query(
        "INSERT INTO idr_contract_current (
            run_id, trust_domain, environment_ref, candidate_kind, record_id, revision,
            record_digest
         ) VALUES ($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(projection.run_id)
    .bind(trust_domain_name(projection.trust_domain)?)
    .bind(projection.environment_ref.as_str())
    .bind(enum_name(&record.record_ref().candidate_kind())?)
    .bind(record.record_ref().record_id())
    .bind(i64_from_u64(record.record_ref().revision())?)
    .bind(record.record_ref().record_digest())
    .execute(&mut **transaction)
    .await
    .map_err(repository_error)?;
    if result.rows_affected() != 1 {
        return Err(IdrRuntimeErrorV1::AggregateVersionConflict);
    }
    Ok(())
}

async fn persist_contract_dependencies(
    transaction: &mut Transaction<'_, Postgres>,
    command: &IdrCommandEnvelopeV1,
    transition: &crate::IdrTransitionV1,
) -> Result<(), IdrRuntimeErrorV1> {
    let Some(record) = transition.authoritative_record() else {
        return Ok(());
    };
    let mut dependencies = BTreeSet::new();
    let records = &transition.projection().records;
    let mut add_current = |kind| {
        if let Some(reference) = records.get(&kind) {
            if reference != record.record_ref() {
                dependencies.insert(reference.clone());
            }
        }
    };
    match command.command() {
        IdrCommandV1::StartRun { .. } => {}
        IdrCommandV1::RecordContext { .. } => add_current(CandidateKindV1::CanonicalInput),
        IdrCommandV1::RecordIntent { .. } => add_current(CandidateKindV1::ContextSnapshot),
        IdrCommandV1::RecordDecision { .. } => add_current(CandidateKindV1::Intent),
        IdrCommandV1::RecordTurn { .. } => {
            add_current(CandidateKindV1::Intent);
            add_current(CandidateKindV1::Decision);
        }
        IdrCommandV1::RecordResponse { .. } => {
            add_current(CandidateKindV1::TurnCoordination);
        }
        IdrCommandV1::RecordAction { .. } => {
            add_current(CandidateKindV1::TurnCoordination);
            if records.contains_key(&CandidateKindV1::Decision) {
                add_current(CandidateKindV1::Decision);
            } else {
                add_current(CandidateKindV1::Intent);
            }
        }
        IdrCommandV1::AdmitAction { action_ref, .. } => {
            dependencies.insert(action_ref.clone());
        }
        IdrCommandV1::CommitExecutionReceipt { .. } => add_current(CandidateKindV1::Action),
        IdrCommandV1::RecordOutcome { .. } => {
            add_current(CandidateKindV1::ExecutionReceipt);
        }
        IdrCommandV1::ProposeHumanModelCandidate { .. } => add_current(CandidateKindV1::Outcome),
        IdrCommandV1::RecordHumanModelPromotion {
            source_candidate_ref,
            ..
        } => {
            dependencies.insert(source_candidate_ref.clone());
        }
        IdrCommandV1::PromoteHumanModelAssertion {
            promotion_decision_ref,
            ..
        } => {
            dependencies.insert(promotion_decision_ref.clone());
        }
        IdrCommandV1::CorrectHumanModelAssertion { assertion_ref, .. } => {
            dependencies.insert(assertion_ref.clone());
        }
        _ => {}
    }

    if record.record_ref().revision() > 1 {
        let previous_revision = i64_from_u64(record.record_ref().revision() - 1)?;
        let result = sqlx::query(
            "INSERT INTO idr_contract_dependencies (
                dependent_record_id, dependent_revision, dependency_record_id,
                dependency_revision, dependency_digest
             )
             SELECT $1,$2,record_id,revision,record_digest
             FROM idr_contract_records
             WHERE record_id = $1 AND revision = $3",
        )
        .bind(record.record_ref().record_id())
        .bind(i64_from_u64(record.record_ref().revision())?)
        .bind(previous_revision)
        .execute(&mut **transaction)
        .await
        .map_err(repository_error)?;
        if result.rows_affected() != 1 {
            return Err(IdrRuntimeErrorV1::MissingDependency);
        }
    }

    for dependency in dependencies {
        sqlx::query(
            "INSERT INTO idr_contract_dependencies (
                dependent_record_id, dependent_revision, dependency_record_id,
                dependency_revision, dependency_digest
             ) VALUES ($1,$2,$3,$4,$5)",
        )
        .bind(record.record_ref().record_id())
        .bind(i64_from_u64(record.record_ref().revision())?)
        .bind(dependency.record_id())
        .bind(i64_from_u64(dependency.revision())?)
        .bind(dependency.record_digest())
        .execute(&mut **transaction)
        .await
        .map_err(repository_error)?;
    }
    Ok(())
}

async fn load_recursive_invalidation_closure(
    transaction: &mut Transaction<'_, Postgres>,
    root: &crate::AuthoritativeRecordRefV1,
) -> Result<
    Vec<(
        Uuid,
        u64,
        CandidateKindV1,
        IdrTrustDomainV1,
        ReferenceV1,
        u64,
        u64,
        String,
    )>,
    IdrRuntimeErrorV1,
> {
    let rows = sqlx::query(
        "WITH RECURSIVE invalidation_closure(record_id, revision) AS (
            SELECT $1::uuid, $2::bigint
            UNION
            SELECT dependency.dependent_record_id, dependency.dependent_revision
            FROM idr_contract_dependencies dependency
            JOIN invalidation_closure current_record
              ON dependency.dependency_record_id = current_record.record_id
             AND dependency.dependency_revision = current_record.revision
         )
         SELECT record.record_id, record.revision, record.candidate_kind,
                record.trust_domain, record.environment_ref,
                EXTRACT(EPOCH FROM record.valid_from)::bigint AS valid_from,
                EXTRACT(EPOCH FROM record.valid_until)::bigint AS valid_until,
                record.record_digest
         FROM invalidation_closure closure
         JOIN idr_contract_records record
           ON record.record_id = closure.record_id
          AND record.revision = closure.revision
         ORDER BY record.issued_at, record.record_id, record.revision",
    )
    .bind(root.record_id())
    .bind(i64_from_u64(root.revision())?)
    .fetch_all(&mut **transaction)
    .await
    .map_err(repository_error)?;
    rows.into_iter()
        .map(|row| {
            let kind: String = row.try_get("candidate_kind").map_err(repository_error)?;
            let kind = serde_json::from_value(Value::String(kind))
                .map_err(|_| IdrRuntimeErrorV1::Serialization)?;
            let revision: i64 = row.try_get("revision").map_err(repository_error)?;
            let domain: String = row.try_get("trust_domain").map_err(repository_error)?;
            let domain = serde_json::from_value(Value::String(domain))
                .map_err(|_| IdrRuntimeErrorV1::Serialization)?;
            let valid_from: i64 = row.try_get("valid_from").map_err(repository_error)?;
            let valid_until: i64 = row.try_get("valid_until").map_err(repository_error)?;
            Ok((
                row.try_get("record_id").map_err(repository_error)?,
                u64::try_from(revision).map_err(|_| IdrRuntimeErrorV1::Serialization)?,
                kind,
                domain,
                ReferenceV1::new(
                    row.try_get::<String, _>("environment_ref")
                        .map_err(repository_error)?,
                )?,
                u64::try_from(valid_from).map_err(|_| IdrRuntimeErrorV1::Serialization)?,
                u64::try_from(valid_until).map_err(|_| IdrRuntimeErrorV1::Serialization)?,
                row.try_get("record_digest").map_err(repository_error)?,
            ))
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
async fn persist_proof_consumptions(
    transaction: &mut Transaction<'_, Postgres>,
    command: &IdrCommandEnvelopeV1,
    verified: &[VerifiedProductionProofV1],
    root_version: u64,
    root_digest: &str,
    trusted_now: u64,
    trust_domain: IdrTrustDomainV1,
) -> Result<(), IdrRuntimeErrorV1> {
    for (envelope, proof) in command.proofs().iter().zip(verified) {
        let claims = envelope.claims();
        sqlx::query(
            "INSERT INTO idr_proof_envelopes (
                proof_id, trust_domain, environment_ref, proof_kind, issuer_ref, key_id,
                tenant_ref, subject_ref, subject_digest, nonce, trust_root_version,
                trust_root_digest, envelope, verified_at
             ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,to_timestamp($14))",
        )
        .bind(proof.proof_id())
        .bind(trust_domain_name(trust_domain)?)
        .bind(command.environment_ref().as_str())
        .bind(enum_name(&proof.proof_kind())?)
        .bind(claims.issuer_ref().as_str())
        .bind(envelope.key_id().as_str())
        .bind(claims.tenant_ref().as_str())
        .bind(claims.subject_ref().as_str())
        .bind(claims.subject_digest())
        .bind(proof.nonce())
        .bind(i64_from_u64(root_version)?)
        .bind(root_digest)
        .bind(serde_json::to_value(envelope).map_err(|_| IdrRuntimeErrorV1::Serialization)?)
        .bind(i64_from_u64(trusted_now)?)
        .execute(&mut **transaction)
        .await
        .map_err(repository_error)?;
        sqlx::query(
            "INSERT INTO idr_proof_consumptions (
                proof_id, trust_domain, environment_ref, tenant_ref, nonce, command_id, run_id
             ) VALUES ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(proof.proof_id())
        .bind(trust_domain_name(trust_domain)?)
        .bind(command.environment_ref().as_str())
        .bind(command.tenant_ref().as_str())
        .bind(proof.nonce())
        .bind(command.command_id())
        .bind(command.run_id())
        .execute(&mut **transaction)
        .await
        .map_err(repository_error)?;
    }
    Ok(())
}

async fn persist_specialized_projection(
    transaction: &mut Transaction<'_, Postgres>,
    command: &IdrCommandEnvelopeV1,
    transition: &crate::IdrTransitionV1,
) -> Result<(), IdrRuntimeErrorV1> {
    if let IdrCommandV1::ConsumeResponseSend {
        response_ref,
        rendered_bytes,
        channel_ref,
        audience_ref,
        send_nonce,
    } = command.command()
    {
        sqlx::query(
            "INSERT INTO idr_response_send_consumptions (
                tenant_ref, response_record_id, response_revision, send_nonce,
                command_id, channel_ref, audience_ref, rendered_content_digest
             ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        )
        .bind(command.tenant_ref().as_str())
        .bind(response_ref.record_id())
        .bind(i64_from_u64(response_ref.revision())?)
        .bind(send_nonce)
        .bind(command.command_id())
        .bind(channel_ref.as_str())
        .bind(audience_ref.as_str())
        .bind(format!("{:x}", Sha256::digest(rendered_bytes)))
        .execute(&mut **transaction)
        .await
        .map_err(repository_error)?;
    }

    let invalidation_reason = match command.command() {
        IdrCommandV1::InvalidateRecord { reason_ref, .. } => reason_ref.as_str(),
        _ if transition
            .event_type()
            .starts_with("action_admission_proof_revoked_") =>
        {
            "reason:bound-action-admission-proof-revoked"
        }
        _ => "reason:automatic-successor-invalidation",
    };
    for record_ref in &transition.projection().invalidated_records {
        sqlx::query(
            "INSERT INTO idr_contract_invalidations (
                invalidation_id, run_id, record_id, revision, reason_ref, command_id
             ) VALUES ($1,$2,$3,$4,$5,$6)",
        )
        .bind(Uuid::new_v4())
        .bind(command.run_id())
        .bind(record_ref.record_id())
        .bind(i64_from_u64(record_ref.revision())?)
        .bind(invalidation_reason)
        .bind(command.command_id())
        .execute(&mut **transaction)
        .await
        .map_err(repository_error)?;
    }

    if let Some(execution) = &transition.projection().execution {
        let fence_digest = idempotency_fence_digest_v1(transition.projection(), execution)?;
        sqlx::query(
            "INSERT INTO idr_operation_idempotency_fences (
                fence_id, trust_domain, environment_ref, tenant_ref,
                operation_ref, idempotency_key, first_run_id,
                first_action_record_id, first_action_revision,
                first_action_digest, fence_digest
             ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
        )
        .bind(execution.reservation_id)
        .bind(trust_domain_name(transition.projection().trust_domain)?)
        .bind(transition.projection().environment_ref.as_str())
        .bind(transition.projection().tenant_ref.as_str())
        .bind(execution.operation_ref.as_str())
        .bind(&execution.idempotency_key)
        .bind(transition.projection().run_id)
        .bind(execution.action_ref.record_id())
        .bind(i64_from_u64(execution.action_ref.revision())?)
        .bind(execution.action_ref.record_digest())
        .bind(&fence_digest)
        .execute(&mut **transaction)
        .await
        .map_err(repository_error)?;
        let persisted_fence: (
            String,
            String,
            String,
            String,
            Uuid,
            Uuid,
            i64,
            String,
            String,
        ) = sqlx::query_as(
            "SELECT trust_domain, environment_ref, tenant_ref, operation_ref,
                        first_run_id, first_action_record_id, first_action_revision,
                        first_action_digest, fence_digest
                 FROM idr_operation_idempotency_fences
                 WHERE fence_id = $1 AND idempotency_key = $2",
        )
        .bind(execution.reservation_id)
        .bind(&execution.idempotency_key)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(repository_error)?
        .ok_or(IdrRuntimeErrorV1::IntegrityViolation)?;
        if persisted_fence
            != (
                trust_domain_name(transition.projection().trust_domain)?.to_string(),
                transition.projection().environment_ref.as_str().to_string(),
                transition.projection().tenant_ref.as_str().to_string(),
                execution.operation_ref.as_str().to_string(),
                transition.projection().run_id,
                execution.action_ref.record_id(),
                i64_from_u64(execution.action_ref.revision())?,
                execution.action_ref.record_digest().to_string(),
                fence_digest,
            )
        {
            return Err(IdrRuntimeErrorV1::IntegrityViolation);
        }
        sqlx::query(
            "INSERT INTO idr_execution_reservations (
                reservation_id, fence_id, run_id, tenant_ref, action_record_id, action_revision,
                action_admission_record_id, action_admission_revision, operation_ref,
                idempotency_key, provider_ref, owner_ref, state, aggregate_version,
                action_digest, parameter_digest, request_digest,
                exact_authorization_proof_id, action_valid_until,
                admission_valid_until, authorization_valid_until
             ) VALUES ($1,$1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,
                       $17,to_timestamp($18),to_timestamp($19),to_timestamp($20))",
        )
        .bind(execution.reservation_id)
        .bind(transition.projection().run_id)
        .bind(transition.projection().tenant_ref.as_str())
        .bind(execution.action_ref.record_id())
        .bind(i64_from_u64(execution.action_ref.revision())?)
        .bind(execution.action_admission_ref.record_id())
        .bind(i64_from_u64(execution.action_admission_ref.revision())?)
        .bind(execution.operation_ref.as_str())
        .bind(&execution.idempotency_key)
        .bind(execution.provider_ref.as_str())
        .bind(execution.owner_ref.as_str())
        .bind(enum_name(&execution.state)?)
        .bind(i64_from_u64(transition.projection().aggregate_version)?)
        .bind(execution.action_ref.record_digest())
        .bind(&execution.parameter_digest)
        .bind(&execution.request_digest)
        .bind(execution.exact_authorization_proof_id)
        .bind(i64_from_u64(execution.action_valid_until)?)
        .bind(i64_from_u64(execution.admission_valid_until)?)
        .bind(i64_from_u64(execution.authorization_valid_until)?)
        .execute(&mut **transaction)
        .await
        .map_err(repository_error)?;
        sqlx::query(
            "INSERT INTO idr_execution_attempts (
                reservation_id, attempt, permit_id, dispatch_nonce, lease_until,
                permit_valid_until, state, provider_ref, owner_ref, started_at, completed_at
             ) VALUES ($1,$2,$3,$4,to_timestamp($5),to_timestamp($6),$7,$8,$9,
                CASE WHEN $7 IN ('dispatch_started','awaiting_provider',
                    'reconciliation_required','succeeded','failed','rejected',
                    'compensated') THEN clock_timestamp() ELSE NULL END,
                CASE WHEN $7 IN ('succeeded','failed','rejected','compensated')
                    THEN clock_timestamp() ELSE NULL END)",
        )
        .bind(execution.reservation_id)
        .bind(i32::try_from(execution.attempt).map_err(|_| IdrRuntimeErrorV1::Serialization)?)
        .bind(execution.permit_id)
        .bind(&execution.dispatch_nonce)
        .bind(i64_from_u64(execution.lease_until)?)
        .bind(i64_from_u64(execution.permit_valid_until)?)
        .bind(enum_name(&execution.state)?)
        .bind(execution.provider_ref.as_str())
        .bind(execution.owner_ref.as_str())
        .execute(&mut **transaction)
        .await
        .map_err(repository_error)?;
    }

    let authoritative_record = transition.authoritative_record();
    match command.command() {
        IdrCommandV1::CommitExecutionReceipt {
            reservation_id,
            permit_id,
            provider_ref,
            dispatch_nonce,
            attempt,
            ..
        } => {
            let record = authoritative_record.ok_or(IdrRuntimeErrorV1::Serialization)?;
            sqlx::query(
                "INSERT INTO idr_execution_receipts (
                    receipt_record_id, receipt_revision, reservation_id, attempt,
                    permit_id, provider_ref, dispatch_nonce, receipt
                 ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
            )
            .bind(record.record_ref().record_id())
            .bind(i64_from_u64(record.record_ref().revision())?)
            .bind(reservation_id)
            .bind(i32::try_from(*attempt).map_err(|_| IdrRuntimeErrorV1::Serialization)?)
            .bind(permit_id)
            .bind(provider_ref.as_str())
            .bind(dispatch_nonce)
            .bind(serde_json::to_value(record).map_err(|_| IdrRuntimeErrorV1::Serialization)?)
            .execute(&mut **transaction)
            .await
            .map_err(repository_error)?;
        }
        IdrCommandV1::RecordOutcome { .. } => {
            let record = authoritative_record.ok_or(IdrRuntimeErrorV1::Serialization)?;
            let receipt_ref = transition
                .projection()
                .records
                .get(&CandidateKindV1::ExecutionReceipt)
                .ok_or(IdrRuntimeErrorV1::MissingDependency)?;
            let proof_id = command_proof_id(command, ProductionProofKindV1::OutcomeObservation)?;
            sqlx::query(
                "INSERT INTO idr_outcome_records (
                    outcome_record_id, outcome_revision, run_id,
                    execution_receipt_record_id, execution_receipt_revision,
                    observation_proof_id, outcome
                 ) VALUES ($1,$2,$3,$4,$5,$6,$7)",
            )
            .bind(record.record_ref().record_id())
            .bind(i64_from_u64(record.record_ref().revision())?)
            .bind(command.run_id())
            .bind(receipt_ref.record_id())
            .bind(i64_from_u64(receipt_ref.revision())?)
            .bind(proof_id)
            .bind(serde_json::to_value(record).map_err(|_| IdrRuntimeErrorV1::Serialization)?)
            .execute(&mut **transaction)
            .await
            .map_err(repository_error)?;
        }
        IdrCommandV1::ProposeHumanModelCandidate { .. } => {
            let record = authoritative_record.ok_or(IdrRuntimeErrorV1::Serialization)?;
            sqlx::query(
                "INSERT INTO idr_human_model_candidates (
                    candidate_record_id, candidate_revision, run_id, tenant_ref,
                    subject_ref, candidate_digest, candidate
                 ) VALUES ($1,$2,$3,$4,$5,$6,$7)",
            )
            .bind(record.record_ref().record_id())
            .bind(i64_from_u64(record.record_ref().revision())?)
            .bind(record.run_id())
            .bind(record.tenant_ref().as_str())
            .bind(record.subject_ref().as_str())
            .bind(record.candidate_digest())
            .bind(serde_json::to_value(record).map_err(|_| IdrRuntimeErrorV1::Serialization)?)
            .execute(&mut **transaction)
            .await
            .map_err(repository_error)?;
        }
        IdrCommandV1::RecordHumanModelPromotion {
            source_candidate_ref,
            ..
        } => {
            let record = authoritative_record.ok_or(IdrRuntimeErrorV1::Serialization)?;
            let proof_id = command_proof_id(command, ProductionProofKindV1::HumanModelPromotion)?;
            let outcome = payload_string(record.payload(), "outcome")?;
            sqlx::query(
                "INSERT INTO idr_human_model_promotion_decisions (
                    decision_record_id, decision_revision, source_candidate_record_id,
                    source_candidate_revision, promotion_proof_id, candidate_digest,
                    outcome, decision
                 ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
            )
            .bind(record.record_ref().record_id())
            .bind(i64_from_u64(record.record_ref().revision())?)
            .bind(source_candidate_ref.record_id())
            .bind(i64_from_u64(source_candidate_ref.revision())?)
            .bind(proof_id)
            .bind(source_candidate_ref.record_digest())
            .bind(outcome)
            .bind(serde_json::to_value(record).map_err(|_| IdrRuntimeErrorV1::Serialization)?)
            .execute(&mut **transaction)
            .await
            .map_err(repository_error)?;
        }
        IdrCommandV1::PromoteHumanModelAssertion {
            promotion_decision_ref,
            ..
        } => {
            let record = authoritative_record.ok_or(IdrRuntimeErrorV1::Serialization)?;
            let lifecycle = payload_string(record.payload(), "lifecycle_state")?;
            let expires_at = record.payload().get("expires_at").and_then(Value::as_u64);
            sqlx::query(
                "INSERT INTO idr_human_model_assertions (
                    assertion_record_id, assertion_revision,
                    promotion_decision_record_id, promotion_decision_revision,
                    tenant_ref, subject_ref, lifecycle_state, expires_at, assertion
                 ) VALUES ($1,$2,$3,$4,$5,$6,$7,
                    CASE WHEN $8::bigint IS NULL THEN NULL ELSE to_timestamp($8) END,$9)",
            )
            .bind(record.record_ref().record_id())
            .bind(i64_from_u64(record.record_ref().revision())?)
            .bind(promotion_decision_ref.record_id())
            .bind(i64_from_u64(promotion_decision_ref.revision())?)
            .bind(record.tenant_ref().as_str())
            .bind(record.subject_ref().as_str())
            .bind(lifecycle)
            .bind(expires_at.map(i64_from_u64).transpose()?)
            .bind(serde_json::to_value(record).map_err(|_| IdrRuntimeErrorV1::Serialization)?)
            .execute(&mut **transaction)
            .await
            .map_err(repository_error)?;
        }
        IdrCommandV1::CorrectHumanModelAssertion { assertion_ref, .. } => {
            let record = authoritative_record.ok_or(IdrRuntimeErrorV1::Serialization)?;
            let lifecycle = payload_string(record.payload(), "lifecycle_state")?;
            let proof_id = command_proof_id(command, ProductionProofKindV1::HumanModelCorrection)?;
            let expires_at = record.payload().get("expires_at").and_then(Value::as_u64);
            sqlx::query(
                "INSERT INTO idr_human_model_assertions (
                    assertion_record_id, assertion_revision,
                    predecessor_assertion_record_id, predecessor_assertion_revision,
                    correction_proof_id, tenant_ref, subject_ref, lifecycle_state,
                    expires_at, assertion
                 ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,
                    CASE WHEN $9::bigint IS NULL THEN NULL ELSE to_timestamp($9) END,$10)",
            )
            .bind(record.record_ref().record_id())
            .bind(i64_from_u64(record.record_ref().revision())?)
            .bind(assertion_ref.record_id())
            .bind(i64_from_u64(assertion_ref.revision())?)
            .bind(proof_id)
            .bind(record.tenant_ref().as_str())
            .bind(record.subject_ref().as_str())
            .bind(lifecycle)
            .bind(expires_at.map(i64_from_u64).transpose()?)
            .bind(serde_json::to_value(record).map_err(|_| IdrRuntimeErrorV1::Serialization)?)
            .execute(&mut **transaction)
            .await
            .map_err(repository_error)?;
        }
        _ => {}
    }
    Ok(())
}

fn command_proof_id(
    command: &IdrCommandEnvelopeV1,
    proof_kind: ProductionProofKindV1,
) -> Result<Uuid, IdrRuntimeErrorV1> {
    command
        .proofs()
        .iter()
        .find(|proof| proof.claims().proof_kind() == proof_kind)
        .map(|proof| proof.claims().proof_id())
        .ok_or(IdrRuntimeErrorV1::MissingRequiredProof)
}

fn payload_string<'a>(payload: &'a Value, field: &str) -> Result<&'a str, IdrRuntimeErrorV1> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(IdrRuntimeErrorV1::BindingMismatch)
}

async fn latest_checkpoint_for_tenant(
    transaction: &mut Transaction<'_, Postgres>,
    trust_domain: IdrTrustDomainV1,
    environment_ref: &str,
    tenant_ref: &str,
) -> Result<AuditCheckpointV1, IdrRuntimeErrorV1> {
    let row = sqlx::query(
        "SELECT checkpoint_id, trust_domain, environment_ref, tenant_ref, audit_sequence,
                chain_root, record_set_root, execution_state_root
         FROM idr_audit_checkpoints
         WHERE trust_domain = $1 AND environment_ref = $2 AND tenant_ref = $3
         ORDER BY audit_sequence DESC LIMIT 1",
    )
    .bind(trust_domain_name(trust_domain)?)
    .bind(environment_ref)
    .bind(tenant_ref)
    .fetch_one(&mut **transaction)
    .await
    .map_err(repository_error)?;
    Ok(AuditCheckpointV1 {
        checkpoint_id: row.try_get("checkpoint_id").map_err(repository_error)?,
        trust_domain,
        environment_ref: row.try_get("environment_ref").map_err(repository_error)?,
        tenant_ref: row.try_get("tenant_ref").map_err(repository_error)?,
        audit_sequence: row.try_get("audit_sequence").map_err(repository_error)?,
        chain_root: row.try_get("chain_root").map_err(repository_error)?,
        record_set_root: row.try_get("record_set_root").map_err(repository_error)?,
        execution_state_root: row
            .try_get("execution_state_root")
            .map_err(repository_error)?,
    })
}

async fn record_set_root_for_tenant(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_ref: &str,
) -> Result<String, IdrRuntimeErrorV1> {
    let rows = sqlx::query(
        "SELECT record_id, revision, record_digest
         FROM idr_contract_records
         WHERE tenant_ref = $1
         ORDER BY record_id, revision",
    )
    .bind(tenant_ref)
    .fetch_all(&mut **transaction)
    .await
    .map_err(repository_error)?;
    let leaves: Vec<(Uuid, i64, String)> = rows
        .into_iter()
        .map(|row| {
            Ok((
                row.try_get("record_id").map_err(repository_error)?,
                row.try_get("revision").map_err(repository_error)?,
                row.try_get("record_digest").map_err(repository_error)?,
            ))
        })
        .collect::<Result<_, IdrRuntimeErrorV1>>()?;
    idr_protocol::production::canonical_digest_v1("idr-authoritative-record-set-v1", &leaves)
        .map_err(Into::into)
}

async fn record_set_root_for_pool(
    pool: &PgPool,
    tenant_ref: &str,
) -> Result<String, IdrRuntimeErrorV1> {
    let rows = sqlx::query(
        "SELECT record_id, revision, record_digest
         FROM idr_contract_records
         WHERE tenant_ref = $1
         ORDER BY record_id, revision",
    )
    .bind(tenant_ref)
    .fetch_all(pool)
    .await
    .map_err(repository_error)?;
    let leaves: Vec<(Uuid, i64, String)> = rows
        .into_iter()
        .map(|row| {
            Ok((
                row.try_get("record_id").map_err(repository_error)?,
                row.try_get("revision").map_err(repository_error)?,
                row.try_get("record_digest").map_err(repository_error)?,
            ))
        })
        .collect::<Result<_, IdrRuntimeErrorV1>>()?;
    idr_protocol::production::canonical_digest_v1("idr-authoritative-record-set-v1", &leaves)
        .map_err(Into::into)
}

fn idempotency_fence_digest_v1(
    projection: &IdrRunProjectionV1,
    execution: &crate::ExecutionProjectionV1,
) -> Result<String, IdrRuntimeErrorV1> {
    idr_protocol::production::canonical_digest_v1(
        "idr-operation-idempotency-fence-v1",
        &(
            trust_domain_name(projection.trust_domain)?,
            projection.environment_ref.as_str(),
            projection.tenant_ref.as_str(),
            execution.operation_ref.as_str(),
            execution.idempotency_key.as_str(),
            projection.run_id,
            execution.action_ref.record_id(),
            execution.action_ref.revision(),
            execution.action_ref.record_digest(),
        ),
    )
    .map_err(Into::into)
}

const IDEMPOTENCY_FENCE_ROOT_QUERY_V1: &str = "
    SELECT jsonb_build_object(
        'fence_id', fence_id,
        'trust_domain', trust_domain,
        'environment_ref', environment_ref,
        'tenant_ref', tenant_ref,
        'operation_ref', operation_ref,
        'idempotency_key', idempotency_key,
        'first_run_id', first_run_id,
        'first_action_record_id', first_action_record_id,
        'first_action_revision', first_action_revision,
        'first_action_digest', first_action_digest,
        'fence_digest', fence_digest,
        'created_at_micros', (EXTRACT(EPOCH FROM created_at) * 1000000)::bigint
    ) AS leaf
    FROM idr_operation_idempotency_fences
    WHERE tenant_ref = $1
    ORDER BY fence_id";

const EXECUTION_RESERVATION_ROOT_QUERY_V1: &str = "
    SELECT jsonb_build_object(
        'reservation_id', reservation_id,
        'fence_id', fence_id,
        'run_id', run_id,
        'tenant_ref', tenant_ref,
        'action_record_id', action_record_id,
        'action_revision', action_revision,
        'action_admission_record_id', action_admission_record_id,
        'action_admission_revision', action_admission_revision,
        'operation_ref', operation_ref,
        'idempotency_key', idempotency_key,
        'provider_ref', provider_ref,
        'owner_ref', owner_ref,
        'state', state,
        'aggregate_version', aggregate_version,
        'action_digest', action_digest,
        'parameter_digest', parameter_digest,
        'request_digest', request_digest,
        'exact_authorization_proof_id', exact_authorization_proof_id,
        'action_valid_until', EXTRACT(EPOCH FROM action_valid_until)::bigint,
        'admission_valid_until', EXTRACT(EPOCH FROM admission_valid_until)::bigint,
        'authorization_valid_until', EXTRACT(EPOCH FROM authorization_valid_until)::bigint,
        'created_at_micros', (EXTRACT(EPOCH FROM created_at) * 1000000)::bigint,
        'updated_at_micros', (EXTRACT(EPOCH FROM updated_at) * 1000000)::bigint
    ) AS leaf
    FROM idr_execution_reservations
    WHERE tenant_ref = $1
    ORDER BY reservation_id";

const EXECUTION_ATTEMPT_ROOT_QUERY_V1: &str = "
    SELECT jsonb_build_object(
        'reservation_id', attempt.reservation_id,
        'attempt', attempt.attempt,
        'permit_id', attempt.permit_id,
        'dispatch_nonce', attempt.dispatch_nonce,
        'lease_until', EXTRACT(EPOCH FROM attempt.lease_until)::bigint,
        'permit_valid_until', EXTRACT(EPOCH FROM attempt.permit_valid_until)::bigint,
        'state', attempt.state,
        'provider_ref', attempt.provider_ref,
        'owner_ref', attempt.owner_ref,
        'started_at_micros', CASE WHEN attempt.started_at IS NULL THEN NULL
            ELSE (EXTRACT(EPOCH FROM attempt.started_at) * 1000000)::bigint END,
        'completed_at_micros', CASE WHEN attempt.completed_at IS NULL THEN NULL
            ELSE (EXTRACT(EPOCH FROM attempt.completed_at) * 1000000)::bigint END
    ) AS leaf
    FROM idr_execution_attempts attempt
    JOIN idr_execution_reservations reservation
      ON reservation.reservation_id = attempt.reservation_id
    WHERE reservation.tenant_ref = $1
    ORDER BY attempt.reservation_id, attempt.attempt";

fn execution_state_root_from_rows(
    fence_rows: Vec<sqlx::postgres::PgRow>,
    reservation_rows: Vec<sqlx::postgres::PgRow>,
    attempt_rows: Vec<sqlx::postgres::PgRow>,
) -> Result<String, IdrRuntimeErrorV1> {
    let fences = fence_rows
        .into_iter()
        .map(|row| row.try_get::<Value, _>("leaf").map_err(repository_error))
        .collect::<Result<Vec<_>, _>>()?;
    let reservations = reservation_rows
        .into_iter()
        .map(|row| row.try_get::<Value, _>("leaf").map_err(repository_error))
        .collect::<Result<Vec<_>, _>>()?;
    let attempts = attempt_rows
        .into_iter()
        .map(|row| row.try_get::<Value, _>("leaf").map_err(repository_error))
        .collect::<Result<Vec<_>, _>>()?;
    idr_protocol::production::canonical_digest_v1(
        "idr-execution-state-set-v1",
        &(fences, reservations, attempts),
    )
    .map_err(Into::into)
}

async fn execution_state_root_for_tenant(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_ref: &str,
) -> Result<String, IdrRuntimeErrorV1> {
    let fence_rows = sqlx::query(IDEMPOTENCY_FENCE_ROOT_QUERY_V1)
        .bind(tenant_ref)
        .fetch_all(&mut **transaction)
        .await
        .map_err(repository_error)?;
    let reservation_rows = sqlx::query(EXECUTION_RESERVATION_ROOT_QUERY_V1)
        .bind(tenant_ref)
        .fetch_all(&mut **transaction)
        .await
        .map_err(repository_error)?;
    let attempt_rows = sqlx::query(EXECUTION_ATTEMPT_ROOT_QUERY_V1)
        .bind(tenant_ref)
        .fetch_all(&mut **transaction)
        .await
        .map_err(repository_error)?;
    execution_state_root_from_rows(fence_rows, reservation_rows, attempt_rows)
}

async fn execution_state_root_for_pool(
    pool: &PgPool,
    tenant_ref: &str,
) -> Result<String, IdrRuntimeErrorV1> {
    let fence_rows = sqlx::query(IDEMPOTENCY_FENCE_ROOT_QUERY_V1)
        .bind(tenant_ref)
        .fetch_all(pool)
        .await
        .map_err(repository_error)?;
    let reservation_rows = sqlx::query(EXECUTION_RESERVATION_ROOT_QUERY_V1)
        .bind(tenant_ref)
        .fetch_all(pool)
        .await
        .map_err(repository_error)?;
    let attempt_rows = sqlx::query(EXECUTION_ATTEMPT_ROOT_QUERY_V1)
        .bind(tenant_ref)
        .fetch_all(pool)
        .await
        .map_err(repository_error)?;
    execution_state_root_from_rows(fence_rows, reservation_rows, attempt_rows)
}

async fn connect_pool_for_schema(
    database_url: &str,
    schema: &str,
) -> Result<PgPool, IdrRuntimeErrorV1> {
    let connect_options = PgConnectOptions::from_str(database_url)
        .map_err(repository_error)?
        .disable_statement_logging();
    let search_path = format!("{schema},pg_catalog,pg_temp");
    PgPoolOptions::new()
        .max_connections(10)
        .after_connect(move |connection, _metadata| {
            let search_path = search_path.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path', $1, false)")
                    .bind(search_path)
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect_with(connect_options)
        .await
        .map_err(repository_error)
}

#[cfg(any(feature = "shadow-mode", feature = "migration"))]
async fn provision_orchestrator_attestation_key(
    pool: &PgPool,
    key: &OrchestratorAttestationKeyV1,
) -> Result<(), IdrRuntimeErrorV1> {
    sqlx::query(
        "INSERT INTO idr_orchestrator_attestation_secrets_v7 (
            key_version, key_digest, secret
         ) VALUES ($1,$2,$3)
         ON CONFLICT (key_version) DO NOTHING",
    )
    .bind(key.version)
    .bind(key.digest())
    .bind(key.bytes.as_slice())
    .execute(pool)
    .await
    .map_err(repository_error)?;
    let stored_digest: String = sqlx::query_scalar(
        "SELECT key_digest
           FROM idr_orchestrator_attestation_secrets_v7
          WHERE key_version = $1 AND retired_at IS NULL",
    )
    .bind(key.version)
    .fetch_one(pool)
    .await
    .map_err(repository_error)?;
    if stored_digest != key.digest() {
        return Err(IdrRuntimeErrorV1::Repository(
            "Orchestrator attestation key does not match the provisioned schema".to_string(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn orchestrator_attestation_material_v1(
    trust_domain: &str,
    environment_ref: &str,
    purpose: &str,
    attestation_id: Uuid,
    command_id: Uuid,
    run_id: Uuid,
    tenant_ref: &str,
    pre_aggregate_version: u64,
    post_aggregate_version: u64,
    command_digest: &str,
    transition_digest: &str,
    database_txid: i64,
    backend_pid: i32,
    nonce: Uuid,
    key_version: i64,
) -> Result<Vec<u8>, IdrRuntimeErrorV1> {
    if command_digest.len() != 64
        || transition_digest.len() != 64
        || post_aggregate_version < pre_aggregate_version
    {
        return Err(IdrRuntimeErrorV1::Serialization);
    }
    Ok(format!(
        "IDR-ORCHESTRATOR-ATTESTATION-V1\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        trust_domain,
        hex_utf8(environment_ref),
        purpose,
        attestation_id,
        command_id,
        run_id,
        hex_utf8(tenant_ref),
        pre_aggregate_version,
        post_aggregate_version,
        command_digest,
        transition_digest,
        database_txid,
        backend_pid,
        nonce,
        key_version
    )
    .into_bytes())
}

fn hex_utf8(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

async fn verify_schema_version_for_pool(pool: &PgPool) -> Result<(), IdrRuntimeErrorV1> {
    let row = sqlx::query(
        "SELECT count(*) AS migration_count,
                COALESCE(max(version), 0) AS latest_version,
                COALESCE(bool_and(success), false) AS all_succeeded
         FROM _sqlx_migrations",
    )
    .fetch_one(pool)
    .await
    .map_err(|_| {
        IdrRuntimeErrorV1::Repository(
            "IDR schema is absent or unreadable; run the separate migrator first".to_string(),
        )
    })?;
    let migration_count: i64 = row.try_get("migration_count").map_err(repository_error)?;
    let latest_version: i64 = row.try_get("latest_version").map_err(repository_error)?;
    let all_succeeded: bool = row.try_get("all_succeeded").map_err(repository_error)?;
    if migration_count != EXPECTED_SCHEMA_MIGRATION_VERSION_V1
        || latest_version != EXPECTED_SCHEMA_MIGRATION_VERSION_V1
        || !all_succeeded
    {
        return Err(IdrRuntimeErrorV1::Repository(format!(
            "IDR schema version mismatch: expected {}, found count={} latest={} success={}",
            EXPECTED_SCHEMA_MIGRATION_VERSION_V1, migration_count, latest_version, all_succeeded
        )));
    }
    Ok(())
}

fn validate_schema_name(schema: &str) -> Result<(), IdrRuntimeErrorV1> {
    if schema.is_empty()
        || schema.len() > 63
        || !schema.bytes().enumerate().all(|(index, byte)| {
            byte == b'_' || byte.is_ascii_lowercase() || (index > 0 && byte.is_ascii_digit())
        })
    {
        return Err(IdrRuntimeErrorV1::Repository(
            "PostgreSQL schema name is not a safe identifier".to_string(),
        ));
    }
    Ok(())
}

fn audience_matches_trust_domain(
    audience_ref: &ReferenceV1,
    trust_domain: IdrTrustDomainV1,
    environment_ref: &ReferenceV1,
) -> bool {
    let domain = match trust_domain {
        IdrTrustDomainV1::Shadow => "shadow",
        IdrTrustDomainV1::Production => "production",
    };
    environment_ref
        .as_str()
        .strip_prefix(&format!("environment:idr:{domain}:"))
        .is_some_and(|environment| {
            !environment.is_empty()
                && audience_ref.as_str() == format!("audience:idr:{domain}:{environment}")
        })
}

fn i64_from_u64(value: u64) -> Result<i64, IdrRuntimeErrorV1> {
    i64::try_from(value).map_err(|_| IdrRuntimeErrorV1::Serialization)
}

fn enum_name<T: Serialize>(value: &T) -> Result<String, IdrRuntimeErrorV1> {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToString::to_string))
        .ok_or(IdrRuntimeErrorV1::Serialization)
}

fn repository_error(error: impl std::fmt::Display) -> IdrRuntimeErrorV1 {
    IdrRuntimeErrorV1::Repository(error.to_string())
}

#[cfg(all(test, feature = "production"))]
mod production_release_gate_tests {
    use super::*;

    #[test]
    fn production_startup_reports_every_independent_blocked_gate() {
        let error = enforce_production_startup_release_gates().unwrap_err();
        let message = error.to_string();
        assert!(message.contains("TRUST CHAIN CLOSURE = INCOMPLETE"));
        assert!(message.contains("PRODUCTION TRUST ROOT = BLOCKED"));
        assert!(message.contains("PRODUCTION AUTHORIZATION = NOT AUTHORIZED"));
        assert!(message.contains("PRODUCTION EXECUTION = NOT AUTHORIZED"));
        assert!(message.contains("HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED"));
    }
}
