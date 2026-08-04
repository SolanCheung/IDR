-- Round 7 closes projection-cache integrity and command-id conflict gaps.
-- Existing non-empty pre-production databases must be replayed/backfilled by
-- the migration tool before writes resume; sentinel defaults fail closed.

ALTER TABLE idr_runs
    ADD COLUMN IF NOT EXISTS projection_digest text NOT NULL
        DEFAULT repeat('0', 64)
        CHECK (length(projection_digest) = 64);

ALTER TABLE idr_runs
    ALTER COLUMN projection_digest DROP DEFAULT;

ALTER TABLE idr_command_receipts
    ADD COLUMN IF NOT EXISTS tenant_ref text NOT NULL
        DEFAULT '__migration_backfill_required__',
    ADD COLUMN IF NOT EXISTS actor_ref text NOT NULL
        DEFAULT '__migration_backfill_required__',
    ADD COLUMN IF NOT EXISTS caller_ref text NOT NULL
        DEFAULT '__migration_backfill_required__',
    ADD COLUMN IF NOT EXISTS command_digest text NOT NULL
        DEFAULT repeat('0', 64)
        CHECK (length(command_digest) = 64);

ALTER TABLE idr_command_receipts
    ALTER COLUMN tenant_ref DROP DEFAULT,
    ALTER COLUMN actor_ref DROP DEFAULT,
    ALTER COLUMN caller_ref DROP DEFAULT,
    ALTER COLUMN command_digest DROP DEFAULT;

CREATE INDEX IF NOT EXISTS idr_audit_run_sequence_idx
    ON idr_audit_events(run_id, sequence DESC);

ALTER TABLE idr_execution_reservations
    ADD COLUMN IF NOT EXISTS action_digest text NOT NULL
        DEFAULT repeat('0', 64) CHECK (length(action_digest) = 64),
    ADD COLUMN IF NOT EXISTS parameter_digest text NOT NULL
        DEFAULT repeat('0', 64) CHECK (length(parameter_digest) = 64),
    ADD COLUMN IF NOT EXISTS request_digest text NOT NULL
        DEFAULT repeat('0', 64) CHECK (length(request_digest) = 64);

ALTER TABLE idr_execution_reservations
    ALTER COLUMN action_digest DROP DEFAULT,
    ALTER COLUMN parameter_digest DROP DEFAULT,
    ALTER COLUMN request_digest DROP DEFAULT;

CREATE OR REPLACE FUNCTION idr_validate_execution_receipt_v2()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    attempt_row idr_execution_attempts%ROWTYPE;
    reservation_row idr_execution_reservations%ROWTYPE;
BEGIN
    SELECT * INTO attempt_row
      FROM idr_execution_attempts
     WHERE reservation_id = NEW.reservation_id AND attempt = NEW.attempt;
    SELECT * INTO reservation_row
      FROM idr_execution_reservations
     WHERE reservation_id = NEW.reservation_id;
    IF attempt_row.state NOT IN (
        'dispatch_started', 'awaiting_provider', 'reconciliation_required',
        'succeeded', 'failed', 'rejected', 'compensated'
    )
       OR attempt_row.permit_id <> NEW.permit_id
       OR attempt_row.provider_ref <> NEW.provider_ref
       OR attempt_row.dispatch_nonce <> NEW.dispatch_nonce
       OR reservation_row.request_digest <> NEW.receipt #>> '{payload,request_digest}' THEN
        RAISE EXCEPTION 'Receipt does not match dispatched attempt and approved request';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS idr_execution_receipt_guard_v1 ON idr_execution_receipts;
CREATE TRIGGER idr_execution_receipt_guard_v2
BEFORE INSERT ON idr_execution_receipts
FOR EACH ROW EXECUTE FUNCTION idr_validate_execution_receipt_v2();

CREATE OR REPLACE FUNCTION idr_enforce_record_lineage_v2()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    run_subject text;
    run_turn text;
    predecessor idr_contract_records%ROWTYPE;
BEGIN
    SELECT projection #>> '{subject_ref}', projection #>> '{turn_id}'
      INTO run_subject, run_turn
      FROM idr_runs WHERE run_id = NEW.run_id;
    IF NEW.record #>> '{subject_ref}' <> run_subject
       OR NEW.record #>> '{turn_id}' <> run_turn THEN
        RAISE EXCEPTION 'Contract subject/turn does not match Run lineage';
    END IF;
    IF NEW.revision > 1 THEN
        SELECT * INTO predecessor FROM idr_contract_records
         WHERE record_id = NEW.record_id AND revision = NEW.revision - 1;
        IF predecessor.run_id <> NEW.run_id
           OR predecessor.tenant_ref <> NEW.tenant_ref
           OR predecessor.subject_ref <> NEW.subject_ref
           OR predecessor.record #>> '{turn_id}' <> NEW.record #>> '{turn_id}'
           OR predecessor.record #>> '{policy_revision_ref}'
              <> NEW.record #>> '{policy_revision_ref}' THEN
            RAISE EXCEPTION 'Contract successor changed immutable lineage';
        END IF;
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER idr_contract_lineage_guard_v2
BEFORE INSERT ON idr_contract_records
FOR EACH ROW EXECUTE FUNCTION idr_enforce_record_lineage_v2();

CREATE OR REPLACE FUNCTION idr_enforce_action_derivation_v2()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    dependent_kind text;
    dependency_kind text;
    action_payload jsonb;
    decision_payload jsonb;
BEGIN
    SELECT candidate_kind, record #> '{payload}'
      INTO dependent_kind, action_payload
      FROM idr_contract_records
     WHERE record_id = NEW.dependent_record_id
       AND revision = NEW.dependent_revision;
    SELECT candidate_kind, record #> '{payload}'
      INTO dependency_kind, decision_payload
      FROM idr_contract_records
     WHERE record_id = NEW.dependency_record_id
       AND revision = NEW.dependency_revision;
    IF dependent_kind = 'action' AND dependency_kind = 'decision'
       AND (
          action_payload #>> '{selected_option_ref}'
             <> decision_payload #>> '{selected_option_ref}'
          OR action_payload #>> '{operation_ref}'
             <> decision_payload #>> '{selected_action_operation_ref}'
          OR action_payload #>> '{parameter_digest}'
             <> decision_payload #>> '{selected_action_parameter_digest}'
       ) THEN
        RAISE EXCEPTION 'Action does not derive from selected Decision option';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER idr_action_derivation_guard_v2
BEFORE INSERT ON idr_contract_dependencies
FOR EACH ROW EXECUTE FUNCTION idr_enforce_action_derivation_v2();

CREATE OR REPLACE FUNCTION idr_enforce_human_model_materialization_v2()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    source_payload jsonb;
    assertion_payload jsonb;
BEGIN
    SELECT candidate.record #> '{payload}'
      INTO source_payload
      FROM idr_human_model_promotion_decisions promotion
      JOIN idr_contract_records candidate
        ON candidate.record_id = promotion.source_candidate_record_id
       AND candidate.revision = promotion.source_candidate_revision
     WHERE promotion.decision_record_id = NEW.promotion_decision_record_id
       AND promotion.decision_revision = NEW.promotion_decision_revision;
    assertion_payload := NEW.assertion #> '{payload}';
    IF source_payload IS NULL
       OR assertion_payload #>> '{predicate}' <> source_payload #>> '{predicate}'
       OR assertion_payload #>> '{value_digest}' <> source_payload #>> '{value_digest}'
       OR assertion_payload #>> '{scope_ref}' <> source_payload #>> '{scope_ref}'
       OR assertion_payload #> '{evidence_refs}' <> source_payload #> '{evidence_refs}'
       OR assertion_payload #> '{allowed_purposes}' <> source_payload #> '{allowed_purposes}'
       OR assertion_payload #>> '{outcome_record_digest}'
          <> source_payload #>> '{outcome_record_digest}' THEN
        RAISE EXCEPTION 'Human Model Assertion changed promoted Candidate content';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER idr_human_model_materialization_guard_v2
BEFORE INSERT ON idr_human_model_assertions
FOR EACH ROW EXECUTE FUNCTION idr_enforce_human_model_materialization_v2();
