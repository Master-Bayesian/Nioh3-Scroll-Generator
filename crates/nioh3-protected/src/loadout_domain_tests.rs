//! All installed TYPE boundaries through the real protected item/slot admission.
//! These controlled in-memory owned records are not a runtime source observation.
use super::*;
use nioh3_save::character::{equipment_offset, EQUIPMENT_RECORD_BYTES, EQUIPMENT_REGION_END};
use std::{collections::BTreeMap, error::Error};

fn native_types(data_root: &Path) -> Result<BTreeMap<u32, Vec<u16>>, Box<dyn Error>> {
    let resource = nioh3_data::load_effect_resource_for_file_version(data_root, (2, 0, 2, 0))?;
    let mut by_type: BTreeMap<u32, Vec<u16>> = BTreeMap::new();
    for index in 0..resource.item.row_count() {
        let Some(row) = resource.item.row(index) else {
            continue;
        };
        if row.len() < 0x160 {
            continue;
        }
        let key = u16::from_le_bytes([row[0x152], row[0x153]]);
        let class = u32::from_le_bytes(row[0x15C..0x160].try_into()?);
        if key != 0 && class < ARMOR_FIRST {
            let keys = by_type.entry(class).or_default();
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
    }
    Ok(by_type)
}

fn put_owned(plain: &mut [u8], index: usize, key: u16) -> Result<(), Box<dyn Error>> {
    let at = equipment_offset(index).ok_or("controlled inventory index out of range")?;
    let record = &mut plain[at..at + EQUIPMENT_RECORD_BYTES];
    record.fill(0);
    record[..2].copy_from_slice(&key.to_le_bytes());
    record[6..8].copy_from_slice(&1u16.to_le_bytes());
    for offset in EQUIPMENT_SET_OFFSETS {
        record[offset..offset + 4].copy_from_slice(&EQUIPMENT_NOT_WORN.to_le_bytes());
    }
    Ok(())
}

fn kusarigama_fixture() -> Bundle {
    Bundle {
        id: "kusarigama-strong".to_owned(),
        name: "Kusarigama strong attack".to_owned(),
        weapon: "Kusarigama".to_owned(),
        damage: "Strong attack".to_owned(),
        school: "ninja".to_owned(),
        weapon_class: Some(12),
        selection_weapon_class: Some(12),
        scene: None,
        diagnostic: false,
        assets: None,
    }
}

#[test]
#[ignore = "requires an explicitly installed PC202 attack bundle set"]
fn all_installed_type_scenes_admit_every_declared_native_type_record() -> Result<(), Box<dyn Error>>
{
    let data = PathBuf::from(
        std::env::var_os("NIOH3_LOADOUT_DOMAIN_DATA_ROOT")
            .ok_or("set NIOH3_LOADOUT_DOMAIN_DATA_ROOT to the staged PC202 data directory")?,
    );
    let types = native_types(&data)?;
    let installed = bundles(&data);
    assert!(!installed.is_empty());
    let mut report = Vec::new();
    for bundle in &installed {
        let Some(class) = bundle.requested_weapon_class() else {
            assert!(
                bundle.scene.is_none(),
                "an ordinary attack cannot omit its TYPE identity"
            );
            report.push(
                json!({"scene":bundle.id,"type_class":null,"classification":"legacy_retained"}),
            );
            continue;
        };
        let keys = types
            .get(&class)
            .ok_or("installed TYPE missing from original item table")?;
        assert!(
            keys.len() > 1,
            "TYPE search must expose distinct weapon models"
        );
        let mut plain = vec![0; EQUIPMENT_REGION_END];
        let mut items = Vec::new();
        for (index, &key) in keys.iter().enumerate() {
            put_owned(&mut plain, index, key)?;
            items.push(json!({"slot_index":index,"slot":0}));
        }
        let params = json!({"items":items});
        let (pack, origin) = backpack(&data, bundle, &plain, &params, None)?;
        assert_eq!(
            pack.pool.len(),
            keys.len(),
            "every owned native TYPE record is requested"
        );
        assert_eq!(origin.len(), keys.len());
        assert_eq!(pack.ranges, [(0, 0, keys.len() as u32)]);
        for (row, &input) in pack.origin.iter().enumerate() {
            assert_eq!(origin[input].item_id, keys[row]);
            assert_eq!(origin[input].slot_index, row);
        }
        let foreign = types
            .iter()
            .find(|(other, _)| **other != class)
            .and_then(|(_, keys)| keys.first())
            .copied()
            .ok_or("foreign TYPE control missing")?;
        put_owned(&mut plain, 0, foreign)?;
        let failure = backpack(&data, bundle, &plain, &params, None)
            .err()
            .ok_or("foreign TYPE control was admitted")?;
        assert!(failure.to_string().contains("LOADOUT_ITEM_SLOT"));
        let expected = if bundle.weapon_class.is_some() {
            SourceAdmissionPolicy::OwnedType
        } else {
            SourceAdmissionPolicy::CapturedWeaponOnly
        };
        assert_eq!(bundle.source_admission(), expected);
        report.push(json!({"scene":bundle.id,"type_class":class,
            "owned_records_requested":keys.len(),"foreign_type_rejected":true,
            "source_policy":format!("{expected:?}"),"item_ids":keys}));
    }
    assert!(
        report.iter().any(|row| row["type_class"] == 12),
        "installed ninja TYPE boundary"
    );
    let out = json!({"schema":"nioh3-installed-type-domain/v1","passed":true,
        "scope":"Actual protected backpack admission and native item TYPE table; controlled owned records; no MAIN/source-production claim",
        "installed_scene_count":installed.len(),"scenes":report,
        "game_access":false,"real_save_write":false});
    if let Some(directory) = std::env::var_os("NIOH3_DOMAIN_ARTIFACT_DIR") {
        let directory = PathBuf::from(directory);
        std::fs::create_dir_all(&directory)?;
        std::fs::write(
            directory.join("installed-type-domain.json"),
            serde_json::to_vec_pretty(&out)?,
        )?;
    }
    println!("Installed scene TYPE admission: {} scenes, {} selectable TYPE scenes; foreign TYPE controls rejected",
        installed.len(), installed.iter().filter(|bundle| bundle.requested_weapon_class().is_some()).count());
    Ok(())
}

#[test]
fn owned_backpack_rejects_duplicate_inventory_identity_and_rarity_outside_contract(
) -> Result<(), Box<dyn Error>> {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let bundle = kusarigama_fixture();
    let types = native_types(&data)?;
    let key = *types
        .get(&12)
        .and_then(|keys| keys.first())
        .ok_or("TYPE12 fixture missing")?;
    let mut plain = vec![0; EQUIPMENT_REGION_END];
    put_owned(&mut plain, 0, key)?;
    let repeated = json!({"items":[{"slot_index":0,"slot":0},{"slot_index":0,"slot":0}]});
    assert!(backpack(&data, &bundle, &plain, &repeated, None).is_err());
    let at = equipment_offset(0).ok_or("fixture inventory index")?;
    plain[at + 0x30] = MAX_RARITY + 1;
    let rare = json!({"items":[{"slot_index":0,"slot":0}]});
    assert!(backpack(&data, &bundle, &plain, &rare, None).is_err());
    Ok(())
}

#[test]
fn attribute_moves_require_an_admitted_source_producer_instead_of_a_silent_freeze(
) -> Result<(), Box<dyn Error>> {
    let failure = validate_source_search(SourceAdmissionPolicy::CapturedWeaponOnly, true)
        .err()
        .ok_or("unproved attribute-source search was admitted")?;
    assert!(failure
        .to_string()
        .contains("LOADOUT_SOURCE_ATTRIBUTES_UNPROVEN"));
    validate_source_search(SourceAdmissionPolicy::CapturedWeaponOnly, false)?;
    validate_source_search(SourceAdmissionPolicy::OwnedType, true)?;
    validate_source_search(SourceAdmissionPolicy::OwnedType, false)?;
    Ok(())
}

#[test]
fn oversized_slot_identifiers_cannot_alias_native_weapon_slots() -> Result<(), Box<dyn Error>> {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let bundle = kusarigama_fixture();
    let types = native_types(&data)?;
    let key = *types
        .get(&12)
        .and_then(|keys| keys.first())
        .ok_or("TYPE12 fixture missing")?;
    let mut plain = vec![0; EQUIPMENT_REGION_END];
    put_owned(&mut plain, 0, key)?;
    let ordinary = json!({"items":[{"slot_index":0,"slot":0}]});
    let (pack, _) = backpack(&data, &bundle, &plain, &ordinary, None)?;
    assert_eq!(pack.ranges, [(0, 0, 1)]);
    for slot in [1_u64 << 32, 2_u64 << 32] {
        let aliased = json!({"items":[{"slot_index":0,"slot":slot}]});
        let failure = backpack(&data, &bundle, &plain, &aliased, None)
            .err()
            .ok_or("oversized slot identifier silently aliased native weapon slot0")?;
        assert!(failure.to_string().contains("LOADOUT_ITEM_SLOT"));
    }
    Ok(())
}
