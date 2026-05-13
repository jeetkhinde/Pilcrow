mod baking;
mod extractor;
pub mod hub;
mod live_props;
mod live_trait;
mod macros;
pub mod store;
pub mod watcher;

pub use baking::{find_s_live_slots, inject_fsr_slots};
pub use hub::fsr_hub_handler;
pub use extractor::extract_live_from_parts;
pub use live_props::{DependencyKey, LiveProps};
pub use live_trait::{LiveQuery, PilcrowLive};
pub use store::{FsrStore, StaleSlot};
pub use watcher::{
    pilcrow_fsr_watcher_tick, spawn_embedded_watcher, watcher_tick, SlotPatch, WatcherConfig,
    WatcherEventTx,
};
