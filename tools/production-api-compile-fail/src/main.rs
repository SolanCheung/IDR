use idr_runtime::PostgresIdrOrchestratorV1;
use uuid::Uuid;

async fn attempt_production_bypasses(orchestrator: &PostgresIdrOrchestratorV1) {
    let _ = orchestrator.pool_for_test();
    let _ = orchestrator.inspect_for_test(Uuid::nil()).await;
    let _ = orchestrator.verify_external_anchor_for_test().await;
}

fn main() {}
