-- Round 9 binds concrete environment identity, authoritative validity, and
-- append-only storage invariants. Existing rows remain explicitly
-- unclassified and force startup failure until independently replayed.

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM idr_runs LIMIT 1)
       OR EXISTS (SELECT 1 FROM idr_contract_records LIMIT 1)
       OR EXISTS (SELECT 1 FROM idr_proof_envelopes LIMIT 1)
       OR EXISTS (SELECT 1 FROM idr_audit_events LIMIT 1)
       OR EXISTS (SELECT 1 FROM idr_execution_reservations LIMIT 1) THEN
        RAISE EXCEPTION
            'Round 9 trust identity/validity migration requires verified replay into a fresh schema; refusing partial in-place mutation';
    END IF;
END;
$$;

ALTER TABLE idr_runs
    ADD COLUMN IF NOT EXISTS environment_ref text NOT NULL
        DEFAULT '__unclassified_environment__';
ALTER TABLE idr_run_events
    ADD COLUMN IF NOT EXISTS environment_ref text NOT NULL
        DEFAULT '__unclassified_environment__';
ALTER TABLE idr_step_snapshots
    ADD COLUMN IF NOT EXISTS environment_ref text NOT NULL
        DEFAULT '__unclassified_environment__';
ALTER TABLE idr_contract_records
    ADD COLUMN IF NOT EXISTS environment_ref text NOT NULL
        DEFAULT '__unclassified_environment__',
    ADD COLUMN IF NOT EXISTS valid_from timestamptz NOT NULL DEFAULT 'epoch',
    ADD COLUMN IF NOT EXISTS valid_until timestamptz NOT NULL DEFAULT 'epoch';
ALTER TABLE idr_contract_current
    ADD COLUMN IF NOT EXISTS trust_domain text NOT NULL DEFAULT 'unclassified'
        CHECK (trust_domain IN ('shadow', 'production', 'unclassified')),
    ADD COLUMN IF NOT EXISTS environment_ref text NOT NULL
        DEFAULT '__unclassified_environment__';
ALTER TABLE idr_proof_envelopes
    ADD COLUMN IF NOT EXISTS environment_ref text NOT NULL
        DEFAULT '__unclassified_environment__';
ALTER TABLE idr_proof_consumptions
    ADD COLUMN IF NOT EXISTS environment_ref text NOT NULL
        DEFAULT '__unclassified_environment__';
ALTER TABLE idr_audit_events
    ADD COLUMN IF NOT EXISTS environment_ref text NOT NULL
        DEFAULT '__unclassified_environment__';
ALTER TABLE idr_outbox_events
    ADD COLUMN IF NOT EXISTS environment_ref text NOT NULL
        DEFAULT '__unclassified_environment__';
ALTER TABLE idr_audit_checkpoints
    ADD COLUMN IF NOT EXISTS environment_ref text NOT NULL
        DEFAULT '__unclassified_environment__',
    ADD COLUMN IF NOT EXISTS record_set_root text NOT NULL DEFAULT repeat('0', 64)
        CHECK (length(record_set_root) = 64);
ALTER TABLE idr_command_receipts
    ADD COLUMN IF NOT EXISTS environment_ref text NOT NULL
        DEFAULT '__unclassified_environment__';
ALTER TABLE idr_execution_reservations
    ADD COLUMN IF NOT EXISTS exact_authorization_proof_id uuid,
    ADD COLUMN IF NOT EXISTS action_valid_until timestamptz NOT NULL DEFAULT 'epoch',
    ADD COLUMN IF NOT EXISTS admission_valid_until timestamptz NOT NULL DEFAULT 'epoch',
    ADD COLUMN IF NOT EXISTS authorization_valid_until timestamptz NOT NULL DEFAULT 'epoch';
ALTER TABLE idr_execution_attempts
    ADD COLUMN IF NOT EXISTS permit_valid_until timestamptz NOT NULL DEFAULT 'epoch';

ALTER TABLE idr_runs ALTER COLUMN environment_ref DROP DEFAULT;
ALTER TABLE idr_run_events ALTER COLUMN environment_ref DROP DEFAULT;
ALTER TABLE idr_step_snapshots ALTER COLUMN environment_ref DROP DEFAULT;
ALTER TABLE idr_contract_records ALTER COLUMN environment_ref DROP DEFAULT;
ALTER TABLE idr_contract_records ALTER COLUMN valid_from DROP DEFAULT;
ALTER TABLE idr_contract_records ALTER COLUMN valid_until DROP DEFAULT;
ALTER TABLE idr_contract_current ALTER COLUMN trust_domain DROP DEFAULT;
ALTER TABLE idr_contract_current ALTER COLUMN environment_ref DROP DEFAULT;
ALTER TABLE idr_proof_envelopes ALTER COLUMN environment_ref DROP DEFAULT;
ALTER TABLE idr_proof_consumptions ALTER COLUMN environment_ref DROP DEFAULT;
ALTER TABLE idr_audit_events ALTER COLUMN environment_ref DROP DEFAULT;
ALTER TABLE idr_outbox_events ALTER COLUMN environment_ref DROP DEFAULT;
ALTER TABLE idr_audit_checkpoints ALTER COLUMN environment_ref DROP DEFAULT;
ALTER TABLE idr_audit_checkpoints ALTER COLUMN record_set_root DROP DEFAULT;
ALTER TABLE idr_command_receipts ALTER COLUMN environment_ref DROP DEFAULT;
ALTER TABLE idr_execution_reservations ALTER COLUMN action_valid_until DROP DEFAULT;
ALTER TABLE idr_execution_reservations ALTER COLUMN admission_valid_until DROP DEFAULT;
ALTER TABLE idr_execution_reservations ALTER COLUMN authorization_valid_until DROP DEFAULT;
ALTER TABLE idr_execution_attempts ALTER COLUMN permit_valid_until DROP DEFAULT;

ALTER TABLE idr_execution_reservations
    ADD CONSTRAINT idr_execution_authority_validity_v4 CHECK (
        action_valid_until > created_at
        AND admission_valid_until > created_at
        AND authorization_valid_until > created_at
    );
ALTER TABLE idr_execution_attempts
    ADD CONSTRAINT idr_execution_permit_validity_v4 CHECK (
        permit_valid_until <= lease_until
        AND permit_valid_until > 'epoch'::timestamptz
    );

ALTER TABLE idr_contract_records
    ADD CONSTRAINT idr_contract_validity_v4
    CHECK (valid_from < valid_until);

CREATE OR REPLACE FUNCTION idr_enforce_run_identity_v4()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.trust_domain = 'unclassified'
       OR NEW.environment_ref = '__unclassified_environment__'
       OR NEW.projection #>> '{trust_domain}' IS DISTINCT FROM NEW.trust_domain
       OR NEW.projection #>> '{environment_ref}' IS DISTINCT FROM NEW.environment_ref THEN
        RAISE EXCEPTION 'Run projection trust identity is absent, unclassified, or mismatched';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS idr_run_projection_domain_guard_v3 ON idr_runs;
CREATE TRIGGER idr_run_identity_guard_v4
BEFORE INSERT OR UPDATE ON idr_runs
FOR EACH ROW EXECUTE FUNCTION idr_enforce_run_identity_v4();

CREATE OR REPLACE FUNCTION idr_enforce_run_child_identity_v4()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    parent_domain text;
    parent_environment text;
BEGIN
    SELECT trust_domain, environment_ref
      INTO parent_domain, parent_environment
      FROM idr_runs WHERE run_id = NEW.run_id;
    IF parent_domain IS NULL
       OR NEW.trust_domain = 'unclassified'
       OR NEW.environment_ref = '__unclassified_environment__'
       OR NEW.trust_domain IS DISTINCT FROM parent_domain
       OR NEW.environment_ref IS DISTINCT FROM parent_environment THEN
        RAISE EXCEPTION 'Run child trust identity does not match its authority Run';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS idr_run_event_domain_guard_v3 ON idr_run_events;
DROP TRIGGER IF EXISTS idr_step_snapshot_domain_guard_v3 ON idr_step_snapshots;
DROP TRIGGER IF EXISTS idr_proof_consumption_domain_guard_v3 ON idr_proof_consumptions;
DROP TRIGGER IF EXISTS idr_audit_event_domain_guard_v3 ON idr_audit_events;
DROP TRIGGER IF EXISTS idr_outbox_event_domain_guard_v3 ON idr_outbox_events;
DROP TRIGGER IF EXISTS idr_command_receipt_domain_guard_v3 ON idr_command_receipts;

CREATE TRIGGER idr_run_event_identity_guard_v4
BEFORE INSERT OR UPDATE ON idr_run_events
FOR EACH ROW EXECUTE FUNCTION idr_enforce_run_child_identity_v4();
CREATE TRIGGER idr_step_snapshot_identity_guard_v4
BEFORE INSERT OR UPDATE ON idr_step_snapshots
FOR EACH ROW EXECUTE FUNCTION idr_enforce_run_child_identity_v4();
CREATE TRIGGER idr_proof_consumption_identity_guard_v4
BEFORE INSERT OR UPDATE ON idr_proof_consumptions
FOR EACH ROW EXECUTE FUNCTION idr_enforce_run_child_identity_v4();
CREATE TRIGGER idr_audit_event_identity_guard_v4
BEFORE INSERT OR UPDATE ON idr_audit_events
FOR EACH ROW EXECUTE FUNCTION idr_enforce_run_child_identity_v4();
CREATE TRIGGER idr_outbox_event_identity_guard_v4
BEFORE INSERT OR UPDATE ON idr_outbox_events
FOR EACH ROW EXECUTE FUNCTION idr_enforce_run_child_identity_v4();
CREATE TRIGGER idr_command_receipt_identity_guard_v4
BEFORE INSERT OR UPDATE ON idr_command_receipts
FOR EACH ROW EXECUTE FUNCTION idr_enforce_run_child_identity_v4();

CREATE OR REPLACE FUNCTION idr_enforce_contract_identity_and_validity_v4()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    parent_domain text;
    parent_environment text;
BEGIN
    SELECT trust_domain, environment_ref
      INTO parent_domain, parent_environment
      FROM idr_runs WHERE run_id = NEW.run_id;
    IF parent_domain IS NULL
       OR NEW.trust_domain = 'unclassified'
       OR NEW.environment_ref = '__unclassified_environment__'
       OR NEW.trust_domain IS DISTINCT FROM parent_domain
       OR NEW.environment_ref IS DISTINCT FROM parent_environment
       OR NEW.record #>> '{trust_domain}' IS DISTINCT FROM NEW.trust_domain
       OR NEW.record #>> '{environment_ref}' IS DISTINCT FROM NEW.environment_ref
       OR NEW.record #>> '{record_ref,trust_domain}' IS DISTINCT FROM NEW.trust_domain
       OR NEW.record #>> '{record_ref,environment_ref}' IS DISTINCT FROM NEW.environment_ref
       OR (NEW.record #>> '{valid_from}')::bigint
          IS DISTINCT FROM EXTRACT(EPOCH FROM NEW.valid_from)::bigint
       OR (NEW.record #>> '{valid_until}')::bigint
          IS DISTINCT FROM EXTRACT(EPOCH FROM NEW.valid_until)::bigint
       OR (NEW.record #>> '{record_ref,valid_from}')::bigint
          IS DISTINCT FROM EXTRACT(EPOCH FROM NEW.valid_from)::bigint
       OR (NEW.record #>> '{record_ref,valid_until}')::bigint
          IS DISTINCT FROM EXTRACT(EPOCH FROM NEW.valid_until)::bigint
       OR NEW.record #>> '{record_ref,record_digest}' IS DISTINCT FROM NEW.record_digest THEN
        RAISE EXCEPTION 'Contract trust identity, validity, or digest columns mismatch record';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS idr_contract_domain_guard_v3 ON idr_contract_records;
CREATE TRIGGER idr_contract_identity_validity_guard_v4
BEFORE INSERT ON idr_contract_records
FOR EACH ROW EXECUTE FUNCTION idr_enforce_contract_identity_and_validity_v4();

CREATE OR REPLACE FUNCTION idr_enforce_current_identity_v4()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    record_domain text;
    record_environment text;
    record_run uuid;
BEGIN
    SELECT trust_domain, environment_ref, run_id
      INTO record_domain, record_environment, record_run
      FROM idr_contract_records
     WHERE record_id = NEW.record_id AND revision = NEW.revision;
    IF record_run IS DISTINCT FROM NEW.run_id
       OR record_domain IS DISTINCT FROM NEW.trust_domain
       OR record_environment IS DISTINCT FROM NEW.environment_ref THEN
        RAISE EXCEPTION 'Current pointer crossed Run or trust identity';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER idr_contract_current_identity_guard_v4
BEFORE INSERT OR UPDATE ON idr_contract_current
FOR EACH ROW EXECUTE FUNCTION idr_enforce_current_identity_v4();

CREATE OR REPLACE FUNCTION idr_enforce_proof_identity_v4()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.trust_domain = 'unclassified'
       OR NEW.environment_ref = '__unclassified_environment__'
       OR NEW.envelope #>> '{claims,trust_domain}' IS DISTINCT FROM NEW.trust_domain
       OR NEW.envelope #>> '{claims,environment_ref}' IS DISTINCT FROM NEW.environment_ref THEN
        RAISE EXCEPTION 'Proof envelope trust identity mismatch';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER idr_proof_envelope_identity_guard_v4
BEFORE INSERT ON idr_proof_envelopes
FOR EACH ROW EXECUTE FUNCTION idr_enforce_proof_identity_v4();

CREATE OR REPLACE FUNCTION idr_enforce_proof_consumption_identity_v4()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    envelope_domain text;
    envelope_environment text;
BEGIN
    SELECT trust_domain, environment_ref
      INTO envelope_domain, envelope_environment
      FROM idr_proof_envelopes WHERE proof_id = NEW.proof_id;
    IF NEW.trust_domain IS DISTINCT FROM envelope_domain
       OR NEW.environment_ref IS DISTINCT FROM envelope_environment THEN
        RAISE EXCEPTION 'Proof consumption crossed trust identity';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS idr_proof_consumption_envelope_domain_guard_v3
    ON idr_proof_consumptions;
CREATE TRIGGER idr_proof_consumption_envelope_identity_guard_v4
BEFORE INSERT ON idr_proof_consumptions
FOR EACH ROW EXECUTE FUNCTION idr_enforce_proof_consumption_identity_v4();

CREATE OR REPLACE FUNCTION idr_validate_execution_authority_v4()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    action_row idr_contract_records%ROWTYPE;
    admission_row idr_contract_records%ROWTYPE;
    authorization_kind text;
    authorization_expiry bigint;
BEGIN
    SELECT * INTO action_row
      FROM idr_contract_records
     WHERE record_id = NEW.action_record_id AND revision = NEW.action_revision;
    SELECT * INTO admission_row
      FROM idr_contract_records
     WHERE record_id = NEW.action_admission_record_id
       AND revision = NEW.action_admission_revision;
    SELECT proof_kind, (envelope #>> '{claims,expires_at}')::bigint
      INTO authorization_kind, authorization_expiry
      FROM idr_proof_envelopes
     WHERE proof_id = NEW.exact_authorization_proof_id;
    IF action_row.candidate_kind IS DISTINCT FROM 'action'
       OR admission_row.candidate_kind IS DISTINCT FROM 'action_admission_decision'
       OR action_row.trust_domain IS DISTINCT FROM admission_row.trust_domain
       OR action_row.environment_ref IS DISTINCT FROM admission_row.environment_ref
       OR action_row.valid_until IS DISTINCT FROM NEW.action_valid_until
       OR admission_row.valid_until IS DISTINCT FROM NEW.admission_valid_until
       OR authorization_kind IS DISTINCT FROM 'exact_authorization'
       OR to_timestamp(authorization_expiry)
          IS DISTINCT FROM NEW.authorization_valid_until
       OR LEAST(
            NEW.action_valid_until,
            NEW.admission_valid_until,
            NEW.authorization_valid_until
          ) <= clock_timestamp() THEN
        RAISE EXCEPTION 'Execution reservation authority is absent, expired, or mismatched';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS idr_execution_reservation_admission_guard_v1
    ON idr_execution_reservations;
CREATE TRIGGER idr_execution_reservation_authority_guard_v4
BEFORE INSERT ON idr_execution_reservations
FOR EACH ROW EXECUTE FUNCTION idr_validate_execution_authority_v4();

CREATE OR REPLACE FUNCTION idr_enforce_audit_identity_v4()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.payload #>> '{trust_domain}' IS DISTINCT FROM NEW.trust_domain
       OR NEW.payload #>> '{environment_ref}' IS DISTINCT FROM NEW.environment_ref THEN
        RAISE EXCEPTION 'Audit payload trust identity mismatch';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS idr_audit_payload_domain_guard_v3 ON idr_audit_events;
CREATE TRIGGER idr_audit_payload_identity_guard_v4
BEFORE INSERT ON idr_audit_events
FOR EACH ROW EXECUTE FUNCTION idr_enforce_audit_identity_v4();

CREATE OR REPLACE FUNCTION idr_reject_authority_mutation_v4()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'IDR authority tables are append-only; write a forward revision';
END;
$$;

CREATE TRIGGER idr_contract_records_append_only_v4
BEFORE UPDATE OR DELETE ON idr_contract_records
FOR EACH ROW EXECUTE FUNCTION idr_reject_authority_mutation_v4();
CREATE TRIGGER idr_contract_current_no_delete_v4
BEFORE DELETE ON idr_contract_current
FOR EACH ROW EXECUTE FUNCTION idr_reject_authority_mutation_v4();
CREATE TRIGGER idr_contract_dependencies_append_only_v4
BEFORE UPDATE OR DELETE ON idr_contract_dependencies
FOR EACH ROW EXECUTE FUNCTION idr_reject_authority_mutation_v4();
CREATE TRIGGER idr_contract_invalidations_append_only_v4
BEFORE UPDATE OR DELETE ON idr_contract_invalidations
FOR EACH ROW EXECUTE FUNCTION idr_reject_authority_mutation_v4();
CREATE TRIGGER idr_proof_envelopes_append_only_v4
BEFORE UPDATE OR DELETE ON idr_proof_envelopes
FOR EACH ROW EXECUTE FUNCTION idr_reject_authority_mutation_v4();
CREATE TRIGGER idr_proof_consumptions_append_only_v4
BEFORE UPDATE OR DELETE ON idr_proof_consumptions
FOR EACH ROW EXECUTE FUNCTION idr_reject_authority_mutation_v4();
CREATE TRIGGER idr_run_events_append_only_v4
BEFORE UPDATE OR DELETE ON idr_run_events
FOR EACH ROW EXECUTE FUNCTION idr_reject_authority_mutation_v4();
CREATE TRIGGER idr_audit_events_append_only_v4
BEFORE UPDATE OR DELETE ON idr_audit_events
FOR EACH ROW EXECUTE FUNCTION idr_reject_authority_mutation_v4();
CREATE TRIGGER idr_command_receipts_append_only_v4
BEFORE UPDATE OR DELETE ON idr_command_receipts
FOR EACH ROW EXECUTE FUNCTION idr_reject_authority_mutation_v4();
CREATE TRIGGER idr_response_consumptions_append_only_v4
BEFORE UPDATE OR DELETE ON idr_response_send_consumptions
FOR EACH ROW EXECUTE FUNCTION idr_reject_authority_mutation_v4();
CREATE TRIGGER idr_execution_receipts_append_only_v4
BEFORE UPDATE OR DELETE ON idr_execution_receipts
FOR EACH ROW EXECUTE FUNCTION idr_reject_authority_mutation_v4();
CREATE TRIGGER idr_outcomes_append_only_v4
BEFORE UPDATE OR DELETE ON idr_outcome_records
FOR EACH ROW EXECUTE FUNCTION idr_reject_authority_mutation_v4();
CREATE TRIGGER idr_hm_candidates_append_only_v4
BEFORE UPDATE OR DELETE ON idr_human_model_candidates
FOR EACH ROW EXECUTE FUNCTION idr_reject_authority_mutation_v4();
CREATE TRIGGER idr_hm_promotions_append_only_v4
BEFORE UPDATE OR DELETE ON idr_human_model_promotion_decisions
FOR EACH ROW EXECUTE FUNCTION idr_reject_authority_mutation_v4();
CREATE TRIGGER idr_hm_assertions_append_only_v4
BEFORE UPDATE OR DELETE ON idr_human_model_assertions
FOR EACH ROW EXECUTE FUNCTION idr_reject_authority_mutation_v4();

REVOKE UPDATE, DELETE ON idr_contract_records FROM PUBLIC;
REVOKE DELETE ON idr_contract_current FROM PUBLIC;
REVOKE UPDATE, DELETE ON idr_contract_dependencies FROM PUBLIC;
REVOKE UPDATE, DELETE ON idr_contract_invalidations FROM PUBLIC;
REVOKE UPDATE, DELETE ON idr_proof_envelopes FROM PUBLIC;
REVOKE UPDATE, DELETE ON idr_proof_consumptions FROM PUBLIC;
REVOKE UPDATE, DELETE ON idr_run_events FROM PUBLIC;
REVOKE UPDATE, DELETE ON idr_audit_events FROM PUBLIC;
REVOKE UPDATE, DELETE ON idr_command_receipts FROM PUBLIC;
REVOKE UPDATE, DELETE ON idr_response_send_consumptions FROM PUBLIC;
REVOKE UPDATE, DELETE ON idr_execution_receipts FROM PUBLIC;
REVOKE UPDATE, DELETE ON idr_outcome_records FROM PUBLIC;
REVOKE UPDATE, DELETE ON idr_human_model_candidates FROM PUBLIC;
REVOKE UPDATE, DELETE ON idr_human_model_promotion_decisions FROM PUBLIC;
REVOKE UPDATE, DELETE ON idr_human_model_assertions FROM PUBLIC;

CREATE UNIQUE INDEX IF NOT EXISTS idr_proof_consumption_environment_nonce_v4
    ON idr_proof_consumptions(trust_domain, environment_ref, tenant_ref, nonce);
CREATE UNIQUE INDEX IF NOT EXISTS idr_checkpoint_environment_sequence_v4
    ON idr_audit_checkpoints(
        trust_domain, environment_ref, tenant_ref, audit_sequence
    );
