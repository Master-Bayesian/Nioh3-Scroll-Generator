//! A partially usable role read still belongs to the exact save the UI selected.

use nioh3_protected::Contract;
use serde_json::json;
use std::path::Path;

#[test]
fn partial_context_preserves_save_hash_spelling() -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/contracts");
    let contract = Contract::load(&root)?;
    let lowercase = "8a".repeat(32);
    let uppercase = lowercase.to_uppercase();
    for (source, digest, valid) in [
        (&uppercase[..], &lowercase[..], true),
        (&lowercase[..], &lowercase[..], true),
        (&uppercase[1..], &lowercase[..], false),
        (&"g0".repeat(32), &lowercase[..], false),
        (&uppercase[..], &uppercase[..], false),
    ] {
        let result = json!({
            "state": "sources_read", "save_id": "current-save", "source_sha256": source,
            "bundle": "current-held", "source": "captured", "school": "samurai", "level": 450,
            "attrs": [5,45,15,200,5,60,155], "submissions": 1, "source_state": "current_maps_matched",
            "source_digest": digest, "detail": "ARTS_KI_DOES_NOT_DEPLETE", "missing": "damage_context",
        });
        let frame = json!({"protocol": 1, "id": "read", "ok": true, "result": {
            "job_id": "ab".repeat(16), "kind": "save.loadout_context", "state": "completed",
            "sequence": 1, "cancellable": false, "progress": null, "error": null, "result": result,
        }});
        assert_eq!(contract.response_valid(&frame), valid, "{frame}");
    }
    Ok(())
}
