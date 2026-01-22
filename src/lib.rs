//! AI-Native Gateway (Yali)
//!
//! A high-performance API Gateway optimized for AI workloads,
//! built on Cloudflare's Pingora framework.
//!
//! # Architecture
//!
//! The gateway uses a three-tier resource model:
//! - **Provider**: The actual AI endpoint (OpenAI, Azure, Anthropic)
//! - **Backend**: Orchestration policy (load balancing, circuit breakers, retries)
//! - **Route**: Traffic entry point (path matching, filters)
//!
//! # Example Configuration
//!
//! ```json
//! {
//!   "providers": [{
//!     "id": "provider_openai",
//!     "name": "OpenAI Production",
//!     "spec": {
//!       "type": "openai",
//!       "endpoint": "https://api.openai.com",
//!       "adapter": {
//!         "auth": { "type": "bearer", "secret_ref": "env://OPENAI_API_KEY" }
//!       }
//!     }
//!   }],
//!   "backends": [{
//!     "id": "backend_main",
//!     "name": "Main Backend",
//!     "spec": {
//!       "providers": [{ "ref": "provider_openai" }]
//!     }
//!   }],
//!   "routes": [{
//!     "id": "route_chat",
//!     "spec": {
//!       "match": { "path": "/v1/chat", "methods": ["POST"] },
//!       "backend_ref": "backend_main"
//!     }
//!   }]
//! }
//! ```

pub mod config;
pub mod proxy;
pub mod state;
