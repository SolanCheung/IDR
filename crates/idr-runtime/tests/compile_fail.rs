#[test]
fn production_authority_types_cannot_be_forged_by_downstream_crates() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/*.rs");
}
