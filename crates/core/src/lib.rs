//! Logika inti DevLock yang dipakai bersama oleh CLI, shim, dan (nanti) GUI.

pub mod config;
pub mod paths;
pub mod project;

pub use config::Config;
pub use paths::Paths;
pub use project::ProjectConfig;
