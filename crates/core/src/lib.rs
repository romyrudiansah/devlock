//! Logika inti DevLock yang dipakai bersama oleh CLI, shim, dan (nanti) GUI.

pub mod config;
pub mod daemon;
pub mod db;
pub mod devport;
pub mod dns;
pub mod editor;
pub mod hosts;
pub mod import;
pub mod launcher;
pub mod paths;
pub mod php;
pub mod ports;
pub mod process;
pub mod project;
pub mod resolve;
pub mod runtime;
pub mod services;
pub mod sites;
pub mod ssl;
pub mod terminal;
pub mod tools;
pub mod version;

#[cfg(feature = "net")]
pub mod catalog;
#[cfg(feature = "net")]
pub mod http;
#[cfg(feature = "net")]
pub mod install;
#[cfg(feature = "net")]
pub mod scaffold;
#[cfg(feature = "net")]
pub mod xdebug;

pub use config::Config;
pub use paths::Paths;
pub use project::ProjectConfig;
pub use runtime::Kind;
