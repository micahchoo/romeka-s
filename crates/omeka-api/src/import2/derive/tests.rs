//! The derive through its public entry point, on evidence generated in
//! memory. Test names say the contract rule or the v1 assumption
//! (`docs/import-v1-assumptions.md`) they close.

#![allow(clippy::expect_used, clippy::too_many_lines)]

mod anomalies;
mod matching;
mod perf;
mod scenarios;
mod slots;
mod steps;
mod support;
