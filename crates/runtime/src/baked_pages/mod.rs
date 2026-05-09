//! Experimental baked-page model types.
//!
//! This module is intentionally model-only for now. It lets route/page code
//! declare bake timing and artifact strategy without changing Pilcrow's current
//! SSR/load request path.

mod model;

pub use model::{
    BakeEligibility, BakedArtifactMode, BakedFragment, BakedLayout, BakedPage,
    BakedRouteDeclaration, BakedSlot, BakedSlotKind, DependencyKey, StaleState,
};
