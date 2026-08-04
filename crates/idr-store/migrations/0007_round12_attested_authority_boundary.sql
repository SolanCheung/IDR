-- Round 12 makes the Rust Orchestrator the exclusive authority writer even
-- when the Production Runtime database login itself is stolen.
--
-- The Runtime role has no DML privilege on authority base tables.  It writes
-- only through identically shaped views whose SECURITY DEFINER bridge requires
-- a transaction-bound HMAC attestation opened by the Rust Orchestrator.

DO $$
DECLARE
    relation_name text;
    row_exists boolean;
    authority_relations constant text[] := ARRAY[
        'idr_runs',
        'idr_run_events',
        'idr_step_snapshots',
        'idr_contract_records',
        'idr_contract_current',
        'idr_contract_dependencies',
        'idr_contract_invalidations',
        'idr_proof_envelopes',
        'idr_proof_consumptions',
        'idr_response_send_consumptions',
        'idr_execution_reservations',
        'idr_execution_attempts',
        'idr_execution_receipts',
        'idr_outcome_records',
        'idr_human_model_candidates',
        'idr_human_model_promotion_decisions',
        'idr_human_model_assertions',
        'idr_audit_events',
        'idr_outbox_events',
        'idr_audit_checkpoints',
        'idr_command_receipts',
        'idr_operation_idempotency_fences'
    ];
BEGIN
    FOREACH relation_name IN ARRAY authority_relations LOOP
        EXECUTE format('SELECT EXISTS (SELECT 1 FROM %I LIMIT 1)', relation_name)
            INTO row_exists;
        IF row_exists THEN
            RAISE EXCEPTION
                'Round 12 authority-boundary migration requires verified replay into a fresh schema; % is not empty',
                relation_name;
        END IF;
    END LOOP;
END;
$$;

CREATE TABLE idr_orchestrator_attestation_secrets_v7 (
    key_version bigint PRIMARY KEY CHECK (key_version > 0),
    key_digest text NOT NULL CHECK (length(key_digest) = 64),
    secret bytea NOT NULL CHECK (octet_length(secret) = 32),
    activated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    retired_at timestamptz,
    CHECK (retired_at IS NULL OR retired_at >= activated_at)
);

CREATE TABLE idr_transition_attestations_v7 (
    attestation_id uuid PRIMARY KEY,
    trust_domain text NOT NULL CHECK (trust_domain IN ('shadow', 'production')),
    environment_ref text NOT NULL,
    purpose text NOT NULL CHECK (
        purpose IN (
            'command_transition',
            'governance_proof_revocation',
            'checkpoint_publication',
            'outbox_delivery'
        )
    ),
    command_id uuid NOT NULL,
    run_id uuid NOT NULL,
    tenant_ref text NOT NULL,
    pre_aggregate_version bigint NOT NULL CHECK (pre_aggregate_version >= 0),
    post_aggregate_version bigint NOT NULL CHECK (post_aggregate_version >= pre_aggregate_version),
    command_digest text NOT NULL CHECK (length(command_digest) = 64),
    transition_digest text NOT NULL CHECK (length(transition_digest) = 64),
    database_txid bigint NOT NULL,
    backend_pid integer NOT NULL,
    nonce uuid NOT NULL UNIQUE,
    key_version bigint NOT NULL
        REFERENCES idr_orchestrator_attestation_secrets_v7(key_version),
    mac text NOT NULL CHECK (length(mac) = 64),
    attested_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (purpose, command_id, transition_digest)
);

CREATE OR REPLACE FUNCTION idr_reject_attestation_mutation_v7()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'IDR Orchestrator attestations and keys are immutable';
END;
$$;

CREATE TRIGGER idr_attestation_secret_update_guard_v7
BEFORE UPDATE OR DELETE ON idr_orchestrator_attestation_secrets_v7
FOR EACH ROW EXECUTE FUNCTION idr_reject_attestation_mutation_v7();

CREATE TRIGGER idr_transition_attestation_update_guard_v7
BEFORE UPDATE OR DELETE ON idr_transition_attestations_v7
FOR EACH ROW EXECUTE FUNCTION idr_reject_attestation_mutation_v7();

CREATE OR REPLACE FUNCTION idr_hmac_sha256_v7(key_bytes bytea, payload bytea)
RETURNS bytea
LANGUAGE plpgsql
IMMUTABLE
STRICT
AS $$
DECLARE
    normalized_key bytea := key_bytes;
    key_block bytea := decode(repeat('00', 64), 'hex');
    inner_pad bytea := decode(repeat('36', 64), 'hex');
    outer_pad bytea := decode(repeat('5c', 64), 'hex');
    index integer;
BEGIN
    IF octet_length(normalized_key) > 64 THEN
        normalized_key := sha256(normalized_key);
    END IF;
    FOR index IN 0..octet_length(normalized_key) - 1 LOOP
        key_block := set_byte(key_block, index, get_byte(normalized_key, index));
    END LOOP;
    FOR index IN 0..63 LOOP
        inner_pad := set_byte(
            inner_pad,
            index,
            get_byte(inner_pad, index) # get_byte(key_block, index)
        );
        outer_pad := set_byte(
            outer_pad,
            index,
            get_byte(outer_pad, index) # get_byte(key_block, index)
        );
    END LOOP;
    RETURN sha256(outer_pad || sha256(inner_pad || payload));
END;
$$;

CREATE OR REPLACE FUNCTION idr_attestation_material_v7(
    claimed_trust_domain text,
    claimed_environment_ref text,
    claimed_purpose text,
    claimed_attestation_id uuid,
    claimed_command_id uuid,
    claimed_run_id uuid,
    claimed_tenant_ref text,
    claimed_pre_aggregate_version bigint,
    claimed_post_aggregate_version bigint,
    claimed_command_digest text,
    claimed_transition_digest text,
    claimed_database_txid bigint,
    claimed_backend_pid integer,
    claimed_nonce uuid,
    claimed_key_version bigint
)
RETURNS bytea
LANGUAGE sql
IMMUTABLE
STRICT
AS $$
    SELECT convert_to(
        'IDR-ORCHESTRATOR-ATTESTATION-V1' || chr(10)
        || claimed_trust_domain || chr(10)
        || encode(convert_to(claimed_environment_ref, 'UTF8'), 'hex') || chr(10)
        || claimed_purpose || chr(10)
        || claimed_attestation_id::text || chr(10)
        || claimed_command_id::text || chr(10)
        || claimed_run_id::text || chr(10)
        || encode(convert_to(claimed_tenant_ref, 'UTF8'), 'hex') || chr(10)
        || claimed_pre_aggregate_version::text || chr(10)
        || claimed_post_aggregate_version::text || chr(10)
        || claimed_command_digest || chr(10)
        || claimed_transition_digest || chr(10)
        || claimed_database_txid::text || chr(10)
        || claimed_backend_pid::text || chr(10)
        || claimed_nonce::text || chr(10)
        || claimed_key_version::text,
        'UTF8'
    )
$$;

CREATE OR REPLACE FUNCTION idr_open_attested_transition_v7(
    claimed_trust_domain text,
    claimed_environment_ref text,
    claimed_purpose text,
    claimed_attestation_id uuid,
    claimed_command_id uuid,
    claimed_run_id uuid,
    claimed_tenant_ref text,
    claimed_pre_aggregate_version bigint,
    claimed_post_aggregate_version bigint,
    claimed_command_digest text,
    claimed_transition_digest text,
    claimed_database_txid bigint,
    claimed_backend_pid integer,
    claimed_nonce uuid,
    claimed_key_version bigint,
    claimed_mac text
)
RETURNS uuid
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path FROM CURRENT
AS $$
DECLARE
    key_row idr_orchestrator_attestation_secrets_v7%ROWTYPE;
    expected_mac text;
BEGIN
    IF claimed_database_txid IS DISTINCT FROM txid_current()::bigint
       OR claimed_backend_pid IS DISTINCT FROM pg_backend_pid()
       OR claimed_post_aggregate_version < claimed_pre_aggregate_version
       OR length(claimed_command_digest) <> 64
       OR length(claimed_transition_digest) <> 64
       OR length(claimed_mac) <> 64 THEN
        RAISE EXCEPTION 'IDR Orchestrator attestation transaction binding is invalid';
    END IF;

    SELECT * INTO STRICT key_row
      FROM idr_orchestrator_attestation_secrets_v7
     WHERE key_version = claimed_key_version
       AND retired_at IS NULL;
    IF key_row.key_digest IS DISTINCT FROM encode(sha256(key_row.secret), 'hex') THEN
        RAISE EXCEPTION 'IDR Orchestrator attestation key integrity failure';
    END IF;

    expected_mac := encode(
        idr_hmac_sha256_v7(
            key_row.secret,
            idr_attestation_material_v7(
                claimed_trust_domain,
                claimed_environment_ref,
                claimed_purpose,
                claimed_attestation_id,
                claimed_command_id,
                claimed_run_id,
                claimed_tenant_ref,
                claimed_pre_aggregate_version,
                claimed_post_aggregate_version,
                claimed_command_digest,
                claimed_transition_digest,
                claimed_database_txid,
                claimed_backend_pid,
                claimed_nonce,
                claimed_key_version
            )
        ),
        'hex'
    );
    IF expected_mac IS DISTINCT FROM lower(claimed_mac) THEN
        RAISE EXCEPTION 'IDR Orchestrator attestation authentication failed';
    END IF;

    INSERT INTO idr_transition_attestations_v7 (
        attestation_id, trust_domain, environment_ref, purpose, command_id,
        run_id, tenant_ref, pre_aggregate_version, post_aggregate_version,
        command_digest, transition_digest, database_txid, backend_pid,
        nonce, key_version, mac
    ) VALUES (
        claimed_attestation_id, claimed_trust_domain, claimed_environment_ref,
        claimed_purpose, claimed_command_id, claimed_run_id, claimed_tenant_ref,
        claimed_pre_aggregate_version, claimed_post_aggregate_version,
        claimed_command_digest, claimed_transition_digest, claimed_database_txid,
        claimed_backend_pid, claimed_nonce, claimed_key_version, lower(claimed_mac)
    );
    PERFORM set_config(
        'idr.active_attestation_id',
        claimed_attestation_id::text,
        true
    );
    RETURN claimed_attestation_id;
END;
$$;

CREATE OR REPLACE FUNCTION idr_current_attestation_v7()
RETURNS idr_transition_attestations_v7
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path FROM CURRENT
AS $$
DECLARE
    active_id uuid;
    attestation idr_transition_attestations_v7%ROWTYPE;
BEGIN
    BEGIN
        active_id := current_setting('idr.active_attestation_id', true)::uuid;
    EXCEPTION WHEN invalid_text_representation THEN
        RAISE EXCEPTION 'IDR authority mutation has no valid Orchestrator attestation';
    END;
    IF active_id IS NULL THEN
        RAISE EXCEPTION 'IDR authority mutation has no Orchestrator attestation';
    END IF;
    SELECT * INTO STRICT attestation
      FROM idr_transition_attestations_v7
     WHERE attestation_id = active_id
       AND database_txid = txid_current()::bigint
       AND backend_pid = pg_backend_pid();
    RETURN attestation;
END;
$$;

ALTER TABLE idr_command_receipts
    ADD COLUMN receipt_mac text NOT NULL CHECK (length(receipt_mac) = 64);

CREATE OR REPLACE FUNCTION idr_execution_state_transition_allowed_v5(
    old_state text,
    new_state text
)
RETURNS boolean
LANGUAGE plpgsql
STABLE
AS $$
DECLARE
    attestation idr_transition_attestations_v7%ROWTYPE;
BEGIN
    IF old_state = new_state
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
       OR (old_state IN ('failed', 'rejected', 'expired') AND new_state = 'permit_issued') THEN
        RETURN true;
    END IF;
    IF new_state = 'reconciliation_required' THEN
        BEGIN
            attestation := idr_current_attestation_v7();
            RETURN attestation.purpose = 'governance_proof_revocation';
        EXCEPTION WHEN OTHERS THEN
            RETURN false;
        END;
    END IF;
    RETURN false;
END;
$$;

DO $$
DECLARE
    relation_name text;
    default_record record;
    authority_relations constant text[] := ARRAY[
        'idr_runs',
        'idr_run_events',
        'idr_step_snapshots',
        'idr_contract_records',
        'idr_contract_current',
        'idr_contract_dependencies',
        'idr_contract_invalidations',
        'idr_proof_envelopes',
        'idr_proof_consumptions',
        'idr_response_send_consumptions',
        'idr_execution_reservations',
        'idr_execution_attempts',
        'idr_execution_receipts',
        'idr_outcome_records',
        'idr_human_model_candidates',
        'idr_human_model_promotion_decisions',
        'idr_human_model_assertions',
        'idr_audit_events',
        'idr_outbox_events',
        'idr_audit_checkpoints',
        'idr_command_receipts',
        'idr_operation_idempotency_fences'
    ];
BEGIN
    FOREACH relation_name IN ARRAY authority_relations LOOP
        EXECUTE format(
            'ALTER TABLE %I ADD COLUMN idr_attestation_id uuid NOT NULL REFERENCES idr_transition_attestations_v7(attestation_id)',
            relation_name
        );
        EXECUTE format(
            'ALTER TABLE %I RENAME TO %I',
            relation_name,
            relation_name || '_authority_v7'
        );
        EXECUTE format(
            'CREATE VIEW %I AS SELECT * FROM %I',
            relation_name,
            relation_name || '_authority_v7'
        );
        FOR default_record IN
            SELECT attribute.attname,
                   pg_get_expr(default_value.adbin, default_value.adrelid) AS expression
              FROM pg_attribute attribute
              JOIN pg_class relation ON relation.oid = attribute.attrelid
              JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
              JOIN pg_attrdef default_value
                ON default_value.adrelid = relation.oid
               AND default_value.adnum = attribute.attnum
             WHERE namespace.nspname = current_schema()
               AND relation.relname = relation_name || '_authority_v7'
               AND attribute.attname NOT IN ('global_sequence', 'sequence')
        LOOP
            EXECUTE format(
                'ALTER VIEW %I ALTER COLUMN %I SET DEFAULT %s',
                relation_name,
                default_record.attname,
                default_record.expression
            );
        END LOOP;
    END LOOP;
END;
$$;

CREATE OR REPLACE FUNCTION idr_attested_authority_view_bridge_v7()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path FROM CURRENT
AS $$
DECLARE
    attestation idr_transition_attestations_v7%ROWTYPE;
    target_name text := TG_TABLE_NAME || '_authority_v7';
    new_value jsonb;
    existing_value jsonb;
    set_clause text;
    where_clause text;
BEGIN
    IF TG_TABLE_NAME <> ALL (ARRAY[
        'idr_runs',
        'idr_run_events',
        'idr_step_snapshots',
        'idr_contract_records',
        'idr_contract_current',
        'idr_contract_dependencies',
        'idr_contract_invalidations',
        'idr_proof_envelopes',
        'idr_proof_consumptions',
        'idr_response_send_consumptions',
        'idr_execution_reservations',
        'idr_execution_attempts',
        'idr_execution_receipts',
        'idr_outcome_records',
        'idr_human_model_candidates',
        'idr_human_model_promotion_decisions',
        'idr_human_model_assertions',
        'idr_audit_events',
        'idr_outbox_events',
        'idr_audit_checkpoints',
        'idr_command_receipts',
        'idr_operation_idempotency_fences'
    ]) THEN
        RAISE EXCEPTION 'Unrecognized IDR authority view';
    END IF;

    attestation := idr_current_attestation_v7();
    new_value := to_jsonb(NEW);
    IF new_value ? 'trust_domain'
       AND new_value ->> 'trust_domain' IS DISTINCT FROM attestation.trust_domain THEN
        RAISE EXCEPTION 'Authority row trust domain is not attested';
    END IF;
    IF new_value ? 'environment_ref'
       AND new_value ->> 'environment_ref' IS DISTINCT FROM attestation.environment_ref THEN
        RAISE EXCEPTION 'Authority row environment is not attested';
    END IF;
    IF new_value ? 'tenant_ref'
       AND attestation.tenant_ref <> '*'
       AND new_value ->> 'tenant_ref' IS DISTINCT FROM attestation.tenant_ref THEN
        RAISE EXCEPTION 'Authority row tenant is not attested';
    END IF;
    IF new_value ? 'run_id'
       AND attestation.run_id <> '00000000-0000-0000-0000-000000000000'::uuid
       AND (new_value ->> 'run_id')::uuid IS DISTINCT FROM attestation.run_id THEN
        RAISE EXCEPTION 'Authority row Run is not attested';
    END IF;
    IF new_value ? 'command_id'
       AND (new_value ->> 'command_id')::uuid IS DISTINCT FROM attestation.command_id THEN
        RAISE EXCEPTION 'Authority row command is not attested';
    END IF;
    IF new_value ? 'issued_by_command_id'
       AND (new_value ->> 'issued_by_command_id')::uuid
            IS DISTINCT FROM attestation.command_id THEN
        RAISE EXCEPTION 'Authority Contract issuer command is not attested';
    END IF;
    IF TG_TABLE_NAME = 'idr_step_snapshots'
       AND (new_value ->> 'step_id')::uuid IS DISTINCT FROM attestation.command_id THEN
        RAISE EXCEPTION 'Authority step is not bound to its attested command';
    END IF;
    IF TG_TABLE_NAME = 'idr_command_receipts'
       AND (new_value ->> 'command_id')::uuid IS DISTINCT FROM attestation.command_id THEN
        RAISE EXCEPTION 'Authority Receipt is not bound to its attested command';
    END IF;

    NEW.idr_attestation_id := attestation.attestation_id;

    IF TG_OP = 'UPDATE' THEN
        SELECT string_agg(
                   format(
                       '%1$I = (jsonb_populate_record(NULL::%2$I, $1)).%1$I',
                       attribute.attname,
                       target_name
                   ),
                   ', ' ORDER BY attribute.attnum
               )
          INTO set_clause
          FROM pg_attribute attribute
          JOIN pg_class relation ON relation.oid = attribute.attrelid
          JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
         WHERE namespace.nspname = current_schema()
           AND relation.relname = target_name
           AND attribute.attnum > 0
           AND NOT attribute.attisdropped;
        SELECT string_agg(
                   format(
                       'target.%1$I IS NOT DISTINCT FROM (jsonb_populate_record(NULL::%2$I, $2)).%1$I',
                       attribute.attname,
                       target_name
                   ),
                   ' AND '
               )
          INTO where_clause
          FROM pg_index index_record
          JOIN pg_class relation ON relation.oid = index_record.indrelid
          JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
          JOIN pg_attribute attribute
            ON attribute.attrelid = relation.oid
           AND attribute.attnum = ANY(index_record.indkey)
         WHERE namespace.nspname = current_schema()
           AND relation.relname = target_name
           AND index_record.indisprimary;
        IF where_clause IS NULL THEN
            RAISE EXCEPTION 'Authority base table has no primary key';
        END IF;
        EXECUTE format(
            'UPDATE %I target SET %s WHERE %s RETURNING target.*',
            target_name,
            set_clause,
            where_clause
        )
        INTO NEW
        USING to_jsonb(NEW), to_jsonb(OLD);
        IF NOT FOUND THEN
            RETURN NULL;
        END IF;
        RETURN NEW;
    END IF;

    new_value := to_jsonb(NEW);
    IF TG_TABLE_NAME = 'idr_run_events'
       AND new_value -> 'global_sequence' = 'null'::jsonb THEN
        NEW := jsonb_populate_record(
            NEW,
            jsonb_build_object(
                'global_sequence',
                nextval(
                    pg_get_serial_sequence(
                        'idr_run_events_authority_v7',
                        'global_sequence'
                    )
                )
            )
        );
    ELSIF TG_TABLE_NAME = 'idr_audit_events'
          AND new_value -> 'sequence' = 'null'::jsonb THEN
        NEW := jsonb_populate_record(
            NEW,
            jsonb_build_object(
                'sequence',
                nextval(
                    pg_get_serial_sequence(
                        'idr_audit_events_authority_v7',
                        'sequence'
                    )
                )
            )
        );
    END IF;

    BEGIN
        EXECUTE format(
            'INSERT INTO %I SELECT ($1).* RETURNING *',
            target_name
        )
        INTO NEW
        USING NEW;
        RETURN NEW;
    EXCEPTION WHEN unique_violation THEN
        IF TG_TABLE_NAME = 'idr_runs' THEN
            SELECT to_jsonb(existing) INTO existing_value
              FROM idr_runs_authority_v7 existing
             WHERE existing.run_id = NEW.run_id
             FOR UPDATE;
            IF existing_value ->> 'trust_domain' IS DISTINCT FROM NEW.trust_domain
               OR existing_value ->> 'environment_ref' IS DISTINCT FROM NEW.environment_ref
               OR (existing_value ->> 'aggregate_version')::bigint + 1
                    IS DISTINCT FROM NEW.aggregate_version THEN
                RAISE EXCEPTION 'Attested Run aggregate update is invalid';
            END IF;
            UPDATE idr_runs_authority_v7
               SET aggregate_version = NEW.aggregate_version,
                   state = NEW.state,
                   projection = NEW.projection,
                   projection_digest = NEW.projection_digest,
                   last_event_sequence = NEW.last_event_sequence,
                   updated_at = clock_timestamp(),
                   idr_attestation_id = attestation.attestation_id
             WHERE run_id = NEW.run_id
             RETURNING * INTO NEW;
            RETURN NEW;
        ELSIF TG_TABLE_NAME = 'idr_contract_current' THEN
            SELECT to_jsonb(existing) INTO existing_value
              FROM idr_contract_current_authority_v7 existing
             WHERE existing.run_id = NEW.run_id
               AND existing.candidate_kind = NEW.candidate_kind
             FOR UPDATE;
            IF (existing_value ->> 'revision')::bigint + 1
                    IS DISTINCT FROM NEW.revision THEN
                RAISE EXCEPTION 'Attested current Contract revision is invalid';
            END IF;
            UPDATE idr_contract_current_authority_v7
               SET record_id = NEW.record_id,
                   revision = NEW.revision,
                   record_digest = NEW.record_digest,
                   idr_attestation_id = attestation.attestation_id
             WHERE run_id = NEW.run_id
               AND candidate_kind = NEW.candidate_kind
             RETURNING * INTO NEW;
            RETURN NEW;
        ELSIF TG_TABLE_NAME = 'idr_execution_reservations' THEN
            SELECT to_jsonb(existing) INTO existing_value
              FROM idr_execution_reservations_authority_v7 existing
             WHERE existing.reservation_id = NEW.reservation_id
             FOR UPDATE;
            IF (existing_value ->> 'aggregate_version')::bigint
                    >= NEW.aggregate_version THEN
                RAISE EXCEPTION 'Attested execution Reservation version did not advance';
            END IF;
            UPDATE idr_execution_reservations_authority_v7
               SET state = NEW.state,
                   aggregate_version = NEW.aggregate_version,
                   updated_at = clock_timestamp(),
                   idr_attestation_id = attestation.attestation_id
             WHERE reservation_id = NEW.reservation_id
             RETURNING * INTO NEW;
            RETURN NEW;
        ELSIF TG_TABLE_NAME = 'idr_execution_attempts' THEN
            UPDATE idr_execution_attempts_authority_v7
               SET state = NEW.state,
                   started_at = COALESCE(started_at, NEW.started_at),
                   completed_at = COALESCE(completed_at, NEW.completed_at),
                   idr_attestation_id = attestation.attestation_id
             WHERE reservation_id = NEW.reservation_id
               AND attempt = NEW.attempt
               AND permit_id = NEW.permit_id
               AND dispatch_nonce = NEW.dispatch_nonce
               AND provider_ref = NEW.provider_ref
               AND owner_ref = NEW.owner_ref
             RETURNING * INTO NEW;
            IF NOT FOUND THEN
                RAISE EXCEPTION 'Attested execution Attempt identity changed';
            END IF;
            RETURN NEW;
        ELSIF TG_TABLE_NAME IN (
            'idr_contract_invalidations',
            'idr_operation_idempotency_fences'
        ) THEN
            RETURN NULL;
        END IF;
        RAISE;
    END;
END;
$$;

DO $$
DECLARE
    relation_name text;
    authority_relations constant text[] := ARRAY[
        'idr_runs',
        'idr_run_events',
        'idr_step_snapshots',
        'idr_contract_records',
        'idr_contract_current',
        'idr_contract_dependencies',
        'idr_contract_invalidations',
        'idr_proof_envelopes',
        'idr_proof_consumptions',
        'idr_response_send_consumptions',
        'idr_execution_reservations',
        'idr_execution_attempts',
        'idr_execution_receipts',
        'idr_outcome_records',
        'idr_human_model_candidates',
        'idr_human_model_promotion_decisions',
        'idr_human_model_assertions',
        'idr_audit_events',
        'idr_outbox_events',
        'idr_audit_checkpoints',
        'idr_command_receipts',
        'idr_operation_idempotency_fences'
    ];
BEGIN
    FOREACH relation_name IN ARRAY authority_relations LOOP
        EXECUTE format(
            'CREATE TRIGGER idr_attested_authority_bridge_v7
             INSTEAD OF INSERT OR UPDATE ON %I
             FOR EACH ROW EXECUTE FUNCTION idr_attested_authority_view_bridge_v7()',
            relation_name
        );
    END LOOP;
END;
$$;

DO $$
DECLARE
    role_digest text := substr(
        encode(sha256(convert_to(current_schema(), 'UTF8')), 'hex'),
        1,
        32
    );
    owner_role text := 'idr_owner_' || role_digest;
    runtime_role text := 'idr_runtime_' || role_digest;
    auditor_role text := 'idr_auditor_' || role_digest;
    old_runtime_role text := CASE
        WHEN current_schema() LIKE 'idr_production_%'
            THEN 'idr_runtime_' || substr(md5(current_schema()), 1, 16)
        ELSE 'idr_shadow_runtime_v1'
    END;
    old_auditor_role text := CASE
        WHEN current_schema() LIKE 'idr_production_%'
            THEN 'idr_auditor_' || substr(md5(current_schema()), 1, 16)
        ELSE 'idr_shadow_auditor_v1'
    END;
    relation_record record;
    function_record record;
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = owner_role) THEN
        EXECUTE format(
            'CREATE ROLE %I NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS',
            owner_role
        );
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = runtime_role) THEN
        EXECUTE format(
            'CREATE ROLE %I NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS',
            runtime_role
        );
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = auditor_role) THEN
        EXECUTE format(
            'CREATE ROLE %I NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS',
            auditor_role
        );
    END IF;
    EXECUTE format('GRANT %I TO %I', owner_role, current_user);

    EXECUTE format('REVOKE ALL ON ALL TABLES IN SCHEMA %I FROM PUBLIC', current_schema());
    EXECUTE format('REVOKE ALL ON ALL SEQUENCES IN SCHEMA %I FROM PUBLIC', current_schema());
    EXECUTE format('REVOKE ALL ON ALL FUNCTIONS IN SCHEMA %I FROM PUBLIC', current_schema());
    EXECUTE format(
        'REVOKE ALL ON ALL TABLES IN SCHEMA %I FROM %I',
        current_schema(),
        old_runtime_role
    );
    EXECUTE format(
        'REVOKE ALL ON ALL SEQUENCES IN SCHEMA %I FROM %I',
        current_schema(),
        old_runtime_role
    );
    EXECUTE format(
        'REVOKE ALL ON ALL TABLES IN SCHEMA %I FROM %I',
        current_schema(),
        old_auditor_role
    );

    FOR relation_record IN
        SELECT relation.relname, relation.relkind
          FROM pg_class relation
          JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
         WHERE namespace.nspname = current_schema()
           AND relation.relkind IN ('r', 'p', 'v')
    LOOP
        IF relation_record.relkind IN ('r', 'p', 'v') THEN
            EXECUTE format(
                'ALTER %s %I OWNER TO %I',
                CASE
                    WHEN relation_record.relkind = 'v' THEN 'VIEW'
                    ELSE 'TABLE'
                END,
                relation_record.relname,
                owner_role
            );
        END IF;
    END LOOP;
    FOR function_record IN
        SELECT procedure.proname,
               pg_get_function_identity_arguments(procedure.oid) AS arguments
          FROM pg_proc procedure
          JOIN pg_namespace namespace ON namespace.oid = procedure.pronamespace
         WHERE namespace.nspname = current_schema()
    LOOP
        EXECUTE format(
            'ALTER FUNCTION %I(%s) OWNER TO %I',
            function_record.proname,
            function_record.arguments,
            owner_role
        );
    END LOOP;

    EXECUTE format('REVOKE CREATE ON SCHEMA %I FROM PUBLIC', current_schema());
    EXECUTE format('GRANT USAGE ON SCHEMA %I TO %I', current_schema(), owner_role);
    EXECUTE format('REVOKE CREATE ON SCHEMA %I FROM %I', current_schema(), runtime_role);
    EXECUTE format('REVOKE CREATE ON SCHEMA %I FROM %I', current_schema(), auditor_role);
    EXECUTE format('GRANT USAGE ON SCHEMA %I TO %I', current_schema(), runtime_role);
    EXECUTE format('GRANT USAGE ON SCHEMA %I TO %I', current_schema(), auditor_role);
    EXECUTE format(
        'GRANT SELECT, INSERT, UPDATE ON ALL TABLES IN SCHEMA %I TO %I',
        current_schema(),
        runtime_role
    );
    EXECUTE format(
        'REVOKE ALL ON idr_orchestrator_attestation_secrets_v7 FROM %I',
        runtime_role
    );
    EXECUTE format(
        'REVOKE INSERT, UPDATE, DELETE, TRUNCATE, REFERENCES, TRIGGER
           ON idr_transition_attestations_v7 FROM %I',
        runtime_role
    );
    EXECUTE format('REVOKE ALL ON _sqlx_migrations FROM %I', runtime_role);
    EXECUTE format('GRANT SELECT ON _sqlx_migrations TO %I', runtime_role);
    FOR relation_record IN
        SELECT relation.relname
          FROM pg_class relation
          JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
         WHERE namespace.nspname = current_schema()
           AND relation.relkind IN ('r', 'p')
           AND relation.relname LIKE '%\_authority\_v7' ESCAPE '\'
    LOOP
        EXECUTE format('REVOKE ALL ON %I FROM %I', relation_record.relname, runtime_role);
    END LOOP;
    EXECUTE format(
        'GRANT EXECUTE ON FUNCTION idr_open_attested_transition_v7(
            text,text,text,uuid,uuid,uuid,text,bigint,bigint,text,text,bigint,integer,uuid,bigint,text
         ) TO %I',
        runtime_role
    );

    EXECUTE format(
        'GRANT SELECT ON ALL TABLES IN SCHEMA %I TO %I',
        current_schema(),
        auditor_role
    );
    EXECUTE format(
        'REVOKE ALL ON idr_orchestrator_attestation_secrets_v7 FROM %I',
        auditor_role
    );
    FOR relation_record IN
        SELECT relation.relname
          FROM pg_class relation
          JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
         WHERE namespace.nspname = current_schema()
           AND relation.relkind IN ('r', 'p')
           AND relation.relname LIKE '%\_authority\_v7' ESCAPE '\'
    LOOP
        EXECUTE format('REVOKE ALL ON %I FROM %I', relation_record.relname, auditor_role);
    END LOOP;

    EXECUTE format(
        'ALTER DEFAULT PRIVILEGES FOR ROLE %I
         REVOKE ALL ON TABLES FROM PUBLIC',
        owner_role
    );
    EXECUTE format(
        'ALTER DEFAULT PRIVILEGES FOR ROLE %I
         REVOKE ALL ON SEQUENCES FROM PUBLIC',
        owner_role
    );
    EXECUTE format(
        'ALTER DEFAULT PRIVILEGES FOR ROLE %I
         REVOKE EXECUTE ON FUNCTIONS FROM PUBLIC',
        owner_role
    );
END;
$$;

COMMENT ON TABLE idr_orchestrator_attestation_secrets_v7 IS
    'Owner-only Orchestrator HMAC key material; never readable by Runtime or Auditor roles';
COMMENT ON TABLE idr_transition_attestations_v7 IS
    'Immutable transaction-bound Orchestrator authority attestations';
COMMENT ON FUNCTION idr_open_attested_transition_v7(
    text,text,text,uuid,uuid,uuid,text,bigint,bigint,text,text,bigint,integer,uuid,bigint,text
) IS
    'The sole Runtime-executable opener for an HMAC-authenticated authority mutation transaction';
