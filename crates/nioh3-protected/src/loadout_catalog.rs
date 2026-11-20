//! Bounded product discovery over the full static action catalog.
//! Static identities and runnable plans are separate facts. Pagination keeps
//! discovery within the protected transport's existing 4 MiB frame limit.
use super::{bundles, valid_bundle_id};
use crate::error::HostError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashSet, path::Path};

const MAX_CATALOG_BYTES: u64 = 32 * 1024 * 1024;
const MAX_SELECTIONS: usize = 100_000;

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Selection {
    resource: String,
    action: u16,
    parameter_sha256: String,
    weapon: Option<String>,
    school: Option<String>,
    weapon_class: Option<u32>,
    name: Option<String>,
    stance: Option<String>,
    kind: Option<String>,
    attack_id: Option<String>,
    static_complete: bool,
    plan_ready: bool,
    scene_ids: Vec<String>,
    missing_facts: Vec<String>,
}

impl Selection {
    fn valid(&self) -> bool {
        !self.resource.is_empty()
            && self.resource.len() <= 512
            && self.parameter_sha256.len() == 64
            && self
                .parameter_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            && self
                .school
                .as_deref()
                .is_none_or(|school| matches!(school, "samurai" | "ninja"))
            && [
                self.weapon.as_deref(),
                self.name.as_deref(),
                self.stance.as_deref(),
                self.kind.as_deref(),
                self.attack_id.as_deref(),
            ]
            .into_iter()
            .flatten()
            .all(|s| s.len() <= 256)
            && self.scene_ids.len() <= 128
            && self.scene_ids.iter().all(|id| valid_bundle_id(id))
            && self.missing_facts.len() < 64
            && self
                .missing_facts
                .iter()
                .all(|fact| !fact.is_empty() && fact.len() <= 256)
    }

    fn matches(&self, query: &str) -> bool {
        query.is_empty()
            || [
                self.resource.as_str(),
                self.weapon.as_deref().unwrap_or(""),
                self.name.as_deref().unwrap_or(""),
                self.school.as_deref().unwrap_or(""),
                self.attack_id.as_deref().unwrap_or(""),
            ]
            .into_iter()
            .any(|text| text.to_lowercase().contains(query))
            || self.action.to_string() == query
    }
}

#[derive(Deserialize)]
struct Catalog {
    version: u32,
    selections: Vec<Selection>,
}

pub(super) fn page(data_root: &Path, params: &Value) -> Result<Value, HostError> {
    let integer = |name: &str, default: u64, max: u64| -> Result<u64, HostError> {
        match params.get(name) {
            None => Ok(default),
            Some(value) => value
                .as_u64()
                .filter(|value| *value <= max)
                .ok_or_else(HostError::invalid_request),
        }
    };
    let offset = integer("catalog_offset", 0, MAX_SELECTIONS as u64)? as usize;
    let limit = integer("catalog_limit", 50, 200)? as usize;
    if limit == 0 {
        return Err(HostError::invalid_request());
    }
    let query = match params.get("catalog_query") {
        None => "",
        Some(value) => value
            .as_str()
            .filter(|value| value.chars().count() <= 256)
            .ok_or_else(HostError::invalid_request)?,
    }
    .trim();
    let path = data_root.join("loadout/catalog.json");
    let unavailable = |status: &str| {
        json!({"version":1,"source_status":status,"total":0,"matched":0,
        "offset":offset,"limit":limit,"query":query,"selections":[]})
    };
    let metadata = match std::fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(unavailable("missing"))
        }
        Err(_) => return Ok(unavailable("invalid")),
    };
    if metadata.len() > MAX_CATALOG_BYTES {
        return Ok(unavailable("invalid"));
    }
    let catalog = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Catalog>(&bytes).ok());
    let Some(mut catalog) = catalog else {
        return Ok(unavailable("invalid"));
    };
    if catalog.version != 1
        || catalog.selections.len() > MAX_SELECTIONS
        || catalog.selections.iter().any(|entry| !entry.valid())
    {
        return Ok(unavailable("invalid"));
    }
    let available: HashSet<_> = bundles(data_root)
        .into_iter()
        .filter(|bundle| !bundle.diagnostic)
        .map(|bundle| bundle.id)
        .collect();
    for selection in &mut catalog.selections {
        selection.scene_ids.retain(|id| available.contains(id));
        // Static knowledge cannot make an absent evaluator selectable.
        if selection.plan_ready && selection.scene_ids.is_empty() {
            selection.plan_ready = false;
            if !selection
                .missing_facts
                .iter()
                .any(|fact| fact == "installed_evaluation_plan")
            {
                selection
                    .missing_facts
                    .push("installed_evaluation_plan".to_string());
            }
        }
    }
    let total = catalog.selections.len();
    let normalized = query.to_lowercase();
    let matching: Vec<_> = catalog
        .selections
        .iter()
        .filter(|entry| entry.matches(&normalized))
        .collect();
    if offset > matching.len() {
        return Err(HostError::invalid_request());
    }
    Ok(
        json!({"version":1,"source_status":"ready","total":total,"matched":matching.len(),
        "offset":offset,"limit":limit,"query":query,
        "selections":matching.into_iter().skip(offset).take(limit).collect::<Vec<_>>() }),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn full_catalog_is_paged_without_admitting_static_or_missing_plans() {
        let root = std::env::temp_dir().join(format!(
            "nioh3-product-catalog-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let data = root.join("loadout");
        std::fs::create_dir_all(data.join("katana-current")).unwrap();
        std::fs::write(
            data.join("katana-current/bundle.json"),
            json!({"id":"katana-current","name":"Katana",
            "weapon":"Katana","damage":"Quick","weapon_class":0,"school":"samurai",
            "scene":{"stance":"mid","kind":"quick","attack_id":"katana-quick","buff":"current"}})
            .to_string(),
        )
        .unwrap();
        let mut entries = Vec::new();
        for action in 0..450 {
            entries.push(json!({"resource":"weapon0/action.param","action":action,"parameter_sha256":"a".repeat(64),
                "weapon":"Katana","school":"samurai","weapon_class":0,"name":if action%2==0 {Some("Named attack")}else{None},
                "stance":null,"kind":null,"attack_id":null,"static_complete":true,"plan_ready":action<2,
                "scene_ids":if action==0 {vec!["katana-current"]} else if action==1 {vec!["not-installed"]} else {Vec::new()},
                "missing_facts":if action>=2 {vec!["runtime_landed_multiplicity"]} else {Vec::new()}}));
        }
        std::fs::write(
            data.join("catalog.json"),
            json!({"version":1,"selections":entries}).to_string(),
        )
        .unwrap();
        let first = page(&root, &json!({})).unwrap();
        assert_eq!(first["total"], 450);
        assert_eq!(first["matched"], 450);
        assert_eq!(first["selections"].as_array().unwrap().len(), 50);
        assert_eq!(first["selections"][0]["plan_ready"], true);
        assert_eq!(first["selections"][1]["plan_ready"], false);
        assert_eq!(
            first["selections"][1]["missing_facts"],
            json!(["installed_evaluation_plan"])
        );
        assert_eq!(first["selections"][2]["plan_ready"], false);
        let last = page(&root, &json!({"catalog_offset":400,"catalog_limit":200})).unwrap();
        assert_eq!(last["selections"].as_array().unwrap().len(), 50);
        assert_eq!(last["selections"][49]["action"], 449);
        let named = page(&root, &json!({"catalog_query":"named","catalog_limit":200})).unwrap();
        assert_eq!(named["matched"], 225);
        let unnamed = page(&root, &json!({"catalog_query":"449"})).unwrap();
        assert_eq!(unnamed["matched"], 1);
        assert!(unnamed["selections"][0]["name"].is_null());
        for params in [
            json!({"catalog_limit":0}),
            json!({"catalog_limit":201}),
            json!({"catalog_offset":451}),
            json!({"catalog_offset":-1}),
            json!({"catalog_query":7}),
            json!({"catalog_query":"a".repeat(257)}),
        ] {
            assert!(
                page(&root, &params).is_err(),
                "accepted invalid page {params}"
            );
        }
        std::fs::write(data.join("catalog.json"), b"{malformed").unwrap();
        assert_eq!(page(&root, &json!({})).unwrap()["source_status"], "invalid");
        std::fs::remove_dir_all(root).unwrap();
    }
}
