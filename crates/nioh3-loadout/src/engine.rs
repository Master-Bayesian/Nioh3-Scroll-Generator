//! One attack's evaluator: the equipment pipeline, the frame writes and the compiled formula, run once per hit of the move.

use crate::context::{EncodedContext, LoadoutContext};
use crate::formula::{Formula, Worker};
pub use crate::pipeline::SetState;
use crate::pipeline::{Character, Frame, Scratch, SetCapacity, Tables};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

#[path = "event_program.rs"]
mod event_program;
use event_program::{EventProgram, ExecutionEvent};
#[cfg(test)]
#[path = "legacy_source_guard_tests.rs"]
mod legacy_source_guard_tests;
#[path = "prepared_overlay.rs"]
mod prepared_overlay;
#[cfg(test)]
#[path = "shared_preparation_tests.rs"]
mod shared_preparation_tests;

pub const ITEM: usize = 0xE8;
const BANK: usize = 0xF70;
/// Candidate positions: equipment slots 4..12 are 0..8, the held melee weapon (slot 0) is 9.
pub const SLOTS: usize = 10;

#[derive(Debug)]
pub struct LoadError {
    pub file: PathBuf,
    pub reason: String,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.file.display(), self.reason)
    }
}

impl std::error::Error for LoadError {}

pub fn position_of(slot: u32) -> Option<usize> {
    match slot {
        0 => Some(9),
        4..=12 => Some(slot as usize - 4),
        _ => None,
    }
}

pub fn slot_of(position: usize) -> u32 {
    if position == 9 {
        0
    } else {
        position as u32 + 4
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Candidate {
    pub attrs: [i32; 7],
    /// Pool row worn in each position, or None to keep what the character wears.
    pub items: [Option<u32>; SLOTS],
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Outcome {
    /// Damage of the whole move, as binary32 bits.
    Damage(u32),
    /// The model refuses this outfit; the reason names the stage.
    Refused(&'static str),
    /// No compiled formula path covers this outfit. It is unevaluated, not bad.
    NoPath,
    OutsideLimits,
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_load: f32,
    pub min_defense: u32,
    pub min_health: u32,
}

impl Limits {
    pub const NONE: Limits = Limits {
        max_load: f32::INFINITY,
        min_defense: 0,
        min_health: 0,
    };
}

/// Numbers the player sees beside a result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Facts {
    pub load: f32,
    pub defense: u32,
    pub health: u32,
}

/// Raw prepared STAT words used for native replay/calibration. Reading these
/// does not invoke either formula evaluator and makes no source-origin claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct PreparedStats {
    pub primary_attack: u32,
    pub secondary_attack: u32,
    pub defense: u32,
    pub health: u32,
}

pub struct Hits {
    addresses: Vec<u64>,
    rows: Vec<Vec<u32>>,
}

/// Whether this attack's graphics-card kernel can be used, decided on first use.
pub enum Kernel {
    Untried,
    Ready(crate::gpu::Gpu),
    /// Why the CPU runs instead, in English for logs and reports.
    Unavailable(String),
}

/// CPU dispatch is selected by the context's explicit scenario tag.
enum CpuEvaluation {
    RecordedScenario,
    CapturedScene(std::sync::Mutex<FullMainState>),
}
enum FullMainState {
    Untried,
    Ready {
        runner: crate::gpu::MainCpu,
        last_failure: Option<crate::gpu::MainCpuFailure>,
    },
    Unavailable(String),
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CpuDiagnostics {
    RecordedScenario,
    CapturedScenePending,
    CapturedSceneReady {
        last_failure: Option<crate::gpu::MainCpuFailure>,
    },
    CapturedSceneUnavailable {
        reason: String,
    },
}

pub struct Engine {
    dir: PathBuf,
    bundle_inputs: Option<crate::bundle_inputs::BundleInputs>,
    context: Option<EncodedContext>,
    cpu_evaluation: CpuEvaluation,
    work_owner: Arc<()>,
    legacy_source_reference:
        OnceLock<Result<crate::pipeline::source_dependency::Signature, &'static str>>,
    kernel: std::sync::Mutex<Kernel>,
    formula: Formula,
    tables: Tables,
    frame: Frame,
    character: Character,
    ops: Vec<(u16, i64)>,
    gain: u32,
    defense: u32,
    hits: Hits,
    event_program: Option<EventProgram>,
    pub start_attrs: [i32; 7],
    /// Groups that can move this attack's damage, and the table-driven groups that write health or defense.
    pub live_groups: HashSet<u16>,
    pub survival_groups: HashSet<u16>,
}

/// The items a search may put on: records grouped by equipment slot.
pub struct Backpack {
    pub pool: Vec<[u8; ITEM]>,
    /// (slot, first pool row, count), one per searched slot.
    pub ranges: Vec<(u32, u32, u32)>,
    /// For each pool row, its index in the list the backpack was built from.
    pub origin: Vec<usize>,
}

impl Backpack {
    /// Group records by slot. Slots outside the searched positions are refused.
    pub fn from_slots(items: &[(u32, [u8; ITEM])]) -> Result<Backpack, &'static str> {
        let mut sorted: Vec<(usize, &(u32, [u8; ITEM]))> = items.iter().enumerate().collect();
        sorted.sort_by_key(|(i, (slot, _))| (*slot, *i));
        let mut pool = Vec::with_capacity(items.len());
        let mut origin = Vec::with_capacity(items.len());
        let mut ranges: Vec<(u32, u32, u32)> = Vec::new();
        for (i, (slot, record)) in sorted {
            if position_of(*slot).is_none() {
                return Err("slot outside the searched positions");
            }
            match ranges.last_mut() {
                Some(r) if r.0 == *slot => r.2 += 1,
                _ => ranges.push((*slot, pool.len() as u32, 1)),
            }
            pool.push(*record);
            origin.push(i);
        }
        Ok(Backpack {
            pool,
            ranges,
            origin,
        })
    }
}

struct Setup {
    gain: u32,
    defense: u32,
    ops: Vec<(u16, i64)>,
    ranges: Vec<(u32, u32, u32)>,
    attrs: [i32; 7],
}

pub(crate) struct ExactSetCapacities {
    pub(crate) retained: SetCapacity,
    pub(crate) rows: Vec<SetCapacity>,
}

pub struct Work {
    owner: Arc<()>,
    scratch: Scratch,
    worker: Worker,
    character: Character,
    writes: Vec<(u64, u32)>,
    weapon_swaps: Vec<(u32, usize)>,
    preparation_stats: PreparationStats,
}

/// Attempted host preparation stages in this scratch owner. These counters
/// describe real pipeline work, independently of whether MAIN executes or a
/// certificate reuses its result. They never change search coverage counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct PreparationStats {
    pub pipeline_runs: u64,
    pub frame_preparations: u64,
    pub set_checks: u64,
    pub shared_pipeline_uses: u64,
}

impl Work {
    pub fn preparation_stats(&self) -> PreparationStats {
        self.preparation_stats
    }

    pub fn reset_preparation_stats(&mut self) {
        self.preparation_stats = PreparationStats::default();
    }
}

pub(crate) type PreparedContacts = Vec<Vec<(u64, u32)>>;

/// An admitted, fully prepared candidate. Keeping the exclusive scratch
/// borrow until scoring prevents another candidate, context or packet from
/// replacing the state between its set check and evaluation.
pub(crate) struct PreparedCandidate<'a> {
    engine: &'a Engine,
    work: &'a mut Work,
}

impl PreparedCandidate<'_> {
    pub(crate) fn evaluate(self) -> Outcome {
        self.engine.evaluate_prepared(self.work, None)
    }

    pub(crate) fn evaluate_with_bound(self, bounds: &mut ReadBounds) -> SweepOutcome {
        self.engine.evaluate_prepared_with_bound(self.work, bounds)
    }

    pub(crate) fn device_contacts(self) -> Result<PreparedContacts, Outcome> {
        self.engine.prepared_device_contacts(self.work)
    }
}

pub(crate) enum SweepOutcome {
    Executed(Outcome),
    Certified(u32),
}

struct InputTrace {
    buffer: Box<[u64]>,
    addresses: Vec<u64>,
}

type ReadKey = (Option<u32>, Vec<Option<u8>>);

struct ReadShape {
    addresses: Vec<u64>,
    scores: HashMap<ReadKey, u32>,
}

/// A transient exact cache pinned to a single immutable scene. Read equality
/// certifies a score only after all writes in the candidate were validated.
pub(crate) struct ReadBounds {
    overlay: Vec<(u64, u8)>,
    overlay_layout: prepared_overlay::WriteLayout,
    overlay_layout_rebuilds: u64,
    overlay_layout_reuses: u64,
    trace: InputTrace,
    shapes: Vec<ReadShape>,
    lookup: ReadKey,
    owned_key_bytes: usize,
    validated_words: HashSet<u64>,
    mutable_bytes: HashSet<u64>,
    duration: Option<u32>,
}

impl ReadBounds {
    pub(crate) fn new() -> Self {
        Self {
            overlay: Vec::new(),
            overlay_layout: prepared_overlay::WriteLayout::default(),
            overlay_layout_rebuilds: 0,
            overlay_layout_reuses: 0,
            trace: InputTrace {
                buffer: vec![0; 4096].into_boxed_slice(),
                addresses: Vec::new(),
            },
            shapes: Vec::new(),
            lookup: (None, Vec::new()),
            owned_key_bytes: 0,
            validated_words: HashSet::new(),
            mutable_bytes: HashSet::new(),
            duration: None,
        }
    }

    fn prepare_overlay(&mut self, writes: &[(u64, u32)]) {
        if self.overlay_layout.fill(writes, &mut self.overlay) {
            self.overlay_layout_rebuilds += 1;
        } else {
            self.overlay_layout_reuses += 1;
        }
    }

    pub(crate) fn overlay_layout_counts(&self) -> (u64, u64) {
        (self.overlay_layout_rebuilds, self.overlay_layout_reuses)
    }

    fn input_byte(overlay: &[(u64, u8)], address: u64) -> Option<u8> {
        overlay
            .binary_search_by_key(&address, |&(address, _)| address)
            .ok()
            .map(|i| overlay[i].1)
    }

    fn certified_score(&mut self, writes: &[(u64, u32)]) -> Option<u32> {
        if !writes
            .iter()
            .all(|(address, _)| self.validated_words.contains(address))
        {
            return None;
        }
        self.lookup.0 = self.duration;
        for shape in self.shapes.iter().rev() {
            self.lookup.1.clear();
            self.lookup.1.extend(
                shape
                    .addresses
                    .iter()
                    .map(|&at| Self::input_byte(&self.overlay, at)),
            );
            if let Some(&bits) = shape.scores.get(&self.lookup) {
                return Some(bits);
            }
        }
        None
    }

    fn remember(&mut self, bits: u32, writes: &[(u64, u32)]) {
        const MAX_KEY_BYTES: usize = 8 * 1024 * 1024;
        const MAX_SHAPES: usize = 64;
        let mut expanded = false;
        for &(address, _) in writes {
            if self.validated_words.insert(address) {
                self.mutable_bytes.extend(address..address + 4);
                expanded = true;
            }
        }
        if expanded || self.owned_key_bytes >= MAX_KEY_BYTES {
            // Earlier certificates omitted bytes in the immutable scene.
            // A newly validated write can make those bytes candidate-owned.
            self.shapes.clear();
            self.owned_key_bytes = 0;
        }
        if self.trace.addresses.is_empty() {
            return;
        }
        self.trace
            .addresses
            .retain(|at| self.mutable_bytes.contains(at));
        self.trace.addresses.sort_unstable();
        self.trace.addresses.dedup();
        let index = match self
            .shapes
            .iter()
            .position(|shape| shape.addresses == self.trace.addresses)
        {
            Some(index) => index,
            None if self.shapes.len() == MAX_SHAPES => return,
            None => {
                self.shapes.push(ReadShape {
                    addresses: self.trace.addresses.clone(),
                    scores: HashMap::new(),
                });
                self.shapes.len() - 1
            }
        };
        let shape = &mut self.shapes[index];
        let values: Vec<_> = shape
            .addresses
            .iter()
            .map(|&at| Self::input_byte(&self.overlay, at))
            .collect();
        let bytes =
            std::mem::size_of::<ReadKey>() + values.len() * std::mem::size_of::<Option<u8>>();
        if shape.scores.insert((self.duration, values), bits).is_none() {
            self.owned_key_bytes += bytes;
        }
    }
}

fn read(dir: &Path, name: &str) -> Result<Vec<u8>, LoadError> {
    let file = dir.join(name);
    fs::read(&file).map_err(|e| LoadError {
        file,
        reason: e.to_string(),
    })
}

fn bad(dir: &Path, name: &str, reason: &str) -> LoadError {
    LoadError {
        file: dir.join(name),
        reason: reason.to_string(),
    }
}

fn numbers<T>(
    line: Option<&str>,
    dir: &Path,
    name: &str,
    radix16: bool,
) -> Result<Vec<T>, LoadError>
where
    T: std::str::FromStr + TryFrom<u64>,
{
    let line = line.ok_or_else(|| bad(dir, name, "file ends early"))?;
    line.split_ascii_whitespace()
        .map(|w| {
            if radix16 {
                u64::from_str_radix(w, 16)
                    .ok()
                    .and_then(|v| T::try_from(v).ok())
            } else {
                w.parse::<T>().ok()
            }
            .ok_or_else(|| bad(dir, name, "not a number"))
        })
        .collect()
}

fn parse_setup(dir: &Path) -> Result<Setup, LoadError> {
    parse_setup_bytes(dir, &read(dir, "gpu_setup.txt")?)
}

fn parse_setup_bytes(dir: &Path, bytes: &[u8]) -> Result<Setup, LoadError> {
    let setup =
        std::str::from_utf8(bytes).map_err(|_| bad(dir, "gpu_setup.txt", "setup is not UTF-8"))?;
    let mut lines = setup.lines();
    let head: Vec<u64> = numbers(lines.next(), dir, "gpu_setup.txt", false)?;
    let [gain, defense, op_n] = head[..] else {
        return Err(bad(dir, "gpu_setup.txt", "bad header"));
    };
    let mut ops = Vec::with_capacity(op_n as usize);
    for _ in 0..op_n {
        let line = lines
            .next()
            .ok_or_else(|| bad(dir, "gpu_setup.txt", "file ends early"))?;
        let mut w = line.split_ascii_whitespace();
        let group = w.next().and_then(|v| v.parse::<u16>().ok());
        let value = w.next().and_then(|v| v.parse::<i64>().ok());
        let (Some(group), Some(value)) = (group, value) else {
            return Err(bad(dir, "gpu_setup.txt", "bad source line"));
        };
        ops.push((group, value));
    }
    let slot_n: Vec<usize> = numbers(lines.next(), dir, "gpu_setup.txt", false)?;
    let mut ranges = Vec::new();
    for _ in 0..slot_n.first().copied().unwrap_or(0) {
        let r: Vec<u32> = numbers(lines.next(), dir, "gpu_setup.txt", false)?;
        let [slot, first, count] = r[..] else {
            return Err(bad(dir, "gpu_setup.txt", "bad slot line"));
        };
        if position_of(slot).is_none() {
            return Err(bad(
                dir,
                "gpu_setup.txt",
                "slot outside the searched positions",
            ));
        }
        ranges.push((slot, first, count));
    }
    let attrs: Vec<i32> = lines
        .next()
        .ok_or_else(|| bad(dir, "gpu_setup.txt", "no attributes"))?
        .split_ascii_whitespace()
        .map(|w| {
            w.parse()
                .map_err(|_| bad(dir, "gpu_setup.txt", "bad attribute"))
        })
        .collect::<Result<_, _>>()?;
    let start_attrs: [i32; 7] = attrs
        .try_into()
        .map_err(|_| bad(dir, "gpu_setup.txt", "need seven attributes"))?;

    Ok(Setup {
        gain: gain as u32,
        defense: defense as u32,
        ops,
        ranges,
        attrs: start_attrs,
    })
}

impl Engine {
    /// An exported attack directory: programs.txt, arena.bin, mapped.bin, tables.bin, frame.bin, characters.bin, hits.txt,
    /// gpu_setup.txt and pool.bin.
    pub fn open(dir: &Path) -> Result<Engine, LoadError> {
        Self::open_inputs(
            dir,
            None,
            crate::bundle_inputs::BundleInputs::load(dir, true)?,
        )
    }

    /// Create a separate immutable evaluation session. The sample character,
    /// setup and frame are replaced together; CPU and GPU share one encoding.
    /// Context provenance is not a claim of live-game or scenario acceptance.
    pub fn open_with_context(dir: &Path, context: &LoadoutContext) -> Result<Engine, LoadError> {
        let legacy_image = matches!(
            context.manifest().scenario,
            crate::context::Scenario::RecordedScenario
        );
        let inputs = crate::bundle_inputs::BundleInputs::load_for_context(dir, legacy_image)?;
        let ranges = parse_setup_bytes(dir, inputs.required("gpu_setup.txt"))?.ranges;
        let encoded = context.encode(&ranges);
        let base =
            Frame::load(inputs.required("frame.bin")).map_err(|r| bad(dir, "frame.bin", r))?;
        let frame = Frame::load(context.frame()).map_err(|r| bad(dir, "context/frame.bin", r))?;
        if !frame.same_layout(&base) {
            return Err(bad(
                dir,
                "context/frame.bin",
                "context frame does not match the attack bundle address layout",
            ));
        }
        Self::open_inputs(dir, Some(encoded), inputs)
    }

    /// Identity of the explicit player context; None is the legacy sample path.
    pub fn context_digest(&self) -> Option<&str> {
        self.context.as_ref().map(EncodedContext::context_digest)
    }

    /// Identity of the immutable attack data owned by this Engine. This is
    /// separate from the player context and does not attest a loaded DLL.
    pub fn bundle_input_digest(&self) -> Option<String> {
        self.bundle_inputs.as_ref().map(|inputs| inputs.digest())
    }

    /// Underlying full-MAIN capability/input errors and native bail evidence.
    /// A captured scene never hides these failures by using recorded paths.
    pub fn cpu_diagnostics(&self) -> CpuDiagnostics {
        match &self.cpu_evaluation {
            CpuEvaluation::RecordedScenario => CpuDiagnostics::RecordedScenario,
            CpuEvaluation::CapturedScene(owner) => match owner.lock() {
                Ok(state) => match &*state {
                    FullMainState::Untried => CpuDiagnostics::CapturedScenePending,
                    FullMainState::Ready { last_failure, .. } => {
                        CpuDiagnostics::CapturedSceneReady {
                            last_failure: last_failure.clone(),
                        }
                    }
                    FullMainState::Unavailable(reason) => {
                        CpuDiagnostics::CapturedSceneUnavailable {
                            reason: reason.clone(),
                        }
                    }
                },
                Err(_) => CpuDiagnostics::CapturedSceneUnavailable {
                    reason: "full-MAIN CPU owner lock is poisoned".into(),
                },
            },
        }
    }

    /// Inspectable exact context buffers used by both CPU parsing and nk_open.
    pub fn encoded_context(&self) -> Option<&EncodedContext> {
        self.context.as_ref()
    }

    fn open_inputs(
        dir: &Path,
        context: Option<EncodedContext>,
        inputs: crate::bundle_inputs::BundleInputs,
    ) -> Result<Engine, LoadError> {
        let input = |name: &str| -> Result<Vec<u8>, LoadError> {
            match context.as_ref().and_then(|c| c.file(name)) {
                Some(bytes) => Ok(bytes.to_vec()),
                None => Ok(inputs.required(name).to_vec()),
            }
        };
        let formula = Formula::load_buffers(
            dir,
            inputs.required("programs.txt"),
            inputs.required("arena.bin"),
            inputs.file("mapped.bin"),
        )?;
        let tables =
            Tables::load(inputs.required("tables.bin")).map_err(|r| bad(dir, "tables.bin", r))?;
        let mut frame = Frame::load(&input("frame.bin")?).map_err(|r| bad(dir, "frame.bin", r))?;
        let character = Frame::characters(&input("characters.bin")?)
            .map_err(|r| bad(dir, "characters.bin", r))?
            .into_iter()
            .next()
            .ok_or_else(|| bad(dir, "characters.bin", "no character"))?;

        let declared_hits = std::str::from_utf8(inputs.required("hits.txt"))
            .map_err(|_| bad(dir, "hits.txt", "declared hits are not UTF8"))?;
        let hits_text = String::from_utf8(input("hits.txt")?)
            .map_err(|_| bad(dir, "hits.txt", "runtime hits are not UTF8"))?;
        let hits = crate::hit_buffer::HitBuffer::parse(&hits_text)
            .map_err(|reason| bad(dir, "hits.txt", reason))?;
        let declared = crate::hit_buffer::HitBuffer::parse(declared_hits)
            .map_err(|reason| bad(dir, "declared hits.txt", reason))?;
        let (addresses, rows) = (hits.addresses, hits.rows);
        if declared.rows.len() != rows.len() || declared.addresses != addresses {
            return Err(bad(
                dir,
                "hits.txt",
                "runtime hit values changed the declared attack layout",
            ));
        }

        if frame.held_reaper
            && (rows.len() != 3
                || character.mode != 1
                || !context
                    .as_ref()
                    .is_some_and(EncodedContext::is_captured_scene))
        {
            return Err(bad(
                dir,
                "frame.bin",
                "Held Reaper requires a captured Ninja context and three damage states",
            ));
        }
        if !frame.hit_packets.is_empty() && frame.hit_packets.len() != rows.len() {
            return Err(bad(
                dir,
                "frame.bin",
                "hit packet policies do not match the hit rows",
            ));
        }
        let event_program = inputs
            .file("event_plan.json")
            .map(|bytes| {
                EventProgram::compile(
                    bytes,
                    &frame,
                    &addresses,
                    &rows,
                    inputs.required("hits.txt"),
                )
                .map_err(|reason| bad(dir, "event_plan.json", &reason))
            })
            .transpose()?;
        let setup = parse_setup_bytes(dir, &input("gpu_setup.txt")?)?;
        frame
            .rebase_source(&tables, &character, &setup.attrs)
            .map_err(|reason| bad(dir, "source_rebuild", reason))?;
        let groups = |name: &str| -> Result<HashSet<u16>, LoadError> {
            std::str::from_utf8(inputs.required(name))
                .map_err(|_| bad(dir, name, "not UTF8"))?
                .split_ascii_whitespace()
                .map(|w| w.parse().map_err(|_| bad(dir, name, "not a group number")))
                .collect()
        };
        let cpu_evaluation = if context
            .as_ref()
            .is_some_and(EncodedContext::is_captured_scene)
        {
            CpuEvaluation::CapturedScene(std::sync::Mutex::new(FullMainState::Untried))
        } else {
            CpuEvaluation::RecordedScenario
        };
        let live_groups = groups("live_groups.txt")?;
        let survival_groups = groups("survival_groups.txt")?;
        Ok(Engine {
            dir: dir.to_path_buf(),
            bundle_inputs: Some(inputs),
            context,
            cpu_evaluation,
            work_owner: Arc::new(()),
            legacy_source_reference: OnceLock::new(),
            kernel: std::sync::Mutex::new(Kernel::Untried),
            formula,
            tables,
            frame,
            character,
            ops: setup.ops,
            gain: setup.gain,
            defense: setup.defense,
            hits: Hits { addresses, rows },
            event_program,
            start_attrs: setup.attrs,
            live_groups,
            survival_groups,
        })
    }

    /// Whether searches can run on the graphics card (loading the kernel if it was not tried yet).
    pub fn has_card(&self) -> bool {
        matches!(*self.kernel(), Kernel::Ready(_))
    }

    /// The loaded kernel can evaluate exact native set eligibility before ranking.
    pub fn supports_set_constraints(&self) -> bool {
        matches!(&*self.kernel(), Kernel::Ready(g) if g.supports_set_constraints())
    }

    /// The graphics-card kernel, loaded on first use. NIOH3_LOADOUT_GPU=0 turns it off.
    pub fn kernel(&self) -> std::sync::MutexGuard<'_, Kernel> {
        let mut k = self.kernel.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(*k, Kernel::Untried) {
            *k = if std::env::var("NIOH3_LOADOUT_GPU").is_ok_and(|v| v == "0") {
                Kernel::Unavailable("turned off by NIOH3_LOADOUT_GPU=0".into())
            } else if self
                .event_program
                .as_ref()
                .is_some_and(|p| !p.gpu_compatible)
            {
                Kernel::Unavailable("event sequence requires resource/state execution outside the dense native hit ABI".into())
            } else {
                let opened = if let Some(inputs) = &self.bundle_inputs {
                    crate::gpu::Gpu::open_with_inputs(
                        &self.dir,
                        &self.formula.dom_base,
                        &self.formula.dom_known,
                        self.context.as_ref(),
                        inputs,
                    )
                } else {
                    match self.context.as_ref() {
                        Some(context) => crate::gpu::Gpu::open_with_context(
                            &self.dir,
                            &self.formula.dom_base,
                            &self.formula.dom_known,
                            context,
                        ),
                        None => crate::gpu::Gpu::open(
                            &self.dir,
                            &self.formula.dom_base,
                            &self.formula.dom_known,
                        ),
                    }
                };
                match opened {
                    Ok(g) => Kernel::Ready(g),
                    Err(reason) => Kernel::Unavailable(reason),
                }
            };
        }
        k
    }

    /// The research backpack an export was built from (pool.bin and the slot lines of gpu_setup.txt), for checks.
    pub fn sample_backpack(dir: &Path) -> Result<Backpack, LoadError> {
        let ranges = parse_setup(dir)?.ranges;
        let raw = read(dir, "pool.bin")?;
        if raw.len() % ITEM != 0 {
            return Err(bad(dir, "pool.bin", "not whole item records"));
        }
        let pool: Vec<[u8; ITEM]> = raw.as_chunks::<ITEM>().0.to_vec();
        if ranges
            .iter()
            .any(|&(_, first, count)| (first + count) as usize > pool.len())
        {
            return Err(bad(dir, "gpu_setup.txt", "slot range outside pool.bin"));
        }
        let origin = (0..pool.len()).collect();
        Ok(Backpack {
            pool,
            ranges,
            origin,
        })
    }

    /// Native guardian/core additions. Does not derive ordinary maps or prove runtime parameter binding.
    pub fn source_numeric_seed(
        &self,
        source: &[u8],
    ) -> Result<crate::pipeline::source_numeric::Seed, &'static str> {
        let raw = self
            .bundle_inputs
            .as_ref()
            .and_then(|inputs| inputs.file("source_numeric.bin"))
            .ok_or("source numeric parameter input is unavailable")?;
        crate::pipeline::source_numeric::Parameters::load(raw)?.derive(
            &self.tables,
            &self.character,
            source,
        )
    }

    pub fn source_numeric_seed_inputs(
        &self,
        inputs: crate::pipeline::source_numeric::Inputs<'_>,
    ) -> Result<crate::pipeline::source_numeric::Seed, &'static str> {
        let raw = self
            .bundle_inputs
            .as_ref()
            .and_then(|inputs| inputs.file("source_numeric.bin"))
            .ok_or("source numeric parameter input is unavailable")?;
        crate::pipeline::source_numeric::Parameters::load(raw)?.derive_inputs(
            &self.tables,
            &self.character,
            inputs,
        )
    }

    pub fn tables(&self) -> &Tables {
        &self.tables
    }

    /// Exact worn record for an optimizer position in the bound character.
    pub fn worn_item(&self, position: usize) -> &[u8] {
        let offset = self.character.active * BANK + slot_of(position) as usize * ITEM;
        &self.character.inv[offset..offset + ITEM]
    }

    pub(crate) fn uses_weapon_element(&self) -> bool {
        self.frame.uses_weapon_element()
    }

    pub fn has_dynamic_sources(&self) -> bool {
        self.frame.has_source_rebuild()
    }

    /// Execution-model scope is separate from requested-domain completeness.
    /// Runtime epoch ingestion is closed, so no runtime-bound scope is emitted.
    pub fn source_model_scope(&self) -> &'static str {
        if self.has_dynamic_sources() {
            "conditional_rebuild"
        } else {
            "legacy_unverified"
        }
    }

    fn legacy_source_reference(
        &self,
    ) -> Result<&crate::pipeline::source_dependency::Signature, &'static str> {
        self.legacy_source_reference
            .get_or_init(|| {
                let mut scratch = Scratch::new();
                self.tables.pipeline(
                    &self.frame,
                    &self.character,
                    &self.start_attrs,
                    &[],
                    &mut scratch,
                )?;
                scratch
                    .legacy_source_signature
                    .ok_or("legacy source reference signature was not produced")
            })
            .as_ref()
            .map_err(|reason| *reason)
    }

    fn check_legacy_attributes(&self, attrs: &[i32; 7]) -> Result<(), &'static str> {
        if !self.has_dynamic_sources() && *attrs != self.start_attrs {
            return Err(crate::pipeline::source_dependency::ATTRIBUTES);
        }
        Ok(())
    }

    /// Every scoring/eligibility path checks the same pre-guardian state before
    /// frame writes, MAIN, constraint rejection, or a cached read certificate.
    fn check_legacy_source(&self, scratch: &Scratch) -> Result<(), &'static str> {
        if self.has_dynamic_sources() {
            return Ok(());
        }
        let reference = self.legacy_source_reference()?;
        let candidate = scratch
            .legacy_source_signature
            .as_ref()
            .ok_or("legacy candidate source signature was not produced")?;
        if candidate != reference {
            return Err(crate::pipeline::source_dependency::CHANGED);
        }
        Ok(())
    }

    /// Structural pool risk precedes all dominance, set bounds, and GPU batch
    /// dispatch. Equal one-item final aggregates would miss jointly activated
    /// set tiers; compare independent native collection inputs for every slot.
    pub(crate) fn legacy_source_pool_risk(&self, pack: &Backpack) -> bool {
        if self.has_dynamic_sources() {
            return false;
        }
        let Ok(reference) = self.legacy_source_reference() else {
            return true;
        };
        for &(slot, first, count) in &pack.ranges {
            if position_of(slot).is_none() {
                return true;
            }
            let Some(end) = first.checked_add(count) else {
                return true;
            };
            let slots: &[u32] = if matches!(slot, 9 | 10) {
                &[9, 10]
            } else {
                std::slice::from_ref(&slot)
            };
            for &placement in slots {
                for row in first..end {
                    let Some(item) = pack.pool.get(row as usize) else {
                        return true;
                    };
                    let expected =
                        &reference.items[self.character.active * 16 + placement as usize];
                    match self.tables.legacy_item_dependency(
                        &self.character,
                        &self.start_attrs,
                        self.character.active,
                        placement as usize,
                        item,
                    ) {
                        Ok(candidate) if &candidate == expected => {}
                        _ => return true,
                    }
                }
            }
        }
        false
    }

    pub(crate) fn character(&self) -> &Character {
        &self.character
    }

    /// Raise an item record to what the blacksmith can make of it: item level to the cap and its + value to the
    /// character's cap. Used to rank gear as it would be after enhancement.
    pub fn enhance(&self, item: &mut [u8; ITEM]) {
        self.character.enhanced(item);
    }

    /// Profiling only: taint of each main program when every byte the outfit writes is taken as varying.
    #[doc(hidden)]
    pub fn taint_report(
        &self,
        c: &Candidate,
        pack: &Backpack,
        w: &mut Work,
    ) -> Vec<(usize, usize, usize)> {
        let _ = self.prepare(c, pack, w);
        let mut varying = vec![false; self.formula.domain_len()];
        for &(address, _) in &w.scratch.writes {
            for k in 0..4 {
                if let Some(o) = self.formula.domain_offsets(address + k) {
                    varying[o] = true;
                }
            }
        }
        for &address in &self.hits.addresses {
            for k in 0..4 {
                if let Some(o) = self.formula.domain_offsets(address + k) {
                    varying[o] = false;
                }
            }
        }
        self.formula.taint(&varying)
    }

    /// The words an outfit writes before MAIN (pipeline and frame), and each hit's words, for a native MAIN run.
    #[doc(hidden)]
    #[allow(clippy::type_complexity)]
    pub fn native_words<'w>(
        &self,
        c: &Candidate,
        pack: &Backpack,
        w: &'w mut Work,
    ) -> Result<(&'w [(u64, u32)], Vec<Vec<(u64, u32)>>), &'static str> {
        self.prepare(c, pack, w)?;
        let events = self.execution_events(w)?;
        let mut hits = Vec::new();
        for event in events {
            let mut words = Vec::new();
            if self.assemble_execution(event, &[], &mut words)? && event.scored {
                hits.push(words);
            }
        }
        if self.reuses_held_templates() {
            let count = crate::sustained::reaper_steps(&w.scratch.pred, &w.scratch.d)?;
            let templates = hits;
            hits = (0..count)
                .map(|step| {
                    templates[if step == 0 {
                        0
                    } else if step % 2 == 1 {
                        1
                    } else {
                        2
                    }]
                    .clone()
                })
                .collect();
        }
        Ok((&w.scratch.writes, hits))
    }

    fn reuses_held_templates(&self) -> bool {
        self.frame.held_reaper
            && self
                .event_program
                .as_ref()
                .is_none_or(EventProgram::reuses_held_templates)
    }

    fn execution_events(&self, w: &Work) -> Result<Vec<ExecutionEvent>, &'static str> {
        if let Some(plan) = &self.event_program {
            return plan.events(&w.scratch.pred, &w.scratch.d);
        }
        // Compatibility for an older immutable bundle that already declares an
        // ordered landed hit list. New exported bundles carry explicit plans.
        Ok((0..self.hits.rows.len())
            .map(|template| ExecutionEvent {
                template,
                ordinal: template as u64,
                epoch: 0,
                phase: crate::sequence::inputs::NextHitPhase::Inactive,
                scored: true,
                ki: None,
                debit: None,
            })
            .collect())
    }

    fn assemble_execution(
        &self,
        event: ExecutionEvent,
        candidate: &[(u64, u32)],
        output: &mut Vec<(u64, u32)>,
    ) -> Result<bool, &'static str> {
        if let Some(plan) = &self.event_program {
            return plan.assemble(event, candidate, output);
        }
        output.clear();
        output.extend_from_slice(candidate);
        output.extend(
            self.hits
                .addresses
                .iter()
                .copied()
                .zip(self.hits.rows[event.template].iter().copied()),
        );
        self.frame.append_hit_packet(event.template, output);
        Ok(true)
    }

    /// Current native pipeline source groups, with their native ordering and
    /// duplicate contributions. Includes the frame's native before/after source
    /// program, when present. No legacy fixed/sample flat ops are added.
    /// This is evidence for a captured observed profile, not a source-producer
    /// proof or permission to treat aggregate subtraction as derived skills.
    pub fn equipment_groups(
        &self,
        c: &Candidate,
        pack: &Backpack,
        w: &mut Work,
    ) -> Result<[Vec<crate::context::CoreEntry>; 2], &'static str> {
        self.prepare_equipment(c, pack, &[], w)?;
        Ok(w.scratch.source.maps.each_ref().map(|entries| {
            entries
                .iter()
                .map(|e| crate::context::CoreEntry {
                    key: e.key,
                    base: e.base,
                    dup: e.dup,
                })
                .collect()
        }))
    }

    /// Immutable production event contract, for audit/UI scope diagnostics.
    /// None denotes an older bundle whose ordered hit list is the contract.
    pub fn event_plan_diagnostics(&self) -> Option<serde_json::Value> {
        self.event_program.as_ref().map(EventProgram::diagnostics)
    }

    pub fn is_held_sequence(&self) -> bool {
        self.frame.held_reaper
    }

    /// Rebuild exact set membership at these attributes. This is the native
    /// equipment stage, before non-equipment profile deltas and MAIN execution.
    pub fn equipment_set_state(
        &self,
        c: &Candidate,
        pack: &Backpack,
        w: &mut Work,
    ) -> Result<SetState, &'static str> {
        self.prepare_equipment(c, pack, &[], w)?;
        Ok(w.scratch.source.set_state())
    }

    /// Bound the saved equipment-side state before any post-set source writes.
    /// These capacities describe the fixed-attribute standalone plan, where
    /// submitted axes replace their positions and unspecified axes retain them.
    pub(crate) fn exact_set_capacities(
        &self,
        pack: &Backpack,
        keys: &[u32],
        swaps: &[(u32, usize)],
    ) -> ExactSetCapacities {
        let c = &self.character;
        let bank = usize::from(c.mode == 1);
        let selected_bank_is_searched = bank == c.active;
        let equipped = |bank: usize, slot: usize| &c.inv[bank * BANK + slot * ITEM..][..ITEM];
        let add_record = |capacity: &mut SetCapacity, record: &[u8], requires_attributes| {
            self.tables.add_set_capacity(
                capacity,
                keys,
                c,
                &self.start_attrs,
                record,
                requires_attributes,
            );
        };
        let accessories = pack
            .ranges
            .iter()
            .filter(|&&(slot, _, _)| matches!(slot, 9 | 10))
            .map(|&(_, _, count)| u64::from(count))
            .sum::<u64>();
        let variable_spares: Vec<_> = (1..=3)
            .filter(|&slot| swaps.iter().any(|&(_, spare)| spare == slot))
            .collect();
        let mut retained = SetCapacity::empty(keys);
        retained.reduction = self.tables.set_reduction_before_equipment(&self.frame);
        // prepared() and body() submit each equipment record to its physical
        // bank. slot15() submits the active record to the mode's bank.
        for slot in 0..=13 {
            let replaced = match slot {
                1..=3 => variable_spares.contains(&slot),
                9 | 10 => accessories >= 2,
                _ => pack
                    .ranges
                    .iter()
                    .any(|&(axis, _, _)| axis as usize == slot),
            };
            if !(selected_bank_is_searched && replaced) {
                add_record(&mut retained, equipped(bank, slot), true);
            }
        }
        add_record(&mut retained, equipped(c.active, 15), false);

        let mut rows = vec![SetCapacity::empty(keys); pack.pool.len()];
        if selected_bank_is_searched {
            for &(slot, first, count) in &pack.ranges {
                for row in first..first + count {
                    let capacity = &mut rows[row as usize];
                    add_record(capacity, &pack.pool[row as usize], true);
                    if slot == 0 {
                        for &spare in &variable_spares {
                            let from = if swaps.contains(&(row, spare)) {
                                0
                            } else {
                                spare
                            };
                            add_record(capacity, equipped(bank, from), true);
                        }
                    }
                }
            }
        }
        ExactSetCapacities { retained, rows }
    }

    /// Membership of the context equipment retained by a partial-slot search.
    /// Replaced positions are cleared before reconstruction; subtracting their
    /// aggregate profiles would not preserve qualification or fixed-set rules.
    pub fn retained_set_state(
        &self,
        pack: &Backpack,
        attrs: &[i32; 7],
        w: &mut Work,
    ) -> Result<SetState, &'static str> {
        let candidate = Candidate {
            attrs: *attrs,
            items: [None; SLOTS],
        };
        self.wear(&candidate, pack, w);
        // The planner treats accessories as one pair, with both orientations.
        // With fewer than two alternatives it retains both existing slots.
        let replace_accessories = pack
            .ranges
            .iter()
            .filter(|r| matches!(r.0, 9 | 10))
            .map(|r| r.2 as usize)
            .sum::<usize>()
            >= 2;
        for &(slot, _, _) in &pack.ranges {
            if position_of(slot).is_none() {
                return Err("slot outside the searched positions");
            }
            if !matches!(slot, 9 | 10) {
                let at = w.character.active * BANK + slot as usize * ITEM;
                w.character.inv[at..at + ITEM].fill(0);
            }
        }
        if replace_accessories {
            for slot in [9, 10] {
                let at = w.character.active * BANK + slot * ITEM;
                w.character.inv[at..at + ITEM].fill(0);
            }
        }
        self.tables
            .pipeline(&self.frame, &w.character, attrs, &[], &mut w.scratch)?;
        Ok(w.scratch.source.set_state())
    }

    pub fn is_single_fixed_contact(&self) -> bool {
        self.hit_count() == 1
            && !self.reuses_held_templates()
            && self
                .event_program
                .as_ref()
                .is_none_or(EventProgram::is_single_fixed_contact)
    }

    pub fn supports_prepared_device(&self) -> bool {
        (self.has_dynamic_sources() || self.dir.join("main_device.dll").is_file())
            && matches!(&self.cpu_evaluation, CpuEvaluation::CapturedScene(_))
            && !self.frame.held_reaper
            && self
                .event_program
                .as_ref()
                .is_none_or(EventProgram::all_fixed_events_scored)
    }

    pub(crate) fn open_prepared_device(&self) -> Result<crate::gpu::MainDevice, String> {
        let context = self
            .context
            .as_ref()
            .ok_or("prepared CUDA MAIN scene missing")?;
        crate::gpu::MainDevice::open(&self.dir, context)
    }

    pub(crate) fn open_prepared_cpu(&self) -> Result<crate::gpu::MainCpu, String> {
        let context = self
            .context
            .as_ref()
            .ok_or("prepared CPU MAIN scene missing")?;
        crate::gpu::MainCpu::open(&self.dir, context)
    }

    /// Unfused reference entry retained for the explicit host acceptance audit.
    /// Production search uses PreparedCandidate::device_contacts instead.
    pub(crate) fn prepare_device_contacts(
        &self,
        candidate: &Candidate,
        pack: &Backpack,
        limits: &Limits,
        work: &mut Work,
    ) -> Result<PreparedContacts, Outcome> {
        self.prepare_eligible(candidate, pack, limits, work)?;
        self.prepared_device_contacts(work)
    }

    /// Share the exact candidate and packet builder with the CPU route. A
    /// device lane represents one executable scored contact of this objective.
    fn prepared_device_contacts(&self, work: &mut Work) -> Result<PreparedContacts, Outcome> {
        let events = self.execution_events(work).map_err(Outcome::Refused)?;
        let mut contacts = Vec::with_capacity(events.len());
        for event in events {
            if !event.scored {
                return Err(Outcome::Refused("prepared CUDA requires scored contacts"));
            }
            if self
                .assemble_execution(event, &work.scratch.writes, &mut work.writes)
                .map_err(Outcome::Refused)?
            {
                contacts.push(work.writes.clone());
            }
        }
        Ok(contacts)
    }

    pub fn hit_count(&self) -> usize {
        self.hits.rows.len()
    }

    pub fn work(&self) -> Work {
        Work {
            owner: Arc::clone(&self.work_owner),
            scratch: Scratch::new(),
            worker: self.formula.worker(),
            character: self.character.clone(),
            writes: Vec::with_capacity(0x900),
            weapon_swaps: Vec::new(),
            preparation_stats: PreparationStats::default(),
        }
    }

    pub(crate) fn work_with_weapon_swaps(
        &self,
        pack: &Backpack,
        swaps: &[(u32, usize)],
    ) -> Result<Work, &'static str> {
        let mut work = self.work();
        if !swaps.is_empty()
            && (!self.has_dynamic_sources()
                || self
                    .context
                    .as_ref()
                    .and_then(EncodedContext::scene_metadata)
                    .map(|m| m.0)
                    != Some(0))
        {
            return Err("weapon swaps require a slot-zero dynamic-source context");
        }
        let mut unique = HashSet::new();
        let bank = self.character.active * BANK;
        for &(row, slot) in swaps {
            let in_weapon_axis = pack
                .ranges
                .iter()
                .any(|&(slot, first, count)| slot == 0 && row >= first && row - first < count);
            if !in_weapon_axis || !(1..=3).contains(&slot) || !unique.insert(row) {
                return Err("weapon swap does not identify one owned spare");
            }
            let at = bank + slot * ITEM;
            if pack.pool.get(row as usize).map(|item| item.as_slice())
                != Some(&self.character.inv[at..at + ITEM])
            {
                return Err("weapon swap record differs from the captured spare");
            }
        }
        work.weapon_swaps.extend_from_slice(swaps);
        Ok(work)
    }

    fn wear(&self, c: &Candidate, pack: &Backpack, w: &mut Work) {
        // Public scratch values may be reused across contexts. Rebind all
        // character scalars/core maps and the formula worker together once;
        // the ordinary same-engine hot path retains its existing allocations.
        if !Arc::ptr_eq(&w.owner, &self.work_owner) {
            w.character = self.character.clone();
            w.worker = self.formula.worker();
            w.owner = Arc::clone(&self.work_owner);
            w.weapon_swaps.clear();
        }
        w.character.inv.copy_from_slice(&self.character.inv);
        if let Some(row) = c.items[position_of(0).expect("the held weapon is a search axis")] {
            if let Some((_, slot)) = w.weapon_swaps.iter().find(|&&(owned, _)| owned == row) {
                let from = self.character.active * BANK;
                let spare = from + slot * ITEM;
                w.character.inv.copy_within(from..from + ITEM, spare);
            }
        }
        for (position, row) in c.items.iter().enumerate() {
            if let Some(row) = row {
                let at = self.character.active * BANK + slot_of(position) as usize * ITEM;
                w.character.inv[at..at + ITEM].copy_from_slice(&pack.pool[*row as usize]);
            }
        }
    }

    fn prepare_equipment(
        &self,
        c: &Candidate,
        pack: &Backpack,
        ops: &[(u16, i64)],
        w: &mut Work,
    ) -> Result<(), &'static str> {
        self.check_legacy_attributes(&c.attrs)?;
        self.wear(c, pack, w);
        w.preparation_stats.pipeline_runs += 1;
        self.tables
            .pipeline(&self.frame, &w.character, &c.attrs, ops, &mut w.scratch)?;
        self.check_legacy_source(&w.scratch)
    }

    fn prepare(&self, c: &Candidate, pack: &Backpack, w: &mut Work) -> Result<(), &'static str> {
        self.prepare_equipment(c, pack, &self.ops, w)?;
        self.prepare_frame(c, w)
    }

    fn prepare_frame(&self, c: &Candidate, w: &mut Work) -> Result<(), &'static str> {
        w.preparation_stats.frame_preparations += 1;
        self.tables.frame_writes(
            &self.frame,
            &w.character,
            &c.attrs,
            self.gain,
            self.defense,
            &mut w.scratch,
        )?;
        if let Some((slot, baseline)) = self
            .context
            .as_ref()
            .and_then(EncodedContext::scene_metadata)
        {
            // Native admitted direct-actor writer: active bank/F68 selects
            // the weapon; record +18 bit20 supplies MAIN+29A. The constructor
            // checked this scene's normalized slot against that selector.
            let at = w.character.active * BANK + slot as usize * ITEM;
            let record = &w.character.inv[at..at + ITEM];
            let occupied = u16::from_le_bytes([record[0], record[1]]) != 0;
            let hell = u32::from(occupied && record[0x1A] & 0x10 != 0);
            let flags = (baseline & !0x00FF_0000) | (hell << 16);
            w.scratch
                .writes
                .push((self.frame.main_address() + 0x298, flags));
        }
        Ok(())
    }

    /// Type identity of a native item,also used by live context admission.
    /// The currently bound attack inputs;context overrides may change values,never layout.
    pub fn hit_inputs(&self) -> (&[u64], &[Vec<u32>]) {
        (&self.hits.addresses, &self.hits.rows)
    }

    pub fn item_type_class(&self, key: u16) -> Option<u32> {
        self.tables.item_type_class(key)
    }

    pub fn prepared_stats(
        &self,
        c: &Candidate,
        pack: &Backpack,
        w: &mut Work,
    ) -> Result<PreparedStats, &'static str> {
        self.prepare(c, pack, w)?;
        Ok(PreparedStats {
            primary_attack: crate::pipeline::u32_at(&w.scratch.pred, 0x24),
            secondary_attack: crate::pipeline::u32_at(&w.scratch.pred, 0x28),
            defense: crate::pipeline::u32_at(&w.scratch.pred, 0x1C8),
            health: crate::pipeline::u32_at(&w.scratch.pred, 0x18),
        })
    }

    pub fn facts(&self, c: &Candidate, pack: &Backpack, w: &mut Work) -> Option<Facts> {
        self.prepare(c, pack, w).ok()?;
        Some(self.read_facts(c, w))
    }

    fn read_facts(&self, c: &Candidate, w: &Work) -> Facts {
        let s = &w.scratch;
        Facts {
            load: self.tables.load_of(&w.character, &c.attrs, &s.stat),
            defense: crate::pipeline::u32_at(&s.pred, 0x1C8),
            health: crate::pipeline::u32_at(&s.pred, 0x18),
        }
    }

    /// Profiling only: nanoseconds spent laying the outfit over the formula's bytes and running its paths, the write
    /// count, and the main-path attempts that failed.
    #[doc(hidden)]
    pub fn formula_parts(
        &self,
        c: &Candidate,
        pack: &Backpack,
        w: &mut Work,
    ) -> (u128, u128, usize, u64) {
        if self.prepare(c, pack, w).is_err() {
            return (0, 0, 0, 0);
        }
        let mut apply = 0;
        let mut run = 0;
        let left = w.worker.left;
        let Ok(events) = self.execution_events(w) else {
            return (0, 0, 0, 0);
        };
        for event in events {
            let t = std::time::Instant::now();
            if !matches!(
                self.assemble_execution(event, &w.scratch.writes, &mut w.writes),
                Ok(true)
            ) {
                continue;
            }
            self.formula.apply(&mut w.worker, &w.writes);
            apply += t.elapsed().as_nanos();
            let t = std::time::Instant::now();
            let _ = self.formula.run(&mut w.worker);
            run += t.elapsed().as_nanos();
        }
        (apply, run, w.scratch.writes.len(), w.worker.left - left)
    }

    fn open_captured(&self, state: &mut FullMainState) {
        if matches!(state, FullMainState::Untried) {
            let opened = self
                .context
                .as_ref()
                .ok_or_else(|| "captured scene input missing".to_string())
                .and_then(|context| crate::gpu::MainCpu::open(&self.dir, context));
            *state = match opened {
                Ok(runner) => FullMainState::Ready {
                    runner,
                    last_failure: None,
                },
                Err(reason) => FullMainState::Unavailable(reason),
            };
        }
    }

    pub(crate) fn supports_read_bounds(&self) -> bool {
        if self
            .event_program
            .as_ref()
            .is_some_and(EventProgram::requires_occurrence_execution)
        {
            return false;
        }
        // Held witnesses also compare the freshly recomputed depletion count.
        // Within this immutable three-template objective, equal MAIN traces and
        // equal count imply the same ordered binary32 sum (no rounding bound).
        let CpuEvaluation::CapturedScene(owner) = &self.cpu_evaluation else {
            return false;
        };
        let Ok(mut state) = owner.lock() else {
            return false;
        };
        self.open_captured(&mut state);
        matches!(&*state, FullMainState::Ready { runner, .. } if runner.has_read_trace())
    }

    fn evaluate_captured(
        &self,
        owner: &std::sync::Mutex<FullMainState>,
        w: &mut Work,
        mut trace: Option<&mut InputTrace>,
    ) -> Outcome {
        let Ok(mut state) = owner.lock() else {
            return Outcome::Refused("full-MAIN CPU owner lock is poisoned");
        };
        self.open_captured(&mut state);
        let FullMainState::Ready {
            runner,
            last_failure,
        } = &mut *state
        else {
            return Outcome::Refused("full-MAIN CPU is unavailable; inspect cpu_diagnostics");
        };
        *last_failure = None;
        let held_steps = if self.reuses_held_templates() {
            match crate::sustained::reaper_steps(&w.scratch.pred, &w.scratch.d) {
                Ok(steps) => Some(steps),
                Err(reason) => return Outcome::Refused(reason),
            }
        } else {
            None
        };
        // An unused template is not part of the objective and must not turn a
        // one-contact or empty sequence into NoPath. The GPU sum uses this same
        // contact prefix when deciding which template bails matter.
        let active_templates = held_steps.map_or(self.hits.rows.len(), |n| n.min(3) as usize);
        let mut total = 0.0f32;
        let mut held_hits = [0u32; 3];
        let events = match self.execution_events(w) {
            Ok(events) => events,
            Err(reason) => return Outcome::Refused(reason),
        };
        for event in events.into_iter().take(if self.reuses_held_templates() {
            active_templates
        } else {
            usize::MAX
        }) {
            let hit = event.template;
            match self.assemble_execution(event, &w.scratch.writes, &mut w.writes) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(reason) => return Outcome::Refused(reason),
            }
            let result = runner.run_with_reads(
                &w.writes,
                trace.as_deref_mut().map(|log| log.buffer.as_mut()),
            );
            match result {
                Ok((bits, read_count)) => {
                    if let Some(log) = trace.as_deref_mut() {
                        if let Some(count) = read_count {
                            log.addresses.extend_from_slice(&log.buffer[..count]);
                        } else {
                            // An incomplete hit cannot certify the move sum.
                            log.addresses.clear();
                            trace = None;
                        }
                    }
                    if self.reuses_held_templates() {
                        held_hits[hit] = bits;
                    }
                    if event.scored {
                        total += f32::from_bits(bits);
                    }
                    if !f32::from_bits(bits).is_finite() || !total.is_finite() {
                        *last_failure = Some(crate::gpu::MainCpuFailure {
                            message: "full MAIN produced non-finite damage".into(),
                            bail: 0,
                            bail_rva: 0,
                        });
                        return Outcome::Refused("full MAIN produced non-finite damage");
                    }
                }
                Err(error) => {
                    let bailed = error.bail != 0;
                    *last_failure = Some(error);
                    return if bailed {
                        Outcome::NoPath
                    } else {
                        Outcome::Refused(
                            "full-MAIN CPU input/evaluation failed; inspect cpu_diagnostics",
                        )
                    };
                }
            }
        }
        if let Some(steps) = held_steps {
            return match crate::sustained::sum_reaper_checked(held_hits, steps) {
                Ok(bits) => Outcome::Damage(bits),
                Err(reason) => Outcome::Refused(reason),
            };
        }
        Outcome::Damage(total.to_bits())
    }

    fn prepare_eligible(
        &self,
        c: &Candidate,
        pack: &Backpack,
        limits: &Limits,
        w: &mut Work,
    ) -> Result<(), Outcome> {
        self.prepare(c, pack, w).map_err(Outcome::Refused)?;
        self.check_limits(c, limits, w)
    }

    fn check_limits(&self, c: &Candidate, limits: &Limits, w: &Work) -> Result<(), Outcome> {
        let facts = self.read_facts(c, w);
        if facts.load > limits.max_load
            || facts.defense < limits.min_defense
            || facts.health < limits.min_health
        {
            return Err(Outcome::OutsideLimits);
        }
        Ok(())
    }

    /// Preserve native admission order while sharing the completed equipment
    /// stage with scoring. Dynamic SOURCE prohibits profile operations, so the
    /// actual captured CPU/CUDA route runs the equipment pipeline once.
    ///
    /// Older recorded bundles can have profile operations. Their set predicate
    /// must still see the empty-ops pipeline first, and a failed set predicate
    /// must precede any ops/frame failure. Keep the old second pipeline in that
    /// compatibility case instead of silently changing failure classification.
    pub(crate) fn prepare_for_search<'a>(
        &'a self,
        c: &Candidate,
        pack: &Backpack,
        limits: &Limits,
        w: &'a mut Work,
        set_admission: Option<impl FnOnce(&SetState) -> bool>,
    ) -> Result<PreparedCandidate<'a>, Outcome> {
        if let Some(admit) = set_admission {
            self.prepare_equipment(c, pack, &[], w)
                .map_err(Outcome::Refused)?;
            w.preparation_stats.set_checks += 1;
            if !admit(&w.scratch.source.set_state()) {
                return Err(Outcome::OutsideLimits);
            }
            if self.ops.is_empty() {
                w.preparation_stats.shared_pipeline_uses += 1;
                self.prepare_frame(c, w).map_err(Outcome::Refused)?;
            } else {
                self.prepare(c, pack, w).map_err(Outcome::Refused)?;
            }
        } else {
            self.prepare(c, pack, w).map_err(Outcome::Refused)?;
        }
        self.check_limits(c, limits, w)?;
        Ok(PreparedCandidate {
            engine: self,
            work: w,
        })
    }

    pub fn evaluate(
        &self,
        c: &Candidate,
        pack: &Backpack,
        limits: &Limits,
        w: &mut Work,
    ) -> Outcome {
        match self.prepare_eligible(c, pack, limits, w) {
            Ok(()) => self.evaluate_prepared(w, None),
            Err(outcome) => outcome,
        }
    }

    /// Re-evaluate the shown candidate through the same prepared event inputs,
    /// retaining native component outputs. Search still ranks combined damage.
    pub fn damage_details(
        &self,
        c: &Candidate,
        pack: &Backpack,
        w: &mut Work,
    ) -> crate::damage::DamageDetails {
        use crate::damage::{DamageAccumulator, DamageDetails, DetailUnavailable};
        let mut evaluate = || -> Result<DamageDetails, DetailUnavailable> {
            self.prepare_eligible(c, pack, &Limits::NONE, w)
                .map_err(|_| DetailUnavailable::EvaluationFailed)?;
            let CpuEvaluation::CapturedScene(owner) = &self.cpu_evaluation else {
                return Err(DetailUnavailable::UnsupportedRecordedScenario);
            };
            let mut state = owner
                .lock()
                .map_err(|_| DetailUnavailable::EvaluationFailed)?;
            self.open_captured(&mut state);
            let FullMainState::Ready {
                runner,
                last_failure,
            } = &mut *state
            else {
                return Err(DetailUnavailable::EvaluationFailed);
            };
            if !runner.has_components() {
                return Err(DetailUnavailable::MissingComponentCapability);
            }
            *last_failure = None;
            let steps = if self.reuses_held_templates() {
                Some(
                    crate::sustained::reaper_steps(&w.scratch.pred, &w.scratch.d)
                        .map_err(|_| DetailUnavailable::EvaluationFailed)?,
                )
            } else {
                None
            };
            let events = self
                .execution_events(w)
                .map_err(|_| DetailUnavailable::EvaluationFailed)?;
            let active = steps.map_or(usize::MAX, |n| n.min(3) as usize);
            let mut accumulator = DamageAccumulator::new();
            let mut held = [crate::damage::DamageWords::default(); 3];
            for event in events.into_iter().take(active) {
                if !self
                    .assemble_execution(event, &w.scratch.writes, &mut w.writes)
                    .map_err(|_| DetailUnavailable::EvaluationFailed)?
                {
                    continue;
                }
                let words = runner.run_components(&w.writes).map_err(|error| {
                    *last_failure = Some(error);
                    DetailUnavailable::EvaluationFailed
                })?;
                if steps.is_some() {
                    held[event.template] = words;
                } else if event.scored {
                    accumulator.push(event.template, words)?;
                }
            }
            if let Some(steps) = steps {
                for step in 0..steps {
                    let template = if step == 0 {
                        0
                    } else if step % 2 == 1 {
                        1
                    } else {
                        2
                    };
                    accumulator.push(template, held[template])?;
                }
            }
            Ok(accumulator.finish())
        };
        match evaluate() {
            Ok(details) => details,
            Err(reason) => DamageDetails::unavailable(reason),
        }
    }

    #[cfg(test)]
    pub(crate) fn evaluate_with_bound(
        &self,
        c: &Candidate,
        pack: &Backpack,
        limits: &Limits,
        w: &mut Work,
        bounds: &mut ReadBounds,
    ) -> SweepOutcome {
        if let Err(outcome) = self.prepare_eligible(c, pack, limits, w) {
            return SweepOutcome::Executed(outcome);
        }
        self.evaluate_prepared_with_bound(w, bounds)
    }

    fn evaluate_prepared_with_bound(&self, w: &mut Work, bounds: &mut ReadBounds) -> SweepOutcome {
        if self
            .event_program
            .as_ref()
            .is_some_and(EventProgram::requires_occurrence_execution)
        {
            return SweepOutcome::Executed(self.evaluate_prepared(w, None));
        }
        bounds.duration = if self.frame.held_reaper {
            match crate::sustained::reaper_steps(&w.scratch.pred, &w.scratch.d) {
                Ok(steps) => Some(steps),
                Err(reason) => return SweepOutcome::Executed(Outcome::Refused(reason)),
            }
        } else {
            None
        };
        bounds.prepare_overlay(&w.scratch.writes);
        if let Some(bits) = bounds.certified_score(&w.scratch.writes) {
            return SweepOutcome::Certified(bits);
        }
        bounds.trace.addresses.clear();
        let outcome = self.evaluate_prepared(w, Some(&mut bounds.trace));
        if let Outcome::Damage(bits) = outcome {
            bounds.remember(bits, &w.scratch.writes);
        }
        SweepOutcome::Executed(outcome)
    }

    fn evaluate_prepared(&self, w: &mut Work, trace: Option<&mut InputTrace>) -> Outcome {
        if let CpuEvaluation::CapturedScene(owner) = &self.cpu_evaluation {
            return self.evaluate_captured(owner, w, trace);
        }
        let held_steps = if self.reuses_held_templates() {
            match crate::sustained::reaper_steps(&w.scratch.pred, &w.scratch.d) {
                Ok(count) => Some(count),
                Err(reason) => return Outcome::Refused(reason),
            }
        } else {
            None
        };
        let mut total = 0.0f32;
        let mut held_hits = [0u32; 3];
        let events = match self.execution_events(w) {
            Ok(events) => events,
            Err(reason) => return Outcome::Refused(reason),
        };
        let active_templates = held_steps.map_or(usize::MAX, |n| n.min(3) as usize);
        for event in events.into_iter().take(active_templates) {
            match self.assemble_execution(event, &w.scratch.writes, &mut w.writes) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(reason) => return Outcome::Refused(reason),
            }
            self.formula.apply(&mut w.worker, &w.writes);
            match self.formula.run(&mut w.worker) {
                Some(bits) => {
                    if held_steps.is_some() {
                        held_hits[event.template] = bits;
                    }
                    if event.scored {
                        total += f32::from_bits(bits);
                    }
                    if !f32::from_bits(bits).is_finite() || !total.is_finite() {
                        return Outcome::Refused("recorded MAIN produced non-finite damage");
                    }
                }
                None => return Outcome::NoPath,
            }
        }
        if let Some(count) = held_steps {
            match crate::sustained::sum_reaper_checked(held_hits, count) {
                Ok(bits) => Outcome::Damage(bits),
                Err(reason) => Outcome::Refused(reason),
            }
        } else {
            Outcome::Damage(total.to_bits())
        }
    }
}

#[cfg(test)]
impl Engine {
    /// Actual production equipment pipeline with a deliberately constant score
    /// oracle for search-contract tests. This is never native damage evidence.
    pub(crate) fn audit_fixture() -> Self {
        let (tables, mut frame, character) = crate::pipeline::audit_fixture();
        frame.held_reaper = false;
        Self {
            dir: PathBuf::new(),
            bundle_inputs: None,
            context: None,
            cpu_evaluation: CpuEvaluation::RecordedScenario,
            work_owner: Arc::new(()),
            legacy_source_reference: OnceLock::new(),
            kernel: std::sync::Mutex::new(Kernel::Unavailable("synthetic audit fixture".into())),
            formula: Formula::audit_constant(1.0f32.to_bits()),
            tables,
            frame,
            character,
            ops: Vec::new(),
            gain: 0,
            defense: 0,
            hits: Hits {
                addresses: vec![0x800000],
                rows: vec![vec![0]],
            },
            event_program: None,
            start_attrs: [5, 45, 15, 200, 5, 60, 155],
            live_groups: HashSet::new(),
            survival_groups: HashSet::new(),
        }
    }

    pub(crate) fn audit_character(&mut self) -> &mut Character {
        self.legacy_source_reference.take();
        &mut self.character
    }

    /// Planning-only conditional producer fixture; it does not attest the
    /// constant-score frame's source baseline or a live input generation.
    pub(crate) fn audit_source_program(
        &mut self,
        program: crate::pipeline::source_rebuild::Program,
    ) {
        self.frame.source_rebuild = Some(program);
    }

    /// Retained raw current character and frame, with the deliberately constant
    /// score oracle above. This exercises native equipment/source admission;
    /// it is not an original MAIN damage or live generation assertion.
    pub(crate) fn audit_legacy_source_fixture() -> Self {
        let mut engine = Self::audit_fixture();
        engine.character = Frame::characters(include_bytes!(
            "../tests/fixtures/legacy_source_characters.bin"
        ))
        .unwrap()
        .remove(0);
        engine.frame = Frame::load(include_bytes!("../tests/fixtures/audit_frame.bin")).unwrap();
        engine.frame.held_reaper = false;
        engine
    }

    /// Inspect the native set producer without claiming source admission or a
    /// score. Used only to distinguish qualification laws from the explicitly
    /// unsupported legacy changed-attribute execution domain.
    pub(crate) fn audit_native_set_state(
        &self,
        candidate: &Candidate,
        pack: &Backpack,
    ) -> Result<SetState, &'static str> {
        let mut work = self.work();
        self.wear(candidate, pack, &mut work);
        self.tables.pipeline(
            &self.frame,
            &work.character,
            &candidate.attrs,
            &[],
            &mut work.scratch,
        )?;
        Ok(work.scratch.source.set_state())
    }
}

#[cfg(test)]
mod objective_cache_tests {
    use super::*;

    #[test]
    fn main_reads_without_repeat_count_cannot_certify_held_damage() {
        let writes = [(0x1000, 10)];
        let mut cache = ReadBounds::new();
        cache.duration = Some(3);
        cache.prepare_overlay(&writes);
        cache.trace.addresses = vec![0x1000, 0x1001, 0x1002, 0x1003];
        cache.remember(30.0f32.to_bits(), &writes);
        cache.duration = Some(4);
        assert_eq!(
            cache.certified_score(&writes),
            None,
            "same contact inputs, different duration"
        );
        cache.duration = Some(3);
        assert_eq!(cache.certified_score(&writes), Some(30.0f32.to_bits()));
        cache.prepare_overlay(&[(0x1000, 11)]);
        assert_eq!(cache.certified_score(&writes), None, "a read byte changed");
    }

    #[test]
    fn exact_witness_reuse_preserves_optimum_and_ties_across_resource_changes() {
        let mut cache = ReadBounds::new();
        let mut answers = Vec::new();
        let mut reference = Vec::new();
        let mut reused = 0;
        for irrelevant in 0..7u32 {
            for duration in 1..9u32 {
                for damage in 1..6u32 {
                    let writes = [(0x1000, damage), (0x2000, irrelevant)];
                    cache.duration = Some(duration);
                    cache.prepare_overlay(&writes);
                    let expected =
                        crate::sustained::sum_reaper([(damage as f32).to_bits(); 3], duration);
                    let bits = match cache.certified_score(&writes) {
                        Some(bits) => {
                            reused += 1;
                            bits
                        }
                        None => {
                            cache.trace.addresses = (0x1000..0x1004).collect();
                            cache.remember(expected, &writes);
                            expected
                        }
                    };
                    answers.push(bits);
                    reference.push(expected);
                }
            }
        }
        assert!(reused > 0);
        assert_eq!(answers, reference);
        assert_eq!(answers.iter().max(), Some(&40.0f32.to_bits()));
    }
}

#[cfg(test)]
mod packet_isolation_tests {
    use super::*;
    use crate::pipeline::HitPacket;

    #[test]
    fn a_projectile_packet_does_not_contaminate_the_next_prepared_contact() {
        let mut engine = Engine::audit_fixture();
        let address = engine.frame.main_address() + 0x18;
        engine.formula = Formula::audit_word(address, 1.0f32.to_bits());
        engine.hits.rows = vec![vec![0]; 3];
        engine.frame.hit_packets = vec![
            HitPacket::Prepared,
            HitPacket::UnimbuedOwnerProjectile {
                power: 2.0f32.to_bits(),
                main_flags: 0,
            },
            HitPacket::Prepared,
        ];
        let mut work = engine.work();
        work.scratch.writes = vec![(address, 1.0f32.to_bits())];
        assert_eq!(
            engine.evaluate_prepared(&mut work, None),
            Outcome::Damage(4.0f32.to_bits())
        );

        // Deliberate prior behavior: applying the candidate only once leaves
        // the middle projectile's power in the final prepared contact.
        engine.formula.apply(&mut work.worker, &work.scratch.writes);
        let mut contaminated = 0.0f32;
        for hit in 0..3 {
            let mut words = Vec::new();
            engine.frame.append_hit_packet(hit, &mut words);
            engine.formula.overlay(&mut work.worker, &words);
            contaminated += f32::from_bits(engine.formula.run(&mut work.worker).unwrap());
        }
        assert_eq!(contaminated, 5.0);
    }

    #[test]
    fn recorded_contacts_reject_an_overflowing_ordered_sum() {
        let mut engine = Engine::audit_fixture();
        engine.formula = Formula::audit_constant(f32::MAX.to_bits());
        engine.hits.rows = vec![vec![0]; 2];
        assert_eq!(
            engine.evaluate_prepared(&mut engine.work(), None),
            Outcome::Refused("recorded MAIN produced non-finite damage")
        );
    }

    #[test]
    fn profiling_rebuilds_the_final_prepared_packet_after_a_projectile() {
        let mut engine = Engine::audit_fixture();
        let address = engine.frame.main_address() + 0x18;
        engine.formula = Formula::audit_word(address, 1.0f32.to_bits());
        engine.hits.rows = vec![vec![0]; 3];
        engine.frame.hit_packets = vec![
            HitPacket::Prepared,
            HitPacket::UnimbuedOwnerProjectile {
                power: 2.0f32.to_bits(),
                main_flags: 0,
            },
            HitPacket::Prepared,
        ];
        let candidate = Candidate {
            attrs: engine.start_attrs,
            items: [None; SLOTS],
        };
        let pack = Backpack::from_slots(&[]).unwrap();
        let mut work = engine.work();
        engine.prepare(&candidate, &pack, &mut work).unwrap();
        engine.formula.apply(&mut work.worker, &work.scratch.writes);
        let prepared = engine.formula.run(&mut work.worker).unwrap();
        assert_ne!(prepared, 2.0f32.to_bits());
        let (_, _, writes, failures) = engine.formula_parts(&candidate, &pack, &mut work);
        assert!(writes > 0);
        assert_eq!(failures, 0);
        assert_eq!(engine.formula.run(&mut work.worker), Some(prepared));
    }
}

#[cfg(test)]
mod bundle_ownership_tests {
    use super::*;

    #[test]
    fn actual_engine_open_keeps_cpu_and_card_named_bytes_in_one_session() {
        let base = std::env::var_os("NIOH3_BUILD_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let dir = base.join(format!(
            "engine-input-e2e-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let write = |name: &str, bytes: &[u8]| fs::write(dir.join(name), bytes).unwrap();
        write(
            "tables.bin",
            include_bytes!("../tests/fixtures/audit_tables.bin"),
        );
        write("frame.bin", &crate::pipeline::audit_frame_bytes());
        write(
            "programs.txt",
            b"regions 0\ndomain 0\n\n\ntables 0\nmains 1\nprogram 0 0 1 0\n3f800000\n0\n\n",
        );
        for name in [
            "arena.bin",
            "mapped.bin",
            "live_groups.txt",
            "survival_groups.txt",
        ] {
            write(name, &[]);
        }
        write("gpu_setup.txt", b"0 0 0\n0\n5 45 15 200 5 60 155\n");
        let mut character: Vec<u8> = [1u32, 0, 180, 20, 1, 0, 100, 0, 0, 0]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        character.resize(40 + 0x2018, 0);
        let mut characters = 5u32.to_le_bytes().to_vec();
        characters.extend_from_slice(b"char0");
        characters.extend_from_slice(&(character.len() as u32).to_le_bytes());
        characters.extend_from_slice(&character);
        write("characters.bin", &characters);
        write("hits.txt", b"1\n800000\n0\n");
        let old = Engine::open(&dir).unwrap();
        let candidate = Candidate {
            attrs: old.start_attrs,
            items: [None; SLOTS],
        };
        let pack = Backpack::from_slots(&[]).unwrap();
        assert_eq!(
            old.evaluate(&candidate, &pack, &Limits::NONE, &mut old.work()),
            Outcome::Damage(1.0f32.to_bits())
        );
        write("hits.txt", b"2\n800000\n0\n0\n");
        assert_eq!(
            old.bundle_inputs.as_ref().unwrap().required("hits.txt"),
            b"1\n800000\n0\n"
        );
        assert_eq!(
            old.evaluate(&candidate, &pack, &Limits::NONE, &mut old.work()),
            Outcome::Damage(1.0f32.to_bits())
        );
        let new = Engine::open(&dir).unwrap();
        assert_ne!(old.bundle_input_digest(), new.bundle_input_digest());
        assert_eq!(
            new.evaluate(&candidate, &pack, &Limits::NONE, &mut new.work()),
            Outcome::Damage(2.0f32.to_bits())
        );
        write("hits.txt", b"1\n800000\n0\ntrailing\n");
        assert!(Engine::open(&dir).is_err());
        write("hits.txt", b"1\n800000\n0\n");
        let frame = crate::pipeline::audit_frame_bytes();
        let inventory = vec![0; crate::context::INVENTORY_BYTES];
        let context = LoadoutContext::new(
            crate::context::ContextManifest {
                schema: crate::context::CONTEXT_SCHEMA.into(),
                provenance: crate::context::Provenance {
                    capture_id: "synthetic-bundle-ownership-e2e".into(),
                    source_sha256: crate::context::sha256(b"synthetic source"),
                    game_version: [2, 0, 2, 0],
                    description: "Synthetic recorded formula; no game acceptance".into(),
                },
                scenario: crate::context::Scenario::RecordedScenario,
                attrs: old.start_attrs,
                character: crate::context::CharacterInputs {
                    mode: 1,
                    active: 0,
                    cap: 180,
                    plus_cap: 20,
                    style: 1,
                    unarmed: 0,
                    level: 100,
                    core_mode: 0,
                    core_maps: [vec![], vec![]],
                },
                non_gear: crate::context::NonGearSources {
                    attack_gain: 0,
                    defense: 0,
                    layers: vec![],
                },
                inventory_sha256: crate::context::sha256(&inventory),
                frame_sha256: crate::context::sha256(&frame),
                native_scalars: None,
            },
            inventory,
            frame,
        )
        .unwrap();
        let contextual = Engine::open_with_context(&dir, &context).unwrap();
        fs::remove_file(dir.join("characters.bin")).unwrap();
        let without_sample = Engine::open_with_context(&dir, &context).unwrap();
        assert_eq!(
            contextual.bundle_input_digest(),
            without_sample.bundle_input_digest()
        );
        assert!(without_sample
            .bundle_inputs
            .as_ref()
            .unwrap()
            .file("characters.bin")
            .is_none());
        assert!(without_sample
            .encoded_context()
            .unwrap()
            .file("characters.bin")
            .is_some());
        assert_eq!(
            without_sample.evaluate(&candidate, &pack, &Limits::NONE, &mut without_sample.work()),
            Outcome::Damage(1.0f32.to_bits())
        );
        assert!(
            Engine::open(&dir).is_err(),
            "legacy open still needs its sample character"
        );
        fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
#[path = "event_program_tests.rs"]
mod production_event_tests;
