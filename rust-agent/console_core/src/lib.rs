//! Pure (no DOM) console helpers — unit-tested on host, used by WASM client.

mod events;
mod markdown;
mod sse;

pub use events::{fold_run_events, run_event_from_sse, RunEvent, RunKind};
pub use markdown::render_markdown;
pub use sse::{feed_sse_buffer, parse_sse_data_line, SseEvent};
