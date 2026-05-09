//! Experimental baked-page model and storage types.
//!
//! This module intentionally stays library-only for now. It lets route/page code
//! declare bake timing, store durable artifacts, and read fresh artifacts without
//! changing Pilcrow's current SSR/load request path.

mod model;
mod patch;
mod prebake;
mod serving;
mod store;

pub use model::{
    BakeEligibility, BakedArtifactMode, BakedFragment, BakedLayout, BakedPage,
    BakedRouteDeclaration, BakedSlot, BakedSlotKind, DependencyKey, StaleState,
};
pub use patch::{
    replace_slot_content, text_slot_content, trusted_html_slot_content, BakedPatchOutcome,
    BakedPatchRegistry, BakedPatchedSlot, SlotValue, TrustedHtml,
};
pub use prebake::{BakedPrebakeReport, BakedRenderedPage};
pub use serving::{BakedServeOutcome, BakedServeState};
pub use store::{BakedArtifactHit, BakedPageStore, ReverseIndex};
