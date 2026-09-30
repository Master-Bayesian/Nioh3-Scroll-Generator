//! Preimplementation E2E: real Windows debugger/shim over an owned helper only.
//! Failure cases: 0xE8 truncation/canary overlap, wrong no-serial byte, corrupted
//! output, duplicate dispatch, serial allocated twice, missing insertion result,
//! stale container and retained ownership. This never opens Nioh 3 or a save.
#![cfg(all(feature = "test-helper", windows))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use nioh3_runtime::mutation::native_abi::{hex, PC_V202_EQUIPMENT_ADD};
use nioh3_runtime::mutation::native_executor::{
    settled, DispatchMode, LiveAddTransport, NativeDebugTransport,
};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Command, Stdio},
};

#[test]
fn equipment_coordinator_prepares_confirms_and_recovers_without_repeating_calls() {
    // Prelisted failure controls: digest mismatch, claim replay, lost response,
    // preparation versus insertion, durable restart and no-serial generation.
    use nioh3_runtime::mutation::equipment_add::EquipmentAddition;
    let helper_path = env!("CARGO_BIN_EXE_runtime_mutation_helper");
    let mut child = Command::new(helper_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    let pid = line
        .trim()
        .split('\t')
        .nth(1)
        .unwrap()
        .parse::<u32>()
        .unwrap();
    let mut record = vec![0u8; 0xF0];
    record[..2].copy_from_slice(&0x1234u16.to_le_bytes());
    record[2..4].copy_from_slice(&0x1234u16.to_le_bytes());
    record[4] = 1;
    record[6] = 1;
    record[8] = 1;
    record[0x28..0x30].fill(0xFF);
    record[0xE8..0xF0].fill(0xAB);
    writeln!(input, "equipment-add {}", hex(&record)).unwrap();
    input.flush().unwrap();
    line.clear();
    output.read_line(&mut line).unwrap();
    let setup: Value = serde_json::from_str(line.trim()).unwrap();
    assert!(setup.get("error").is_none(), "{setup}");
    let state = PathBuf::from("D:/Nioh3_v080_deliverables/tmp")
        .join(format!("equipment-core-{}", std::process::id()));
    let mut app = EquipmentAddition::for_owned_helper(pid, &state).unwrap();
    // A crash after the parent request but before native receipt creation has
    // no dispatch to replay. The exact persisted request remains recoverable.
    let interrupted = "88888888-8888-4888-8888-888888888888";
    std::fs::write(state.join("equipment-add/requests").join(format!("{interrupted}.json")),serde_json::to_vec(&json!({"operation_id":interrupted,"preview_operation_id":"99999999-9999-4999-8999-999999999999","pid":pid,"input":{}})).unwrap()).unwrap();
    assert_eq!(
        app.recover(interrupted).unwrap()["state"],
        "rejected_before_dispatch"
    );
    let id = "44444444-4444-4444-8444-444444444444";
    let prepared = app
        .prepare(
            id,
            &json!({"item_id":0x1234,"level":1,"plus":0,"rarity":0,"seed":0}),
        )
        .unwrap();
    assert_eq!(prepared["state"], "prepared", "{prepared}");
    assert_eq!(prepared["preview_record_hex"], hex(&record));
    let digest = prepared["plan_digest"].as_str().unwrap().to_owned();
    assert!(app.execute(id, &"0".repeat(64)).is_err());
    assert_eq!(
        app.prepare(id, &json!({})).unwrap()["plan_digest"],
        digest,
        "prepare retry is status only"
    );
    let executed = app.execute(id, &digest).unwrap();
    assert_eq!(executed["state"], "verified", "{executed}");
    assert_eq!(
        app.execute(id, &digest).unwrap()["state"],
        "verified",
        "execute retry is status only"
    );
    drop(app);
    let mut restarted = EquipmentAddition::for_owned_helper(pid, &state).unwrap();
    assert_eq!(restarted.recover(id).unwrap()["state"], "verified");
    assert!(restarted.safe_to_shutdown());
    writeln!(input, "live-add-calls").unwrap();
    input.flush().unwrap();
    line.clear();
    output.read_line(&mut line).unwrap();
    assert_eq!(line.trim(), "live-add-calls\t2");
    let output_dir = PathBuf::from(
        "D:/Nioh3_v080_deliverables/deliverables/codex-v083-missing-features-20260930/backend",
    );
    std::fs::create_dir_all(&output_dir).unwrap();
    std::fs::write(output_dir.join("equipment-coordinator-e2e.json"),serde_json::to_vec_pretty(&json!({"pass":true,
        "boundary":"real Windows helper; native dispatch and coordinator; stand-in generation functions; no game/save",
        "prepared":prepared,"executed":executed,"restartRecovered":true,"builderCalls":2})).unwrap()).unwrap();
    writeln!(input, "quit").unwrap();
    input.flush().unwrap();
    child.wait().unwrap();
}

#[test]
fn changed_builder_output_is_not_inserted_and_is_never_replayed() {
    use nioh3_runtime::mutation::equipment_add::EquipmentAddition;
    let mut child = Command::new(env!("CARGO_BIN_EXE_runtime_mutation_helper"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    let pid = line
        .trim()
        .split('\t')
        .nth(1)
        .unwrap()
        .parse::<u32>()
        .unwrap();
    let mut record = vec![0u8; 0xF0];
    record[..2].copy_from_slice(&0x1234u16.to_le_bytes());
    record[6] = 1;
    record[8] = 1;
    record[0x28..0x30].fill(0xFF);
    writeln!(input, "equipment-add {}", hex(&record)).unwrap();
    input.flush().unwrap();
    line.clear();
    output.read_line(&mut line).unwrap();
    let setup: Value = serde_json::from_str(line.trim()).unwrap();
    let state = PathBuf::from("D:/Nioh3_v080_deliverables/tmp")
        .join(format!("equipment-mismatch-{}", std::process::id()));
    let id = "55555555-5555-4555-8555-555555555555";
    let mut app = EquipmentAddition::for_owned_helper(pid, &state).unwrap();
    let prepared = app
        .prepare(
            id,
            &json!({"item_id":0x1234,"level":1,"plus":0,"rarity":0,"seed":0}),
        )
        .unwrap();
    assert_eq!(prepared["state"], "prepared");
    writeln!(
        input,
        "poke-at {:x} 02",
        setup["template"].as_u64().unwrap() + 6
    )
    .unwrap();
    input.flush().unwrap();
    line.clear();
    output.read_line(&mut line).unwrap();
    let result = app
        .execute(id, prepared["plan_digest"].as_str().unwrap())
        .unwrap();
    assert_eq!(result["state"], "rejected_before_insertion");
    assert_eq!(
        app.execute(id, prepared["plan_digest"].as_str().unwrap())
            .unwrap()["state"],
        "rejected_before_insertion"
    );
    let mut transport = NativeDebugTransport::new(
        pid,
        PC_V202_EQUIPMENT_ADD,
        "runtime_mutation_helper.exe",
        &state.join("read-only"),
    )
    .unwrap();
    assert_eq!(
        transport
            .read(setup["data"].as_u64().unwrap() + 0x10, 2500 * 0xF0)
            .unwrap(),
        vec![0u8; 2500 * 0xF0]
    );
    writeln!(input, "live-add-calls").unwrap();
    input.flush().unwrap();
    line.clear();
    output.read_line(&mut line).unwrap();
    assert_eq!(line.trim(), "live-add-calls\t2");
    assert!(app.safe_to_shutdown());
    let out = PathBuf::from(
        "D:/Nioh3_v080_deliverables/deliverables/codex-v083-missing-features-20260930/backend",
    );
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("equipment-preview-mismatch-e2e.json"),serde_json::to_vec_pretty(&json!({"pass":true,"boundary":"owned helper; changed stand-in result; no game/save","containerUnchanged":true,"builderCalls":2,"result":result})).unwrap()).unwrap();
    writeln!(input, "quit").unwrap();
    input.flush().unwrap();
    child.wait().unwrap();
}

#[test]
fn native_equipment_preview_insert_and_recovery_are_exact_and_never_replayed() {
    let helper_path = env!("CARGO_BIN_EXE_runtime_mutation_helper");
    let mut child = Command::new(helper_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    let fields = line.trim().split('\t').collect::<Vec<_>>();
    let pid = fields[1].parse::<u32>().unwrap();
    let base = fields[2].parse::<u64>().unwrap();
    let mut record = vec![0u8; 0xF0];
    record[0..2].copy_from_slice(&0x1234u16.to_le_bytes());
    record[2..4].copy_from_slice(&0x1234u16.to_le_bytes());
    record[4] = 1;
    record[0x28..0x30].fill(0xFF);
    record[0xE8..0xF0].copy_from_slice(&0xF0E8DDAACC551122u64.to_le_bytes());
    writeln!(input, "equipment-add {}", hex(&record)).unwrap();
    input.flush().unwrap();
    line.clear();
    output.read_line(&mut line).unwrap();
    let setup: Value = serde_json::from_str(line.trim()).unwrap();
    let directory = PathBuf::from("D:/Nioh3_v080_deliverables/tmp")
        .join(format!("equipment-native-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let module = std::path::Path::new(helper_path)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let mut transport =
        NativeDebugTransport::new(pid, PC_V202_EQUIPMENT_ADD, module, &directory).unwrap();
    let mut descriptor = vec![0u8; 0xCC];
    descriptor[0] = 0x34;
    descriptor[1] = 0x12;
    descriptor[0x13] = 1;
    let preview = json!({"operation_id":"22222222-2222-4222-8222-222222222222","process_creation_time":setup["creation"],
        "descriptor_hex":hex(&descriptor),"expected_record_hex":hex(&record),"builder_code_hex":setup["builder_hex"]});
    let receipt = transport.dispatch(DispatchMode::Preview, &preview).unwrap();
    assert!(settled(&receipt), "{receipt}");
    assert_eq!(receipt["phase"], "completed");
    assert_eq!(receipt["source_hex"], hex(&record));
    assert_eq!(
        receipt["equipment_preview_before"],
        receipt["equipment_preview_after"]
    );
    assert!(
        transport.dispatch(DispatchMode::Preview, &preview).is_err(),
        "duplicate operation must not replay"
    );
    descriptor[0x13] = 0;
    let mut allocated = record.clone();
    allocated[0x28..0x30].copy_from_slice(&0x3345u64.to_le_bytes());
    let insert = json!({"operation_id":"33333333-3333-4333-8333-333333333333","process_creation_time":setup["creation"],
        "descriptor_hex":hex(&descriptor),"expected_record_hex":hex(&allocated),"builder_code_hex":setup["builder_hex"],
        "insertion_code_hex":setup["insertion_hex"],"container_hex":setup["container_hex"],
        "manager":setup["manager"],"data":setup["data"],"serial":0x3345,"slot":0,
        "function_address":base+PC_V202_EQUIPMENT_ADD.insertion_rva,"scheduler_owner":setup["scheduler"]});
    let inserted = transport.dispatch(DispatchMode::Insert, &insert).unwrap();
    assert!(settled(&inserted), "{inserted}");
    assert_eq!(inserted["phase"], "completed");
    let mut stored = allocated.clone();
    stored[0x24..0x28].fill(0);
    stored[0xE4..0xF0].fill(0);
    stored[0x18] |= 0x80;
    assert_eq!(inserted["destination_hex"], hex(&stored));
    assert_eq!(inserted["status"], 3);
    assert!(
        transport.dispatch(DispatchMode::Insert, &insert).is_err(),
        "insertion cannot replay"
    );
    assert_eq!(
        transport
            .status("33333333-3333-4333-8333-333333333333")
            .unwrap()["source_hex"],
        hex(&allocated)
    );
    writeln!(input, "live-add-calls").unwrap();
    input.flush().unwrap();
    line.clear();
    output.read_line(&mut line).unwrap();
    assert_eq!(
        line.trim(),
        "live-add-calls\t2",
        "status/recovery runs no builder"
    );
    let artifact = PathBuf::from(
        "D:/Nioh3_v080_deliverables/deliverables/codex-v083-missing-features-20260930/backend",
    );
    std::fs::create_dir_all(&artifact).unwrap();
    std::fs::write(artifact.join("equipment-native-helper-e2e.json"),serde_json::to_vec_pretty(&json!({"pass":true,
        "boundary":"real Windows helper debugger; synthetic builder/insertion; no game/save","preview":receipt,"insertion":inserted})).unwrap()).unwrap();
    writeln!(input, "quit").unwrap();
    input.flush().unwrap();
    child.wait().unwrap();
}
