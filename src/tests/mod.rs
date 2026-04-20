//! All `#[cfg(test)]` test modules live here so source files (api.rs,
//! state.rs, util.rs, …) stay focused on production code rather than
//! padding their bottom half with assertions. Each submodule is a flat
//! `#[test]` collection; cfg(test) gates the whole tree from `main.rs`.

mod api_tests;
mod i18n_tests;
mod state_tests;
mod util_tests;
