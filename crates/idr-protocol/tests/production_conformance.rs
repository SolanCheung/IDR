use idr_protocol::production::{
    canonical_digest_v1, canonical_json_bytes_v1, ExpectedProductionProofV1, ProductionIssuerKeyV1,
    ProductionProofClaimsV1, ProductionProofEnvelopeV1, ProductionProofKindV1,
    ProductionReleaseGatesV1, TrustDomainV1, TrustRootSnapshotV1,
};
use idr_protocol::ReferenceV1;
use serde_json::Value;
use std::collections::BTreeSet;

fn reference(value: &str) -> ReferenceV1 {
    ReferenceV1::new(value).unwrap()
}

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../contracts/production/v1/canonical-golden-v1.json"
    ))
    .unwrap()
}

#[test]
fn every_production_release_gate_is_fail_closed() {
    let gates = ProductionReleaseGatesV1::BLOCKED;
    assert!(!gates.trust_chain_closure_is_complete());
    assert!(gates.production_trust_root_is_blocked());
    assert!(!gates.production_authorization_is_enabled());
    assert!(!gates.production_execution_is_enabled());
    assert!(!gates.human_model_long_term_write_is_enabled());
    assert!(!gates.aegis_production_adapter_is_enabled());
}

#[test]
fn rust_golden_vector_is_self_consistent_and_signature_verifies() {
    let fixture = fixture();
    let value = &fixture["canonical_value"];
    let canonical = String::from_utf8(canonical_json_bytes_v1(value).unwrap()).unwrap();
    assert_eq!(canonical, fixture["expected_canonical_utf8"]);
    assert_eq!(
        canonical_digest_v1(fixture["digest_domain"].as_str().unwrap(), value).unwrap(),
        fixture["expected_digest"]
    );

    let claims: ProductionProofClaimsV1 =
        serde_json::from_value(fixture["proof_claims"].clone()).unwrap();
    assert_eq!(
        String::from_utf8(claims.canonical_signing_bytes().unwrap()).unwrap(),
        fixture["expected_signing_utf8"]
    );
    let envelope = ProductionProofEnvelopeV1::new(
        claims.clone(),
        reference("key:golden:v1"),
        fixture["signature_hex"].as_str().unwrap(),
    )
    .unwrap();
    let key = ProductionIssuerKeyV1::new(
        TrustDomainV1::Shadow,
        reference("environment:idr:shadow:golden"),
        reference("issuer:golden"),
        reference("key:golden:v1"),
        fixture["public_key_hex"].as_str().unwrap(),
        reference("audience:idr:shadow:golden"),
        reference("tenant:golden"),
        BTreeSet::from([ProductionProofKindV1::InputAdmission]),
        1_799_999_900,
        1_800_000_600,
        None,
    )
    .unwrap();
    let root = TrustRootSnapshotV1::new(
        TrustDomainV1::Shadow,
        reference("environment:idr:shadow:golden"),
        1,
        1_800_000_000,
        vec![key],
    )
    .unwrap();
    let expected = ExpectedProductionProofV1 {
        trust_domain: TrustDomainV1::Shadow,
        environment_ref: reference("environment:idr:shadow:golden"),
        proof_kind: ProductionProofKindV1::InputAdmission,
        issuer_ref: reference("issuer:golden"),
        subject_ref: claims.subject_ref().clone(),
        subject_digest: claims.subject_digest().to_string(),
        audience_ref: reference("audience:idr:shadow:golden"),
        tenant_ref: reference("tenant:golden"),
        scope_ref: reference("scope:interaction"),
        purpose_ref: reference("purpose:conformance"),
        policy_revision_ref: reference("policy:idr:v1"),
    };
    root.verify_at(&envelope, &expected, 1_800_000_100).unwrap();

    let mut foreign_environment = expected.clone();
    foreign_environment.environment_ref = reference("environment:idr:shadow:foreign");
    assert!(
        root.verify_at(&envelope, &foreign_environment, 1_800_000_100)
            .is_err(),
        "a signed proof must not verify in another environment"
    );
    let mut foreign_domain = expected;
    foreign_domain.trust_domain = TrustDomainV1::Production;
    assert!(
        root.verify_at(&envelope, &foreign_domain, 1_800_000_100)
            .is_err(),
        "a signed Shadow proof must not verify in Production"
    );
}

#[test]
fn canonical_sorting_uses_utf16_code_units_and_floats_fail_closed() {
    assert_eq!(
        String::from_utf8(
            canonical_json_bytes_v1(&serde_json::json!({"\u{e000}": 2, "😀": 1})).unwrap()
        )
        .unwrap(),
        "{\"😀\":1,\"\":2}"
    );
    assert!(canonical_json_bytes_v1(&serde_json::json!({"unsafe": 1.5})).is_err());
    assert!(
        canonical_json_bytes_v1(&serde_json::json!({"unsafe": 9_007_199_254_740_992_u64})).is_err()
    );
    assert!(
        canonical_json_bytes_v1(&serde_json::json!({"unsafe": -9_007_199_254_740_992_i64}))
            .is_err()
    );
}

#[test]
fn deterministic_mutation_fuzz_preserves_canonical_and_digest_stability() {
    let mut state = 0x8a5c_d789_635d_2dff_u64;
    for case in 0..1_024_u64 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let safe_value = (state % 18_014_398_509_481_983) as i64 - 9_007_199_254_740_991_i64;
        let value = serde_json::json!({
            format!("key-\u{e000}-{case}"): safe_value,
            format!("key-😀-{case}"): [case % 97, state & 1 == 1],
            "escaped": format!("mutation-{case}\\\"\n"),
        });
        let first = canonical_json_bytes_v1(&value).unwrap();
        let second = canonical_json_bytes_v1(&value).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            canonical_digest_v1("idr:test:mutation:v1", &value).unwrap(),
            canonical_digest_v1("idr:test:mutation:v1", &value).unwrap()
        );
        serde_json::from_slice::<Value>(&first).unwrap();
    }
}
