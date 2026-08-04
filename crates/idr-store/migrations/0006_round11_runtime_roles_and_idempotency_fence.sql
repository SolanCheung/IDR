-- Round 11 separates Production runtime authority from migration/DDL
-- authority and makes the operation/idempotency identity independent of the
-- mutable execution lifecycle projection.

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM idr_execution_reservations LIMIT 1)
       OR EXISTS (SELECT 1 FROM idr_execution_attempts LIMIT 1) THEN
        RAISE EXCEPTION
            'Round 11 idempotency fence migration requires verified replay into a fresh schema';
    END IF;
END;
$$;

CREATE TABLE idr_operation_idempotency_fences (
    fence_id uuid PRIMARY KEY,
    trust_domain text NOT NULL CHECK (trust_domain IN ('shadow', 'production')),
    environment_ref text NOT NULL,
    tenant_ref text NOT NULL,
    operation_ref text NOT NULL,
    idempotency_key text NOT NULL,
    first_run_id uuid NOT NULL REFERENCES idr_runs(run_id),
    first_action_record_id uuid NOT NULL,
    first_action_revision bigint NOT NULL CHECK (first_action_revision > 0),
    first_action_digest text NOT NULL CHECK (length(first_action_digest) = 64),
    fence_digest text NOT NULL CHECK (length(fence_digest) = 64),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (trust_domain, environment_ref, tenant_ref, operation_ref, idempotency_key),
    FOREIGN KEY (first_action_record_id, first_action_revision)
        REFERENCES idr_contract_records(record_id, revision)
);

ALTER TABLE idr_execution_reservations
    ADD COLUMN fence_id uuid NOT NULL UNIQUE
        REFERENCES idr_operation_idempotency_fences(fence_id);

CREATE OR REPLACE FUNCTION idr_reject_idempotency_fence_mutation_v6()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION
        'Operation/idempotency fence is append-only and can never release its identity';
END;
$$;

CREATE TRIGGER idr_operation_idempotency_fence_update_guard_v6
BEFORE UPDATE ON idr_operation_idempotency_fences
FOR EACH ROW EXECUTE FUNCTION idr_reject_idempotency_fence_mutation_v6();
CREATE TRIGGER idr_operation_idempotency_fence_delete_guard_v6
BEFORE DELETE ON idr_operation_idempotency_fences
FOR EACH ROW EXECUTE FUNCTION idr_reject_idempotency_fence_mutation_v6();

CREATE OR REPLACE FUNCTION idr_validate_execution_fence_v6()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    fence idr_operation_idempotency_fences%ROWTYPE;
    run_domain text;
    run_environment text;
BEGIN
    SELECT * INTO fence
      FROM idr_operation_idempotency_fences
     WHERE fence_id = NEW.fence_id;
    SELECT trust_domain, environment_ref
      INTO run_domain, run_environment
      FROM idr_runs
     WHERE run_id = NEW.run_id;
    IF NOT FOUND
       OR NEW.fence_id IS DISTINCT FROM NEW.reservation_id
       OR fence.trust_domain IS DISTINCT FROM run_domain
       OR fence.environment_ref IS DISTINCT FROM run_environment
       OR fence.tenant_ref IS DISTINCT FROM NEW.tenant_ref
       OR fence.operation_ref IS DISTINCT FROM NEW.operation_ref
       OR fence.idempotency_key IS DISTINCT FROM NEW.idempotency_key
       OR fence.first_run_id IS DISTINCT FROM NEW.run_id
       OR fence.first_action_record_id IS DISTINCT FROM NEW.action_record_id
       OR fence.first_action_revision IS DISTINCT FROM NEW.action_revision
       OR fence.first_action_digest IS DISTINCT FROM NEW.action_digest THEN
        RAISE EXCEPTION 'Execution Reservation does not match its immutable fence';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER idr_execution_reservation_fence_guard_v6
BEFORE INSERT OR UPDATE ON idr_execution_reservations
FOR EACH ROW EXECUTE FUNCTION idr_validate_execution_fence_v6();

REVOKE ALL ON idr_operation_idempotency_fences FROM PUBLIC;

-- A production Runtime login is granted membership in this schema-specific
-- NOLOGIN role by deployment. The migrator login is deliberately not granted
-- membership. Shadow schemas share a test-only role to avoid global-role
-- accumulation during ephemeral integration tests.
DO $$
DECLARE
    runtime_role text;
    auditor_role text;
BEGIN
    runtime_role := CASE
        WHEN current_schema() LIKE 'idr_production_%'
            THEN 'idr_runtime_' || substr(md5(current_schema()), 1, 16)
        ELSE 'idr_shadow_runtime_v1'
    END;
    auditor_role := CASE
        WHEN current_schema() LIKE 'idr_production_%'
            THEN 'idr_auditor_' || substr(md5(current_schema()), 1, 16)
        ELSE 'idr_shadow_auditor_v1'
    END;
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = runtime_role) THEN
        EXECUTE format(
            'CREATE ROLE %I NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS',
            runtime_role
        );
    END IF;
    EXECUTE format(
        'ALTER ROLE %I NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS',
        runtime_role
    );
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = auditor_role) THEN
        EXECUTE format(
            'CREATE ROLE %I NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS',
            auditor_role
        );
    END IF;
    EXECUTE format(
        'ALTER ROLE %I NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS',
        auditor_role
    );
    EXECUTE format('REVOKE CREATE ON SCHEMA %I FROM %I', current_schema(), runtime_role);
    EXECUTE format('GRANT USAGE ON SCHEMA %I TO %I', current_schema(), runtime_role);
    EXECUTE format(
        'GRANT SELECT, INSERT, UPDATE ON ALL TABLES IN SCHEMA %I TO %I',
        current_schema(),
        runtime_role
    );
    EXECUTE format(
        'GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA %I TO %I',
        current_schema(),
        runtime_role
    );
    -- Identity columns are not update-authorized for Runtime at the ACL layer.
    -- The guards remain defense in depth, not the primary role boundary.
    EXECUTE format(
        'REVOKE UPDATE ON idr_execution_reservations FROM %I',
        runtime_role
    );
    EXECUTE format(
        'GRANT UPDATE (state, aggregate_version, updated_at)
           ON idr_execution_reservations TO %I',
        runtime_role
    );
    EXECUTE format(
        'REVOKE UPDATE ON idr_execution_attempts FROM %I',
        runtime_role
    );
    EXECUTE format(
        'GRANT UPDATE (state, started_at, completed_at)
           ON idr_execution_attempts TO %I',
        runtime_role
    );
    EXECUTE format(
        'REVOKE UPDATE, DELETE, TRUNCATE, REFERENCES, TRIGGER
           ON idr_operation_idempotency_fences FROM %I',
        runtime_role
    );
    EXECUTE format(
        'GRANT SELECT, INSERT ON idr_operation_idempotency_fences TO %I',
        runtime_role
    );

    EXECUTE format('REVOKE CREATE ON SCHEMA %I FROM %I', current_schema(), auditor_role);
    EXECUTE format('GRANT USAGE ON SCHEMA %I TO %I', current_schema(), auditor_role);
    EXECUTE format(
        'GRANT SELECT ON ALL TABLES IN SCHEMA %I TO %I',
        current_schema(),
        auditor_role
    );
    EXECUTE format(
        'GRANT SELECT ON ALL SEQUENCES IN SCHEMA %I TO %I',
        current_schema(),
        auditor_role
    );
END;
$$;
