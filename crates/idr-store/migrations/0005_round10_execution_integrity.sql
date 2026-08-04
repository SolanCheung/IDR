-- Round 10 closes the mutable Execution identity/index gap. Execution
-- lifecycle rows remain updatable only through a constrained state transition;
-- identity, exactly-once keys and permit bindings are immutable.

ALTER TABLE idr_audit_checkpoints
    ADD COLUMN IF NOT EXISTS execution_state_root text NOT NULL
        DEFAULT repeat('0', 64)
        CHECK (length(execution_state_root) = 64);
ALTER TABLE idr_audit_checkpoints
    ALTER COLUMN execution_state_root DROP DEFAULT;

CREATE OR REPLACE FUNCTION idr_execution_state_transition_allowed_v5(
    old_state text,
    new_state text
)
RETURNS boolean LANGUAGE sql IMMUTABLE AS $$
    SELECT old_state = new_state
        OR (old_state = 'reservation_created' AND new_state = 'permit_issued')
        OR (old_state = 'permit_issued'
            AND new_state IN ('permit_delivered', 'expired', 'cancelled'))
        OR (old_state = 'permit_delivered'
            AND new_state IN ('dispatch_started', 'expired', 'cancelled'))
        OR (old_state IN ('dispatch_started', 'awaiting_provider')
            AND new_state IN (
                'reconciliation_required', 'succeeded', 'failed', 'rejected', 'compensated'
            ))
        OR (old_state = 'reconciliation_required'
            AND new_state IN ('succeeded', 'failed', 'rejected', 'compensated'))
        OR (old_state IN ('failed', 'rejected', 'expired') AND new_state = 'permit_issued');
$$;

CREATE OR REPLACE FUNCTION idr_guard_execution_reservation_update_v5()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.reservation_id IS DISTINCT FROM OLD.reservation_id
       OR NEW.run_id IS DISTINCT FROM OLD.run_id
       OR NEW.tenant_ref IS DISTINCT FROM OLD.tenant_ref
       OR NEW.action_record_id IS DISTINCT FROM OLD.action_record_id
       OR NEW.action_revision IS DISTINCT FROM OLD.action_revision
       OR NEW.action_admission_record_id IS DISTINCT FROM OLD.action_admission_record_id
       OR NEW.action_admission_revision IS DISTINCT FROM OLD.action_admission_revision
       OR NEW.operation_ref IS DISTINCT FROM OLD.operation_ref
       OR NEW.idempotency_key IS DISTINCT FROM OLD.idempotency_key
       OR NEW.provider_ref IS DISTINCT FROM OLD.provider_ref
       OR NEW.owner_ref IS DISTINCT FROM OLD.owner_ref
       OR NEW.action_digest IS DISTINCT FROM OLD.action_digest
       OR NEW.parameter_digest IS DISTINCT FROM OLD.parameter_digest
       OR NEW.request_digest IS DISTINCT FROM OLD.request_digest
       OR NEW.exact_authorization_proof_id IS DISTINCT FROM OLD.exact_authorization_proof_id
       OR NEW.action_valid_until IS DISTINCT FROM OLD.action_valid_until
       OR NEW.admission_valid_until IS DISTINCT FROM OLD.admission_valid_until
       OR NEW.authorization_valid_until IS DISTINCT FROM OLD.authorization_valid_until
       OR NEW.created_at IS DISTINCT FROM OLD.created_at THEN
        RAISE EXCEPTION 'Execution reservation identity and exactly-once key are immutable';
    END IF;
    IF NEW.aggregate_version <> OLD.aggregate_version + 1
       OR NEW.updated_at < OLD.updated_at
       OR NOT idr_execution_state_transition_allowed_v5(OLD.state, NEW.state) THEN
        RAISE EXCEPTION 'Execution reservation update is not a contiguous legal transition';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER idr_execution_reservation_update_guard_v5
BEFORE UPDATE ON idr_execution_reservations
FOR EACH ROW EXECUTE FUNCTION idr_guard_execution_reservation_update_v5();
CREATE TRIGGER idr_execution_reservation_delete_guard_v5
BEFORE DELETE ON idr_execution_reservations
FOR EACH ROW EXECUTE FUNCTION idr_reject_authority_mutation_v4();

CREATE OR REPLACE FUNCTION idr_guard_execution_attempt_update_v5()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.reservation_id IS DISTINCT FROM OLD.reservation_id
       OR NEW.attempt IS DISTINCT FROM OLD.attempt
       OR NEW.permit_id IS DISTINCT FROM OLD.permit_id
       OR NEW.dispatch_nonce IS DISTINCT FROM OLD.dispatch_nonce
       OR NEW.lease_until IS DISTINCT FROM OLD.lease_until
       OR NEW.permit_valid_until IS DISTINCT FROM OLD.permit_valid_until
       OR NEW.provider_ref IS DISTINCT FROM OLD.provider_ref
       OR NEW.owner_ref IS DISTINCT FROM OLD.owner_ref THEN
        RAISE EXCEPTION 'Execution attempt identity and Permit binding are immutable';
    END IF;
    IF NOT idr_execution_state_transition_allowed_v5(OLD.state, NEW.state)
       OR (OLD.started_at IS NOT NULL AND NEW.started_at IS DISTINCT FROM OLD.started_at)
       OR (OLD.completed_at IS NOT NULL AND NEW.completed_at IS DISTINCT FROM OLD.completed_at)
       OR (NEW.completed_at IS NOT NULL AND NEW.started_at IS NULL) THEN
        RAISE EXCEPTION 'Execution attempt update is not a legal monotonic transition';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER idr_execution_attempt_update_guard_v5
BEFORE UPDATE ON idr_execution_attempts
FOR EACH ROW EXECUTE FUNCTION idr_guard_execution_attempt_update_v5();
CREATE TRIGGER idr_execution_attempt_delete_guard_v5
BEFORE DELETE ON idr_execution_attempts
FOR EACH ROW EXECUTE FUNCTION idr_reject_authority_mutation_v4();

CREATE OR REPLACE FUNCTION idr_guard_checkpoint_update_v5()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.checkpoint_id IS DISTINCT FROM OLD.checkpoint_id
       OR NEW.trust_domain IS DISTINCT FROM OLD.trust_domain
       OR NEW.environment_ref IS DISTINCT FROM OLD.environment_ref
       OR NEW.tenant_ref IS DISTINCT FROM OLD.tenant_ref
       OR NEW.audit_sequence IS DISTINCT FROM OLD.audit_sequence
       OR NEW.chain_root IS DISTINCT FROM OLD.chain_root
       OR NEW.record_set_root IS DISTINCT FROM OLD.record_set_root
       OR NEW.execution_state_root IS DISTINCT FROM OLD.execution_state_root
       OR NEW.signed_checkpoint IS DISTINCT FROM OLD.signed_checkpoint
       OR NEW.created_at IS DISTINCT FROM OLD.created_at
       OR NEW.published_at IS NULL
       OR (OLD.published_at IS NOT NULL AND NEW.published_at < OLD.published_at) THEN
        RAISE EXCEPTION 'Checkpoint identity is immutable; publication time is monotonic';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER idr_audit_checkpoint_update_guard_v5
BEFORE UPDATE ON idr_audit_checkpoints
FOR EACH ROW EXECUTE FUNCTION idr_guard_checkpoint_update_v5();
CREATE TRIGGER idr_audit_checkpoint_delete_guard_v5
BEFORE DELETE ON idr_audit_checkpoints
FOR EACH ROW EXECUTE FUNCTION idr_reject_authority_mutation_v4();

REVOKE DELETE ON idr_execution_reservations FROM PUBLIC;
REVOKE DELETE ON idr_execution_attempts FROM PUBLIC;
REVOKE DELETE ON idr_audit_checkpoints FROM PUBLIC;
