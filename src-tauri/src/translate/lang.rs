//! `Lang` moved verbatim into the Tauri-free `echo-config` crate (it is a field
//! type of `AppSettings`). Re-exported here so `crate::translate::lang::Lang`
//! and `crate::translate::Lang` call sites keep resolving unchanged.
pub use echo_config::Lang;
