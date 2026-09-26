//! Item type classes from the shipped PC v2.02 item table.
//!
//! Row `+0x152` is the item id and `+0x15C` its type class: one value per
//! weapon type, per armour slot and weight, accessories and soul cores. A few
//! rows store another value there, so only small classes are kept; the rest
//! stay unknown rather than guessed.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

/// Largest value treated as a type class.
const MAX_TYPE_CLASS: u32 = 64;

static ITEM_KINDS: OnceLock<BTreeMap<u16, u32>> = OnceLock::new();

fn load(data_root: &Path) -> BTreeMap<u16, u32> {
    let mut kinds = BTreeMap::new();
    let Ok(resource) = nioh3_data::load_effect_resource_for_file_version(data_root, (2, 0, 2, 0))
    else {
        return kinds;
    };
    for index in 0..resource.item.row_count() {
        let Some(row) = resource.item.row(index) else {
            continue;
        };
        if row.len() < 0x160 {
            continue;
        }
        let id = u16::from_le_bytes([row[0x152], row[0x153]]);
        let class = u32::from_le_bytes([row[0x15C], row[0x15D], row[0x15E], row[0x15F]]);
        if id != 0 && class <= MAX_TYPE_CLASS {
            kinds.insert(id, class);
        }
    }
    kinds
}

/// The type class of `item_id`, when the shipped table names one.
pub fn type_class(data_root: &Path, item_id: u16) -> Option<u32> {
    ITEM_KINDS
        .get_or_init(|| load(data_root))
        .get(&item_id)
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_items_name_their_type_class() {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../nioh3_scroll_editor/data");
        // 甲斐国江 (a katana), a Bloodedge Demon soul core, White Camellia Hairpin.
        assert_eq!(type_class(&data, 0x8D5B), Some(0));
        assert_eq!(type_class(&data, 0x6D36), Some(54));
        assert_eq!(type_class(&data, 0xAF66), Some(40));
        assert_eq!(type_class(&data, 0x0000), None);
    }
}
