use idr_protocol::production::CandidateKindV1;
use idr_runtime::AuthoritativeRecordRefV1;
use uuid::Uuid;

fn main() {
    let _ = AuthoritativeRecordRefV1 {
        record_id: Uuid::new_v4(),
        revision: 1,
        candidate_kind: CandidateKindV1::Action,
        record_digest: "00".repeat(32),
    };
}
