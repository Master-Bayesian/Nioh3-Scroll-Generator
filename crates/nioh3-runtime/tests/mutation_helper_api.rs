//! Real Windows API proof for the override session.
//!
//! The injected adapter proves the ownership state machine; it cannot prove the
//! syscalls. This test drives the real path against a helper process the test
//! itself spawns: `OpenProcess` with the override rights, `ReadProcessMemory`
//! signature check, `VirtualQueryEx`/`VirtualAllocEx` near allocation,
//! `WriteProcessMemory` of the trampoline and counter, `VirtualProtectEx` +
//! `WriteProcessMemory` + `FlushInstructionCache` of the patch, a real counter
//! read, a foreign rewrite by the helper itself, a real restore, and a real
//! confirmed exit. It never touches Nioh 3, a game, or a save.
//!
//! Only the install and restore path is exercised: the hooked trampoline is not
//! executed, because the auxiliary trampoline's matching branch expects the
//! game's own descriptor frame. Execution semantics stay covered by the
//! trampoline byte parity and by the injected ownership tests.
#![cfg(feature = "test-helper")]

use nioh3_runtime::mutation::{
    ChallengeOverrideProfile, OverrideProfile, OverrideSession, WindowsSessionMemory,
};
use nioh3_runtime::profile::{default_pc_v2_00_02, NativeRuntimeProfile, ProfileSite};
use nioh3_runtime::RuntimeError;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// The shipped auxiliary hook signature, which the helper page starts with.
const SITE_SIGNATURE: [u8; 5] = [0x48, 0x89, 0x5C, 0x24, 0x08];
/// The full eleven bytes of the helper page: signature, `mov eax, 3`, `ret`.
const PAGE_BYTES: [u8; 11] = [
    0x48, 0x89, 0x5C, 0x24, 0x08, 0xB8, 0x03, 0x00, 0x00, 0x00, 0xC3,
];

struct Helper {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    pid: u32,
    module_base: u64,
    page: u64,
}

impl Helper {
    fn spawn() -> Result<Self, RuntimeError> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_runtime_mutation_helper"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|error| RuntimeError::Io {
                path: "runtime_mutation_helper".to_string(),
                detail: error.to_string(),
            })?;
        let stdin = child.stdin.take().ok_or(RuntimeError::SessionNotOpen)?;
        let stdout = BufReader::new(child.stdout.take().ok_or(RuntimeError::SessionNotOpen)?);
        let mut helper = Self {
            child,
            stdin,
            stdout,
            pid: 0,
            module_base: 0,
            page: 0,
        };
        let ready = helper.line()?;
        let fields: Vec<&str> = ready.trim().split('\t').collect();
        if fields.first() != Some(&"ready") {
            return Err(RuntimeError::SessionNotOpen);
        }
        helper.pid = fields[1].parse().unwrap_or(0);
        helper.module_base = fields[2].parse().unwrap_or(0);
        helper.page = fields[3].parse().unwrap_or(0);
        Ok(helper)
    }

    fn line(&mut self) -> Result<String, RuntimeError> {
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .map_err(|error| RuntimeError::Io {
                path: "helper stdout".to_string(),
                detail: error.to_string(),
            })?;
        Ok(line)
    }

    fn command(&mut self, text: &str) -> Result<String, RuntimeError> {
        self.stdin
            .write_all(format!("{text}\n").as_bytes())
            .and_then(|()| self.stdin.flush())
            .map_err(|error| RuntimeError::Io {
                path: "helper stdin".to_string(),
                detail: error.to_string(),
            })?;
        self.line()
    }

    /// The page as the helper currently holds it.
    fn page_bytes(&mut self) -> Result<Vec<u8>, RuntimeError> {
        let line = self.command("bytes")?;
        let hex = line.trim().split('\t').nth(1).unwrap_or_default();
        Ok(hex
            .as_bytes()
            .chunks(2)
            .filter_map(|pair| {
                let text = std::str::from_utf8(pair).ok()?;
                u8::from_str_radix(text, 16).ok()
            })
            .collect())
    }

    fn poke(&mut self, bytes: &[u8]) -> Result<(), RuntimeError> {
        let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        self.command(&format!("poke {hex}"))?;
        Ok(())
    }

    fn quit(&mut self) -> Result<(), RuntimeError> {
        let _ = self.stdin.write_all(b"quit\n");
        let _ = self.stdin.flush();
        let _ = self.child.wait();
        Ok(())
    }

    /// A profile whose descriptor site is this helper's own page.
    fn profile(&self) -> NativeRuntimeProfile {
        let mut profile = default_pc_v2_00_02();
        profile.descriptor_complete = ProfileSite {
            name: "descriptor_complete",
            rva: self.page - self.module_base,
            signature: SITE_SIGNATURE.to_vec(),
        };
        profile
    }

    fn session(&self, seed: u32) -> Result<OverrideSession<WindowsSessionMemory>, RuntimeError> {
        Ok(OverrideSession::auxiliary(
            OverrideProfile {
                seed,
                enemy_groups: Vec::new(),
                special_rule_keys: Some([1, 2, 3]),
                terrain_value: None,
            },
            self.pid,
            self.profile(),
            WindowsSessionMemory,
        )?
        .with_module_name(
            std::path::Path::new(env!("CARGO_BIN_EXE_runtime_mutation_helper"))
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
        ))
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn the_real_api_path_installs_validates_and_restores_a_hook() -> Result<(), RuntimeError> {
    let mut helper = Helper::spawn()?;
    assert!(helper.page > 0);
    assert_eq!(helper.page_bytes()?, PAGE_BYTES.to_vec());

    let mut session = helper.session(0x0102_0304)?;
    session.start()?;
    assert!(session.active(), "the hook is installed");
    assert!(session.allocation() > 0, "a real trampoline was allocated");
    assert_eq!(
        session.identity().map(|identity| identity.pid),
        Some(helper.pid)
    );

    // The helper sees the patch written by another process.
    let patched = helper.page_bytes()?;
    assert_ne!(
        patched,
        PAGE_BYTES.to_vec(),
        "the hook was written for real"
    );
    assert_eq!(patched[0], 0xE9, "the five-byte rel32 jump");
    assert_eq!(
        &patched[..5],
        session.patch().unwrap_or_default(),
        "the helper sees exactly the patch bytes this session wrote"
    );

    // A real counter read through the session's own handle.
    assert_eq!(session.hit_count()?, 0);

    // Another writer changes the hook: the owner must not overwrite it.
    let foreign = [0xCCu8, 0xCC, 0xCC, 0xCC, 0xCC];
    helper.poke(&foreign)?;
    let error = session.stop();
    assert_eq!(
        error.err(),
        Some(RuntimeError::HookModified {
            address: session.hook_address()
        })
    );
    assert_eq!(&helper.page_bytes()?[..5], &foreign, "nothing overwritten");

    // The helper restores its own bytes; the hook then restores cleanly.
    helper.poke(&PAGE_BYTES)?;
    session.stop()?;
    assert_eq!(helper.page_bytes()?, PAGE_BYTES.to_vec());
    Ok(())
}

#[test]
fn a_confirmed_helper_exit_releases_the_owner() -> Result<(), RuntimeError> {
    let mut helper = Helper::spawn()?;
    let mut session = helper.session(7)?;
    session.start()?;
    assert!(session.active());
    helper.quit()?;
    // The process is gone, so the hook cannot be read; ownership is released
    // only because the operating system positively reports the exit.
    session.stop()?;
    assert!(!session.active());
    Ok(())
}

#[test]
fn the_challenge_session_still_requires_a_verified_capacity_getter() {
    let error = OverrideSession::challenge(
        ChallengeOverrideProfile {
            seed: 1,
            capacity: 4,
        },
        4242,
        default_pc_v2_00_02(),
        WindowsSessionMemory,
    )
    .err()
    .unwrap_or(RuntimeError::RuntimeBusy);
    assert_eq!(
        error,
        RuntimeError::InvalidOverrideProfile {
            detail: "Challenge capacity override requires verified PC v2.01 or PC v2.02"
                .to_string(),
        }
    );
}
