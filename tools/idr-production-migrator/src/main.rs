use idr_store::PostgresIdrMigratorV1;

#[tokio::main]
async fn main() {
    let mut arguments = std::env::args().skip(1);
    let schema = arguments.next();
    if schema.is_none() || arguments.next().is_some() {
        eprintln!(
            "usage: idr-production-migrator <idr_production_schema>\n\
             requires IDR_MIGRATOR_DATABASE_URL and IDR_ORCHESTRATOR_ATTESTATION_KEY_FILE"
        );
        std::process::exit(2);
    }
    if let Err(error) =
        PostgresIdrMigratorV1::migrate_production_from_environment(&schema.unwrap()).await
    {
        eprintln!("IDR production migration failed: {error}");
        std::process::exit(1);
    }
}
