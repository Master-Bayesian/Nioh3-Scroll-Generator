//! Inputs for a bounded, independent original1AB5300 comparison.
//! Only an explicitly requested path receives a machine-readable receipt.
use super::{late_max, Entry, Tables};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn entry(key: u16, base: u32, dup: u16) -> Entry {
    Entry { key, base, dup }
}

fn rows(entries: &[Entry]) -> Vec<Value> {
    entries.iter().map(|e| json!({"key": e.key, "base": e.base, "dup": e.dup})).collect()
}

#[test]
fn late_max_original_instruction_control() -> Result<(), Box<dyn std::error::Error>> {
    let raw_tables = include_bytes!("../tests/fixtures/audit_tables.bin");
    let tables = Tables::load(raw_tables)?;
    let implementation = include_str!("source_rebuild.rs");
    let start = implementation.find("fn late_max(").ok_or("missing late_max definition")?;
    let end = start + implementation[start..].find("\n#[derive(Clone, Debug)]").ok_or("missing end of late_max definition")?;
    let key = 0x1ee2;
    let unknown = 0xfffe;
    assert!(tables.groups.row(u32::from(key)).is_some());
    assert!(tables.groups.row(u32::from(unknown)).is_none());
    let cases = vec![
        ("below_base_plus_duplicate", vec![entry(key, 100, 5)], key, 104),
        ("equal_base_plus_duplicate", vec![entry(key, 100, 5)], key, 105),
        ("raise_base_retain_duplicate", vec![entry(key, 100, 5)], key, 106),
        ("unsigned_comparison", vec![entry(key, 0x7fff_ffff, 0)], key, 0x8000_0000),
        ("wrapped_existing_total", vec![entry(key, u32::MAX, 2)], key, 2),
        ("absent_valid_zero_value", vec![], key, 0),
        ("absent_unknown_group", vec![], unknown, 7),
        ("existing_unknown_group_is_not_qualified", vec![entry(unknown, 100, 5)], unknown, 7),
        ("full_capacity_no_insert", (0..256).map(|n| entry(0x7f69, n, 0)).collect(), key, 1),
        ("zero_group_no_data_access", vec![], 0, 37),
    ];
    let mut outputs = Vec::new();
    for (name, before, group, value) in cases {
        let mut after = before.clone();
        late_max(&mut after, group, value, &tables);
        outputs.push(json!({"name": name, "group": group, "value": value,
            "group_known": tables.groups.row(u32::from(group)).is_some(),
            "before": rows(&before), "after": rows(&after)}));
    }
    if let Some(path) = std::env::var_os("NIOH3_NATIVE_LATE_RUST_OUTPUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({
            "schema": "nioh3-late-source-rust-control/v1", "cases": outputs,
            "source_rebuild_sha256": format!("{:x}", Sha256::digest(implementation.as_bytes())),
            "late_max_sha256": format!("{:x}", Sha256::digest(&implementation.as_bytes()[start..end])),
            "tables_sha256": format!("{:x}", Sha256::digest(raw_tables)),
            "scope": "Actual source_rebuild::late_max on explicitly controlled inputs; native comparison is separate."
        }))?)?;
    }
    // The independent original instruction tool consumes all rows above.
    // Keep these route-defining safety checks in ordinary cargo tests too.
    assert_eq!(outputs[2]["after"][0]["base"], 106);
    assert_eq!(outputs[2]["after"][0]["dup"], 5);
    assert_eq!(outputs[3]["after"][0]["base"], 0x8000_0000u32);
    assert_eq!(outputs[5]["after"][0]["key"], key);
    assert_eq!(outputs[9]["after"], json!([]), "original1AB5300 returns before reading a zero group's source");
    Ok(())
}
