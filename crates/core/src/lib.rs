//! Logika inti DevLock yang dipakai bersama oleh CLI, shim, dan (nanti) GUI.

pub mod config;
pub mod paths;
pub mod php;
pub mod project;
pub mod runtime;
pub mod version;

#[cfg(feature = "net")]
pub mod catalog;
#[cfg(feature = "net")]
pub mod http;
#[cfg(feature = "net")]
pub mod install;

pub use config::Config;
pub use paths::Paths;
pub use project::ProjectConfig;
pub use runtime::Kind;
