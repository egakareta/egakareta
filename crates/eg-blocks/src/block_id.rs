/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/

use std::{fmt::Display, ops::Deref};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockId(String);

impl PartialEq<&str> for BlockId {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl Deref for BlockId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for BlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for BlockId {
    fn default() -> Self {
        BlockId(Self::DEFAULT_BLOCK_ID.to_string())
    }
}

impl BlockId {
    pub const DEFAULT_BLOCK_ID: &str = "core/stone";

    /// Normalizes block ID text without consulting an asset catalog.
    pub fn canonical_block_id_text(block_id: &str) -> String {
        block_id.trim().replace('\\', "/").to_ascii_lowercase()
    }

    /// Create a new BlockId, normalizing the input and falling back to the default block when empty.
    pub fn new(block_id: &str) -> Self {
        let normalized = Self::canonical_block_id_text(block_id);

        let block_id = if normalized.is_empty() {
            Self::DEFAULT_BLOCK_ID.to_string()
        } else if normalized.contains('/') {
            normalized
        } else {
            format!("core/{normalized}")
        };

        BlockId(block_id)
    }
}

#[cfg(test)]
mod tests {
    use super::BlockId;

    #[test]
    fn normalizes_empty_block_ids_to_default() {
        assert_eq!(BlockId::new("  "), BlockId::new(BlockId::DEFAULT_BLOCK_ID));
    }

    #[test]
    fn normalizes_legacy_bare_block_ids_to_core_namespace() {
        assert_eq!(BlockId::new(" Stone "), BlockId::new("core/stone"));
    }

    #[test]
    fn keeps_namespaced_block_ids() {
        assert_eq!(BlockId::new("custom/Stone"), BlockId::new("custom/stone"));
    }
}
