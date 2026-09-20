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
        "  --live-add-candidate-dry-run <json>    pinned PC v2.02 candidate dry run (test-fake build)\n",
        "  --live-add-candidate-preflight <json>  read-only real-process candidate preflight\n",
        "  --live-add-candidate-noop <json>       bounded real noop dispatch (no inventory mutation)\n",
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
        #[cfg(feature = "test-fake")]
        "--live-add-candidate-dry-run" => {
            live_add_candidate_dry_run(&read_payload(argument(rest, 0)?)?)
        }
        "--live-add-candidate-preflight" => {
            let payload = argument(rest, 0)?;
            let path = Path::new(payload);
            let text = if path.is_file() {
                std::fs::read_to_string(path)
                    .map_err(|error| format!("preflight payload {}: {error}", path.display()))?
            } else {
                payload.to_string()
            };
            live_add_candidate_preflight(&text)
        }
        "--live-add-candidate-noop" => {
            let payload = argument(rest, 0)?;
            let path = Path::new(payload);
            let text = if path.is_file() {
                std::fs::read_to_string(path)
                    .map_err(|error| format!("noop payload {}: {error}", path.display()))?
            } else {
                payload.to_string()
            };
            live_add_candidate_noop(&text)
        }
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

/// The opt-in PC v2.02 candidate dry run.
///
/// It drives the real protected application (`LiveAddApplication`) over the
/// concrete `NativeLiveAddExecutor` and the injected transport, end to end on
/// synthetic memory. No game process, debugger, Cheat Engine session or real
/// save is touched: the checkpoint and save live under this run's temporary
/// directory and are removed afterwards.
///
/// The request must name the candidate binding, and the executor still refuses
/// unless the transport proves the pinned executable identity. This is the
/// research seam; no product path names it.
#[cfg(feature = "test-fake")]
fn live_add_candidate_dry_run(payload: &str) -> Result<(), String> {
    use nioh3_runtime::mutation::inventory::{CAPACITY, RECORD_SIZE};
    use nioh3_runtime::mutation::live_add::CandidateStage;
    use nioh3_runtime::mutation::live_fakes::{
        assembly_record, candidate_payload, FakeSaveBackup, InventoryFixture, NoCatalogPolicy,
    };
    use nioh3_runtime::mutation::native_abi::{
        CANDIDATE_DISPLAY_VERSION, PC_V202_CANDIDATE_EXECUTABLE_SHA256, PC_V202_LIVE_ADD_CANDIDATE,
    };
    use nioh3_runtime::mutation::native_executor::NativeLiveAddExecutor;
    use nioh3_runtime::mutation::native_fakes::{FakeLiveAddTransport, NativeFaults};
    use nioh3_runtime::mutation::{LiveAddApplication, OperationSnapshot, SCROLL_GROUP_OFFSET};

    let request: Value = serde_json::from_str(payload)
        .map_err(|error| format!("candidate dry-run input is not valid JSON: {error}"))?;
    let context_digest =
        field_string(&request, "context_digest").map_err(|error| error.message())?;
    let seed = request
        .get("seed")
        .and_then(Value::as_u64)
        .unwrap_or(0x0BAD) as u32;
    let rarity = request.get("rarity").and_then(Value::as_u64).unwrap_or(4) as u8;
    let serial_counter = request
        .get("serial_counter")
        .and_then(Value::as_u64)
        .unwrap_or(0x4001);
    let acquisition = request
        .get("acquisition_order_counter")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    let identity = request
        .get("executable_sha256")
        .and_then(Value::as_str)
        .unwrap_or(PC_V202_CANDIDATE_EXECUTABLE_SHA256)
        .to_string();
    let records: Vec<(usize, u64, u32)> = request
        .get("records")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let values = item.as_array()?;
                    Some((
                        values.first()?.as_u64()? as usize,
                        values.get(1)?.as_u64()?,
                        values.get(2)?.as_u64()? as u32,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let steps: Vec<String> = request
        .get("steps")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_else(|| {
            vec![
                "prepare".to_string(),
                "execute".to_string(),
                "status".to_string(),
            ]
        });

    let root = std::env::temp_dir().join(format!("nioh3-candidate-dry-run-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let save_directory = root.join("76561198000000000").join("SAVEDATA00");
    std::fs::create_dir_all(&save_directory).map_err(|error| error.to_string())?;
    let save_path = save_directory.join("SAVEDATA.BIN");

    let fixture = InventoryFixture::new_for_layout(
        nioh3_runtime::mutation::inventory::PC_V202_INVENTORY_LAYOUT_CANDIDATE,
        &records,
        serial_counter,
        acquisition,
    );
    // The synthetic save the persistence check compares against: the same
    // occupied records in the shipped save shape.
    let container = fixture.container().map_err(|error| error.message())?;
    let mut save = vec![0u8; SCROLL_GROUP_OFFSET + CAPACITY as usize * RECORD_SIZE];
    for slot in 0..CAPACITY as usize {
        let start = slot * RECORD_SIZE;
        if container[start] == 0 && container[start + 1] == 0 {
            continue;
        }
        let target = SCROLL_GROUP_OFFSET + start;
        save[target..target + RECORD_SIZE].copy_from_slice(&container[start..start + RECORD_SIZE]);
    }
    std::fs::write(&save_path, &save).map_err(|error| error.to_string())?;

    let record = assembly_record(0x1E82, seed, rarity);
    let candidate = candidate_payload(
        &context_digest,
        seed,
        rarity,
        &record,
        None,
        CandidateStage::FinalRecord,
    )
    .map_err(|error| error.message())?;
    let transport = FakeLiveAddTransport::with_layout_faults(
        &root,
        fixture,
        PC_V202_LIVE_ADD_CANDIDATE,
        Some(identity.as_str()),
        NativeFaults::default(),
    )
    .map_err(|error| error.message())?;
    let executor = NativeLiveAddExecutor::candidate(transport);
    println!("binding\t{}", PC_V202_LIVE_ADD_CANDIDATE.profile_id);
    println!("display_version\t{CANDIDATE_DISPLAY_VERSION}");
    println!("executable_sha256\t{identity}");

    let mut application = LiveAddApplication::new(
        &root,
        &context_digest,
        Box::new(executor),
        Box::new(FakeSaveBackup::new(&root)),
        Box::new(NoCatalogPolicy::default()),
    )
    .map_err(|error| error.message())?;

    let mut operation_id = String::new();
    let mut plan_digest = String::new();
    for (index, step) in steps.iter().enumerate() {
        let report = |value: &Result<OperationSnapshot, runtime::RuntimeError>| match value {
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
        match step.as_str() {
            "prepare" => match application.prepare(&candidate, &save_path, None) {
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
            "status" => report(&application.status(&operation_id)),
            "recover" => report(&application.recover(&operation_id)),
            other => return Err(format!("unknown candidate dry-run step {other}")),
        }
    }
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

/// The real-process candidate preflight: read-only, no dispatch.
///
/// It proves, in order, that a bounded PC v2.02 candidate run could proceed:
/// exact executable identity, single owner, process instance, module extent,
/// dispatch/builder/insertion site bytes, the inventory owner chain, the live
/// counters, the container and its capacity, the native index, the scheduler
/// owner, and a fresh debugger state. It writes nothing and dispatches nothing.
///
/// Request fields (all optional): `pid`, `create_report` (write a JSON report
/// path), `identity_report` (write the collected facts as JSON).
fn live_add_candidate_preflight(payload: &str) -> Result<(), String> {
    use nioh3_runtime::mutation::count::sha256_hex;
    use nioh3_runtime::mutation::inventory::{
        capture_index, capture_inventory, InventoryProcess, ReadView,
        PC_V202_INVENTORY_LAYOUT_CANDIDATE,
    };
    use nioh3_runtime::mutation::native_abi::{
        CANDIDATE_DISPLAY_VERSION, PC_V202_CANDIDATE_EXECUTABLE_SHA256, PC_V202_LIVE_ADD_CANDIDATE,
    };
    use nioh3_runtime::mutation::native_executor::SAVE_GENERATION_SERIAL_MAX;
    use nioh3_runtime::platform::{
        module_range, process_creation_filetime, query_image_path, single_process_id,
        GAME_IMAGE_NAME, GAME_MODULE_NAME,
    };

    let request: Value = serde_json::from_str(payload)
        .map_err(|error| format!("preflight input is not valid JSON: {error}"))?;
    let layout = PC_V202_LIVE_ADD_CANDIDATE;
    let mut facts = serde_json::Map::new();
    let expect_exact = |value: bool, label: &str| {
        if value {
            println!("ok\t{label}");
        } else {
            println!("fail\t{label}");
        }
        value
    };

    // 1. Single owner, or a typed absence.
    let pid = match request.get("pid").and_then(Value::as_u64) {
        Some(pid) => pid as u32,
        None => match single_process_id(GAME_IMAGE_NAME) {
            Ok(pid) => pid,
            Err(error) if error.is_absence() => {
                println!("absent\t0");
                return Ok(());
            }
            Err(error) => {
                print_error(&error);
                return Ok(());
            }
        },
    };
    facts.insert("pid".to_string(), json!(pid));

    // 2. Exact executable identity: path, hash and version resource.
    let executable = match query_image_path(pid) {
        Ok(path) => path,
        Err(error) => {
            print_error(&error);
            return Ok(());
        }
    };
    let executable_bytes = match std::fs::read(&executable) {
        Ok(bytes) => bytes,
        Err(error) => {
            println!("fail\texecutable unreadable: {error}");
            return Ok(());
        }
    };
    let executable_sha256 = sha256_hex(&executable_bytes);
    facts.insert("executable".to_string(), json!(executable));
    facts.insert("executable_size".to_string(), json!(executable_bytes.len()));
    facts.insert(
        "executable_sha256".to_string(),
        json!(executable_sha256.to_uppercase()),
    );
    // `sha256_hex` renders lower case; the pinned identity is upper case, so the
    // comparison is case-insensitive on purpose.
    expect_exact(
        executable_sha256.eq_ignore_ascii_case(PC_V202_CANDIDATE_EXECUTABLE_SHA256),
        "executable_sha256 matches the pinned candidate build",
    );
    println!("executable_sha256\t{}", executable_sha256.to_uppercase());
    match runtime::file_version(&executable) {
        Ok(version) => {
            println!("version\t{}", version.display());
            facts.insert("file_version".to_string(), json!(version.display()));
        }
        Err(error) => print_error(&error),
    }

    // 3. Process instance and module extent.
    let creation = match process_creation_filetime(pid) {
        Ok(Some(creation)) => creation,
        Ok(None) => {
            println!("absent\t0");
            return Ok(());
        }
        Err(error) => {
            print_error(&error);
            return Ok(());
        }
    };
    facts.insert("creation_filetime".to_string(), json!(creation.to_string()));
    let module = match module_range(pid, GAME_MODULE_NAME) {
        Ok(module) => module,
        Err(error) => {
            print_error(&error);
            return Ok(());
        }
    };
    facts.insert("module_base".to_string(), json!(module.base));
    facts.insert("module_size".to_string(), json!(module.size));
    println!("creation\t{creation}");
    println!("module\t{:x}\t{:x}", module.base, module.size);

    // 4. Every candidate site must fit the loaded image before any read.
    let insertion_end = layout.insertion_rva + layout.insertion_size;
    let builder_end = layout.builder_rva + layout.builder_size;
    for (label, end) in [
        ("dispatch site", layout.dispatch_rva + 7),
        ("builder window", builder_end),
        ("insertion window", insertion_end),
        ("manager slot", layout.manager_pointer_rva + 8),
        ("scheduler slot", layout.scheduler_pointer_rva + 8),
        ("serial index", layout.serial_index_offset + 0x40),
    ] {
        expect_exact(end <= module.size, &format!("{label} fits the image"));
    }

    // 5. Read view: the same minimal access mask the shipped reader requests.
    let mut reader = match runtime::mutation::WindowsProcess::open_read(pid) {
        Ok(reader) => reader,
        Err(error) => {
            print_error(&error);
            return Ok(());
        }
    };
    println!("read_access\t{:#x}", reader.access());
    let mut view = ReadView {
        pid,
        module_base: module.base,
        reader: &mut reader,
    };
    if let Ok(instance) = view.creation_time() {
        expect_exact(
            instance == creation.to_string(),
            "read handle names the same process instance",
        );
    }

    // 6. Code identity of the three candidate sites.
    let dispatch_now = view
        .read(
            module.base + layout.dispatch_rva,
            layout.dispatch_signature.len(),
        )
        .map_err(|error| error.message())?;
    expect_exact(
        dispatch_now == layout.dispatch_signature,
        "dispatch prologue matches the accepted candidate signature",
    );
    println!("dispatch_bytes\t{}", hex(&dispatch_now));
    for (label, rva, size) in [
        ("builder", layout.builder_rva, layout.builder_size as usize),
        (
            "insertion",
            layout.insertion_rva,
            layout.insertion_size as usize,
        ),
    ] {
        let bytes = view
            .read(module.base + rva, size)
            .map_err(|error| error.message())?;
        facts.insert(format!("{label}_sha256"), json!(sha256_hex(&bytes)));
        println!("{label}_sha256\t{}", sha256_hex(&bytes));
        let insertion_signature = view
            .read(module.base + rva, 16)
            .map_err(|error| error.message())?;
        if label == "insertion" {
            expect_exact(
                insertion_signature == nioh3_runtime::mutation::INSERTION_SIGNATURE.to_vec(),
                "insertion prologue matches the accepted signature",
            );
        }
    }

    // 7. Inventory owner chain, counters, container, capacity and index.
    let version = CANDIDATE_DISPLAY_VERSION;
    let inventory = capture_inventory(&mut view, &PC_V202_INVENTORY_LAYOUT_CANDIDATE, version)
        .map_err(|error| error.message())?;
    println!("inventory_entries\t{}", inventory.entries.len());
    println!("serial_counter\t{}", inventory.serial_counter);
    println!(
        "acquisition_order_counter\t{}",
        inventory.acquisition_order_counter
    );
    println!("container_sha256\t{}", inventory.container_sha256);
    facts.insert(
        "inventory_entries".to_string(),
        json!(inventory.entries.len()),
    );
    facts.insert(
        "serial_counter".to_string(),
        json!(inventory.serial_counter.clone()),
    );
    facts.insert(
        "container_sha256".to_string(),
        json!(inventory.container_sha256.clone()),
    );
    expect_exact(
        inventory.capacity == layout.capacity,
        "container capacity matches the accepted 400",
    );
    let serial: u64 = inventory.serial_counter.parse().unwrap_or(0);
    expect_exact(
        serial > 0 && serial < SAVE_GENERATION_SERIAL_MAX,
        "serial counter is inside the save format domain",
    );
    expect_exact(
        inventory.acquisition_order_counter != u32::MAX,
        "acquisition-order counter can be advanced",
    );
    // The manager/data pointers the dispatch would re-check.
    let manager = read_view_u64(&mut view, module.base + layout.manager_pointer_rva)?;
    let data = read_view_u64(&mut view, manager)?;
    expect_exact(
        manager != 0 && data != 0,
        "inventory manager and data objects are loaded",
    );
    println!("manager\t{manager:#x}");
    println!("data\t{data:#x}");
    facts.insert("manager".to_string(), json!(manager));
    facts.insert("data".to_string(), json!(data));

    let index = capture_index(&mut view, &PC_V202_INVENTORY_LAYOUT_CANDIDATE, version)
        .map_err(|error| error.message())?;
    println!("index_nodes\t{}", index.entries.len());
    facts.insert("index_nodes".to_string(), json!(index.entries.len()));
    // Every retained serial resolves to its actual slot.
    let mut mismatches = 0usize;
    for entry in &inventory.entries {
        match index
            .entries
            .iter()
            .find(|node| node.serial == entry.serial)
        {
            Some(node) if node.slot as usize == entry.slot_index => {}
            _ => mismatches += 1,
        }
    }
    expect_exact(
        mismatches == 0,
        "every retained serial joins its container slot",
    );

    // 8. Scheduler owner and its accepted phase bytes.
    let scheduler = read_view_u64(&mut view, module.base + layout.scheduler_pointer_rva)?;
    println!("scheduler_owner\t{scheduler:#x}");
    facts.insert("scheduler_owner".to_string(), json!(scheduler));
    expect_exact(scheduler != 0, "scheduler owner is loaded");

    // 9. Fresh debugger state: no debug object on the target.
    let debugger = debugger_state(pid);
    println!("debugger_attached\t{}", debugger.0);
    facts.insert("debugger_attached".to_string(), json!(debugger.0));
    expect_exact(
        !debugger.0,
        "no debugger is attached to the target (fresh, not stale)",
    );

    // 10. Recovery and backup availability on the product's state root.
    let state_root = product_state_root();
    let native_executor = state_root.join("live-add").join("native-executor");
    let backups = state_root.join("backups");
    let receipts = receipt_names(&native_executor);
    println!("state_root\t{}", state_root.display());
    println!("native_executor_dir\t{}", native_executor.is_dir());
    println!("unresolved_receipts\t{}", receipts.len());
    for name in &receipts {
        println!("receipt\t{name}");
    }
    println!("backups_dir\t{}", backups.is_dir());
    println!("backup_sets\t{}", backup_set_count(&backups));
    facts.insert(
        "native_executor_dir".to_string(),
        json!(native_executor.is_dir()),
    );
    facts.insert("unresolved_receipts".to_string(), json!(receipts.len()));
    facts.insert("backups_dir".to_string(), json!(backups.is_dir()));
    facts.insert("backup_sets".to_string(), json!(backup_set_count(&backups)));
    expect_exact(
        receipts.is_empty(),
        "no unresolved native receipt owns the executor",
    );
    expect_exact(
        backups.is_dir(),
        "save-backup root exists for the checkpoint",
    );
    expect_exact(
        SAVE_GENERATION_SERIAL_MAX == 0xFFFF_FFFC,
        "serial guard mirrors the save format cap",
    );

    if let Some(path) = request.get("identity_report").and_then(Value::as_str) {
        let text = serde_json::to_string_pretty(&Value::Object(facts.clone()))
            .map_err(|error| error.to_string())?;
        std::fs::write(path, text).map_err(|error| error.to_string())?;
        println!("report\t{path}");
    }
    Ok(())
}

fn read_view_u64(
    view: &mut nioh3_runtime::mutation::inventory::ReadView<'_>,
    address: u64,
) -> Result<u64, String> {
    use nioh3_runtime::mutation::inventory::InventoryProcess;

    let raw = view.read(address, 8).map_err(|error| error.message())?;
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&raw);
    Ok(u64::from_le_bytes(bytes))
}

fn debugger_state(pid: u32) -> (bool, u32) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Diagnostics::Debug::CheckRemoteDebuggerPresent;
        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION};

        let handle = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION, 0, pid) };
        if handle.is_null() {
            return (false, 0);
        }
        let mut present = 0i32;
        let ok = unsafe { CheckRemoteDebuggerPresent(handle, &mut present) };
        unsafe { CloseHandle(handle) };
        if ok == 0 {
            return (false, 0);
        }
        (present != 0, present as u32)
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        (false, 0)
    }
}

/// The product's state root, mirroring `RuntimeApplication::state_root`.
fn product_state_root() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("APPDATA").unwrap_or_default())
        .join("io.github.master-bayesian.nioh3-studio")
}

fn receipt_names(directory: &std::path::Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn backup_set_count(root: &std::path::Path) -> usize {
    std::fs::read_dir(root)
        .map(|entries| entries.filter_map(Result::ok).count())
        .unwrap_or(0)
}

/// One bounded real-process noop dispatch at the candidate dispatch entry.
///
/// It is a redirect-and-return with no builder and no insertion: the shim stops
/// at the accepted dispatch entry, runs no game leaf, and returns to the
/// continuation. The inventory must be byte-identical before and after, and the
/// receipt must be settled with its allocation released.
///
/// Request fields: `pid`, `arm` (required - `"noop"` acknowledges the one
/// dispatch), `state_root` (product state root override), `report`.
fn live_add_candidate_noop(payload: &str) -> Result<(), String> {
    use nioh3_runtime::mutation::inventory::{
        capture_index, capture_inventory, ReadView, PC_V202_INVENTORY_LAYOUT_CANDIDATE,
    };
    use nioh3_runtime::mutation::native_abi::CANDIDATE_DISPLAY_VERSION;
    use nioh3_runtime::mutation::native_abi::PC_V202_LIVE_ADD_CANDIDATE;
    use nioh3_runtime::mutation::native_executor::{
        DispatchMode, LiveAddTransport, NativeDebugTransport,
    };
    use nioh3_runtime::platform::{
        module_range, process_creation_filetime, query_image_path, single_process_id,
        GAME_IMAGE_NAME, GAME_MODULE_NAME,
    };

    let request: Value = serde_json::from_str(payload)
        .map_err(|error| format!("noop input is not valid JSON: {error}"))?;
    if request.get("arm").and_then(Value::as_str) != Some("noop") {
        println!("refused\tarm must be \"noop\" to allow the single dispatch");
        return Ok(());
    }
    let layout = PC_V202_LIVE_ADD_CANDIDATE;
    let pid = match request.get("pid").and_then(Value::as_u64) {
        Some(pid) => pid as u32,
        None => match single_process_id(GAME_IMAGE_NAME) {
            Ok(pid) => pid,
            Err(error) if error.is_absence() => {
                println!("absent\t0");
                return Ok(());
            }
            Err(error) => {
                print_error(&error);
                return Ok(());
            }
        },
    };

    // Identity and instance before any handle with write rights is requested.
    let executable = query_image_path(pid).map_err(|error| error.message())?;
    let sha = nioh3_runtime::mutation::count::sha256_hex(
        &std::fs::read(&executable).map_err(|error| error.to_string())?,
    );
    if !sha.eq_ignore_ascii_case(
        nioh3_runtime::mutation::native_abi::PC_V202_CANDIDATE_EXECUTABLE_SHA256,
    ) {
        println!("refused\texecutable identity is not the pinned candidate build");
        return Ok(());
    }
    println!("executable_sha256\t{}", sha.to_uppercase());
    let creation = process_creation_filetime(pid)
        .map_err(|error| error.message())?
        .ok_or("target process is gone")?;
    let module = module_range(pid, GAME_MODULE_NAME).map_err(|error| error.message())?;
    println!("pid\t{pid}");
    println!("creation\t{creation}");
    println!("module\t{:x}\t{:x}", module.base, module.size);

    // A fresh read view first: capture the inventory the noop must not change.
    let mut reader =
        nioh3_runtime::mutation::WindowsProcess::open_read(pid).map_err(|e| e.message())?;
    let (before, before_index) = {
        let mut view = ReadView {
            pid,
            module_base: module.base,
            reader: &mut reader,
        };
        let inventory = capture_inventory(
            &mut view,
            &PC_V202_INVENTORY_LAYOUT_CANDIDATE,
            CANDIDATE_DISPLAY_VERSION,
        )
        .map_err(|error| error.message())?;
        let index = capture_index(
            &mut view,
            &PC_V202_INVENTORY_LAYOUT_CANDIDATE,
            CANDIDATE_DISPLAY_VERSION,
        )
        .map_err(|error| error.message())?;
        (inventory, index)
    };
    drop(reader);
    println!("before_entries\t{}", before.entries.len());
    println!("before_serial\t{}", before.serial_counter);
    println!("before_acquisition\t{}", before.acquisition_order_counter);
    println!("before_container\t{}", before.container_sha256);

    let state_root = request
        .get("state_root")
        .and_then(Value::as_str)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(product_state_root);
    let directory = state_root.join("live-add").join("native-executor");
    let mut transport = NativeDebugTransport::new(pid, layout, GAME_MODULE_NAME, &directory)
        .map_err(|error| error.message())?;
    let profile_id = transport.profile_id();
    println!("profile_id\t{profile_id}");
    let operation_id = format!("v202-noop-{}", std::process::id());
    let params = json!({
        "operation_id": operation_id,
        "pid": pid,
        "process_creation_time": creation.to_string(),
    });
    let mut dispatch_failure: Option<String> = None;
    let mut receipt = match transport.dispatch(DispatchMode::Noop, &params) {
        Ok(receipt) => Some(receipt),
        Err(error) => {
            dispatch_failure = Some(error.message());
            print_error(&error);
            println!("dispatch_failed\ttrue");
            None
        }
    };
    if receipt.is_none() {
        // A failed attempt still leaves its durable receipt behind (settled or
        // uncertain with a retained allocation); read it back so the report
        // carries the failure and its diagnostics instead of nothing.
        let durable = directory.join(format!("{operation_id}.json"));
        receipt = std::fs::read_to_string(&durable)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok());
        println!(
            "durable_receipt\t{}",
            if receipt.is_some() {
                "present"
            } else {
                "absent"
            }
        );
    }
    if let Some(receipt) = &receipt {
        let phase = receipt
            .get("phase")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        println!("noop_phase\t{phase}");
        println!(
            "noop_redirect_count\t{}",
            receipt
                .get("redirect_count")
                .and_then(Value::as_u64)
                .unwrap_or(u64::MAX)
        );
        println!(
            "noop_thread_id\t{}",
            receipt
                .get("thread_id")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        );
        println!(
            "noop_allocation\t{}",
            receipt
                .get("allocation")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        );
        println!(
            "noop_error\t{}",
            receipt.get("error").and_then(Value::as_str).unwrap_or("-")
        );
        if let Some(diagnostics) = receipt.get("diagnostics") {
            println!("noop_diagnostics\t{diagnostics}");
        }
    }

    // Independent after-view: the inventory must be byte-identical.
    let mut reader =
        nioh3_runtime::mutation::WindowsProcess::open_read(pid).map_err(|e| e.message())?;
    let (after, after_index) = {
        let mut view = ReadView {
            pid,
            module_base: module.base,
            reader: &mut reader,
        };
        let inventory = capture_inventory(
            &mut view,
            &PC_V202_INVENTORY_LAYOUT_CANDIDATE,
            CANDIDATE_DISPLAY_VERSION,
        )
        .map_err(|error| error.message())?;
        let index = capture_index(
            &mut view,
            &PC_V202_INVENTORY_LAYOUT_CANDIDATE,
            CANDIDATE_DISPLAY_VERSION,
        )
        .map_err(|error| error.message())?;
        (inventory, index)
    };
    drop(reader);
    println!("after_entries\t{}", after.entries.len());
    println!("after_serial\t{}", after.serial_counter);
    println!("after_acquisition\t{}", after.acquisition_order_counter);
    println!("after_container\t{}", after.container_sha256);
    let unchanged = before.container_sha256 == after.container_sha256
        && before.serial_counter == after.serial_counter
        && before.acquisition_order_counter == after.acquisition_order_counter
        && before.entries.len() == after.entries.len()
        && before_index.entries.len() == after_index.entries.len();
    println!("inventory_unchanged\t{unchanged}");

    if let Some(path) = request.get("report").and_then(Value::as_str) {
        let report = json!({
            "schema": "nioh3-live-add-v202-noop/v1",
            "pid": pid,
            "creation_filetime": creation.to_string(),
            "module_base": module.base,
            "module_size": module.size,
            "executable_sha256": sha.to_uppercase(),
            "profile_id": profile_id,
            "operation_id": operation_id,
            "dispatch_failed": dispatch_failure.is_some(),
            "failure": dispatch_failure,
            "receipt": receipt.unwrap_or(Value::Null),
            "inventory_unchanged": unchanged,
            "before_container_sha256": before.container_sha256,
            "after_container_sha256": after.container_sha256,
            "before_serial": before.serial_counter,
            "after_serial": after.serial_counter,
            "before_acquisition_order": before.acquisition_order_counter,
            "after_acquisition_order": after.acquisition_order_counter,
            "before_entries": before.entries.len(),
            "after_entries": after.entries.len(),
            "before_index_nodes": before_index.entries.len(),
            "after_index_nodes": after_index.entries.len(),
            "scope": "Noop redirect-and-return only; no builder, no insertion, no serial allocation, no save write",
        });
        let text = serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?;
        std::fs::write(path, text).map_err(|error| error.to_string())?;
        println!("report\t{path}");
    }
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
