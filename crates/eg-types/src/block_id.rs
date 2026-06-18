/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
/// Canonical fallback block used when no specific block can be resolved.
pub const DEFAULT_BLOCK_ID: &str = "core/stone";

/// Normalizes block ID text without consulting an asset catalog.
pub fn canonical_block_id_text(block_id: &str) -> String {
    block_id.trim().replace('\\', "/").to_ascii_lowercase()
}

/// Normalizes a block ID and falls back to the default block when empty.
pub fn normalize_block_id(block_id: &str) -> String {
    let normalized = canonical_block_id_text(block_id);
    if normalized.is_empty() {
        DEFAULT_BLOCK_ID.to_string()
    } else if normalized.contains('/') {
        normalized
    } else {
        format!("core/{normalized}")
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_block_id, DEFAULT_BLOCK_ID};

    #[test]
    fn normalizes_empty_block_ids_to_default() {
        assert_eq!(normalize_block_id("  "), DEFAULT_BLOCK_ID);
    }

    #[test]
    fn normalizes_legacy_bare_block_ids_to_core_namespace() {
        assert_eq!(normalize_block_id(" Stone "), "core/stone");
    }

    #[test]
    fn keeps_namespaced_block_ids() {
        assert_eq!(normalize_block_id("custom/Stone"), "custom/stone");
    }
}
