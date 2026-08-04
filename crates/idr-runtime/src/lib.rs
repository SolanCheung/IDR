//! Command-driven IDR authority runtime.
//!
//! Production mutation is exposed only through the Orchestrator and its
//! transactional repository capability boundary.

#[cfg(all(feature = "production", feature = "test-support"))]
compile_error!("IDR production and test-support are mutually exclusive");

#[cfg(all(feature = "production", feature = "shadow-mode"))]
compile_error!("IDR production and shadow-mode are mutually exclusive");

#[cfg(all(feature = "production", feature = "migration"))]
compile_error!("IDR production runtime and migration authority are mutually exclusive");

#[cfg(all(feature = "shadow-mode", feature = "migration"))]
compile_error!("IDR Shadow runtime and migration authority are mutually exclusive");

mod trust_chain;

pub use trust_chain::*;

#[cfg(feature = "postgres-authority")]
pub mod postgres_authority;
#[cfg(feature = "postgres-authority")]
pub use postgres_authority::*;

#[cfg(feature = "legacy-shadow-api")]
mod legacy;
#[cfg(feature = "legacy-shadow-api")]
pub use legacy::*;

#[cfg(feature = "legacy-shadow-api")]
mod orchestrator;
#[cfg(feature = "legacy-shadow-api")]
pub use orchestrator::*;
