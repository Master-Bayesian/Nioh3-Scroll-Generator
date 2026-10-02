#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Compatibility policy uses only owned synthetic files and scripted identities.
use nioh3_protected::compatibility::{validate_feature, CompatibilitySession, ExecutableIdentity};
use std::path::PathBuf;

fn fixture(name: &str) -> (PathBuf, ExecutableIdentity) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "nioh3-policy-{name}-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    (
        root,
        ExecutableIdentity {
            pid: 42,
            creation_filetime: 77,
            path: "D:/owned-fixture/Nioh3.exe".into(),
            version: "2.0.2.0".into(),
            sha256: "a".repeat(64),
        },
    )
}

#[test]
fn missing_backup_cannot_be_accepted_by_manual_confirmation() {
    let (root, identity) = fixture("missing");
    let mut session = CompatibilitySession::default();
    assert_eq!(
        session.prepare(&identity, &root, &[])["backup"]["verified"],
        false
    );
    assert!(session.accept(&identity, "", true, true).is_err());
    assert!(session.require(&identity).is_err());
    let repaired = prepare(&mut session, &root, &identity);
    assert!(session
        .accept(&identity, plan_id(&repaired), true, true)
        .is_ok());
}

#[test]
fn seventeen_sources_are_all_copied_without_silent_truncation() {
    let (root, identity) = fixture("seventeen");
    let sources: Vec<PathBuf> = (0..17)
        .map(|index| {
            let path = root.join(format!("{index}-SAVEDATA.BIN"));
            std::fs::write(&path, format!("owned synthetic save {index}")).unwrap();
            path
        })
        .collect();
    let mut session = CompatibilitySession::default();
    let report = session.prepare(&identity, &root, &sources);
    assert_eq!(report["backup"]["verified"], true);
    assert_eq!(report["backup"]["paths"].as_array().unwrap().len(), 17);
    assert_eq!(report["backup"]["files"].as_array().unwrap().len(), 17);
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(report["backup"]["manifest"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["files"].as_array().unwrap().len(), 17);
    for (index, source) in sources.iter().enumerate() {
        let expected = format!("owned synthetic save {index}");
        assert_eq!(std::fs::read(source).unwrap(), expected.as_bytes());
        assert_eq!(
            std::fs::read(report["backup"]["paths"][index].as_str().unwrap()).unwrap(),
            expected.as_bytes()
        );
        assert_eq!(
            report["backup"]["files"][index]["source"].as_str().unwrap(),
            source.to_str().unwrap()
        );
    }
    assert!(session
        .accept(&identity, plan_id(&report), true, true)
        .is_ok());
}

#[test]
fn unknown_version_reports_a_hard_block_without_a_plan() {
    let (_, mut identity) = fixture("unknown");
    identity.version = "9.9.9.9".into();
    let report = CompatibilitySession::default().report(&identity);
    assert_eq!(report["hard_blocks"][0]["code"], "unsupported_version");
    assert!(report["plan"].is_null());
}

fn prepare(
    session: &mut CompatibilitySession,
    root: &std::path::Path,
    identity: &ExecutableIdentity,
) -> serde_json::Value {
    let source = root.join("owned-SAVEDATA.BIN");
    std::fs::write(&source, b"owned synthetic save bytes").unwrap();
    let report = session.prepare(identity, root, &[source]);
    assert_eq!(report["backup"]["verified"], true);
    report
}
fn plan_id(report: &serde_json::Value) -> &str {
    report["plan"]["plan_id"].as_str().unwrap()
}
fn copy_path(report: &serde_json::Value) -> PathBuf {
    report["backup"]["paths"][0].as_str().unwrap().into()
}

#[test]
fn reference_identity_needs_no_warning_or_forced_consent() {
    let (_, mut identity) = fixture("reference");
    identity.sha256 = nioh3_runtime::INVENTORY_EXECUTABLE_SHA256.to_lowercase();
    let mut session = CompatibilitySession::default();
    let report = session.report(&identity);
    assert_eq!(report["warning"], false);
    assert_eq!(report["differences"], serde_json::json!([]));
    assert_eq!(report["hard_blocks"], serde_json::json!([]));
    assert!(report["plan"].is_null());
    assert!(session.require(&identity).is_ok());
}

#[test]
fn known_versions_publish_exact_differences_and_durable_consent() {
    for version in ["2.0.0.2", "2.0.1.0", "2.0.2.0"] {
        let (root, mut identity) = fixture(version);
        identity.version = version.into();
        let mut session = CompatibilitySession::default();
        let report = prepare(&mut session, &root, &identity);
        let id = plan_id(&report);
        assert_eq!(id.len(), 64);
        assert!(id.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(report["accepted"], false);
        assert_eq!(report["plan"]["audit_path"], serde_json::Value::Null);
        assert_eq!(report["hard_blocks"], serde_json::json!([]));
        let differences = report["differences"].as_array().unwrap();
        let hash = differences
            .iter()
            .find(|value| value["code"] == "executable_sha256")
            .unwrap();
        assert_eq!(hash["expected"], nioh3_runtime::INVENTORY_EXECUTABLE_SHA256);
        assert_eq!(hash["actual"], identity.sha256);
        assert_eq!(
            differences
                .iter()
                .any(|value| value["code"] == "character_layout_evidence"),
            version != "2.0.2.0"
        );
        assert_eq!(
            report["features"]["live_equipment_add"],
            version == "2.0.2.0"
        );
        assert!(session.require(&identity).is_err());
        let accepted = session.accept(&identity, id, true, true).unwrap();
        assert_eq!(accepted["accepted"], true);
        let audit: serde_json::Value = serde_json::from_slice(
            &std::fs::read(accepted["plan"]["audit_path"].as_str().unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(audit["plan_id"], id);
        assert_eq!(
            audit["identity"]["creation_filetime"],
            identity.creation_filetime
        );
        assert_eq!(audit["identity"]["sha256"], identity.sha256);
        assert_eq!(audit["bypassed_checks"], report["plan"]["bypassed_checks"]);
        assert_eq!(audit["backup_manifest"], report["backup"]["manifest"]);
        assert!(
            audit["accepted_unix_nanos"]
                .as_str()
                .unwrap()
                .parse::<u128>()
                .unwrap()
                > 0
        );
        assert!(session.require(&identity).is_ok());
        assert_eq!(
            std::fs::read(root.join("owned-SAVEDATA.BIN")).unwrap(),
            b"owned synthetic save bytes"
        );
    }
}

#[test]
fn either_unchecked_confirmation_prevents_consent_and_audit() {
    let (root, identity) = fixture("unchecked");
    let mut session = CompatibilitySession::default();
    let report = prepare(&mut session, &root, &identity);
    for (confirmed, backup_confirmed) in [(false, false), (true, false), (false, true)] {
        assert!(session
            .accept(&identity, plan_id(&report), confirmed, backup_confirmed)
            .is_err());
        assert_eq!(session.report(&identity)["accepted"], false);
        assert!(session.require(&identity).is_err());
    }
    assert!(!root.join("compatibility-consents").exists());
    assert!(session
        .accept(&identity, plan_id(&report), true, true)
        .is_ok());
}

#[test]
fn reprepare_and_cancel_invalidate_accepted_and_pending_plans() {
    let (root, identity) = fixture("cancel");
    let mut session = CompatibilitySession::default();
    let first = prepare(&mut session, &root, &identity);
    session
        .accept(&identity, plan_id(&first), true, true)
        .unwrap();
    let second = prepare(&mut session, &root, &identity);
    assert_ne!(plan_id(&first), plan_id(&second));
    assert_eq!(second["accepted"], false);
    assert!(session.require(&identity).is_err());
    assert!(session
        .accept(&identity, plan_id(&first), true, true)
        .is_err());
    session
        .accept(&identity, plan_id(&second), true, true)
        .unwrap();
    session.cancel();
    let cancelled = session.report(&identity);
    assert_eq!(cancelled["accepted"], false);
    assert!(cancelled["plan"].is_null());
    assert_eq!(
        cancelled["backup"]["verified"], true,
        "cancel retains diagnostic backup history"
    );
    assert!(session.require(&identity).is_err());
    assert!(session
        .accept(&identity, plan_id(&second), true, true)
        .is_err());
    assert!(copy_path(&first).is_file());
    assert!(copy_path(&second).is_file());
    let recovered = prepare(&mut session, &root, &identity);
    assert!(session
        .accept(&identity, plan_id(&recovered), true, true)
        .is_ok());
    assert!(session.require(&identity).is_ok());
}

#[test]
fn process_birth_change_invalidates_exact_consent_binding() {
    let (root, mut identity) = fixture("birth");
    let mut session = CompatibilitySession::default();
    let report = prepare(&mut session, &root, &identity);
    session
        .accept(&identity, plan_id(&report), true, true)
        .unwrap();
    identity.creation_filetime += 1;
    assert!(session.require(&identity).is_err());
    let changed = session.report(&identity);
    assert_eq!(changed["accepted"], false);
    assert!(changed["plan"].is_null());
    assert!(session
        .accept(&identity, plan_id(&report), true, true)
        .is_err());
    let recovered = prepare(&mut session, &root, &identity);
    assert_ne!(plan_id(&recovered), plan_id(&report));
    assert!(session
        .accept(&identity, plan_id(&recovered), true, true)
        .is_ok());
    assert!(session.require(&identity).is_ok());
}

#[test]
fn missing_or_corrupt_backup_is_rejected_before_acceptance() {
    for missing in [false, true] {
        let (root, identity) = fixture(if missing {
            "missing-copy"
        } else {
            "corrupt-copy"
        });
        let mut session = CompatibilitySession::default();
        let report = prepare(&mut session, &root, &identity);
        if missing {
            std::fs::remove_file(copy_path(&report)).unwrap();
        } else {
            std::fs::write(copy_path(&report), b"corrupted synthetic save!").unwrap();
        }
        assert!(session
            .accept(&identity, plan_id(&report), true, true)
            .is_err());
        let blocked = session.report(&identity);
        assert_eq!(blocked["accepted"], false);
        assert_eq!(blocked["backup"]["verified"], false);
        assert_eq!(blocked["hard_blocks"][0]["code"], "backup_unverified");
        assert!(blocked["plan"].is_null());
        assert!(!root.join("compatibility-consents").exists());
        let recovered = prepare(&mut session, &root, &identity);
        assert!(session
            .accept(&identity, plan_id(&recovered), true, true)
            .is_ok());
        assert!(session.require(&identity).is_ok());
    }
}

#[test]
fn forced_operations_recheck_backup_and_manifest_after_acceptance() {
    for mode in ["copy", "manifest", "missing"] {
        let (root, identity) = fixture(mode);
        let mut session = CompatibilitySession::default();
        let report = prepare(&mut session, &root, &identity);
        session
            .accept(&identity, plan_id(&report), true, true)
            .unwrap();
        match mode {
            "manifest" => {
                std::fs::write(report["backup"]["manifest"].as_str().unwrap(), b"{}").unwrap()
            }
            "missing" => std::fs::remove_file(copy_path(&report)).unwrap(),
            _ => std::fs::write(copy_path(&report), b"corrupted synthetic save!").unwrap(),
        }
        assert!(session.require(&identity).is_err());
        assert_eq!(session.report(&identity)["accepted"], false);
        let recovered = prepare(&mut session, &root, &identity);
        assert!(session
            .accept(&identity, plan_id(&recovered), true, true)
            .is_ok());
    }
}

#[test]
fn audit_persistence_failure_never_activates_consent() {
    let (root, identity) = fixture("audit-failure");
    let mut session = CompatibilitySession::default();
    let report = prepare(&mut session, &root, &identity);
    std::fs::write(
        root.join("compatibility-consents"),
        b"owned file blocks audit directory",
    )
    .unwrap();
    let error = session
        .accept(&identity, plan_id(&report), true, true)
        .unwrap_err();
    assert!(error.message.contains("COMPATIBILITY_AUDIT_FAILED"));
    let blocked = session.report(&identity);
    assert_eq!(blocked["accepted"], false);
    assert_eq!(blocked["hard_blocks"][0]["code"], "consent_audit_failed");
    assert!(session.require(&identity).is_err());
    std::fs::remove_file(root.join("compatibility-consents")).unwrap();
    let recovered = prepare(&mut session, &root, &identity);
    assert!(session
        .accept(&identity, plan_id(&recovered), true, true)
        .is_ok());
}

#[test]
fn discovery_failure_clears_previous_consent_and_preserves_the_exact_reason() {
    let (root, identity) = fixture("discovery-failure");
    let mut session = CompatibilitySession::default();
    let report = prepare(&mut session, &root, &identity);
    session
        .accept(&identity, plan_id(&report), true, true)
        .unwrap();
    let blocked = session.prepare_failed(&identity, "Synthetic discovery root is inaccessible");
    assert_eq!(
        blocked["backup"]["error"],
        "Synthetic discovery root is inaccessible"
    );
    assert_eq!(blocked["accepted"], false);
    assert!(blocked["plan"].is_null());
    assert!(session.require(&identity).is_err());
    let recovered = prepare(&mut session, &root, &identity);
    assert!(session
        .accept(&identity, plan_id(&recovered), true, true)
        .is_ok());
}

#[test]
fn unknown_version_remains_blocked_with_reference_hash_and_verified_backup() {
    let (root, mut identity) = fixture("unknown-matching-hash");
    identity.version = "9.9.9.9".into();
    identity.sha256 = nioh3_runtime::INVENTORY_EXECUTABLE_SHA256.into();
    let mut session = CompatibilitySession::default();
    let report = prepare(&mut session, &root, &identity);
    assert_eq!(report["hard_blocks"][0]["code"], "unsupported_version");
    assert!(report["plan"].is_null());
    assert!(session
        .accept(&identity, &"0".repeat(64), true, true)
        .is_err());
    assert!(session.require(&identity).is_err());
    assert_eq!(report["features"]["live_equipment_add"], false);
}

#[test]
fn schema_accepts_cancel_and_exact_plan_ids_but_refuses_arbitrary_paths() {
    let contract = nioh3_protected::Contract::load(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/contracts"),
    )
    .unwrap();
    let request = |params| serde_json::json!({"protocol":1,"id":"policy","method":"runtime.compatibility","params":params});
    assert!(contract.request_valid(&request(serde_json::json!({"action":"cancel"}))));
    assert!(contract.request_valid(&request(serde_json::json!({"action":"accept","plan_id":"a".repeat(64),"confirmed":true,"backup_confirmed":true}))));
    assert!(!contract.request_valid(&request(
        serde_json::json!({"action":"accept","plan_id":"wrong"})
    )));
    assert!(!contract.request_valid(&request(
        serde_json::json!({"action":"accept","plan_id":"a".repeat(64),"audit_path":"D:/elsewhere"})
    )));
}

#[test]
fn changed_source_before_acceptance_requires_a_new_backup_plan() {
    let (root, identity) = fixture("source-changed");
    let mut session = CompatibilitySession::default();
    let report = prepare(&mut session, &root, &identity);
    std::fs::write(
        root.join("owned-SAVEDATA.BIN"),
        b"newer synthetic save bytes",
    )
    .unwrap();
    assert!(session
        .accept(&identity, plan_id(&report), true, true)
        .is_err());
    let blocked = session.report(&identity);
    assert_eq!(blocked["accepted"], false);
    assert!(blocked["plan"].is_null());
    assert!(blocked["backup"]["error"]
        .as_str()
        .unwrap()
        .contains("Source changed"));
    assert!(!root.join("compatibility-consents").exists());
    assert_eq!(
        std::fs::read(copy_path(&report)).unwrap(),
        b"owned synthetic save bytes"
    );
    let recovered = session.prepare(&identity, &root, &[root.join("owned-SAVEDATA.BIN")]);
    assert_eq!(
        std::fs::read(copy_path(&recovered)).unwrap(),
        b"newer synthetic save bytes"
    );
    assert!(session
        .accept(&identity, plan_id(&recovered), true, true)
        .is_ok());
}

#[test]
fn admitted_operations_keep_the_verified_backup_snapshot_after_source_progresses() {
    let (root, identity) = fixture("source-progressed");
    let mut session = CompatibilitySession::default();
    let report = prepare(&mut session, &root, &identity);
    session
        .accept(&identity, plan_id(&report), true, true)
        .unwrap();
    std::fs::write(
        root.join("owned-SAVEDATA.BIN"),
        b"later synthetic save state",
    )
    .unwrap();
    assert!(session.require_feature(&identity, "live_character").is_ok());
    assert_eq!(
        std::fs::read(copy_path(&report)).unwrap(),
        b"owned synthetic save bytes"
    );
}

#[test]
fn consent_authorizes_only_the_reviewed_version_specific_operation_classes() {
    for version in ["2.0.0.2", "2.0.1.0", "2.0.2.0"] {
        let (root, mut identity) = fixture("capabilities");
        identity.version = version.into();
        let mut session = CompatibilitySession::default();
        let report = prepare(&mut session, &root, &identity);
        session
            .accept(&identity, plan_id(&report), true, true)
            .unwrap();
        for feature in ["live_character", "native_generation", "temporary_override"] {
            assert!(report["plan"]["allowed_features"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!(feature)));
            assert!(session.require_feature(&identity, feature).is_ok());
        }
        for feature in [
            "live_count_edit",
            "live_scroll_add",
            "challenge_capacity_override",
        ] {
            assert_eq!(
                session.require_feature(&identity, feature).is_ok(),
                version != "2.0.0.2"
            );
        }
        assert_eq!(
            session
                .require_feature(&identity, "live_equipment_add")
                .is_ok(),
            version == "2.0.2.0"
        );
        for feature in ["arbitrary_native_write", "offline_search", "save_edit"] {
            assert!(!report["plan"]["allowed_features"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!(feature)));
            assert!(session.require_feature(&identity, feature).is_err());
        }
    }
}

#[test]
fn discovery_names_the_failed_root_and_recovers_when_the_save_appears() {
    let (root, _) = fixture("discover-retry");
    let save_root = root.join("Savedata");
    let missing = nioh3_protected::compatibility::discover_sources(&save_root).unwrap_err();
    assert!(missing.message.contains(save_root.to_str().unwrap()));
    std::fs::create_dir(&save_root).unwrap();
    let empty = nioh3_protected::compatibility::discover_sources(&save_root).unwrap_err();
    assert!(empty.message.contains("<account>/SAVEDATAxx/SAVEDATA.BIN"));
    let slot = save_root.join("123/SAVEDATA00");
    std::fs::create_dir_all(&slot).unwrap();
    let save = slot.join("SAVEDATA.BIN");
    std::fs::write(&save, b"newly created synthetic save").unwrap();
    assert_eq!(
        nioh3_protected::compatibility::discover_sources(&save_root).unwrap(),
        vec![save]
    );
}

#[test]
fn live_additions_use_operation_capability_without_global_consent() {
    for version in ["2.0.1.0", "2.0.2.0"] {
        let (root, mut identity) = fixture("operation-admission");
        identity.version = version.into();
        // A known FILEVERSION with a different whole-executable hash has no
        // global consent or global backup. Its operation still has to prepare.
        let mut session = CompatibilitySession::default();
        assert!(session
            .require_feature(&identity, "live_scroll_add")
            .is_ok());
        assert_eq!(
            session
                .require_feature(&identity, "live_equipment_add")
                .is_ok(),
            version == "2.0.2.0"
        );
        assert!(session
            .require_feature(&identity, "live_character")
            .is_err());
        let report = session.report(&identity);
        assert_eq!(report["accepted"], false);
        assert!(report["plan"].is_null());
        assert!(report["backup"].is_null());
        assert_eq!(
            report["operation_scoped_features"],
            if version == "2.0.2.0" {
                serde_json::json!(["live_scroll_add", "live_equipment_add"])
            } else {
                serde_json::json!(["live_scroll_add"])
            }
        );
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        if version == "2.0.1.0" {
            let error = validate_feature(&identity, "live_equipment_add").unwrap_err();
            assert!(error.message.contains("COMPATIBILITY_FEATURE_UNSUPPORTED"));
            assert!(error.message.contains(version));
            assert!(error.message.contains("reconnect"));
        }
    }
}

#[test]
fn live_addition_admission_is_not_global_backup_or_cancellation_authority() {
    let (root, identity) = fixture("operation-isolation");
    let mut session = CompatibilitySession::default();
    let prepared = prepare(&mut session, &root, &identity);
    for feature in ["live_scroll_add", "live_equipment_add"] {
        assert!(!prepared["plan"]["allowed_features"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!(feature)));
    }
    session
        .accept(&identity, plan_id(&prepared), true, true)
        .unwrap();
    session.cancel();
    // A missing unrelated global backup does not replace the operation's own
    // backup verification, and cancellation does not manufacture consent.
    std::fs::remove_file(copy_path(&prepared)).unwrap();
    for feature in ["live_scroll_add", "live_equipment_add"] {
        assert!(session.require_feature(&identity, feature).is_ok());
    }
    let report = session.report(&identity);
    assert_eq!(report["accepted"], false);
    assert!(report["plan"].is_null());
    assert_eq!(report["backup"]["verified"], false);
    assert!(session
        .require_feature(&identity, "live_character")
        .is_err());
    assert!(session
        .accept(&identity, plan_id(&prepared), true, true)
        .is_err());
    let failed = session.prepare_failed(&identity, "Unrelated global save root is inaccessible");
    assert_eq!(failed["accepted"], false);
    assert!(failed["plan"].is_null());
    for feature in ["live_scroll_add", "live_equipment_add"] {
        assert!(session.require_feature(&identity, feature).is_ok());
    }
    assert!(session
        .require_feature(&identity, "native_generation")
        .is_err());
}

#[test]
fn live_addition_capability_never_guesses_an_unknown_version_binding() {
    // Ver 2.00.01 has no authenticated four-part FILEVERSION mapping. Neither
    // its public label nor a guessed tuple is a registered operation binding.
    for version in ["2.00.01", "2.0.0.1", "9.9.9.9", "2.0.0.2"] {
        let (root, mut identity) = fixture("operation-unsupported");
        identity.version = version.into();
        identity.sha256 = nioh3_runtime::INVENTORY_EXECUTABLE_SHA256.into();
        let mut session = CompatibilitySession::default();
        let prepared = prepare(&mut session, &root, &identity);
        session.cancel();
        for feature in ["live_scroll_add", "live_equipment_add"] {
            let error = session.require_feature(&identity, feature).unwrap_err();
            assert!(error.message.contains("COMPATIBILITY_FEATURE_UNSUPPORTED"));
            assert!(error.message.contains(feature));
            assert!(error.message.contains(version));
            assert!(validate_feature(&identity, feature).is_err());
        }
        let report = session.report(&identity);
        assert_eq!(report["operation_scoped_features"], serde_json::json!([]));
        assert_eq!(report["accepted"], false);
        assert!(report["plan"].is_null());
        assert_eq!(
            std::fs::read(copy_path(&prepared)).unwrap(),
            b"owned synthetic save bytes"
        );
    }
}
