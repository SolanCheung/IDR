//! IDR transactional persistence boundaries.

#[cfg(all(feature = "production", feature = "dev-file-store"))]
compile_error!(
    "IDR production and dev-file-store are mutually exclusive; use --no-default-features --features production"
);

#[cfg(all(feature = "production", feature = "shadow-mode"))]
compile_error!("IDR production and shadow-mode are mutually exclusive");

#[cfg(all(feature = "production", feature = "migration"))]
compile_error!("IDR production runtime and migration authority are mutually exclusive");

#[cfg(feature = "postgres-store")]
pub use idr_runtime::postgres_authority::*;

#[cfg(feature = "dev-file-store")]
mod file_store;
#[cfg(feature = "dev-file-store")]
pub use file_store::*;
