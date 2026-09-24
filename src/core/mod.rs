//! Harness-independent TTC core.
//!
//! Execution, classification, filtering, and raw storage live below this
//! module. Nothing in this module tree may depend on a harness adapter.

mod classification;
pub(crate) mod config;
pub(crate) mod execution;
mod filters;
pub(crate) mod raw_store;
pub(crate) mod streaming;
