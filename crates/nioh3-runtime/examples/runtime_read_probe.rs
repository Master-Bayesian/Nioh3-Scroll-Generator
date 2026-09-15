//! Deterministic probe for the read-only runtime adapter.
//!
//! Every mode prints tab-separated lines and exits 0 for both a successful read
//! and an expected typed rejection, so the cross-language gate can compare
//! semantics rather than exit codes. Only malformed usage exits 2.
//!
//! The probe opens no writable handle, allocates nothing in the target, starts
//! no thread, and neither launches nor closes any process.

use nioh3_runtime as runtime;
use serde_json::{json, Value};
use std::path::Path;

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match dispatch(&args) {
        Ok(()) => std::process::ExitCode::from(0),
        Err(message) => {
            println!("error\tUSAGE\t{message}");
            std::process::ExitCode::from(2)
        }
    }
}

fn usage() -> String {
    concat!(
        "usage: runtime_read_probe <mode> [arguments]\n",
        "  --status-ops <json>                    drive the runtime.status ownership model\n",
        "  --list-versions                        supported executable versions\n",
        "  --discover <image>                     every matching process id\n",
        "  --single <image>                       the single owner, or a typed absence\n",
        "  --creation-time <pid>                  creation FILETIME of a live process\n",
        "  --file-version <path>                  fixed version resource of an image\n",
        "  --verify-executable <path>             supported/unsupported/unreadable\n",
        "  --module <pid> <module>                module base and image size\n",
        "  --profile <path>                       parse a research profile document\n",
        "  --profile-for-version <a.b.c.d> <dir>  select the approved profile\n",
        "  --bounds <path> <module-size-hex>      per-site range validation\n",
        "  --identify <image> <module> <dir>      running-game identity\n",
        "  --verify-signatures <profile> <image> <module>\n",
        "                                         validated read view and signatures\n",
        "  --canonical <json>                     canonical JSON and its SHA-256\n",
        "  --trampoline <json>                    trampoline bytes for one profile\n",
        "  --count-scenario <json>                scripted count edit (test-fake build)\n",
        "  --descriptor <json>                    assembly descriptor bytes for one record\n",
        "  --native-abi <json>                    remote machine code for one emitter request\n",
        "  --inventory <json-file>                read-only inventory capture (test-fake build)\n",
        "  --live-add-scenario <json-file>        scripted live add or batch (test-fake build)\n",
    )
    .to_string()
}

fn dispatch(args: &[String]) -> Result<(), String> {
    let mode = args.first().map(String::as_str).ok_or_else(usage)?;
    let rest = &args[1..];
    match mode {
        "--status-ops" => status_ops(argument(rest, 0)?),
        "--list-versions" => {
            for (version, display) in runtime::SUPPORTED_GAME_VERSIONS {
                println!("version\t{}\t{display}", version.display());
            }
            println!("default\t{}", runtime::SUPPORTED_GAME_VERSION);
            Ok(())
        }
        "--discover" => {
            let image = argument(rest, 0)?;
            match runtime::discover_process_ids(image) {
                Ok(ids) => {
                    for pid in ids {
                        println!("pid\t{pid}");
                    }
                }
                Err(error) => print_error(&error),
            }
            Ok(())
        }
        "--single" => {
            let image = argument(rest, 0)?;
            match runtime::single_process_id(image) {
                Ok(pid) => println!("pid\t{pid}"),
                Err(error) if error.is_absence() => println!("absent\t0"),
                Err(error) => print_error(&error),
            }
            Ok(())
        }
        "--creation-time" => {
            let pid = parse_u32(argument(rest, 0)?)?;
            match runtime::process_creation_filetime(pid) {
                Ok(Some(filetime)) => println!("creation\t{filetime}"),
                Ok(None) => println!("exited\t0"),
                Err(error) => print_error(&error),
            }
            Ok(())
        }
        "--file-version" => {
            let path = argument(rest, 0)?;
            match runtime::file_version(path) {
                Ok(version) => println!("version\t{}", version.display()),
                Err(error) => print_error(&error),
            }
            Ok(())
        }
        "--verify-executable" => {
            let path = argument(rest, 0)?;
            let status = runtime::verify_game_executable(path);
            let version = status
                .file_version
                .map(runtime::FileVersion::display)
                .unwrap_or_else(|| "-".to_string());
            println!("state\t{}\t{version}", status.state.as_str());
            Ok(())
        }
        "--module" => {
            let pid = parse_u32(argument(rest, 0)?)?;
            let module = argument(rest, 1)?;
            match runtime::module_range(pid, module) {
                Ok(range) => println!("module\t{:x}\t{:x}", range.base, range.size),
                Err(error) => print_error(&error),
            }
            Ok(())
        }
        "--profile" => {
            let path = argument(rest, 0)?;
            match runtime::load_research_profile(Path::new(path)) {
                Ok(profile) => print_profile(&profile),
                Err(error) => print_error(&error),
            }
            Ok(())
        }
        "--profile-for-version" => {
            let version = parse_version(argument(rest, 0)?)?;
            let directory = argument(rest, 1)?;
            match runtime::profile_for_game_version(version, Path::new(directory)) {
                Ok(profile) => print_profile(&profile),
                Err(error) => print_error(&error),
            }
            Ok(())
        }
        "--bounds" => {
            let path = argument(rest, 0)?;
            let size = parse_hex_u64(argument(rest, 1)?)?;
            let profile = match runtime::load_research_profile(Path::new(path)) {
                Ok(profile) => profile,
                Err(error) => {
                    print_error(&error);
                    return Ok(());
                }
            };
            println!("module_size\t{size:#x}");
            let mut aggregate = "ok";
            for site in profile.text_sites() {
                let length = site.signature.len() as u64;
                let fits = site.rva.checked_add(length).is_some_and(|end| end <= size);
                if !fits {
                    aggregate = "RANGE_OUT_OF_BOUNDS";
                }
                println!(
                    "bounds\t{}\t{}",
                    site.name,
                    if fits { "ok" } else { "RANGE_OUT_OF_BOUNDS" }
                );
            }
            println!("aggregate\t{aggregate}");
            Ok(())
        }
        "--identify" => {
            let image = argument(rest, 0)?;
            let module = argument(rest, 1)?;
            let directory = argument(rest, 2)?;
            match runtime::identify_running_game_named(image, module, Path::new(directory)) {
                Ok(identity) => println!(
                    "identity\t{}\t{}\t{}\t{}\t{:x}\t{:x}\t{}",
                    identity.identity.pid,
                    identity.identity.creation_filetime,
                    identity.executable,
                    identity.file_version.display(),
                    identity.module.base,
                    identity.module.size,
                    identity.profile.identity_digest(),
                ),
                Err(error) if error.is_absence() => println!("absent\t0"),
                Err(error) => print_error(&error),
            }
            Ok(())
        }
        "--verify-signatures" => {
            let profile_path = argument(rest, 0)?;
            let image = argument(rest, 1)?;
            let module = argument(rest, 2)?;
            let profile = match runtime::load_research_profile(Path::new(profile_path)) {
                Ok(profile) => profile,
                Err(error) => {
                    print_error(&error);
                    return Ok(());
                }
            };
            let pid = match runtime::single_process_id(image) {
                Ok(pid) => pid,
                Err(error) if error.is_absence() => {
                    println!("absent\t0");
                    return Ok(());
                }
                Err(error) => {
                    print_error(&error);
                    return Ok(());
                }
            };
            let expected = runtime::process_creation_filetime(pid).ok().flatten();
            match runtime::ValidatedProcess::open(pid, module, profile, expected) {
                Ok(process) => match process.verify_profile_signatures() {
                    Ok(count) => println!("verified\t{count}"),
                    Err(error) => print_error(&error),
                },
                Err(error) => print_error(&error),
            }
            Ok(())
        }
        "--canonical" => {
            let value: Value = serde_json::from_str(argument(rest, 0)?)
                .map_err(|error| format!("canonical input is not valid JSON: {error}"))?;
            let canonical = nioh3_runtime::mutation::canonical_json(&value);
            println!("canonical\t{canonical}");
            println!(
                "digest\t{}",
                nioh3_runtime::mutation::count::sha256_hex(canonical.as_bytes())
            );
            Ok(())
        }
        "--trampoline" => {
            let request: Value = serde_json::from_str(argument(rest, 0)?)
                .map_err(|error| format!("trampoline input is not valid JSON: {error}"))?;
            match build_trampoline(&request) {
                Ok(code) => {
                    println!("code\t{}", hex(&code));
                    Ok(())
                }
                Err(error) => {
                    print_error(&error);
                    Ok(())
                }
            }
        }
        #[cfg(feature = "test-fake")]
        "--count-scenario" => count_scenario(argument(rest, 0)?),
        "--descriptor" => descriptor(argument(rest, 0)?),
        "--native-abi" => native_abi(argument(rest, 0)?),
        #[cfg(feature = "test-fake")]
        "--inventory" => inventory_scenario(&read_payload(argument(rest, 0)?)?),
        #[cfg(feature = "test-fake")]
        "--live-add-scenario" => live_add_scenario(&read_payload(argument(rest, 0)?)?),
        other => Err(format!("unknown mode {other}\n{}", usage())),
    }
}

/// `live_add_descriptor` bytes for one canonical installation record.
fn descriptor(payload: &str) -> Result<(), String> {
    use nioh3_runtime::mutation::{assembly_descriptor, new_assembly_record};
    let request: Value = serde_json::from_str(payload)
        .map_err(|error| format!("descriptor input is not valid JSON: {error}"))?;
    let record =
        parse_hex_bytes(&field_string(&request, "record_hex").map_err(|error| error.message())?)
            .map_err(|error| error.message())?;
    let allocate = request
        .get("allocate_serial")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    match assembly_descriptor(&record, allocate) {
        Ok(descriptor) => println!("descriptor\t{}", hex(&descriptor)),
        Err(error) => print_error(&error),
    }
    match new_assembly_record(&record) {
        Ok(assembly) => println!("assembly\t{}", hex(&assembly)),
        Err(error) => print_error(&error),
    }
    Ok(())
}

/// One remote-code request for the cross-language ABI gate.
///
/// Every kind is a pure emitter: the probe prints the bytes it computed and
/// touches no process, so the Python side can compare with the shipped
/// emitters on the same values.
fn native_abi(payload: &str) -> Result<(), String> {
    use nioh3_runtime::mutation::native_abi as abi;
    let request: Value = serde_json::from_str(payload)
        .map_err(|error| format!("native ABI input is not valid JSON: {error}"))?;
    let kind = request
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let number = |key: &str| request.get(key).and_then(Value::as_u64);
    let required = |key: &str| -> Result<u64, runtime::RuntimeError> {
        number(key).ok_or_else(|| runtime::RuntimeError::NativeAbi {
            detail: format!("native ABI request is missing {key}"),
        })
    };
    let bytes = |key: &str| -> Result<Vec<u8>, runtime::RuntimeError> {
        parse_hex_bytes(request.get(key).and_then(Value::as_str).unwrap_or_default())
    };
    let result: Result<Vec<u8>, runtime::RuntimeError> = (|| match kind {
        "dispatch" => {
            let insertion = request.get("insertion").map(|value| abi::InsertionArgs {
                serial: value.get("serial").and_then(Value::as_u64).unwrap_or(0),
                data: value.get("data").and_then(Value::as_u64).unwrap_or(0),
                manager: value.get("manager").and_then(Value::as_u64).unwrap_or(0),
                function_address: value
                    .get("function_address")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                serial_counter_offset: value
                    .get("serial_counter_offset")
                    .and_then(Value::as_u64)
                    .unwrap_or(8),
            });
            abi::build_dispatch_code(
                required("memory")?,
                required("resume")?,
                &bytes("original_hex")?,
                number("leaf"),
                required("argument")?,
                number("second_argument"),
                insertion,
                request
                    .get("preserve_rarity5")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            )
        }
        "batch" => abi::build_batch_wrapper(
            required("source")?,
            required("destination")?,
            required("function")?,
            required("count")? as u32,
        ),
        "seed_range" => abi::build_seed_range_wrapper(
            required("source")?,
            required("destination")?,
            required("function")?,
            required("start_seed")? as u32,
            required("seed_step")? as u32,
            required("count")? as u32,
        ),
        "finalizer" => abi::build_effect_finalizer_wrapper(
            required("source")?,
            required("destination")?,
            required("function")?,
            required("effect_index")? as u32,
            request
                .get("reveal")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        ),
        "finalizer_batch" => abi::build_effect_finalizer_batch_wrapper(
            required("source")?,
            required("destination")?,
            required("function")?,
            required("count")? as u32,
            required("effect_index")? as u32,
            request
                .get("reveal")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        ),
        "explicit_playthrough" => {
            let profile = runtime::default_pc_v2_00_02();
            let chain = abi::PlaythroughChainRvas::from_profile(&profile)?;
            abi::build_explicit_playthrough_seed_range_wrapper(
                required("source")?,
                required("destination")?,
                required("module_base")?,
                required("start_seed")? as u32,
                required("seed_step")? as u32,
                required("count")? as u32,
                required("playthrough")? as u32,
                number("generation_mode").unwrap_or(0) as u32,
                &chain,
            )
        }
        "source_record" => abi::build_source_record(
            &bytes("template_hex")?,
            required("seed")? as u32,
            required("rarity")? as u8,
            required("level")? as u16,
            required("recommended_level")? as u16,
            number("transfer_count").unwrap_or(0) as u32,
        ),
        other => Err(runtime::RuntimeError::NativeAbi {
            detail: format!("unknown native ABI kind {other}"),
        }),
    })();
    match result {
        Ok(code) => println!("code\t{}", hex(&code)),
        Err(error) => println!("refused\t{}\t{}", error.code(), error.message()),
    }
    Ok(())
}

/// The read-only inventory capture over a synthetic container, for the
/// cross-language gate. Both sides read exactly these bytes.
#[cfg(feature = "test-fake")]
fn inventory_scenario(payload: &str) -> Result<(), String> {
    use nioh3_runtime::mutation::live_fakes::InventoryFixture;
    let request: Value = serde_json::from_str(payload)
        .map_err(|error| format!("inventory input is not valid JSON: {error}"))?;
    let container =
        parse_hex_bytes(&field_string(&request, "container_hex").map_err(|error| error.message())?)
            .map_err(|error| error.message())?;
    let serial_counter = request
        .get("serial_counter")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let acquisition = request
        .get("acquisition_order_counter")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    let mut fixture =
        match InventoryFixture::from_container(&container, serial_counter, acquisition) {
            Ok(fixture) => fixture,
            Err(error) => {
                print_error(&error);
                return Ok(());
            }
        };
    apply_corruption(&mut fixture, request.get("corrupt").and_then(Value::as_str));
    let mut view = nioh3_runtime::mutation::live_fakes::fixture_view(&fixture);
    match nioh3_runtime::mutation::capture_inventory(
        &mut view,
        &fixture.layout,
        nioh3_runtime::mutation::LIVE_ADD_DISPLAY_VERSION,
    ) {
        Ok(inventory) => {
            println!("pid\t{}", inventory.pid);
            println!("creation\t{}", inventory.process_creation_time);
            println!("capacity\t{}", inventory.capacity);
            for entry in &inventory.entries {
                println!(
                    "entry\t{}\t{}\t{}\t{}",
                    entry.slot_index, entry.serial, entry.seed, entry.record_hex
                );
            }
            for duplicate in &inventory.duplicate_scroll_serials {
                println!("duplicate\t{duplicate}");
            }
            println!("serial_counter\t{}", inventory.serial_counter);
            println!(
                "acquisition_order_counter\t{}",
                inventory.acquisition_order_counter
            );
            println!("container_sha256\t{}", inventory.container_sha256);
        }
        Err(error) => print_error(&error),
    }
    Ok(())
}

/// Scripted live addition, driving the real application and receipt store with
/// the injected executor. Success and expected termination both exit 0 so the
/// gate can compare semantics instead of exit codes.
#[cfg(feature = "test-fake")]
fn live_add_scenario(payload: &str) -> Result<(), String> {
    use nioh3_runtime::mutation::live_fakes::{
        FakeLiveAddExecutor, FakeSaveBackup, InventoryFixture, LiveAddFaults, NoCatalogPolicy,
    };
    use nioh3_runtime::mutation::{LiveAddApplication, LiveAddBatch, OperationSnapshot};
    let request: Value = serde_json::from_str(payload)
        .map_err(|error| format!("live-add input is not valid JSON: {error}"))?;
    let container =
        parse_hex_bytes(&field_string(&request, "container_hex").map_err(|error| error.message())?)
            .map_err(|error| error.message())?;
    let serial_counter = request
        .get("serial_counter")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let acquisition = request
        .get("acquisition_order_counter")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    let save =
        parse_hex_bytes(&field_string(&request, "save_hex").map_err(|error| error.message())?)
            .map_err(|error| error.message())?;
    let candidates = request
        .get("candidates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut faults = LiveAddFaults::default();
    match request
        .get("fault")
        .and_then(Value::as_str)
        .unwrap_or("none")
    {
        "none" => {}
        "reply_lost" => faults.reply_lost = true,
        "absent" => faults.submission_absent = true,
        "idle_miss" => faults.idle_miss = true,
        "readback_changed" => faults.readback_changed = true,
        "pid_reuse" => faults.pid_reuse = true,
        other => return Err(format!("unknown fault {other}")),
    }
    let cancel_after = request.get("cancel_after").and_then(Value::as_u64);
    let context_digest =
        field_string(&request, "context_digest").map_err(|error| error.message())?;

    let root = std::env::temp_dir().join(format!(
        "nioh3-runtime-live-{}-{}",
        std::process::id(),
        serial_counter
    ));
    let _ = std::fs::remove_dir_all(&root);
    let save_directory = root.join("76561198000000000").join("SAVEDATA00");
    std::fs::create_dir_all(&save_directory).map_err(|error| error.to_string())?;
    let save_path = save_directory.join("SAVEDATA.BIN");
    std::fs::write(&save_path, &save).map_err(|error| error.to_string())?;

    let mut fixture = InventoryFixture::from_container(&container, serial_counter, acquisition)
        .map_err(|error| error.message())?;
    apply_corruption(&mut fixture, request.get("corrupt").and_then(Value::as_str));
    let mut application = LiveAddApplication::new(
        &root,
        &context_digest,
        Box::new(FakeLiveAddExecutor::with_faults(fixture, faults)),
        Box::new(FakeSaveBackup::new(&root)),
        Box::new(NoCatalogPolicy::default()),
    )
    .map_err(|error| error.message())?;

    let mut operation_id = String::new();
    let mut plan_digest = String::new();
    let mut batch_id = String::new();
    let mut batch_digest = String::new();
    let steps = request
        .get("steps")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for (index, step) in steps.iter().enumerate() {
        let name = step.as_str().unwrap_or_default();
        let report = |value: &Result<OperationSnapshot, nioh3_runtime::RuntimeError>| match value {
            Ok(snapshot) => println!(
                "step\t{index}\t{}\t{}",
                snapshot.state.as_str(),
                snapshot
                    .receipt
                    .as_ref()
                    .and_then(|receipt| receipt.get("error"))
                    .and_then(Value::as_str)
                    .unwrap_or("-")
            ),
            Err(error) => println!("step\t{index}\terror\t{}", error.message()),
        };
        match name {
            "prepare" => match application.prepare(&candidates[0], &save_path, None) {
                Ok(prepared) => {
                    operation_id = prepared.snapshot.operation_id.clone();
                    plan_digest = prepared.snapshot.plan_digest.clone();
                    println!(
                        "prepared\t{}\t{}\t{}\t{}",
                        prepared.snapshot.state.as_str(),
                        prepared.seed,
                        prepared.rarity,
                        prepared.count_before
                    );
                }
                Err(error) => println!("prepared\terror\t{}", error.message()),
            },
            "execute" => report(&application.execute(&operation_id, &plan_digest)),
            "recover" => report(&application.recover(&operation_id)),
            "status" => report(&application.status(&operation_id)),
            "cancel" => report(&application.cancel(&operation_id)),
            "batch_prepare" => {
                match LiveAddBatch::prepare(&mut application, &candidates, &save_path) {
                    Ok(value) => {
                        batch_id = value
                            .get("batch_id")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();
                        batch_digest = value
                            .get("plan_digest")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();
                        println!(
                            "batch\t{}\t{}\t{}",
                            value
                                .get("state")
                                .and_then(Value::as_str)
                                .unwrap_or("unknown"),
                            value.get("count").and_then(Value::as_u64).unwrap_or(0),
                            0
                        );
                    }
                    Err(error) => println!("batch\terror\t{}\t0", error.message()),
                }
            }
            "batch_execute" => {
                let mut seen = 0u64;
                let mut cancel = || {
                    seen += 1;
                    cancel_after.is_some_and(|limit| seen > limit)
                };
                match LiveAddBatch::execute(
                    &mut application,
                    &batch_id,
                    &batch_digest,
                    &mut cancel,
                    &mut |_value| {},
                ) {
                    Ok(receipt) => println!(
                        "batch\t{}\t{}\t{}",
                        receipt
                            .get("state")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown"),
                        receipt
                            .get("verified_count")
                            .and_then(Value::as_u64)
                            .unwrap_or(0),
                        0
                    ),
                    Err(error) => println!("batch\terror\t{}\t0", error.message()),
                }
            }
            "batch_status" => match LiveAddBatch::status(&application, &batch_id) {
                Ok(value) => println!(
                    "batch\t{}\t{}\t{}",
                    value
                        .get("state")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown"),
                    0,
                    value
                        .get("children")
                        .and_then(Value::as_array)
                        .map(Vec::len)
                        .unwrap_or(0)
                ),
                Err(error) => println!("batch\terror\t{}\t0", error.message()),
            },
            "batch_cancel" => match LiveAddBatch::cancel(&mut application, &batch_id) {
                Ok(value) => println!(
                    "batch\t{}\t{}\t0",
                    value
                        .get("state")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown"),
                    0
                ),
                Err(error) => println!("batch\terror\t{}\t0", error.message()),
            },
            other => return Err(format!("unknown live-add step {other}")),
        }
    }
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

fn build_trampoline(request: &Value) -> Result<Vec<u8>, runtime::RuntimeError> {
    use nioh3_runtime::mutation::{
        build_challenge_trampoline, build_override_trampoline, EnemyGroup, OverrideProfile,
    };
    let return_address = hex_u64(&field_string(request, "return_address")?)?;
    let counter = match request.get("counter_address") {
        Some(Value::String(text)) if !text.is_empty() => Some(hex_u64(text)?),
        _ => None,
    };
    let original = parse_hex_bytes(&field_string(request, "original_bytes")?)?;
    match field_string(request, "kind")?.as_str() {
        "auxiliary" => {
            let groups = request
                .get("enemy_groups")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            Some(EnemyGroup {
                                lookup_key: item.get("lookup_key")?.as_u64()? as u32,
                                role: item.get("role")?.as_u64()? as u32,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let rules = match request.get("special_rule_keys").and_then(Value::as_array) {
                Some(items) if items.len() == 3 => {
                    let mut keys = [0u16; 3];
                    for (index, item) in items.iter().enumerate() {
                        keys[index] = item.as_u64().unwrap_or(0) as u16;
                    }
                    Some(keys)
                }
                _ => None,
            };
            let terrain = request
                .get("terrain_value")
                .and_then(Value::as_u64)
                .map(|value| value as u8);
            build_override_trampoline(
                &OverrideProfile {
                    seed: field_u32(request, "seed")?,
                    enemy_groups: groups,
                    special_rule_keys: rules,
                    terrain_value: terrain,
                },
                return_address,
                counter,
                &original,
            )
        }
        "challenge" => {
            let counter = counter.ok_or_else(|| runtime::RuntimeError::InvalidOverrideProfile {
                detail: "the challenge trampoline needs its counter slot".to_string(),
            })?;
            build_challenge_trampoline(
                field_u32(request, "seed")?,
                request.get("capacity").and_then(Value::as_u64).unwrap_or(0) as u8,
                return_address,
                counter,
                &original,
            )
        }
        other => Err(runtime::RuntimeError::InvalidOverrideProfile {
            detail: format!("unknown trampoline kind {other}"),
        }),
    }
}

/// A scenario payload is a JSON file because a synthetic 400-record container
/// does not fit a Windows command line. Inline JSON still works for small ones.
#[cfg(feature = "test-fake")]
fn read_payload(argument: &str) -> Result<String, String> {
    let path = Path::new(argument);
    if path.is_file() {
        return std::fs::read_to_string(path)
            .map_err(|error| format!("scenario payload {}: {error}", path.display()));
    }
    Ok(argument.to_string())
}

/// The fail-closed inventory cases the gate compares.
#[cfg(feature = "test-fake")]
fn apply_corruption(
    fixture: &mut nioh3_runtime::mutation::live_fakes::InventoryFixture,
    corruption: Option<&str>,
) {
    let base = fixture.base;
    let layout = fixture.layout;
    match corruption.unwrap_or("none") {
        "signature" => fixture
            .memory
            .write(base + layout.insertion_rva, &[0u8; 16]),
        "capacity" => {
            let data = fixture.data().unwrap_or(0);
            fixture.memory.write(
                data + layout.container_offset + layout.capacity_offset,
                &399u64.to_le_bytes(),
            );
        }
        "owner" => fixture
            .memory
            .write(base + layout.manager_pointer_rva, &[0u8; 8]),
        _ => {}
    }
}

fn field_string(value: &Value, key: &str) -> Result<String, runtime::RuntimeError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| runtime::RuntimeError::InvalidOverrideProfile {
            detail: format!("the request needs a {key} string"),
        })
}

fn field_u32(value: &Value, key: &str) -> Result<u32, runtime::RuntimeError> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .map(|number| number as u32)
        .ok_or_else(|| runtime::RuntimeError::InvalidOverrideProfile {
            detail: format!("the request needs a {key} number"),
        })
}

fn parse_hex_bytes(text: &str) -> Result<Vec<u8>, runtime::RuntimeError> {
    let digits: Vec<u8> = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    let invalid = || runtime::RuntimeError::InvalidOverrideProfile {
        detail: format!("{text} is not hexadecimal"),
    };
    if !digits.len().is_multiple_of(2) {
        return Err(invalid());
    }
    let mut bytes = Vec::with_capacity(digits.len() / 2);
    for pair in digits.chunks(2) {
        let high = (pair[0] as char).to_digit(16).ok_or_else(invalid)?;
        let low = (pair[1] as char).to_digit(16).ok_or_else(invalid)?;
        bytes.push((high * 16 + low) as u8);
    }
    Ok(bytes)
}

/// `0x`-prefixed or decimal `u64` inside a trampoline request.
fn hex_u64(text: &str) -> Result<u64, runtime::RuntimeError> {
    let trimmed = text.trim();
    let digits = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    let parsed = if digits == trimmed {
        trimmed
            .parse::<u64>()
            .or_else(|_| u64::from_str_radix(digits, 16))
    } else {
        u64::from_str_radix(digits, 16)
    };
    parsed.map_err(|error| runtime::RuntimeError::InvalidOverrideProfile {
        detail: format!("{text} is not an address: {error}"),
    })
}

/// Scripted count edit against the fault-injecting adapter.
#[cfg(feature = "test-fake")]
fn count_scenario(payload: &str) -> Result<(), String> {
    use nioh3_runtime::mutation::{count::sha256_hex, CountEditor, FakeCountMemory, Faults};
    let request: Value = serde_json::from_str(payload)
        .map_err(|error| format!("scenario input is not valid JSON: {error}"))?;
    let record =
        parse_hex_bytes(&field_string(&request, "record_hex").map_err(|error| error.message())?)
            .map_err(|error| error.message())?;
    let new_count = request
        .get("new_count")
        .and_then(Value::as_u64)
        .unwrap_or(0) as i64;
    let mut faults = Faults::default();
    match request
        .get("fault")
        .and_then(Value::as_str)
        .unwrap_or("none")
    {
        "none" => {}
        "quiet" => faults.quiet_writes = true,
        "readback" => faults.readback_after_write = Some(record.clone()),
        "write_fail" => faults.fail_write_call = Some(1),
        other => return Err(format!("unknown fault {other}")),
    }

    let root = std::env::temp_dir().join(format!(
        "nioh3-runtime-scenario-{}-{}",
        std::process::id(),
        record[0x33]
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).map_err(|error| format!("scenario root: {error}"))?;
    let save = root.join("SAVEDATA.BIN");
    let backup = root.join("BACKUP.BIN");
    std::fs::write(&save, b"scenario-save").map_err(|error| error.to_string())?;
    std::fs::write(&backup, b"scenario-save").map_err(|error| error.to_string())?;

    let mut editor = CountEditor::new(
        &root,
        Box::new(FakeCountMemory::with_faults(record.clone(), faults)),
    )
    .map_err(|error| error.message())?;
    let prepared = editor
        .prepare(
            &save,
            &sha256_hex(b"scenario-save"),
            &hex(&record),
            &backup,
            new_count,
        )
        .map_err(|error| error.message())?;
    println!(
        "prepared\t{}\t{}",
        prepared.operation_id, prepared.plan_digest
    );

    let steps = request
        .get("steps")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for (index, step) in steps.iter().enumerate() {
        let name = step.as_str().unwrap_or_default();
        let outcome = match name {
            "execute" => editor.execute(&prepared.operation_id, &prepared.plan_digest),
            "recover" => editor.recover(&prepared.operation_id),
            "status" => editor.status(&prepared.operation_id),
            other => return Err(format!("unknown scenario step {other}")),
        };
        match outcome {
            Ok(status) => println!(
                "step\t{index}\t{}\t{}",
                status.state.as_str(),
                status.error.unwrap_or_else(|| "-".to_string())
            ),
            Err(error) => println!("step\t{index}\terror\t{}", error.message()),
        }
    }
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

fn status_ops(payload: &str) -> Result<(), String> {
    let value: Value = serde_json::from_str(payload)
        .map_err(|error| format!("status ops are not valid JSON: {error}"))?;
    let ops = value
        .get("ops")
        .and_then(Value::as_array)
        .ok_or_else(|| "status ops require an \"ops\" array".to_string())?;
    let mut ownership = runtime::RuntimeOwnership::new();
    for (index, op) in ops.iter().enumerate() {
        apply_op(&mut ownership, op)?;
        let mut snapshot = ownership.status().to_json();
        if let Some(object) = snapshot.as_object_mut() {
            object.insert(
                "retired_retained".to_string(),
                json!(ownership.retained_retired_oracles()),
            );
        }
        println!("snapshot\t{index}\t{snapshot}");
    }
    Ok(())
}

fn apply_op(ownership: &mut runtime::RuntimeOwnership, op: &Value) -> Result<(), String> {
    let name = op
        .get("op")
        .and_then(Value::as_str)
        .ok_or_else(|| "each status op needs an \"op\" name".to_string())?;
    match name {
        "retire" => {
            ownership.retire_oracle(op.get("pending").and_then(Value::as_bool).unwrap_or(false))
        }
        "live_add" => ownership
            .set_live_add_unsafe(op.get("unsafe").and_then(Value::as_bool).unwrap_or(false)),
        "session" => ownership.set_session(runtime::OverrideSession::Active {
            hit_count: op.get("hits").and_then(Value::as_u64).unwrap_or(0),
        }),
        "session_fault" => ownership.set_session(runtime::OverrideSession::Faulted {
            message: op
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        }),
        "session_stop" => ownership.set_session(runtime::OverrideSession::Absent),
        other => return Err(format!("unknown status op {other}")),
    }
    Ok(())
}

fn print_profile(profile: &runtime::NativeRuntimeProfile) {
    println!("display\t{}", profile.display_version);
    for site in profile.text_sites() {
        println!(
            "site\t{}\t{:#x}\t{}",
            site.name,
            site.rva,
            hex(&site.signature)
        );
    }
    println!(
        "data\tplaythrough_selector_pointer\t{:#x}",
        profile.playthrough_selector_pointer_rva
    );
    println!("digest\t{}", profile.identity_digest());
}

fn print_error(error: &runtime::RuntimeError) {
    println!("error\t{}\t{}", error.code(), error.message());
}

fn hex(bytes: &[u8]) -> String {
    let mut rendered = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        rendered.push_str(&format!("{byte:02x}"));
    }
    rendered
}

fn argument(args: &[String], index: usize) -> Result<&str, String> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("missing argument {index}\n{}", usage()))
}

fn parse_u32(text: &str) -> Result<u32, String> {
    text.trim()
        .parse::<u32>()
        .map_err(|error| format!("{text} is not a process id: {error}"))
}

fn parse_hex_u64(text: &str) -> Result<u64, String> {
    let trimmed = text.trim();
    let digits = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    u64::from_str_radix(digits, 16).map_err(|error| format!("{text} is not hexadecimal: {error}"))
}

fn parse_version(text: &str) -> Result<runtime::FileVersion, String> {
    let parts: Vec<&str> = text.trim().split('.').collect();
    if parts.len() != 4 {
        return Err(format!("{text} is not a four-part file version"));
    }
    let mut values = [0u16; 4];
    for (index, part) in parts.iter().enumerate() {
        values[index] = part
            .parse::<u16>()
            .map_err(|error| format!("{text} is not a four-part file version: {error}"))?;
    }
    Ok(runtime::FileVersion::new(
        values[0], values[1], values[2], values[3],
    ))
}
