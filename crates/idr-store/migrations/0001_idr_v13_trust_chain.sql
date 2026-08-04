-- IDR V1.3 Trust-Chain Closure authoritative PostgreSQL schema.
-- FileHumanCenteredContractStoreV1 is not represented here and remains
-- dev/test/shadow-only.

CREATE TABLE IF NOT EXISTS idr_runs (
    run_id uuid PRIMARY KEY,
    tenant_ref text NOT NULL,
    aggregate_version bigint NOT NULL CHECK (aggregate_version >= 0),
    state text NOT NULL,
    projection jsonb NOT NULL,
    last_event_sequence bigint NOT NULL DEFAULT 0 CHECK (last_event_sequence >= 0),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

CREATE TABLE IF NOT EXISTS idr_run_events (
    global_sequence bigserial PRIMARY KEY,
    event_id uuid NOT NULL UNIQUE,
    run_id uuid NOT NULL REFERENCES idr_runs(run_id),
    tenant_ref text NOT NULL,
    run_event_sequence bigint NOT NULL CHECK (run_event_sequence > 0),
    aggregate_version bigint NOT NULL CHECK (aggregate_version > 0),
    event_type text NOT NULL,
    correlation_ref text NOT NULL,
    causation_ref text NOT NULL,
    actor_ref text NOT NULL,
    caller_ref text NOT NULL,
    event_payload jsonb NOT NULL,
    previous_hash text NOT NULL CHECK (length(previous_hash) = 64),
    event_hash text NOT NULL CHECK (length(event_hash) = 64),
    db_timestamp timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (run_id, run_event_sequence),
    UNIQUE (run_id, aggregate_version)
);

CREATE TABLE IF NOT EXISTS idr_step_snapshots (
    run_id uuid NOT NULL REFERENCES idr_runs(run_id),
    step_id uuid NOT NULL,
    step_kind text NOT NULL,
    step_state text NOT NULL,
    aggregate_version bigint NOT NULL CHECK (aggregate_version > 0),
    snapshot jsonb NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (run_id, step_id)
);

CREATE TABLE IF NOT EXISTS idr_contract_records (
    record_id uuid NOT NULL,
    revision bigint NOT NULL CHECK (revision > 0),
    candidate_kind text NOT NULL,
    run_id uuid NOT NULL REFERENCES idr_runs(run_id),
    tenant_ref text NOT NULL,
    subject_ref text NOT NULL,
    record_digest text NOT NULL CHECK (length(record_digest) = 64),
    record jsonb NOT NULL,
    issued_by_command_id uuid NOT NULL,
    issued_at timestamptz NOT NULL,
    PRIMARY KEY (record_id, revision),
    UNIQUE (candidate_kind, record_id, revision),
    UNIQUE (record_digest)
);

CREATE TABLE IF NOT EXISTS idr_contract_current (
    run_id uuid NOT NULL REFERENCES idr_runs(run_id),
    candidate_kind text NOT NULL,
    record_id uuid NOT NULL,
    revision bigint NOT NULL,
    record_digest text NOT NULL CHECK (length(record_digest) = 64),
    PRIMARY KEY (run_id, candidate_kind),
    FOREIGN KEY (record_id, revision)
        REFERENCES idr_contract_records(record_id, revision)
);

CREATE TABLE IF NOT EXISTS idr_contract_dependencies (
    dependent_record_id uuid NOT NULL,
    dependent_revision bigint NOT NULL,
    dependency_record_id uuid NOT NULL,
    dependency_revision bigint NOT NULL,
    dependency_digest text NOT NULL CHECK (length(dependency_digest) = 64),
    PRIMARY KEY (
        dependent_record_id,
        dependent_revision,
        dependency_record_id,
        dependency_revision
    ),
    FOREIGN KEY (dependent_record_id, dependent_revision)
        REFERENCES idr_contract_records(record_id, revision),
    FOREIGN KEY (dependency_record_id, dependency_revision)
        REFERENCES idr_contract_records(record_id, revision),
    CHECK (
        dependent_record_id <> dependency_record_id
        OR dependent_revision > dependency_revision
    )
);

CREATE TABLE IF NOT EXISTS idr_contract_invalidations (
    invalidation_id uuid PRIMARY KEY,
    run_id uuid NOT NULL REFERENCES idr_runs(run_id),
    record_id uuid NOT NULL,
    revision bigint NOT NULL,
    reason_ref text NOT NULL,
    command_id uuid NOT NULL,
    invalidated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (record_id, revision),
    FOREIGN KEY (record_id, revision)
        REFERENCES idr_contract_records(record_id, revision)
);

CREATE TABLE IF NOT EXISTS idr_proof_envelopes (
    proof_id uuid PRIMARY KEY,
    proof_kind text NOT NULL,
    issuer_ref text NOT NULL,
    key_id text NOT NULL,
    tenant_ref text NOT NULL,
    subject_ref text NOT NULL,
    subject_digest text NOT NULL CHECK (length(subject_digest) = 64),
    nonce text NOT NULL CHECK (length(nonce) = 64),
    trust_root_version bigint NOT NULL CHECK (trust_root_version > 0),
    trust_root_digest text NOT NULL CHECK (length(trust_root_digest) = 64),
    envelope jsonb NOT NULL,
    verified_at timestamptz NOT NULL
);

CREATE TABLE IF NOT EXISTS idr_proof_consumptions (
    proof_id uuid PRIMARY KEY REFERENCES idr_proof_envelopes(proof_id),
    tenant_ref text NOT NULL,
    nonce text NOT NULL CHECK (length(nonce) = 64),
    command_id uuid NOT NULL,
    run_id uuid NOT NULL REFERENCES idr_runs(run_id),
    consumed_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (tenant_ref, nonce)
);

CREATE TABLE IF NOT EXISTS idr_response_send_consumptions (
    tenant_ref text NOT NULL,
    response_record_id uuid NOT NULL,
    response_revision bigint NOT NULL,
    send_nonce text NOT NULL CHECK (length(send_nonce) = 64),
    command_id uuid NOT NULL,
    channel_ref text NOT NULL,
    audience_ref text NOT NULL,
    rendered_content_digest text NOT NULL CHECK (length(rendered_content_digest) = 64),
    consumed_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_ref, send_nonce),
    FOREIGN KEY (response_record_id, response_revision)
        REFERENCES idr_contract_records(record_id, revision)
);

CREATE TABLE IF NOT EXISTS idr_execution_reservations (
    reservation_id uuid PRIMARY KEY,
    run_id uuid NOT NULL REFERENCES idr_runs(run_id),
    tenant_ref text NOT NULL,
    action_record_id uuid NOT NULL,
    action_revision bigint NOT NULL,
    action_admission_record_id uuid NOT NULL,
    action_admission_revision bigint NOT NULL,
    operation_ref text NOT NULL,
    idempotency_key text NOT NULL,
    provider_ref text NOT NULL,
    owner_ref text NOT NULL,
    state text NOT NULL,
    aggregate_version bigint NOT NULL CHECK (aggregate_version > 0),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (action_record_id, action_revision),
    UNIQUE (tenant_ref, operation_ref, idempotency_key),
    FOREIGN KEY (action_record_id, action_revision)
        REFERENCES idr_contract_records(record_id, revision),
    FOREIGN KEY (action_admission_record_id, action_admission_revision)
        REFERENCES idr_contract_records(record_id, revision)
);

CREATE TABLE IF NOT EXISTS idr_execution_attempts (
    reservation_id uuid NOT NULL REFERENCES idr_execution_reservations(reservation_id),
    attempt integer NOT NULL CHECK (attempt > 0),
    permit_id uuid NOT NULL UNIQUE,
    dispatch_nonce text NOT NULL CHECK (length(dispatch_nonce) = 64),
    lease_until timestamptz NOT NULL,
    state text NOT NULL,
    provider_ref text NOT NULL,
    owner_ref text NOT NULL,
    started_at timestamptz,
    completed_at timestamptz,
    PRIMARY KEY (reservation_id, attempt),
    UNIQUE (reservation_id, dispatch_nonce),
    CHECK (completed_at IS NULL OR started_at IS NOT NULL),
    CHECK (completed_at IS NULL OR completed_at >= started_at)
);

CREATE TABLE IF NOT EXISTS idr_execution_receipts (
    receipt_record_id uuid NOT NULL,
    receipt_revision bigint NOT NULL,
    reservation_id uuid NOT NULL,
    attempt integer NOT NULL,
    permit_id uuid NOT NULL,
    provider_ref text NOT NULL,
    dispatch_nonce text NOT NULL CHECK (length(dispatch_nonce) = 64),
    receipt jsonb NOT NULL,
    committed_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (receipt_record_id, receipt_revision),
    UNIQUE (reservation_id, attempt),
    FOREIGN KEY (receipt_record_id, receipt_revision)
        REFERENCES idr_contract_records(record_id, revision),
    FOREIGN KEY (reservation_id, attempt)
        REFERENCES idr_execution_attempts(reservation_id, attempt)
);

CREATE TABLE IF NOT EXISTS idr_outcome_records (
    outcome_record_id uuid NOT NULL,
    outcome_revision bigint NOT NULL,
    run_id uuid NOT NULL REFERENCES idr_runs(run_id),
    execution_receipt_record_id uuid NOT NULL,
    execution_receipt_revision bigint NOT NULL,
    observation_proof_id uuid NOT NULL REFERENCES idr_proof_envelopes(proof_id),
    outcome jsonb NOT NULL,
    PRIMARY KEY (outcome_record_id, outcome_revision),
    FOREIGN KEY (outcome_record_id, outcome_revision)
        REFERENCES idr_contract_records(record_id, revision),
    FOREIGN KEY (execution_receipt_record_id, execution_receipt_revision)
        REFERENCES idr_execution_receipts(receipt_record_id, receipt_revision)
);

CREATE TABLE IF NOT EXISTS idr_human_model_candidates (
    candidate_record_id uuid NOT NULL,
    candidate_revision bigint NOT NULL,
    run_id uuid NOT NULL REFERENCES idr_runs(run_id),
    tenant_ref text NOT NULL,
    subject_ref text NOT NULL,
    candidate_digest text NOT NULL CHECK (length(candidate_digest) = 64),
    candidate jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (candidate_record_id, candidate_revision),
    FOREIGN KEY (candidate_record_id, candidate_revision)
        REFERENCES idr_contract_records(record_id, revision)
);

CREATE TABLE IF NOT EXISTS idr_human_model_promotion_decisions (
    decision_record_id uuid NOT NULL,
    decision_revision bigint NOT NULL,
    source_candidate_record_id uuid NOT NULL,
    source_candidate_revision bigint NOT NULL,
    promotion_proof_id uuid NOT NULL REFERENCES idr_proof_envelopes(proof_id),
    candidate_digest text NOT NULL CHECK (length(candidate_digest) = 64),
    outcome text NOT NULL,
    decision jsonb NOT NULL,
    decided_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (decision_record_id, decision_revision),
    FOREIGN KEY (decision_record_id, decision_revision)
        REFERENCES idr_contract_records(record_id, revision),
    FOREIGN KEY (source_candidate_record_id, source_candidate_revision)
        REFERENCES idr_human_model_candidates(candidate_record_id, candidate_revision)
);

CREATE TABLE IF NOT EXISTS idr_human_model_assertions (
    assertion_record_id uuid NOT NULL,
    assertion_revision bigint NOT NULL,
    promotion_decision_record_id uuid,
    promotion_decision_revision bigint,
    predecessor_assertion_record_id uuid,
    predecessor_assertion_revision bigint,
    correction_proof_id uuid REFERENCES idr_proof_envelopes(proof_id),
    tenant_ref text NOT NULL,
    subject_ref text NOT NULL,
    lifecycle_state text NOT NULL,
    expires_at timestamptz,
    assertion jsonb NOT NULL,
    PRIMARY KEY (assertion_record_id, assertion_revision),
    FOREIGN KEY (assertion_record_id, assertion_revision)
        REFERENCES idr_contract_records(record_id, revision),
    FOREIGN KEY (promotion_decision_record_id, promotion_decision_revision)
        REFERENCES idr_human_model_promotion_decisions(decision_record_id, decision_revision),
    FOREIGN KEY (predecessor_assertion_record_id, predecessor_assertion_revision)
        REFERENCES idr_human_model_assertions(assertion_record_id, assertion_revision),
    CHECK (
        (
            promotion_decision_record_id IS NOT NULL
            AND promotion_decision_revision IS NOT NULL
            AND predecessor_assertion_record_id IS NULL
            AND predecessor_assertion_revision IS NULL
            AND correction_proof_id IS NULL
        )
        OR
        (
            promotion_decision_record_id IS NULL
            AND promotion_decision_revision IS NULL
            AND predecessor_assertion_record_id IS NOT NULL
            AND predecessor_assertion_revision IS NOT NULL
            AND correction_proof_id IS NOT NULL
        )
    )
);

CREATE TABLE IF NOT EXISTS idr_audit_events (
    sequence bigserial PRIMARY KEY,
    event_id uuid NOT NULL UNIQUE,
    tenant_ref text NOT NULL,
    run_id uuid NOT NULL REFERENCES idr_runs(run_id),
    aggregate_version bigint NOT NULL,
    actor_ref text NOT NULL,
    caller_ref text NOT NULL,
    correlation_ref text NOT NULL,
    causation_ref text NOT NULL,
    contract_refs jsonb NOT NULL,
    proof_refs jsonb NOT NULL,
    policy_revision_ref text NOT NULL,
    previous_hash text NOT NULL CHECK (length(previous_hash) = 64),
    event_hash text NOT NULL CHECK (length(event_hash) = 64),
    payload jsonb NOT NULL,
    db_timestamp timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (tenant_ref, sequence)
);

CREATE TABLE IF NOT EXISTS idr_outbox_events (
    outbox_id uuid PRIMARY KEY,
    tenant_ref text NOT NULL,
    run_id uuid NOT NULL REFERENCES idr_runs(run_id),
    aggregate_version bigint NOT NULL,
    event_type text NOT NULL,
    payload jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    available_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    claim_owner text,
    claimed_at timestamptz,
    claim_until timestamptz,
    delivered_at timestamptz,
    delivery_attempts integer NOT NULL DEFAULT 0 CHECK (delivery_attempts >= 0),
    CHECK (
        (claim_owner IS NULL AND claimed_at IS NULL AND claim_until IS NULL)
        OR
        (claim_owner IS NOT NULL AND claimed_at IS NOT NULL AND claim_until > claimed_at)
    ),
    CHECK (delivered_at IS NULL OR delivered_at >= created_at)
);

CREATE TABLE IF NOT EXISTS idr_audit_checkpoints (
    checkpoint_id uuid PRIMARY KEY,
    tenant_ref text NOT NULL,
    audit_sequence bigint NOT NULL CHECK (audit_sequence > 0),
    chain_root text NOT NULL CHECK (length(chain_root) = 64),
    signed_checkpoint jsonb NOT NULL,
    published_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (tenant_ref, audit_sequence)
);

CREATE TABLE IF NOT EXISTS idr_command_receipts (
    command_id uuid PRIMARY KEY,
    run_id uuid NOT NULL REFERENCES idr_runs(run_id),
    aggregate_version bigint NOT NULL,
    event_sequence bigint NOT NULL,
    receipt jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

CREATE INDEX IF NOT EXISTS idr_run_events_run_sequence_idx
    ON idr_run_events(run_id, run_event_sequence);
CREATE INDEX IF NOT EXISTS idr_contract_records_run_kind_idx
    ON idr_contract_records(run_id, candidate_kind, revision);
CREATE INDEX IF NOT EXISTS idr_outbox_pending_idx
    ON idr_outbox_events(available_at, claim_until) WHERE delivered_at IS NULL;
CREATE INDEX IF NOT EXISTS idr_audit_tenant_sequence_idx
    ON idr_audit_events(tenant_ref, sequence DESC);

CREATE OR REPLACE FUNCTION idr_enforce_contract_revision_v1()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    expected_revision bigint;
BEGIN
    SELECT COALESCE(MAX(revision) + 1, 1)
      INTO expected_revision
      FROM idr_contract_records
     WHERE record_id = NEW.record_id;
    IF NEW.revision <> expected_revision THEN
        RAISE EXCEPTION 'contract revision must be contiguous: expected %, found %',
            expected_revision, NEW.revision;
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS idr_contract_revision_guard_v1 ON idr_contract_records;
CREATE TRIGGER idr_contract_revision_guard_v1
BEFORE INSERT ON idr_contract_records
FOR EACH ROW EXECUTE FUNCTION idr_enforce_contract_revision_v1();

CREATE OR REPLACE FUNCTION idr_reject_dependency_cycle_v1()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    cycle_exists boolean;
BEGIN
    WITH RECURSIVE reachable(record_id, revision) AS (
        SELECT NEW.dependency_record_id, NEW.dependency_revision
        UNION
        SELECT dependency.dependency_record_id, dependency.dependency_revision
          FROM idr_contract_dependencies dependency
          JOIN reachable current_record
            ON dependency.dependent_record_id = current_record.record_id
           AND dependency.dependent_revision = current_record.revision
    )
    SELECT EXISTS (
        SELECT 1 FROM reachable
         WHERE record_id = NEW.dependent_record_id
           AND revision = NEW.dependent_revision
    ) INTO cycle_exists;
    IF cycle_exists THEN
        RAISE EXCEPTION 'contract dependency cycle';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS idr_contract_dependency_cycle_guard_v1
    ON idr_contract_dependencies;
CREATE TRIGGER idr_contract_dependency_cycle_guard_v1
BEFORE INSERT ON idr_contract_dependencies
FOR EACH ROW EXECUTE FUNCTION idr_reject_dependency_cycle_v1();

CREATE OR REPLACE FUNCTION idr_validate_execution_reservation_v1()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    action_kind text;
    admission_kind text;
    admission_depends_on_action boolean;
BEGIN
    SELECT candidate_kind INTO action_kind
      FROM idr_contract_records
     WHERE record_id = NEW.action_record_id AND revision = NEW.action_revision;
    SELECT candidate_kind INTO admission_kind
      FROM idr_contract_records
     WHERE record_id = NEW.action_admission_record_id
       AND revision = NEW.action_admission_revision;
    SELECT EXISTS (
        SELECT 1 FROM idr_contract_dependencies
         WHERE dependent_record_id = NEW.action_admission_record_id
           AND dependent_revision = NEW.action_admission_revision
           AND dependency_record_id = NEW.action_record_id
           AND dependency_revision = NEW.action_revision
    ) INTO admission_depends_on_action;
    IF action_kind <> 'action'
       OR admission_kind <> 'action_admission_decision'
       OR NOT admission_depends_on_action THEN
        RAISE EXCEPTION 'reservation requires exact Action Admission dependency';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS idr_execution_reservation_admission_guard_v1
    ON idr_execution_reservations;
CREATE TRIGGER idr_execution_reservation_admission_guard_v1
BEFORE INSERT ON idr_execution_reservations
FOR EACH ROW EXECUTE FUNCTION idr_validate_execution_reservation_v1();

CREATE OR REPLACE FUNCTION idr_enforce_execution_attempt_sequence_v1()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    expected_attempt integer;
BEGIN
    IF EXISTS (
        SELECT 1 FROM idr_execution_attempts
         WHERE reservation_id = NEW.reservation_id
           AND attempt = NEW.attempt
           AND permit_id = NEW.permit_id
    ) THEN
        RETURN NEW;
    END IF;
    SELECT COALESCE(MAX(attempt) + 1, 1)
      INTO expected_attempt
      FROM idr_execution_attempts
     WHERE reservation_id = NEW.reservation_id;
    IF NEW.attempt <> expected_attempt OR NEW.state <> 'permit_issued' THEN
        RAISE EXCEPTION 'execution attempt must be contiguous and start permit_issued';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS idr_execution_attempt_sequence_guard_v1
    ON idr_execution_attempts;
CREATE TRIGGER idr_execution_attempt_sequence_guard_v1
BEFORE INSERT ON idr_execution_attempts
FOR EACH ROW EXECUTE FUNCTION idr_enforce_execution_attempt_sequence_v1();

CREATE OR REPLACE FUNCTION idr_validate_execution_receipt_v1()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    attempt_row idr_execution_attempts%ROWTYPE;
BEGIN
    SELECT * INTO attempt_row
      FROM idr_execution_attempts
     WHERE reservation_id = NEW.reservation_id AND attempt = NEW.attempt;
    IF NOT FOUND
       OR attempt_row.permit_id <> NEW.permit_id
       OR attempt_row.provider_ref <> NEW.provider_ref
       OR attempt_row.dispatch_nonce <> NEW.dispatch_nonce
       OR attempt_row.started_at IS NULL
       OR attempt_row.state NOT IN (
           'dispatch_started', 'awaiting_provider', 'reconciliation_required',
           'succeeded', 'failed', 'rejected', 'compensated'
       ) THEN
        RAISE EXCEPTION 'receipt does not bind a dispatched execution attempt';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS idr_execution_receipt_guard_v1 ON idr_execution_receipts;
CREATE TRIGGER idr_execution_receipt_guard_v1
BEFORE INSERT ON idr_execution_receipts
FOR EACH ROW EXECUTE FUNCTION idr_validate_execution_receipt_v1();
