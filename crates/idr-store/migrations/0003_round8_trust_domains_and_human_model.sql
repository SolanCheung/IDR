-- Round 8 creates explicit, fail-closed trust domains and closes Human Model
-- materialization escalation. Existing rows are intentionally marked
-- `unclassified`; no Shadow or Production authority may start until such a
-- database is independently replayed into a domain-specific schema.

ALTER TABLE idr_runs
    ADD COLUMN IF NOT EXISTS trust_domain text NOT NULL DEFAULT 'unclassified'
        CHECK (trust_domain IN ('shadow', 'production', 'unclassified'));
ALTER TABLE idr_run_events
    ADD COLUMN IF NOT EXISTS trust_domain text NOT NULL DEFAULT 'unclassified'
        CHECK (trust_domain IN ('shadow', 'production', 'unclassified'));
ALTER TABLE idr_step_snapshots
    ADD COLUMN IF NOT EXISTS trust_domain text NOT NULL DEFAULT 'unclassified'
        CHECK (trust_domain IN ('shadow', 'production', 'unclassified'));
ALTER TABLE idr_contract_records
    ADD COLUMN IF NOT EXISTS trust_domain text NOT NULL DEFAULT 'unclassified'
        CHECK (trust_domain IN ('shadow', 'production', 'unclassified'));
ALTER TABLE idr_proof_envelopes
    ADD COLUMN IF NOT EXISTS trust_domain text NOT NULL DEFAULT 'unclassified'
        CHECK (trust_domain IN ('shadow', 'production', 'unclassified'));
ALTER TABLE idr_proof_consumptions
    ADD COLUMN IF NOT EXISTS trust_domain text NOT NULL DEFAULT 'unclassified'
        CHECK (trust_domain IN ('shadow', 'production', 'unclassified'));
ALTER TABLE idr_audit_events
    ADD COLUMN IF NOT EXISTS trust_domain text NOT NULL DEFAULT 'unclassified'
        CHECK (trust_domain IN ('shadow', 'production', 'unclassified'));
ALTER TABLE idr_outbox_events
    ADD COLUMN IF NOT EXISTS trust_domain text NOT NULL DEFAULT 'unclassified'
        CHECK (trust_domain IN ('shadow', 'production', 'unclassified'));
ALTER TABLE idr_audit_checkpoints
    ADD COLUMN IF NOT EXISTS trust_domain text NOT NULL DEFAULT 'unclassified'
        CHECK (trust_domain IN ('shadow', 'production', 'unclassified'));
ALTER TABLE idr_command_receipts
    ADD COLUMN IF NOT EXISTS trust_domain text NOT NULL DEFAULT 'unclassified'
        CHECK (trust_domain IN ('shadow', 'production', 'unclassified'));

ALTER TABLE idr_runs ALTER COLUMN trust_domain DROP DEFAULT;
ALTER TABLE idr_run_events ALTER COLUMN trust_domain DROP DEFAULT;
ALTER TABLE idr_step_snapshots ALTER COLUMN trust_domain DROP DEFAULT;
ALTER TABLE idr_contract_records ALTER COLUMN trust_domain DROP DEFAULT;
ALTER TABLE idr_proof_envelopes ALTER COLUMN trust_domain DROP DEFAULT;
ALTER TABLE idr_proof_consumptions ALTER COLUMN trust_domain DROP DEFAULT;
ALTER TABLE idr_audit_events ALTER COLUMN trust_domain DROP DEFAULT;
ALTER TABLE idr_outbox_events ALTER COLUMN trust_domain DROP DEFAULT;
ALTER TABLE idr_audit_checkpoints ALTER COLUMN trust_domain DROP DEFAULT;
ALTER TABLE idr_command_receipts ALTER COLUMN trust_domain DROP DEFAULT;

CREATE OR REPLACE FUNCTION idr_enforce_run_projection_domain_v3()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.trust_domain = 'unclassified'
       OR NEW.projection #>> '{trust_domain}' IS DISTINCT FROM NEW.trust_domain THEN
        RAISE EXCEPTION 'Run projection trust domain is absent, unclassified, or mismatched';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER idr_run_projection_domain_guard_v3
BEFORE INSERT OR UPDATE ON idr_runs
FOR EACH ROW EXECUTE FUNCTION idr_enforce_run_projection_domain_v3();

CREATE OR REPLACE FUNCTION idr_enforce_run_child_domain_v3()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    parent_domain text;
BEGIN
    SELECT trust_domain INTO parent_domain FROM idr_runs WHERE run_id = NEW.run_id;
    IF parent_domain IS NULL
       OR NEW.trust_domain = 'unclassified'
       OR NEW.trust_domain IS DISTINCT FROM parent_domain THEN
        RAISE EXCEPTION 'Run child trust domain does not match its authority Run';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER idr_run_event_domain_guard_v3
BEFORE INSERT OR UPDATE ON idr_run_events
FOR EACH ROW EXECUTE FUNCTION idr_enforce_run_child_domain_v3();
CREATE TRIGGER idr_step_snapshot_domain_guard_v3
BEFORE INSERT OR UPDATE ON idr_step_snapshots
FOR EACH ROW EXECUTE FUNCTION idr_enforce_run_child_domain_v3();
CREATE TRIGGER idr_proof_consumption_domain_guard_v3
BEFORE INSERT OR UPDATE ON idr_proof_consumptions
FOR EACH ROW EXECUTE FUNCTION idr_enforce_run_child_domain_v3();
CREATE TRIGGER idr_audit_event_domain_guard_v3
BEFORE INSERT OR UPDATE ON idr_audit_events
FOR EACH ROW EXECUTE FUNCTION idr_enforce_run_child_domain_v3();
CREATE TRIGGER idr_outbox_event_domain_guard_v3
BEFORE INSERT OR UPDATE ON idr_outbox_events
FOR EACH ROW EXECUTE FUNCTION idr_enforce_run_child_domain_v3();
CREATE TRIGGER idr_command_receipt_domain_guard_v3
BEFORE INSERT OR UPDATE ON idr_command_receipts
FOR EACH ROW EXECUTE FUNCTION idr_enforce_run_child_domain_v3();

CREATE OR REPLACE FUNCTION idr_enforce_contract_domain_v3()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    parent_domain text;
BEGIN
    SELECT trust_domain INTO parent_domain FROM idr_runs WHERE run_id = NEW.run_id;
    IF parent_domain IS NULL
       OR NEW.trust_domain = 'unclassified'
       OR NEW.trust_domain IS DISTINCT FROM parent_domain
       OR NEW.record #>> '{trust_domain}' IS DISTINCT FROM NEW.trust_domain THEN
        RAISE EXCEPTION 'Contract trust domain does not match its authority Run and record';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER idr_contract_domain_guard_v3
BEFORE INSERT OR UPDATE ON idr_contract_records
FOR EACH ROW EXECUTE FUNCTION idr_enforce_contract_domain_v3();

CREATE OR REPLACE FUNCTION idr_enforce_proof_consumption_envelope_domain_v3()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    envelope_domain text;
BEGIN
    SELECT trust_domain INTO envelope_domain
      FROM idr_proof_envelopes WHERE proof_id = NEW.proof_id;
    IF envelope_domain IS NULL
       OR NEW.trust_domain IS DISTINCT FROM envelope_domain THEN
        RAISE EXCEPTION 'Proof consumption crossed trust domains';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER idr_proof_consumption_envelope_domain_guard_v3
BEFORE INSERT OR UPDATE ON idr_proof_consumptions
FOR EACH ROW EXECUTE FUNCTION idr_enforce_proof_consumption_envelope_domain_v3();

CREATE OR REPLACE FUNCTION idr_enforce_audit_payload_domain_v3()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.payload #>> '{trust_domain}' IS DISTINCT FROM NEW.trust_domain THEN
        RAISE EXCEPTION 'Audit payload trust domain does not match audit column';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER idr_audit_payload_domain_guard_v3
BEFORE INSERT OR UPDATE ON idr_audit_events
FOR EACH ROW EXECUTE FUNCTION idr_enforce_audit_payload_domain_v3();

DROP TRIGGER IF EXISTS idr_human_model_materialization_guard_v2
    ON idr_human_model_assertions;

CREATE OR REPLACE FUNCTION idr_enforce_human_model_materialization_v3()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    source_payload jsonb;
    assertion_payload jsonb;
    promotion_outcome text;
    promotion_digest text;
    expected_lifecycle text;
BEGIN
    -- Corrections follow their separately proof-bound predecessor path. This
    -- trigger closes the initial promotion/materialization path.
    IF NEW.promotion_decision_record_id IS NULL THEN
        RETURN NEW;
    END IF;

    SELECT candidate.record #> '{payload}', promotion.outcome,
           decision.record_digest
      INTO source_payload, promotion_outcome, promotion_digest
      FROM idr_human_model_promotion_decisions promotion
      JOIN idr_contract_records candidate
        ON candidate.record_id = promotion.source_candidate_record_id
       AND candidate.revision = promotion.source_candidate_revision
      JOIN idr_contract_records decision
        ON decision.record_id = promotion.decision_record_id
       AND decision.revision = promotion.decision_revision
     WHERE promotion.decision_record_id = NEW.promotion_decision_record_id
       AND promotion.decision_revision = NEW.promotion_decision_revision;

    expected_lifecycle := CASE promotion_outcome
        WHEN 'promote_provisional' THEN 'provisional'
        WHEN 'promote_user_confirmed' THEN 'user_confirmed'
        WHEN 'promote_outcome_supported' THEN 'outcome_supported'
        ELSE NULL
    END;
    assertion_payload := NEW.assertion #> '{payload}';

    IF source_payload IS NULL
       OR expected_lifecycle IS NULL
       OR NEW.lifecycle_state IS DISTINCT FROM expected_lifecycle
       OR assertion_payload #>> '{lifecycle_state}' IS DISTINCT FROM expected_lifecycle
       OR assertion_payload #>> '{source_candidate_digest}'
          IS DISTINCT FROM (
              SELECT candidate_digest
                FROM idr_human_model_promotion_decisions
               WHERE decision_record_id = NEW.promotion_decision_record_id
                 AND decision_revision = NEW.promotion_decision_revision
          )
       OR assertion_payload #>> '{promotion_decision_record_id}'
          IS DISTINCT FROM NEW.promotion_decision_record_id::text
       OR (assertion_payload #>> '{promotion_decision_revision}')::bigint
          IS DISTINCT FROM NEW.promotion_decision_revision
       OR assertion_payload #>> '{promotion_decision_record_digest}'
          IS DISTINCT FROM promotion_digest
       OR assertion_payload #>> '{predicate}' IS DISTINCT FROM source_payload #>> '{predicate}'
       OR assertion_payload #>> '{value_digest}' IS DISTINCT FROM source_payload #>> '{value_digest}'
       OR assertion_payload #>> '{scope_ref}' IS DISTINCT FROM source_payload #>> '{scope_ref}'
       OR assertion_payload #> '{evidence_refs}' IS DISTINCT FROM source_payload #> '{evidence_refs}'
       OR assertion_payload #> '{allowed_purposes}' IS DISTINCT FROM source_payload #> '{allowed_purposes}'
       OR assertion_payload #>> '{outcome_record_id}'
          IS DISTINCT FROM source_payload #>> '{outcome_record_id}'
       OR assertion_payload #>> '{outcome_revision}'
          IS DISTINCT FROM source_payload #>> '{outcome_revision}'
       OR assertion_payload #>> '{outcome_record_digest}'
          IS DISTINCT FROM source_payload #>> '{outcome_record_digest}'
       OR (assertion_payload #>> '{maximum_impact_basis_points}')::integer
          > (source_payload #>> '{maximum_impact_basis_points}')::integer THEN
        RAISE EXCEPTION 'Human Model Assertion escalated or changed promoted Candidate authority';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER idr_human_model_materialization_guard_v3
BEFORE INSERT ON idr_human_model_assertions
FOR EACH ROW EXECUTE FUNCTION idr_enforce_human_model_materialization_v3();
