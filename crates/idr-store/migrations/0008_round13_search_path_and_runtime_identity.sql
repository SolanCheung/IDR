-- Round 13 closes the temporary-schema shadowing path found by the independent
-- Round 12 review.  Migration 0007 remains the immutable historical boundary;
-- this migration pins every owner-executed function before schema version 8
-- can be accepted by a Runtime.

CREATE OR REPLACE FUNCTION idr_constant_time_equal_v8(
    left_value bytea,
    right_value bytea
)
RETURNS boolean
LANGUAGE plpgsql
IMMUTABLE
STRICT
SET search_path = pg_catalog, pg_temp
AS $$
DECLARE
    difference integer := octet_length(left_value) # octet_length(right_value);
    index integer;
BEGIN
    IF octet_length(left_value) <> 32 OR octet_length(right_value) <> 32 THEN
        RETURN false;
    END IF;
    FOR index IN 0..31 LOOP
        difference := difference
            | (get_byte(left_value, index) # get_byte(right_value, index));
    END LOOP;
    RETURN difference = 0;
END;
$$;

DO $migration$
DECLARE
    trusted_schema text := current_schema();
BEGIN
    -- pg_temp is explicit and last.  It therefore cannot precede the trusted
    -- schema through PostgreSQL's implicit temporary-schema search behavior.
    EXECUTE format(
        'ALTER FUNCTION %1$I.idr_open_attested_transition_v7(
            text,text,text,uuid,uuid,uuid,text,bigint,bigint,text,text,
            bigint,integer,uuid,bigint,text
         ) SET search_path TO %1$I, pg_catalog, pg_temp',
        trusted_schema
    );
    EXECUTE format(
        'ALTER FUNCTION %1$I.idr_current_attestation_v7()
         SET search_path TO %1$I, pg_catalog, pg_temp',
        trusted_schema
    );
    EXECUTE format(
        'ALTER FUNCTION %1$I.idr_attested_authority_view_bridge_v7()
         SET search_path TO %1$I, pg_catalog, pg_temp',
        trusted_schema
    );

    -- Version 8 is the only Runtime-executable attestation opener.  It performs
    -- the MAC comparison in fixed work and never delegates authentication to
    -- the legacy text-comparison path.
    EXECUTE format($definition$
        CREATE OR REPLACE FUNCTION %1$I.idr_open_attested_transition_v8(
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
        STRICT
        SET search_path = %1$I, pg_catalog, pg_temp
        AS $function$
        DECLARE
            key_secret bytea;
            stored_key_digest text;
            expected_mac bytea;
        BEGIN
            IF claimed_database_txid IS DISTINCT FROM txid_current()::bigint
               OR claimed_backend_pid IS DISTINCT FROM pg_backend_pid()
               OR claimed_post_aggregate_version < claimed_pre_aggregate_version
               OR claimed_command_digest !~ '^[0-9a-f]{64}$'
               OR claimed_transition_digest !~ '^[0-9a-f]{64}$'
               OR claimed_mac !~ '^[0-9a-f]{64}$' THEN
                RAISE EXCEPTION
                    'IDR Orchestrator attestation transaction binding is invalid';
            END IF;

            SELECT secret, key_digest
              INTO STRICT key_secret, stored_key_digest
              FROM %1$I.idr_orchestrator_attestation_secrets_v7
             WHERE key_version = claimed_key_version
               AND retired_at IS NULL;
            IF stored_key_digest IS DISTINCT FROM
                    encode(sha256(key_secret), 'hex') THEN
                RAISE EXCEPTION
                    'IDR Orchestrator attestation key integrity failure';
            END IF;

            expected_mac := %1$I.idr_hmac_sha256_v7(
                key_secret,
                %1$I.idr_attestation_material_v7(
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
            );
            IF NOT %1$I.idr_constant_time_equal_v8(
                expected_mac,
                decode(claimed_mac, 'hex')
            ) THEN
                RAISE EXCEPTION
                    'IDR Orchestrator attestation authentication failed';
            END IF;

            INSERT INTO %1$I.idr_transition_attestations_v7 (
                attestation_id, trust_domain, environment_ref, purpose,
                command_id, run_id, tenant_ref, pre_aggregate_version,
                post_aggregate_version, command_digest, transition_digest,
                database_txid, backend_pid, nonce, key_version, mac
            ) VALUES (
                claimed_attestation_id, claimed_trust_domain,
                claimed_environment_ref, claimed_purpose,
                claimed_command_id, claimed_run_id, claimed_tenant_ref,
                claimed_pre_aggregate_version, claimed_post_aggregate_version,
                claimed_command_digest, claimed_transition_digest,
                claimed_database_txid, claimed_backend_pid, claimed_nonce,
                claimed_key_version, claimed_mac
            );
            PERFORM set_config(
                'idr.active_attestation_id',
                claimed_attestation_id::text,
                true
            );
            RETURN claimed_attestation_id;
        END;
        $function$
    $definition$, trusted_schema);
END;
$migration$;

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
BEGIN
    EXECUTE format('ALTER ROLE %I NOINHERIT', owner_role);
    EXECUTE format('ALTER ROLE %I NOINHERIT', runtime_role);
    EXECUTE format('ALTER ROLE %I NOINHERIT', auditor_role);

    EXECUTE format(
        'ALTER FUNCTION idr_constant_time_equal_v8(bytea,bytea) OWNER TO %I',
        owner_role
    );
    EXECUTE format(
        'ALTER FUNCTION idr_open_attested_transition_v8(
            text,text,text,uuid,uuid,uuid,text,bigint,bigint,text,text,
            bigint,integer,uuid,bigint,text
         ) OWNER TO %I',
        owner_role
    );

    REVOKE ALL ON FUNCTION idr_constant_time_equal_v8(bytea,bytea) FROM PUBLIC;
    REVOKE ALL ON FUNCTION idr_open_attested_transition_v8(
        text,text,text,uuid,uuid,uuid,text,bigint,bigint,text,text,
        bigint,integer,uuid,bigint,text
    ) FROM PUBLIC;
    EXECUTE format(
        'REVOKE ALL ON FUNCTION idr_open_attested_transition_v7(
            text,text,text,uuid,uuid,uuid,text,bigint,bigint,text,text,
            bigint,integer,uuid,bigint,text
         ) FROM %I',
        runtime_role
    );
    EXECUTE format(
        'GRANT EXECUTE ON FUNCTION idr_open_attested_transition_v8(
            text,text,text,uuid,uuid,uuid,text,bigint,bigint,text,text,
            bigint,integer,uuid,bigint,text
         ) TO %I',
        runtime_role
    );

    -- Production uses a dedicated database.  Revoking PUBLIC is necessary
    -- because PostgreSQL has additive privileges and has no per-role DENY.
    IF current_schema() LIKE 'idr_production_%' THEN
        EXECUTE format(
            'REVOKE TEMPORARY ON DATABASE %I FROM PUBLIC',
            current_database()
        );
    END IF;
END;
$$;

COMMENT ON FUNCTION idr_open_attested_transition_v8(
    text,text,text,uuid,uuid,uuid,text,bigint,bigint,text,text,
    bigint,integer,uuid,bigint,text
) IS
    'Sole Runtime opener: fixed search_path and constant-work HMAC comparison';
COMMENT ON FUNCTION idr_constant_time_equal_v8(bytea,bytea) IS
    'Fixed-work comparison for canonical 32-byte Orchestrator HMAC values';
