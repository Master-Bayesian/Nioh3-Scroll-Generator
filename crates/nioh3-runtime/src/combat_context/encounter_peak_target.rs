use super::{add, layout, Image, CHUNK, R};

const ACTOR_OWNER: u64 = 0x3a0;
const PHYSICAL_DAMAGE_SCALE: u64 = 0x2dfc;
const ELEMENTAL_DAMAGE_SCALE: u64 = 0x2e00;
const STATUS_TREE: u64 = 0x3108;
const WATER_PHYSICAL_SCALE: f32 = 1.2;
const PAGE_MASK: u64 = CHUNK as u64 - 1;
const CONFUSION_VTABLE: u64 = 0x3911b38;
const WEAKNESS_KEY: u16 = 0x347;
const WEAKNESS_VTABLE: u64 = 0x3aa8438;
const WEAKNESS_SCALE: f32 = 1.20;

// This page is beyond the captured opponent heap and separate from the action
// and parameter tables used by the scorer.
const STATUS_SENTINEL: u64 = 0x6200_0000_0000;
const NODE_STRIDE: u64 = 0x40;
const STATUS_OBJECTS: u64 = STATUS_SENTINEL + 0x200;
const OBJECT_STRIDE: u64 = 0x100;

#[derive(Clone, Copy, Default)]
pub struct OpponentAilments {
    pub fire: bool,
    pub water: bool,
    pub lightning: bool,
    pub wind: bool,
}

enum Confusion {
    FromAilments,
    Standard,
}

impl Confusion {
    fn parameters(self, ailments: usize) -> (u16, f32, u32) {
        match (self, ailments) {
            (Self::Standard, _) | (Self::FromAilments, 2) => (0x2e9, 1.4, 0),
            (Self::FromAilments, 3) => (0x1d8, 1.7, 1),
            (Self::FromAilments, 4) => (0x15, 1.7, 2),
            (Self::FromAilments, _) => (0, 1.0, 0),
        }
    }
}

/// Read the selected actor's owner pointer after relocating its captured spans.
pub(super) fn saturated_opponent_owner(image: &Image, target_actor: u64) -> R<u64> {
    let owner = u64::from_le_bytes(read(image, add(target_actor, ACTOR_OWNER)?)?);
    if owner == 0 {
        return Err(layout("Saturated opponent has no owner"));
    }
    Ok(owner)
}

/// Project the selected opponent in the already Saturated state.
///
/// PC202 MAIN reads the physical scale at owner+0x2DFC. The Water status
/// aggregate scales that lane alone. The 78CDF8 lookup also uses the active
/// status entry to enable the attacker's damage-vs-ailment bonuses.
pub(super) fn install_saturated_opponent(image: &mut Image, target_owner: u64) -> R<()> {
    install(
        image,
        target_owner,
        None,
        OpponentAilments {
            water: true,
            ..Default::default()
        },
        false,
        Confusion::FromAilments,
    )
}

/// The theoretical equipment comparison starts with Water, standard
/// Confusion and ordinary Weakness on the reference opponent.
pub(super) fn install_theoretical_peak_opponent(
    image: &mut Image,
    target_owner: u64,
    module: u64,
) -> R<()> {
    install(
        image,
        target_owner,
        Some(module),
        OpponentAilments {
            water: true,
            ..Default::default()
        },
        true,
        Confusion::Standard,
    )
}

/// Project compatible effects on the benchmark human opponent.
pub(super) fn install_peak_opponent(
    image: &mut Image,
    target_owner: u64,
    module: u64,
    ailments: OpponentAilments,
    weakness: bool,
) -> R<()> {
    install(
        image,
        target_owner,
        Some(module),
        ailments,
        weakness,
        Confusion::FromAilments,
    )
}

fn install(
    image: &mut Image,
    target_owner: u64,
    module: Option<u64>,
    ailments: OpponentAilments,
    weakness: bool,
    confusion: Confusion,
) -> R<()> {
    let mut statuses: Vec<(u16, u64)> = [
        (ailments.fire, 0x39, 0x3911230),
        (ailments.water, 0x22f, 0x3aa76d0),
        (ailments.lightning, 0x226, 0x38efd90),
        (ailments.wind, 0xf9, 0x3aa7af0),
    ]
    .into_iter()
    .filter_map(|(active, key, vtable)| active.then_some((key, vtable)))
    .collect();
    let (confusion_key, confusion_scale, confusion_selector) = confusion.parameters(statuses.len());
    if confusion_key != 0 {
        statuses.push((confusion_key, CONFUSION_VTABLE));
    }
    if weakness {
        statuses.push((WEAKNESS_KEY, WEAKNESS_VTABLE));
    }
    if statuses.is_empty() {
        return Ok(());
    }
    statuses.sort_unstable_by_key(|(key, _)| *key);

    let physical_scale = add(target_owner, PHYSICAL_DAMAGE_SCALE)?;
    let elemental_scale = add(target_owner, ELEMENTAL_DAMAGE_SCALE)?;
    let status_head = add(target_owner, STATUS_TREE)?;
    let status_count = add(status_head, 8)?;
    let target_end = add(status_count, 8)?;
    let arena_end = STATUS_SENTINEL + CHUNK as u64;
    if target_owner == 0
        || (target_owner < arena_end && STATUS_SENTINEL < target_end)
        || image.0.contains_key(&STATUS_SENTINEL)
    {
        return Err(layout("invalid Saturated opponent arena"));
    }

    // 136AC4 recognizes an empty tree by the sentinel's self-link. The neutral
    // references retain that link and omit the unconsumed size word.
    let previous_head = u64::from_le_bytes(read(image, status_head)?);
    if previous_head == 0 || u64::from_le_bytes(read(image, previous_head)?) != previous_head {
        return Err(layout("peak opponent requires an empty status tree"));
    }
    if u32::from_le_bytes(read(image, physical_scale)?) != 1_f32.to_bits() {
        return Err(layout("peak opponent requires a neutral physical scale"));
    }
    let previous_elemental = f32::from_le_bytes(read(image, elemental_scale)?);

    for (index, (key, vtable)) in statuses.iter().enumerate() {
        let object = STATUS_OBJECTS + index as u64 * OBJECT_STRIDE;
        image.set(object + 0x14, &[1])?;
        if let Some(module) = module {
            image.pointer(object, add(module, *vtable)?)?;
        }
        if *key == confusion_key {
            image.set(object + 0xac, &confusion_selector.to_le_bytes())?;
        } else if *key == WEAKNESS_KEY {
            // ED03 definition 3DF, original class-1 preparation and apply.
            image.set(object + 0xa8, &WEAKNESS_SCALE.to_le_bytes())?;
            image.set(object + 0xac, &0.80_f32.to_le_bytes())?;
        }
        image.set(node(index) + 0x20, &key.to_le_bytes())?;
        image.pointer(node(index) + 0x28, object)?;
    }
    let deepest = statuses.len().ilog2();
    let root = link_tree(image, 0, statuses.len(), STATUS_SENTINEL, deepest)?;
    image.pointer(STATUS_SENTINEL, node(0))?;
    image.pointer(STATUS_SENTINEL + 8, root)?;
    image.pointer(STATUS_SENTINEL + 0x10, node(statuses.len() - 1))?;
    image.set(STATUS_SENTINEL + 0x18, &[1, 1])?;

    let water_scale = if ailments.water {
        WATER_PHYSICAL_SCALE
    } else {
        1.0
    };
    let vulnerability = if weakness { WEAKNESS_SCALE } else { 1.0 };
    // The native in-order aggregate visits 347 after all four elements and
    // the admitted Confusion tier. The class-1 factor is a separate multiply.
    let physical = (water_scale * confusion_scale) * vulnerability;
    replace(image, physical_scale, physical.to_le_bytes())?;
    if confusion_key != 0 || weakness {
        replace(
            image,
            elemental_scale,
            ((previous_elemental * confusion_scale) * vulnerability).to_le_bytes(),
        )?;
    }
    if confusion_key != 0 {
        // 97DEA0 takes this Ki recovery branch on the fixed human opponent.
        replace(image, add(target_owner, 0x2e7c)?, 0.5_f32.to_le_bytes())?;
    }
    if ailments.wind {
        // Original 2090454, status modifier +0xA4 = 0.
        for offset in [0x2e04, 0x2e08, 0x2e80] {
            replace(image, add(target_owner, offset)?, 1.5_f32.to_le_bytes())?;
        }
    }
    replace(image, status_head, STATUS_SENTINEL.to_le_bytes())?;
    replace(image, status_count, (statuses.len() as u64).to_le_bytes())
}

fn node(index: usize) -> u64 {
    STATUS_SENTINEL + NODE_STRIDE * (index as u64 + 1)
}

/// The midpoint gives a balanced in-order MSVC tree. Coloring its deepest
/// nodes red, except for a lone root, makes each path have the same black depth.
fn link_tree(image: &mut Image, start: usize, end: usize, parent: u64, black_depth: u32) -> R<u64> {
    if start == end {
        return Ok(STATUS_SENTINEL);
    }
    let mid = (start + end) / 2;
    let address = node(mid);
    let next_depth = black_depth.saturating_sub(1);
    let left = link_tree(image, start, mid, address, next_depth)?;
    let right = link_tree(image, mid + 1, end, address, next_depth)?;
    image.pointer(address, left)?;
    image.pointer(address + 8, parent)?;
    image.pointer(address + 0x10, right)?;
    let black = black_depth != 0 || parent == STATUS_SENTINEL;
    image.set(address + 0x18, &[u8::from(black), 0])?;
    Ok(address)
}

fn read<const N: usize>(image: &Image, address: u64) -> R<[u8; N]> {
    let missing = || layout("peak opponent operand is unknown");
    let mut bytes = [0; N];
    for (byte, at) in bytes.iter_mut().zip(address..add(address, N as u64)?) {
        let page = image.0.get(&(at & !PAGE_MASK)).ok_or_else(missing)?;
        let offset = (at & PAGE_MASK) as usize;
        if page.known[offset / 8] & (1 << (offset % 8)) == 0 {
            return Err(missing());
        }
        *byte = page.data[offset];
    }
    Ok(bytes)
}

// Only these target fields are replaced. Incoming observations continue to use
// Image::set's normal byte agreement check.
fn replace<const N: usize>(image: &mut Image, address: u64, bytes: [u8; N]) -> R<()> {
    for at in address..add(address, N as u64)? {
        if let Some(page) = image.0.get_mut(&(at & !PAGE_MASK)) {
            let offset = (at & PAGE_MASK) as usize;
            page.known[offset / 8] &= !(1 << (offset % 8));
        }
    }
    image.set(address, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_projects_the_physical_scale_and_single_active_status() -> R<()> {
        let mut image = Image::default();
        // Exercise a relocated address and retain a distinct elemental scale.
        let owner = 0x4400_0000_4320;
        let actor = 0x4400_0000_8120;
        image.pointer(actor + ACTOR_OWNER, owner)?;
        image.set(owner + PHYSICAL_DAMAGE_SCALE, &1_f32.to_le_bytes())?;
        image.set(owner + PHYSICAL_DAMAGE_SCALE + 4, &0.75_f32.to_le_bytes())?;
        let previous_head = owner + 0x4000;
        image.pointer(owner + STATUS_TREE, previous_head)?;
        image.pointer(previous_head, previous_head)?;
        assert!(read::<8>(&image, owner + STATUS_TREE + 8).is_err());

        let resolved_owner = saturated_opponent_owner(&image, actor)?;
        assert_eq!(resolved_owner, owner);
        install_saturated_opponent(&mut image, resolved_owner)?;
        let water_node = node(0);

        assert_eq!(
            u32::from_le_bytes(read(&image, owner + PHYSICAL_DAMAGE_SCALE)?),
            0x3f99_999a
        );
        assert_eq!(
            f32::from_le_bytes(read(&image, owner + PHYSICAL_DAMAGE_SCALE + 4)?),
            0.75
        );
        assert_eq!(
            u64::from_le_bytes(read(&image, owner + STATUS_TREE)?),
            STATUS_SENTINEL
        );
        assert_eq!(
            u64::from_le_bytes(read(&image, owner + STATUS_TREE + 8)?),
            1
        );
        for link in [0, 8, 0x10] {
            assert_eq!(
                u64::from_le_bytes(read(&image, STATUS_SENTINEL + link)?),
                water_node
            );
            assert_eq!(
                u64::from_le_bytes(read(&image, water_node + link)?),
                STATUS_SENTINEL
            );
        }
        assert_eq!(read(&image, STATUS_SENTINEL + 0x18)?, [1, 1]);
        assert_eq!(read(&image, water_node + 0x18)?, [1, 0]);
        assert_eq!(u16::from_le_bytes(read(&image, water_node + 0x20)?), 0x22f);
        assert_eq!(
            u64::from_le_bytes(read(&image, water_node + 0x28)?),
            STATUS_OBJECTS
        );
        assert_eq!(read(&image, STATUS_OBJECTS + 0x14)?, [1]);
        assert!(read::<1>(&image, STATUS_OBJECTS + 0x15).is_err());
        Ok(())
    }

    #[test]
    fn four_ailments_project_the_original_five_key_tree_and_aggregate_lanes() -> R<()> {
        let mut image = Image::default();
        let owner = 0x4400_0000_4320;
        let module = 0x7ff6_7bce_0000;
        let previous_head = owner + 0x4000;
        image.pointer(owner + STATUS_TREE, previous_head)?;
        image.pointer(previous_head, previous_head)?;
        for offset in [PHYSICAL_DAMAGE_SCALE, ELEMENTAL_DAMAGE_SCALE] {
            image.set(owner + offset, &1_f32.to_le_bytes())?;
        }
        let ailments = OpponentAilments {
            fire: true,
            water: true,
            lightning: true,
            wind: true,
        };

        install_peak_opponent(&mut image, owner, module, ailments, false)?;

        assert_eq!(
            u64::from_le_bytes(read(&image, owner + STATUS_TREE + 8)?),
            5
        );
        assert_eq!(u64::from_le_bytes(read(&image, STATUS_SENTINEL)?), node(0));
        assert_eq!(
            u64::from_le_bytes(read(&image, STATUS_SENTINEL + 8)?),
            node(2)
        );
        assert_eq!(
            u64::from_le_bytes(read(&image, STATUS_SENTINEL + 0x10)?),
            node(4)
        );
        for (index, key, vtable) in [
            (0, 0x15, 0x3911b38),
            (1, 0x39, 0x3911230),
            (2, 0xf9, 0x3aa7af0),
            (3, 0x226, 0x38efd90),
            (4, 0x22f, 0x3aa76d0),
        ] {
            let object = STATUS_OBJECTS + index as u64 * OBJECT_STRIDE;
            assert_eq!(u16::from_le_bytes(read(&image, node(index) + 0x20)?), key);
            assert_eq!(u64::from_le_bytes(read(&image, object)?), module + vtable);
            assert_eq!(read(&image, object + 0x14)?, [1]);
        }
        assert_eq!(read(&image, node(2) + 0x18)?, [1, 0]);
        assert_eq!(read(&image, node(0) + 0x18)?, [0, 0]);
        assert_eq!(read(&image, node(1) + 0x18)?, [1, 0]);
        assert_eq!(u32::from_le_bytes(read(&image, STATUS_OBJECTS + 0xac)?), 2);
        for (offset, word) in [
            (PHYSICAL_DAMAGE_SCALE, 0x4002_8f5d),
            (ELEMENTAL_DAMAGE_SCALE, 0x3fd9_999a),
            (0x2e04, 0x3fc0_0000),
            (0x2e08, 0x3fc0_0000),
            (0x2e7c, 0x3f00_0000),
            (0x2e80, 0x3fc0_0000),
        ] {
            assert_eq!(u32::from_le_bytes(read(&image, owner + offset)?), word);
        }
        Ok(())
    }

    #[test]
    fn weakness_projects_both_lanes_without_counting_toward_confusion() -> R<()> {
        let owner = 0x4400_0000_4320;
        let module = 0x7ff6_7bce_0000;
        let previous_head = owner + 0x4000;
        for (water, physical) in [(false, 0x3f99_999a), (true, 0x3fb8_51ec)] {
            let mut image = Image::default();
            image.pointer(owner + STATUS_TREE, previous_head)?;
            image.pointer(previous_head, previous_head)?;
            for offset in [PHYSICAL_DAMAGE_SCALE, ELEMENTAL_DAMAGE_SCALE] {
                image.set(owner + offset, &1_f32.to_le_bytes())?;
            }
            let ailments = OpponentAilments { water, ..Default::default() };
            install_peak_opponent(&mut image, owner, module, ailments, true)?;
            assert_eq!(u64::from_le_bytes(read(&image, owner + STATUS_TREE + 8)?), 1 + u64::from(water));
            let index = usize::from(water);
            let object = STATUS_OBJECTS + index as u64 * OBJECT_STRIDE;
            assert_eq!(u16::from_le_bytes(read(&image, node(index) + 0x20)?), WEAKNESS_KEY);
            assert_eq!(u64::from_le_bytes(read(&image, object)?), module + WEAKNESS_VTABLE);
            assert_eq!(u32::from_le_bytes(read(&image, object + 0xac)?), 0x3f4c_cccd);
            assert_eq!(u32::from_le_bytes(read(&image, owner + PHYSICAL_DAMAGE_SCALE)?), physical);
            assert_eq!(u32::from_le_bytes(read(&image, owner + ELEMENTAL_DAMAGE_SCALE)?), 0x3f99_999a);
            assert!(read::<4>(&image, owner + 0x2e7c).is_err());
        }
        Ok(())
    }

    #[test]
    fn theoretical_peak_uses_water_standard_confusion_and_weakness() -> R<()> {
        let mut image = Image::default();
        let owner = 0x4400_0000_4320;
        let module = 0x7ff6_7bce_0000;
        let previous_head = owner + 0x4000;
        image.pointer(owner + STATUS_TREE, previous_head)?;
        image.pointer(previous_head, previous_head)?;
        for offset in [PHYSICAL_DAMAGE_SCALE, ELEMENTAL_DAMAGE_SCALE] {
            image.set(owner + offset, &1_f32.to_le_bytes())?;
        }

        install_theoretical_peak_opponent(&mut image, owner, module)?;

        assert_eq!(u64::from_le_bytes(read(&image, owner + STATUS_TREE + 8)?), 3);
        for (index, key) in [0x22f, 0x2e9, WEAKNESS_KEY].into_iter().enumerate() {
            assert_eq!(u16::from_le_bytes(read(&image, node(index) + 0x20)?), key);
        }
        assert_eq!(
            u32::from_le_bytes(read(&image, STATUS_OBJECTS + OBJECT_STRIDE + 0xac)?),
            0
        );
        // Original PC202 target aggregation, Water + standard Confusion + Weakness.
        assert_eq!(u32::from_le_bytes(read(&image, owner + PHYSICAL_DAMAGE_SCALE)?), 0x4001_0626);
        assert_eq!(u32::from_le_bytes(read(&image, owner + ELEMENTAL_DAMAGE_SCALE)?), 0x3fd7_0a3e);
        assert!(read::<4>(&image, owner + 0x2e80).is_err());
        Ok(())
    }

    #[test]
    fn water_requires_a_known_owner_pointer() -> R<()> {
        let mut image = Image::default();
        let actor = 0x4400_0000_5120;
        image.set(actor + ACTOR_OWNER, &[1])?;
        assert!(saturated_opponent_owner(&image, actor).is_err());
        Ok(())
    }

    #[test]
    fn water_preserves_an_opponent_with_a_nonempty_status_tree() -> R<()> {
        let mut image = Image::default();
        let owner = 0x4400_0000_4320;
        let previous_head = owner + 0x4000;
        let previous_node = owner + 0x4100;
        image.pointer(owner + STATUS_TREE, previous_head)?;
        image.pointer(previous_head, previous_node)?;
        assert!(install_saturated_opponent(&mut image, owner).is_err());
        assert_eq!(
            u64::from_le_bytes(read(&image, previous_head)?),
            previous_node
        );
        assert!(!image.0.contains_key(&STATUS_SENTINEL));
        Ok(())
    }

    #[test]
    fn water_rejects_a_collision_in_the_reserved_arena() -> R<()> {
        let mut image = Image::default();
        image.set(STATUS_SENTINEL + 0x10, &[1])?;
        assert!(install_saturated_opponent(&mut image, 0x44_0000).is_err());
        assert_eq!(read(&image, STATUS_SENTINEL + 0x10)?, [1]);
        Ok(())
    }
}
