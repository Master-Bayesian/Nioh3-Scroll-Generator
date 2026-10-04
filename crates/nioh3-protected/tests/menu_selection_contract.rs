#![allow(clippy::unwrap_used)]
//! `runtime.menu_selection` replies must pass the protected contract in every
//! menu state. v0.8.3 added closed-menu diagnostics the contract did not list,
//! so every poll with the inventory menu closed failed as INVALID_RESULT and
//! live editing's follow mode stopped at once.
use nioh3_protected::Contract;
use serde_json::{json, Value};
use std::path::PathBuf;

fn frame(result: Value) -> Value {
    json!({"protocol": 1, "id": "menu", "ok": true, "result": result})
}

#[test]
fn every_menu_state_is_a_valid_reply() {
    let contract =
        Contract::load(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/contracts"))
            .unwrap();
    // Captured from PC v2.02 with the inventory menu closed.
    let closed = json!({
        "process_id": 46196, "menu_open": false,
        "diagnostics": {
            "closed_flag_offset": 28, "detail_signature_rva": 36367175, "detail_signature_verified": true,
            "expected_closed_flag": 0, "expected_open_flag": 1, "game_file_version": "2.0.2.0",
            "menu_pointer_rva": 73175456, "menu_profile": "pc_v2_02", "menu_vtable_rva": 67179128,
            "observation": {"closed_flag": 1, "menu_pointer_present": true, "menu_vtable_verified": true, "open_flag": 0, "reason": "menu_flags"},
            "open_flag_offset": 24824, "player_roots_verified": true,
        },
    });
    for result in [
        closed,
        json!({"process_id": 1, "menu_open": true, "slot_index": null}),
        json!({"process_id": 1, "menu_open": true, "container": "equipment", "slot_index": 12, "item_id": 63208}),
        json!({"process_id": 1, "menu_open": true, "container": "held", "slot_index": 3, "item_id": 100}),
    ] {
        assert!(contract.response_valid(&frame(result.clone())), "{result}");
    }
}
