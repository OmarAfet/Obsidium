//! Obsidium Minecraft Server Library
//!
//! A high-performance, modular Minecraft server implementation written in Rust.
//!
//! This library provides a complete implementation of the Minecraft Java Edition
//! protocol (version 1.21.5, protocol 770) with a focus on performance,
//! scalability, and maintainability.
//!
//! # Architecture
//!
//! The server is organized into several key modules:
//!
//! - [`protocol`] - Minecraft protocol implementation including packet handling,
//!   data types, and compression
//! - [`network`] - Low-level networking layer for connection management
//! - [`game`] - Game logic including players, worlds, and entities
//! - [`server`] - Core server implementation and orchestration
//! - [`config`] - Configuration management
//!
//! # Example
//!
//! ```rust,no_run
//! use obsidium::server::MinecraftServer;
//! use obsidium::config::ServerConfig;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ServerConfig::default();
//!     let server = MinecraftServer::new(config).await?;
//!     server.run().await?;
//!     Ok(())
//! }
//! ```

#![deny(clippy::too_many_lines, missing_docs, clippy::panic)]

pub mod config;
pub mod data;
pub mod error;
pub mod game;
pub mod logger;
pub mod network;
pub mod protocol;
pub mod server;

pub use error::{Result, ServerError};
