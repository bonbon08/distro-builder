//! Shared logic for Distro Builder: project configuration, archiso profile
//! generation, and asset (Ollama model / container image) fetching.

pub mod config;
pub mod fetch;
pub mod installer_conf;
pub mod profile;

pub use config::Config;
