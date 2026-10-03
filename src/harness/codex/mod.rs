//! Codex adapter; the execution/filter core never imports this module.

mod config;
mod hook;
mod install;

pub(crate) use hook::run as hook;
pub(crate) use install::{install, uninstall};
