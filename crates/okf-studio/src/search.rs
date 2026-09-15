//! The shared search engine, re-exported from okf-core.
//!
//! The engine — the fuzzy scorer, the omnisearch index, and the shared
//! query syntax — lives in [`okf_core::search`] so the CLI (`okf search`)
//! and the static site generator consume the exact same index and scoring
//! the studio palette does. This module keeps the `crate::search::…` paths
//! every studio caller already uses.

pub use okf_core::search::*;
