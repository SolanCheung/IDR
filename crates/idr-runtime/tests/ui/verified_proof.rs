use idr_protocol::production::{
    ProductionProofKindV1, VerifiedProductionProofV1,
};
use uuid::Uuid;

fn main() {
    let _ = VerifiedProductionProofV1 {
        proof_id: Uuid::new_v4(),
        nonce: "00".repeat(32),
        proof_kind: ProductionProofKindV1::ActionAdmission,
        trust_root_version: 1,
        trust_root_digest: "11".repeat(32),
        verified_at: 1,
    };
}
