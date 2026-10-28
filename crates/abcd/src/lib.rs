//! abcd: the abc server. Owns all state and does all filesystem and git
//! access; clients act only through it, over a Unix socket.

pub mod files;
pub mod git;
pub mod github;
pub mod registry;
pub mod server;
pub mod state;
