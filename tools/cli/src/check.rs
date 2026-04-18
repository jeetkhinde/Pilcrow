use std::path::Path;

/// Placeholder — BFF enforcement has been removed.
/// Structural rules (Props-only layouts/ui) are enforced at compile time by pilcrow-routekit.
pub fn check_arch(_root: &Path) -> Result<(), String> {
    Ok(())
}
