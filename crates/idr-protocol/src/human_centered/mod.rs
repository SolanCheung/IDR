//! Protocol layer for The Human-Centered Intent & Decision Runtime (IDR).
//!
//! This module is the passive constitutional layer for user-facing semantic
//! contracts. It deliberately does not call models, read repositories, grant
//! authority, render host output, or execute external effects.

mod action;
mod common;
mod decision;
mod execution;
mod guards;
mod human_model;
mod input;
mod intent;
mod outcome;
mod response;
mod runtime;
mod trust;
mod turn;

pub use action::*;
pub use common::*;
pub use decision::*;
pub use execution::*;
pub use guards::*;
pub use human_model::*;
pub use input::*;
pub use intent::*;
pub use outcome::*;
pub use response::*;
pub use runtime::*;
pub use trust::*;
pub use turn::*;
