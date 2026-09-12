use super::*;
use std::io::Cursor;
use tempfile::TempDir;
use zip::write::SimpleFileOptions;

fn product(flavor: &str, extra: &[(&str, &[u8])]) -> Vec<u8> {
    let mut records = vec![
        (ENTRY, flavor.as_bytes()),
        (STUB, b"stub".as_slice()),
        ("worker/nioh3-search-worker.exe", b"search".as_slice()),
        ("worker/nioh3-protected-worker.exe", b"protected".as_slice()),
        ("packages/contracts/request.schema.json", b"{}".as_slice()),
        ("packages/contracts/response.schema.json", b"{}".as_slice()),
        (
            "packages/contracts/protected-request.schema.json",
            b"{}".as_slice(),
        ),
        (
            "packages/contracts/protected-response.schema.json",
            b"{}".as_slice(),
        ),
    ];
    records.extend_from_slice(extra);
    let manifest = serde_json::json!({"schema":"nioh3-tauri-manifest/v1","version":"0.7.3",
        "files":records.iter().map(|(name,data)|serde_json::json!({"path":name,"size":data.len(),"sha256":sha256(data)})).collect::<Vec<_>>()});
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in records {
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.start_file("build-manifest.json", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(manifest.to_string().as_bytes()).unwrap();
    zip.finish().unwrap().into_inner()
}
fn outer(root: &Path, label: &str, zip: &[u8]) -> PathBuf {
    let path = root.join(format!("{label}.exe"));
    let mut file = File::create(&path).unwrap();
    file.write_all(b"MZ-synthetic-stub").unwrap();
    file.write_all(zip).unwrap();
    file.write_all(MAGIC).unwrap();
    file.write_all(&(zip.len() as u64).to_le_bytes()).unwrap();
    file.write_all(Sha256::digest(zip).as_slice()).unwrap();
    path
}
fn prepare(root: &Path, flavor: &str) -> RuntimeLease {
    let file = outer(root, flavor, &product(flavor, &[]));
    RuntimeLease::prepare(&root.join("cache"), &mut Payload::open(&file).unwrap()).unwrap()
}

#[test]
fn fixed_footer_and_slice_reader_are_exact() {
    assert_eq!(MAGIC.len() + 8 + 32, FOOTER_SIZE as usize);
    let root = TempDir::new().unwrap();
    let zip = product("valid", &[]);
    let path = outer(root.path(), "valid", &zip);
    let mut payload = Payload::open(&path).unwrap();
    assert_eq!(payload.digest, sha256(&zip));
    let mut reader = payload.reader();
    reader.seek(SeekFrom::End(-1)).unwrap();
    let mut byte = [0; 2];
    assert_eq!(reader.read(&mut byte).unwrap(), 1);
    assert_eq!(byte[0], zip[zip.len() - 1]);
    assert!(reader.seek(SeekFrom::End(1)).is_err());
    assert!(reader.seek(SeekFrom::Start(u64::MAX)).is_err());
}

#[test]
fn corrupted_footer_length_and_payload_hash_are_rejected() {
    let root = TempDir::new().unwrap();
    let path = outer(root.path(), "invalid", &product("valid", &[]));
    let original = fs::read(&path).unwrap();
    for offset in [original.len() - 56, original.len() - 40, 18] {
        let mut damaged = original.clone();
        damaged[offset] ^= 0x7f;
        fs::write(&path, damaged).unwrap();
        assert!(Payload::open(&path).is_err());
    }
    fs::write(&path, b"too short").unwrap();
    assert!(Payload::open(&path).is_err());
}

#[test]
fn unsafe_zip_paths_case_duplicates_and_file_parents_are_rejected() {
    for names in [
        vec!["../escape"],
        vec!["C:/escape"],
        vec!["dir\\escape"],
        vec!["NUL.txt"],
        vec!["trailing."],
        vec![".LEASE"],
        vec!["same.txt", "Same.txt"],
        vec!["parent", "parent/child"],
    ] {
        let root = TempDir::new().unwrap();
        let additions: Vec<_> = names
            .iter()
            .map(|name| (*name, b"data".as_slice()))
            .collect();
        let file = outer(root.path(), "unsafe", &product("unsafe", &additions));
        assert!(
            RuntimeLease::prepare(
                &root.path().join("cache"),
                &mut Payload::open(&file).unwrap()
            )
            .is_err(),
            "{names:?}"
        );
        assert!(!root.path().join("escape").exists());
    }
}

#[test]
fn zip_symlink_and_expansion_limit_are_rejected_before_extraction() {
    let root = TempDir::new().unwrap();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.add_symlink("link", "outside", SimpleFileOptions::default())
        .unwrap();
    let data = zip.finish().unwrap().into_inner();
    let file = outer(root.path(), "symlink", &data);
    assert!(RuntimeLease::prepare(
        &root.path().join("cache"),
        &mut Payload::open(&file).unwrap()
    )
    .is_err());
    let mut oversized = product("oversized", &[]);
    let central = oversized
        .windows(4)
        .position(|n| n == [0x50, 0x4b, 0x01, 0x02])
        .unwrap();
    oversized[central + 24..central + 28]
        .copy_from_slice(&((MAX_ZIP_BYTES + 1) as u32).to_le_bytes());
    let file = outer(root.path(), "oversized", &oversized);
    assert!(RuntimeLease::prepare(
        &root.path().join("cache"),
        &mut Payload::open(&file).unwrap()
    )
    .is_err());
}

#[test]
fn repeated_launches_share_one_verified_runtime_and_keep_profile_separate() {
    let root = TempDir::new().unwrap();
    let profile = root.path().join("profile");
    fs::create_dir(&profile).unwrap();
    fs::write(profile.join("save.bin"), b"user-progress").unwrap();
    let first = prepare(root.path(), "same");
    let modified = fs::metadata(first.directory.join(ENTRY))
        .unwrap()
        .modified()
        .unwrap();
    let second = prepare(root.path(), "same");
    assert_eq!(first.directory, second.directory);
    assert_eq!(
        modified,
        fs::metadata(second.directory.join(ENTRY))
            .unwrap()
            .modified()
            .unwrap()
    );
    assert!(open_lock(&first.directory.join(LEASE))
        .unwrap()
        .try_lock()
        .is_err());
    drop(first);
    assert!(open_lock(&second.directory.join(LEASE))
        .unwrap()
        .try_lock()
        .is_err());
    let directory = second.directory.clone();
    drop(second);
    assert!(open_lock(&directory.join(LEASE))
        .unwrap()
        .try_lock()
        .is_ok());
    assert_eq!(
        fs::read(profile.join("save.bin")).unwrap(),
        b"user-progress"
    );
}

#[test]
fn concurrent_launches_do_not_race_extraction_or_leave_staging_trees() {
    let root = TempDir::new().unwrap();
    let outer = outer(root.path(), "concurrent", &product("concurrent", &[]));
    let cache = root.path().join("cache");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let (outer, cache, barrier) = (outer.clone(), cache.clone(), barrier.clone());
            thread::spawn(move || {
                barrier.wait();
                RuntimeLease::prepare(&cache, &mut Payload::open(&outer).unwrap()).unwrap()
            })
        })
        .collect();
    let leases: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(leases[0].directory, leases[1].directory);
    assert_eq!(fs::read_dir(cache).unwrap().count(), 2); // One global lock and one runtime.
}

#[test]
fn inactive_cache_is_repaired_from_payload_but_active_cache_is_never_overwritten() {
    let root = TempDir::new().unwrap();
    let first = prepare(root.path(), "repair");
    fs::write(first.directory.join(ENTRY), b"altered").unwrap();
    let file = outer(root.path(), "repair", &product("repair", &[]));
    let mut payload = Payload::open(&file).unwrap();
    assert!(
        RuntimeLease::prepare(&root.path().join("cache"), &mut payload)
            .err()
            .unwrap()
            .contains("ACTIVE_CACHE")
    );
    let directory = first.directory.clone();
    drop(first);
    let repaired = RuntimeLease::prepare(&root.path().join("cache"), &mut payload).unwrap();
    assert_eq!(repaired.directory, directory);
    assert_eq!(fs::read(repaired.directory.join(ENTRY)).unwrap(), b"repair");
}

#[test]
fn cache_manifest_itself_is_bound_to_original_embedded_zip() {
    let root = TempDir::new().unwrap();
    let lease = prepare(root.path(), "trusted");
    let directory = lease.directory.clone();
    drop(lease);
    fs::write(directory.join(ENTRY), b"replaced-app").unwrap();
    let path = directory.join("build-manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["files"][0]["size"] = serde_json::json!(b"replaced-app".len());
    manifest["files"][0]["sha256"] = serde_json::json!(sha256(b"replaced-app"));
    fs::write(&path, manifest.to_string()).unwrap();
    assert!(verify_runtime(&directory).is_ok());
    let repaired = prepare(root.path(), "trusted");
    assert_eq!(
        fs::read(repaired.directory.join(ENTRY)).unwrap(),
        b"trusted"
    );
}

#[test]
fn pruning_keeps_active_leases_then_bounds_inactive_versions() {
    let root = TempDir::new().unwrap();
    let first = prepare(root.path(), "one");
    let first_directory = first.directory.clone();
    let second = prepare(root.path(), "two");
    let third = prepare(root.path(), "three");
    assert!(first_directory.exists()); // Active leases override the inactive-cache budget.
    drop(first);
    drop(second);
    let reused = prepare(root.path(), "three");
    assert!(!first_directory.exists());
    assert_eq!(fs::read_dir(root.path().join("cache")).unwrap().count(), 3);
    assert!(third.directory.exists());
    drop(reused);
}

#[test]
fn cleanup_refuses_unexpected_user_files() {
    let root = TempDir::new().unwrap();
    let first = prepare(root.path(), "user-file");
    let directory = first.directory.clone();
    drop(first);
    fs::write(directory.join("my-save.bin"), b"preserve-me").unwrap();
    drop(prepare(root.path(), "second"));
    drop(prepare(root.path(), "third"));
    assert_eq!(
        fs::read(directory.join("my-save.bin")).unwrap(),
        b"preserve-me"
    );
    let file = outer(root.path(), "user-file", &product("user-file", &[]));
    assert!(RuntimeLease::prepare(
        &root.path().join("cache"),
        &mut Payload::open(&file).unwrap()
    )
    .is_err());
    assert_eq!(
        fs::read(directory.join("my-save.bin")).unwrap(),
        b"preserve-me"
    );
}
