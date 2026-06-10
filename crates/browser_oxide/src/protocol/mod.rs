//! Chrome DevTools Protocol (CDP) server for browser_oxide.
//!
//! Provides a WebSocket server that speaks CDP JSON-RPC, enabling
//! Puppeteer and Playwright to drive browser_oxide as a drop-in
//! replacement for headless Chrome.

pub mod server;
pub mod session;
pub mod types;

pub use server::CdpServer;
pub use session::CdpSession;
pub use types::*;
