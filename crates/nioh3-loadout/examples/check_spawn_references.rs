//! Check the production selector against the rows extracted from original PC202 resources.
use nioh3_loadout::action_rows::{scheduled_projectiles, PrimaryDamage, ScheduledProjectile};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, env, fs};

#[derive(Deserialize)]
struct Evidence {
    bindings: Vec<Expected>,
}

#[derive(Deserialize)]
struct Expected {
    resource: String,
    owner_action: u16,
    parameter_sha256: String,
    slot: u16,
    event_key: u16,
    tick: u16,
    carrier: u32,
    primary_row: String,
    global_index: Option<u16>,
    row_sha256: Option<String>,
    requested_action: u16,
    requested_row: i8,
    checks_owner_imbue: bool,
    descriptor_fields: [u32; 3],
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args_os()
        .nth(1)
        .ok_or("expected original resource evidence JSON")?;
    let expected: Evidence = serde_json::from_slice(&fs::read(path)?)?;
    let mut grouped = BTreeMap::<_, Vec<_>>::new();
    for row in &expected.bindings {
        grouped
            .entry((&row.resource, row.owner_action, &row.parameter_sha256))
            .or_default()
            .push(row);
    }
    let mut checked = 0usize;
    for ((resource, action, digest), mut expected) in grouped {
        let actual = scheduled_projectiles(resource, action, digest)?;
        if actual.len() != expected.len() {
            return Err("different number of scheduled references".into());
        }
        expected.sort_by_key(|r| r.slot);
        for (r, e) in actual.iter().zip(expected) {
            if (
                r.slot,
                r.tick,
                r.event_key,
                r.carrier,
                r.requested_action,
                r.requested_row,
                r.imbue.checks_owner(),
                r.descriptor_fields,
            ) != (
                e.slot,
                e.tick,
                e.event_key,
                e.carrier,
                e.requested_action,
                e.requested_row,
                e.checks_owner_imbue,
                e.descriptor_fields,
            ) {
                return Err(format!("occurrence mismatch: {resource}/{action}/{}", e.slot).into());
            }
            let api = ScheduledProjectile {
                resource: resource.clone(),
                action,
                parameter_sha256: digest.clone(),
                projectile_slot: e.slot,
            };
            match (r.primary, e.primary_row.as_str()) {
                (PrimaryDamage::Action { .. }, "explicit_action_request") => {}
                (PrimaryDamage::Global { index }, "global_damage_row")
                    if e.global_index == Some(index) => {}
                (PrimaryDamage::NoPrimary, "no_primary_row")
                    if e.row_sha256.is_none() && api.resolve().is_err() =>
                {
                    checked += 1;
                    continue;
                }
                _ => return Err("primary damage row category differs".into()),
            }
            if Some(format!("{:x}", Sha256::digest(api.resolve()?.damage.bytes))) != e.row_sha256 {
                return Err(format!("damage row mismatch: {resource}/{action}/{}", e.slot).into());
            }
            checked += 1;
        }
    }
    if checked == 0 {
        return Err("empty resource set".into());
    }
    println!("Verified {checked} scheduled projectile references");
    Ok(())
}
