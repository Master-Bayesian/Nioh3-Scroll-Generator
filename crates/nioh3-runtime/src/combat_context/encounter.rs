//! Materialize the selected, declared PC202 encounter in the translated MAIN's
//! virtual address space. Only the compiled PC202 recipe can request live
//! reads. All character and shared table values come from the current process.
use super::*;
use serde::Deserialize;
use std::sync::OnceLock;

const CHUNK: usize = 65536;
type R<T> = Result<T, RuntimeError>;

#[derive(Deserialize)]
struct Recipe {
    roots: BTreeMap<String, u64>,
    action_row_key: [u8; 32],
    form: i32,
    held_slot: u32,
    item_category: u32,
    selected_action: SelectedAction,
    #[serde(default)]
    projectiles: Vec<OwnedProjectile>,
    tables: Vec<ParameterTable>,
    fields: Vec<Field>,
}
#[derive(Deserialize)]
struct SelectedAction {
    controller_link: u64,
    first_index: u16,
    packet_row: usize,
    parameter: String,
    rows: Vec<String>,
}

// These are addresses in the scorer's private image. The game process only
// supplies the character, never the player action being simulated.
const ACTION_STATE: u64 = 0x6000_0000_0000;
const ACTION_PARAM: u64 = ACTION_STATE + 0x1000;
const ACTION_TABLE: u64 = ACTION_STATE + 0x10000;
const ACTION_ROWS: u64 = ACTION_STATE + 0x100000;
const MEMBERSHIP_GLOBAL: u64 = 79330664;
const MEMBERSHIP_OFFSET: u64 = 0x2820;

/// 78C974 tests a 16-bit key in the manager's FNV-1a bucket table. Rebuild
/// a complete equivalent lookup from the checked list and bucket membership.
fn install_membership(image: &mut Image, header: u64, keys: &BTreeMap<u32, Vec<u8>>) -> R<()> {
    let sentinel = ACTION_STATE + 0x500000;
    let buckets = sentinel + 0x1000;
    let nodes = sentinel + 0x80000;
    let count = (keys.len().max(8)).next_power_of_two();
    let mut grouped = vec![Vec::new(); count];
    for &key in keys.keys() {
        let key = u16::try_from(key).map_err(|_| layout("invalid membership key"))?;
        let hash = key
            .to_le_bytes()
            .iter()
            .fold(0xcbf29ce484222325u64, |h, b| {
                (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
            });
        grouped[hash as usize & (count - 1)].push(key);
    }
    image.pointer(header + 8, sentinel)?;
    image.pointer(header + 16, keys.len() as u64)?;
    image.pointer(header + 24, buckets)?;
    image.pointer(header + 32, buckets + 16 * count as u64)?;
    image.pointer(header + 48, (count - 1) as u64)?;
    let mut previous = sentinel;
    let mut node = nodes;
    for (i, group) in grouped.iter().enumerate() {
        let first = if group.is_empty() { sentinel } else { node };
        for key in group {
            image.pointer(previous, node)?;
            image.pointer(node + 8, previous)?;
            image.set(node + 16, &key.to_le_bytes())?;
            previous = node;
            node += 24;
        }
        image.pointer(buckets + 16 * i as u64, first)?;
        image.pointer(
            buckets + 16 * i as u64 + 8,
            if group.is_empty() { sentinel } else { previous },
        )?;
    }
    image.pointer(previous, sentinel)?;
    image.pointer(sentinel + 8, previous)
}

/// The ordinary mode-1 unimbued projectile constructor. Only the descriptor
/// words read by the verified damage path are installed. Other bytes stay
/// unknown, including any unobserved constructor output.
#[derive(Deserialize)]
struct OwnedProjectile {
    object: u64,
    row_address: u64,
    descriptor_head: [u32; 4],
    action: u16,
    selector: u8,
    damage_row: String,
}

impl OwnedProjectile {
    fn install(&self, image: &mut Image, actor: u64) -> R<()> {
        install_projectile(
            image,
            self.object,
            actor,
            self.descriptor_head,
            self.action,
            self.selector,
        )?;
        image.set(self.row_address, &static_bytes(&self.damage_row, 192)?)
    }
}

fn install_projectile(
    image: &mut Image,
    object: u64,
    actor: u64,
    descriptor_head: [u32; 4],
    action: u16,
    selector: u8,
) -> R<()> {
    let tag = descriptor_head[0].wrapping_shl(4) | 4;
    image.set(object, &tag.to_le_bytes())?;
    image.pointer(object + 0x260, actor)?;
    image.set(object + 0x590, &1.0f32.to_le_bytes())?;
    image.set(object + 0x594, &u64::from(1.0f32.to_bits()).to_le_bytes())?;
    image.set(object + 0x59c, &u32::MAX.to_le_bytes())?;
    image.set(object + 0x5a0, &1u32.to_le_bytes())?;
    for (i, word) in descriptor_head.iter().enumerate() {
        image.set(object + 0x5d0 + i as u64 * 4, &word.to_le_bytes())?;
    }
    image.set(object + 0x758, &u32::from(action).to_le_bytes())?;
    image.set(object + 0x75c, &[selector])
}

fn static_bytes(hex: &str, size: usize) -> R<Vec<u8>> {
    if hex.len() != 2 * size || !hex.is_ascii() {
        return Err(layout("invalid compiled action bytes"));
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|_| layout("invalid compiled action byte"))
        })
        .collect()
}

/// Original FC12DC / 556F7B resolve the selected skill through table 00294.
/// Only immutable rows and hash slots are retained. Pointers belong to this image.
fn install_skill_table(image: &mut Image, module: u64) -> R<()> {
    let (rows, rest) = include_bytes!("pc202_skill_lookup.bin").split_at(211400);
    let (slots, rest) = rest.split_at(17624);
    let (variants, variant_slots) = rest.split_at(3068);
    for (global, context, rows, slots) in [
        (0x4ba3e20, 0x6000_0300_0000, rows, slots),
        (0x4ba45b8, 0x6000_0310_0000, variants, variant_slots),
    ] {
        let index = context + 0x40;
        let hash = context + 0x100;
        let table = context + 0x10000;
        image.pointer(add(module, global)?, context)?;
        image.pointer(context, table)?;
        image.pointer(context + 0x20, index)?;
        image.set(index, &rows[4..8])?;
        image.set(index + 4, &0xffff_u32.to_le_bytes())?;
        image.pointer(index + 8, hash)?;
        image.pointer(index + 16, hash + slots.len() as u64)?;
        image.set(hash, slots)?;
        image.set(table, rows)?;
    }
    Ok(())
}

#[path = "encounter_peak_target.rs"]
mod encounter_peak_target;
pub use encounter_peak_target::OpponentAilments;

#[derive(Clone, Copy)]
pub enum TargetCondition {
    Benchmark,
    Saturated,
    TheoreticalPeak,
    Peak { ailments: OpponentAilments, weakness: bool },
}

#[derive(Clone, Copy)]
pub enum ActorElementCondition {
    Current,
    ProjectedUncoated,
}

#[derive(Clone, Copy)]
pub struct PacketLayout {
    pub address: u64,
    pub direct_only: bool,
    pub direct_owner_element: bool,
    pub actor_damage_kind_from_row: bool,
}

pub struct ControllerInput<'a> {
    pub layout: Option<PacketLayout>,
    pub actor_skill_key: Option<u16>,
    pub reference_target: Option<&'a [u8]>,
    pub target_condition: TargetCondition,
    pub actor_element_condition: ActorElementCondition,
    pub parameter: &'a [u8; 336],
    pub first_index: u16,
    pub packet_row: PacketRow,
    pub rows: &'a [&'a [u8; 192]],
}

#[derive(Clone, Copy)]
pub enum PacketRow {
    Actor(usize),
    OwnedProjectile {
        object: u64,
        descriptor_head: [u32; 4],
        action: u16,
        selector: u8,
        global_key: i32,
        descriptor_5f8: i32,
        descriptor_5fc: u32,
        packet_flags: u32,
    },
}

impl ControllerInput<'_> {
    fn install(
        &self,
        image: &mut Image,
        link: u64,
        packet_row_address: u64,
        packet_row: &[u8; 192],
    ) -> R<()> {
        image.pointer(link, ACTION_STATE)?;
        image.pointer(ACTION_STATE + 0x20, ACTION_PARAM)?;
        image.set(ACTION_PARAM, self.parameter)?;
        image.pointer(ACTION_STATE + 0x40, ACTION_TABLE)?;
        image.set(ACTION_STATE + 0x48, &self.first_index.to_le_bytes())?;
        let count = u16::try_from(self.rows.len())
            .map_err(|_| layout("invalid static action row count"))?;
        image.set(ACTION_STATE + 0x4a, &count.to_le_bytes())?;
        let direct_index = match self.packet_row {
            PacketRow::Actor(index) => {
                if self.rows.get(index).copied() != Some(packet_row) {
                    return Err(layout("attack parameter and packet use different rows"));
                }
                Some(index)
            }
            PacketRow::OwnedProjectile { .. } => {
                image.set(packet_row_address, packet_row)?;
                None
            }
        };
        for (i, &bytes) in self.rows.iter().enumerate() {
            let address = if Some(i) == direct_index {
                packet_row_address
            } else {
                ACTION_ROWS + 192 * i as u64
            };
            image.set(address, bytes)?;
            image.pointer(
                ACTION_TABLE + 8 * (u64::from(self.first_index) + i as u64),
                address,
            )?;
        }
        Ok(())
    }
}

impl SelectedAction {
    fn install(&self, image: &mut Image, packet_row_address: u64, packet_row: &[u8; 192]) -> R<()> {
        let fixed = || layout("invalid retained controller record");
        let parameter = static_bytes(&self.parameter, 336)?
            .try_into()
            .map_err(|_| fixed())?;
        let rows = self
            .rows
            .iter()
            .map(|r| static_bytes(r, 192)?.try_into().map_err(|_| fixed()))
            .collect::<R<Vec<[u8; 192]>>>()?;
        let refs = rows.iter().collect::<Vec<_>>();
        ControllerInput {
            layout: None,
            actor_skill_key: None,
            reference_target: None,
            target_condition: TargetCondition::Benchmark,
            actor_element_condition: ActorElementCondition::Current,
            parameter: &parameter,
            first_index: self.first_index,
            packet_row: PacketRow::Actor(self.packet_row),
            rows: &refs,
        }
        .install(image, self.controller_link, packet_row_address, packet_row)
    }
}
#[derive(Deserialize)]
struct ParameterTable {
    kind: TableKind,
    global_offset: u64,
    context: u64,
    begin: u64,
    index: u64,
    extent: usize,
    index_extent: usize,
    stride: usize,
    hash_header: Option<u64>,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum TableKind {
    Curve,
    Scalar,
}
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Root {
    Module,
    SourceActor,
    SourceOwner,
    Stat,
    Source,
    D,
    Packet,
    TargetActor,
    TargetOwner,
    TargetStat,
    Stack,
    Teb,
}
impl Root {
    fn live(self, s: &EncounterRole) -> Option<u64> {
        match self {
            Self::Module => Some(s.module_base),
            Self::SourceActor => Some(s.player),
            Self::SourceOwner => Some(s.owner),
            Self::Stat => Some(s.stat),
            Self::D => Some(s.materializer),
            Self::Source => Some(s.source),
            Self::Packet
            | Self::TargetActor
            | Self::TargetOwner
            | Self::TargetStat
            | Self::Stack
            | Self::Teb => None,
        }
    }
}
#[derive(Deserialize)]
struct Field {
    at: u64,
    n: usize,
    root: Root,
    path: Vec<i64>,
    #[serde(flatten)]
    value: ValueKind,
}
#[derive(Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ValueKind {
    Read,
    InitializedThread {
        guard_rva: u64,
    },
    Pointer {
        to: u64,
    },
    ModulePointer {
        rva: u64,
    },
    VectorEnd {
        begin_path: Vec<i64>,
        virtual_begin: u64,
        extent: u64,
    },
    Weapon,
    Zero,
    Benchmark {
        bits: String,
    },
}

#[derive(PartialEq, Eq)]
pub struct Encounter {
    pub image: Vec<u8>,
    pub known: Vec<u8>,
    pub entry: Vec<u8>,
    pub flags: u32,
}

struct Page {
    data: Vec<u8>,
    known: Vec<u8>,
}
#[derive(Default)]
struct Image(BTreeMap<u64, Page>);
impl Image {
    fn page(&mut self, a: u64) -> &mut Page {
        self.0.entry(a & !65535).or_insert_with(|| Page {
            data: vec![0; CHUNK],
            known: vec![0; CHUNK / 8],
        })
    }
    fn set(&mut self, a: u64, bytes: &[u8]) -> R<()> {
        for (i, &v) in bytes.iter().enumerate() {
            let at = add(a, i as u64)?;
            let off = at as usize & 65535;
            let p = self.page(at);
            let bit = 1 << (off % 8);
            if p.known[off / 8] & bit != 0 && p.data[off] != v {
                return Err(layout("encounter operands disagree on a virtual byte"));
            }
            p.known[off / 8] |= bit;
            p.data[off] = v;
        }
        Ok(())
    }
    fn pointer(&mut self, a: u64, v: u64) -> R<()> {
        self.set(a, &v.to_le_bytes())
    }
    fn encode(self) -> (Vec<u8>, Vec<u8>) {
        let mut image = Vec::with_capacity(self.0.len() * (CHUNK + 8));
        let mut known = Vec::with_capacity(self.0.len() * (CHUNK / 8 + 8));
        for (base, p) in self.0 {
            image.extend(base.to_le_bytes());
            known.extend(base.to_le_bytes());
            image.extend(p.data);
            known.extend(p.known);
        }
        (image, known)
    }
}

struct Reader<'a> {
    memory: &'a dyn InventoryMemory,
    pointers: BTreeMap<u64, u64>,
}

/// Address-independent captured opponent data is relocated to a private,
/// disjoint heap before packing. Only the MAIN target pointer is redirected.
fn install_reference_target(image: &mut Image, raw: &[u8], packet: u64, module: u64) -> R<u64> {
    let invalid = || layout("invalid fixed opponent image");
    let (head, mut rest) = raw.split_first_chunk::<36>().ok_or_else(invalid)?;
    let actor = u64_at(head, 24)?;
    let heap = 0x4000_0000_0000..0x6000_0000_0000;
    if &head[..8] != b"N3FOE001"
        || u64_at(head, 8)? != packet
        || u64_at(head, 16)? != module
        || !heap.contains(&actor)
    {
        return Err(invalid());
    }
    let count = u32_at(head, 32)?;
    let mut previous = heap.start;
    let mut has_actor = false;
    for _ in 0..count {
        let (span, tail) = rest.split_first_chunk::<12>().ok_or_else(invalid)?;
        let at = u64_at(span, 0)?;
        let size = u32_at(span, 8)? as usize;
        let end = add(at, size as u64)?;
        if at < previous || size == 0 || end > heap.end {
            return Err(invalid());
        }
        let (bytes, remaining) = tail.split_at_checked(size).ok_or_else(invalid)?;
        image.set(at, bytes)?;
        has_actor |= (at..end).contains(&actor);
        previous = end;
        rest = remaining;
    }
    if !rest.is_empty() || !has_actor {
        return Err(invalid());
    }
    image.pointer(add(packet, 0x1c8)?, actor)?;
    Ok(actor)
}
impl Reader<'_> {
    fn pointer(&mut self, address: u64) -> R<u64> {
        if let Some(v) = self.pointers.get(&address) {
            return Ok(*v);
        }
        let v = q(self.memory, address)?;
        self.pointers.insert(address, v);
        Ok(v)
    }
    fn address(&mut self, mut base: u64, path: &[i64]) -> R<Option<u64>> {
        let (last, steps) = path
            .split_last()
            .ok_or_else(|| layout("empty encounter field path"))?;
        for off in steps {
            let at = base
                .checked_add_signed(*off)
                .ok_or_else(|| layout("encounter path overflow"))?;
            base = self.pointer(at)?;
            if base == 0 {
                return Ok(None);
            }
        }
        base.checked_add_signed(*last)
            .map(Some)
            .ok_or_else(|| layout("encounter address overflow"))
    }
    fn bytes(&self, base: u64, n: usize) -> R<Vec<u8>> {
        let mut out = Vec::with_capacity(n);
        while out.len() < n {
            let at = add(base, out.len() as u64)?;
            let size = (n - out.len()).min(4096 - (at as usize & 4095));
            out.extend(read(self.memory, at, size)?);
        }
        Ok(out)
    }
}
impl ParameterTable {
    fn install(&self, reader: &mut Reader<'_>, s: &EncounterRole, image: &mut Image) -> R<()> {
        let ctx = reader.pointer(add(s.module_base, self.global_offset)?)?;
        let begin = reader.pointer(ctx)?;
        image.pointer(self.context, self.begin)?;
        match self.kind {
            TableKind::Curve => {
                let end = reader.pointer(add(ctx, 8)?)?;
                let index = reader.pointer(add(ctx, 24)?)?;
                let limit = reader.pointer(add(ctx, 32)?)?;
                if end.checked_sub(begin) != Some(self.extent as u64)
                    || limit.checked_sub(index) != Some(self.index_extent as u64)
                    || self.extent / self.stride != self.index_extent / 8
                {
                    return Err(layout("encounter curve table geometry changed"));
                }
                image.pointer(self.context + 8, self.begin + self.extent as u64)?;
                image.pointer(self.context + 24, self.index)?;
                image.pointer(self.context + 32, self.index + self.index_extent as u64)?;
                image.set(self.begin, &reader.bytes(begin, self.extent)?)?;
                let pointers = reader.bytes(index, self.index_extent)?;
                let mut seen = BTreeSet::new();
                for (n, p) in pointers.as_chunks::<8>().0.iter().enumerate() {
                    let offset = u64::from_le_bytes(*p)
                        .checked_sub(begin)
                        .filter(|v| *v < self.extent as u64 && v % self.stride as u64 == 0)
                        .ok_or_else(|| layout("curve pointer outside parameter rows"))?;
                    if !seen.insert(offset) {
                        return Err(layout("duplicate row in curve index"));
                    }
                    image.pointer(self.index + 8 * n as u64, self.begin + offset)?;
                }
            }
            TableKind::Scalar => {
                let hash = reader.pointer(add(ctx, 32)?)?;
                let index = reader.pointer(add(hash, 8)?)?;
                let end = reader.pointer(add(hash, 16)?)?;
                let count = u(reader.memory, add(begin, 4)?)? as usize;
                if end.checked_sub(index) != Some(self.index_extent as u64)
                    || count != (self.extent - 8) / self.stride
                {
                    return Err(layout("encounter scalar table geometry changed"));
                }
                let vh = self
                    .hash_header
                    .ok_or_else(|| layout("scalar hash layout is missing"))?;
                image.pointer(self.context + 32, vh)?;
                image.set(vh, &read(reader.memory, hash, 8)?)?;
                image.pointer(vh + 8, self.index)?;
                image.pointer(vh + 16, self.index + self.index_extent as u64)?;
                image.set(self.begin, &reader.bytes(begin, self.extent)?)?;
                image.set(self.index, &reader.bytes(index, self.index_extent)?)?;
            }
        }
        Ok(())
    }
}

fn recipes() -> R<&'static Vec<Recipe>> {
    static RECIPES: OnceLock<Result<Vec<Recipe>, serde_json::Error>> = OnceLock::new();
    RECIPES
        .get_or_init(|| {
            let mut recipes: Vec<Recipe> =
                serde_json::from_str(include_str!("samurai_direct_pc202.json"))?;
            recipes.push(serde_json::from_str(include_str!(
                "kusarigama_strong_pc202.json"
            ))?);
            recipes.push(serde_json::from_str(include_str!(
                "water_projectile_pc202.json"
            ))?);
            Ok(recipes)
        })
        .as_ref()
        .map_err(|_| layout("invalid built-in PC202 encounters"))
}

fn recipe(key: &[u8; 32]) -> R<&'static Recipe> {
    recipes()?
        .iter()
        .find(|recipe| &recipe.action_row_key == key)
        .ok_or_else(|| layout("selected attack has no encounter with verified inputs"))
}

/// Read the fixed-opponent operands of the installed PC202 layouts from one
/// selected opponent. A recording InventoryMemory retains addresses and bytes;
/// null branches stay absent, and a failed reachable read is reported.
#[derive(serde::Serialize)]
pub struct TargetOperandCoverage {
    pub observed_ranges: usize,
    pub missing: BTreeSet<String>,
}

pub fn observe_target_operands(
    memory: &dyn InventoryMemory,
    target: &super::target_conditions::TargetConditions,
) -> R<TargetOperandCoverage> {
    let mut reader = Reader {
        memory,
        pointers: BTreeMap::new(),
    };
    let mut ranges = BTreeSet::new();
    let mut missing = BTreeSet::new();
    for recipe in recipes()? {
        for field in &recipe.fields {
            if field.root == Root::TargetActor
                && field.path.starts_with(&[0x3b0, 8, 0xe8, 0x40])
                && field.path.len() > 4
            {
                // Row indices belong to the observed action. Read its list below.
                continue;
            }
            let base = match field.root {
                Root::TargetActor => target.actor,
                Root::TargetOwner if field.path.first() == Some(&0x33b0) => continue, // Observed by native list order.
                Root::TargetOwner => target.owner,
                Root::TargetStat => target.stat,
                Root::Module
                | Root::SourceActor
                | Root::SourceOwner
                | Root::Stat
                | Root::Source
                | Root::D
                | Root::Packet
                | Root::Stack
                | Root::Teb => continue,
            };
            let observed = (|| -> R<()> {
                if let Some(address) = reader.address(base, &field.path)? {
                    if !ranges.contains(&(address, field.n)) {
                        read(memory, address, field.n)?;
                        ranges.insert((address, field.n));
                    }
                }
                Ok(())
            })();
            match observed {
                Err(e @ RuntimeError::RegionNotReadable { .. }) => {
                    missing.insert(format!(
                        "{:?} {:?} ({} bytes): {e}",
                        field.root, field.path, field.n
                    ));
                }
                other => other?,
            }
        }
    }
    read(memory, target.stat, 0x900)?;
    // 2E98F0 uses the observed animation's first row. An opponent idle in
    // another animation need not have the historical template's row index.
    if let Some(field) = reader.address(target.actor, &[0x3b0, 8, 0xe8])? {
        let state = reader.pointer(field)?;
        if state != 0 {
            let parameter = reader.pointer(add(state, 0x20)?)?;
            if parameter != 0 {
                read(memory, parameter, 336)?;
            }
            let header = read(memory, add(state, 0x48)?, 4)?;
            let first = u16::from_le_bytes([header[0], header[1]]);
            let count = u16::from_le_bytes([header[2], header[3]]);
            if count != 0 {
                let table = reader.pointer(add(state, 0x40)?)?;
                for i in 0..u32::from(count) {
                    let at = reader.pointer(add(table, 8 * (u64::from(first) + u64::from(i)))?)?;
                    if at != 0 {
                        // The row modifier at +72 is consumed by the target
                        // predicate. The broad capture also retains the row.
                        read(memory, add(at, 0x72)?, 2)?;
                        match read(memory, at, 192) {
                            Err(e @ RuntimeError::RegionNotReadable { .. }) => {
                                missing.insert(e.to_string());
                            }
                            other => {
                                other?;
                            }
                        }
                    }
                }
            }
        }
    }
    for offset in [
        0x2ebc, 0x2ec0, 0x2ec4, 0x2f14, 0x2fa4, 0x2fa8, 0x2fac, 0x2ffc,
    ] {
        read(memory, add(target.owner, offset)?, 4)?;
    }
    Ok(TargetOperandCoverage {
        observed_ranges: ranges.len(),
        missing,
    })
}
pub fn direct_encounter(
    memory: &dyn InventoryMemory,
    s: &CombatSnapshot,
    stat: &[u8],
    category: u32,
    row_key: &[u8; 32],
    row: &[u8; 192],
) -> R<Encounter> {
    direct_encounter_with_controller(memory, s, stat, category, row_key, row, None)
}

pub fn direct_encounter_with_controller(
    memory: &dyn InventoryMemory,
    s: &CombatSnapshot,
    stat: &[u8],
    category: u32,
    row_key: &[u8; 32],
    row: &[u8; 192],
    controller: Option<&ControllerInput>,
) -> R<Encounter> {
    let equipped =
        s.inputs.active_bank as usize * BANK_BYTES + s.inputs.held_weapon_slot as usize * 0xe8;
    let role = EncounterRole {
        module_base: s.module_base,
        player: s.roots.player,
        owner: s.roots.owner,
        stat: s.roots.stat,
        materializer: s.roots.materializer,
        source: s.roots.source,
        form: s.inputs.mode,
        held_slot: s.inputs.held_weapon_slot,
        hell_weapon: s.inputs.inventory[equipped + 0x1a] & 0x10 != 0,
    };
    project_encounter(
        memory,
        &role,
        stat,
        category,
        (row_key, row),
        Some(&s.source_raw),
        controller,
    )
}

/// The roots already verified by the combat snapshot reader. Host replays
/// can supply the same logical inputs from an independently retained actor.
#[derive(Deserialize)]
pub struct EncounterRole {
    pub module_base: u64,
    pub player: u64,
    pub owner: u64,
    pub stat: u64,
    pub materializer: u64,
    pub source: u64,
    pub form: i32,
    pub held_slot: u32,
    pub hell_weapon: bool,
}

pub fn materialize_encounter(
    memory: &dyn InventoryMemory,
    s: &EncounterRole,
    stat: &[u8],
    category: u32,
    row_key: &[u8; 32],
    row: &[u8; 192],
) -> R<Encounter> {
    project_encounter(memory, s, stat, category, (row_key, row), None, None)
}

fn project_encounter(
    memory: &dyn InventoryMemory,
    s: &EncounterRole,
    stat: &[u8],
    category: u32,
    selected_row: (&[u8; 32], &[u8; 192]),
    source: Option<&[u8]>,
    controller: Option<&ControllerInput>,
) -> R<Encounter> {
    let (row_key, row) = selected_row;
    let requested = controller.and_then(|state| state.layout);
    let projectile = controller.and_then(|c| match c.packet_row {
        p @ PacketRow::OwnedProjectile { .. } => Some(p),
        PacketRow::Actor(_) => None,
    });
    let shared = requested.filter(|v| v.direct_only || projectile.is_some());
    let mut fields = BTreeMap::new();
    let r = if let Some(layout_id) = shared {
        // A layout identifies the native ABI and fixed opponent. The selected
        // action owns its controller, and the observed actor owns its weapon.
        let mut family = recipes()?.iter().filter(|r| {
            r.form == s.form
                && r.projectiles.is_empty()
                && r.roots.get("packet") == Some(&layout_id.address)
        });
        let base = family
            .next()
            .ok_or_else(|| layout("unknown direct encounter ABI"))?;
        for member in std::iter::once(base).chain(family) {
            if member.roots != base.roots
                || member.selected_action.controller_link != base.selected_action.controller_link
            {
                return Err(layout("inconsistent direct encounter ABI"));
            }
            for field in &member.fields {
                let previous: Option<&Field> = fields.insert((field.at, field.n), field);
                if let Some(prev) = previous {
                    if prev.value != field.value {
                        return Err(layout("conflicting direct encounter operands"));
                    }
                }
            }
        }
        base
    } else {
        let fixed = recipe(row_key)?;
        if requested.is_some_and(|q| fixed.roots.get("packet") != Some(&q.address)) {
            return Err(layout("incompatible projectile or fixed encounter ABI"));
        }
        for field in &fixed.fields {
            fields.insert((field.at, field.n), field);
        }
        fixed
    };
    let root = |key: &str| {
        r.roots
            .get(key)
            .copied()
            .ok_or_else(|| layout("missing virtual encounter anchor"))
    };
    if stat.len() != 0x900
        || s.form != r.form
        || s.held_slot != r.held_slot
        || (controller.is_none() && category != r.item_category)
    {
        return Err(layout(
            "current role does not match this encounter's form, held slot and weapon category",
        ));
    }
    let mut reader = Reader {
        memory,
        pointers: BTreeMap::new(),
    };
    let mut image = Image::default();
    if let Some((input, packet)) = controller.and_then(|input| {
        input
            .layout
            .filter(|packet| packet.direct_owner_element)
            .map(|packet| (input, packet))
    }) {
        // 2B5631..2B569C. With these direct rows the previous element
        // object's test cannot change selection: row+20 is unsigned 255.
        if !matches!(input.packet_row, PacketRow::Actor(_))
            || !packet.direct_only
            || !input.rows.iter().all(|r| {
                r[0x20] == 0xff && (packet.actor_damage_kind_from_row || r[4] & 0xc0 == 0)
            })
        {
            return Err(layout("direct owner element requires ordinary actor rows"));
        }
        let owner_flags = u(memory, add(s.owner, 0x188)?)?;
        if owner_flags & 2 != 0 {
            return Err(layout("the actor uses the alternate owner element branch"));
        }
        let private_owner = root("source_owner")?;
        image.set(add(private_owner, 0x188)?, &owner_flags.to_le_bytes())?;
        let selected = match input.actor_element_condition {
            ActorElementCondition::Current => u(memory, add(s.owner, 0x2e9c)?)?,
            ActorElementCondition::ProjectedUncoated => u32::MAX,
        };
        image.set(add(private_owner, 0x2e9c)?, &selected.to_le_bytes())?;
        if selected != u32::MAX {
            let power = u(memory, add(s.owner, 0x2ea4)?)?;
            image.set(add(private_owner, 0x2ea4)?, &power.to_le_bytes())?;
        }
    }
    if let Some(source) = source {
        // 2EB61B bounds the 74-way dispatch; the table entries are PC202 RVAs.
        image.set(
            add(root("module")?, 0x2ebe08)?,
            include_bytes!("pc202_action_flag_targets.bin"),
        )?;
        // PC202 794596 compares the opponent's indexed STAT scalar against
        // this immutable .rdata constant (00 00 7a 44).
        image.set(add(root("module")?, 0x3bd4810)?, &1000_f32.to_le_bytes())?;
        // PC202 2081C7D scales the held item's table result from percent.
        image.set(add(root("module")?, 0x3bd4890)?, &0.01_f32.to_le_bytes())?;
        image.pointer(add(root("stat")?, 0x7d8)?, root("source")?)?;
        // The reset footprint identifies numeric storage for every weapon.
        // Read its actual baseline; no reset defaults are used as observations.
        let damage = read(memory, s.materializer, 0xb00)?;
        let materializer = root("d")?;
        for &(start, end, _) in super::reset::DAMAGE {
            image.set(add(materializer, start as u64)?, &damage[start..end])?;
        }
        let target = root("target_stat")?;
        let benchmark = include_bytes!("pc202_benchmark_stats.bin")
            .as_chunks::<{ 8 + 0x900 + 1 + 32 }>()
            .0
            .iter()
            .find(|row| row[..8] == target.to_le_bytes());
        let source_stat = root("stat")?;
        if let Some(bytes) = benchmark {
            let target_owner = root("target_owner")?;
            for (i, offset) in [
                0x2ebc, 0x2ec0, 0x2ec4, 0x2f14, 0x2fa4, 0x2fa8, 0x2fac, 0x2ffc,
            ]
            .into_iter()
            .enumerate()
            {
                if bytes[8 + 0x900] & (1 << i) != 0 {
                    let start = 8 + 0x900 + 1 + 4 * i;
                    image.set(add(target_owner, offset)?, &bytes[start..start + 4])?;
                }
            }
        }
        for &(start, end, _) in super::reset::STAT {
            if let Some(bytes) = benchmark {
                image.set(add(target, start as u64)?, &bytes[8 + start..8 + end])?;
            }
            image.set(add(source_stat, start as u64)?, &stat[start..end])?;
        }
        // 40264C/F65D8 resolve the held record from the actual inventory
        // bank and its slot selector. This binding is common to both forms.
        let inventory = add(s.owner, 0x590)?;
        let bank = u(memory, add(inventory, 0x1ee0)?)?;
        if bank > 1 {
            return Err(layout("invalid held inventory bank"));
        }
        let bank_offset = u64::from(bank) * 0xf70;
        let selector = read(memory, add(inventory, bank_offset + 0xf68)?, 1)?[0];
        let held_offset = bank_offset + u64::from(selector == 0) * 0xe8;
        let private_inventory = add(root("source_owner")?, 0x590)?;
        image.set(add(private_inventory, 0x1ee0)?, &bank.to_le_bytes())?;
        image.set(add(private_inventory, bank_offset + 0xf68)?, &[selector])?;
        for word in [0, 0x14, 0x18] {
            let offset = held_offset + word;
            image.set(
                add(private_inventory, offset)?,
                &read(memory, add(inventory, offset)?, 4)?,
            )?;
        }
        for offset in [0x2e48, 0x2e4c, 0x2e68, 0x2e6c, 0x3668] {
            if let Ok(value) = read(memory, add(s.owner, offset)?, 4) {
                image.set(add(root("source_owner")?, offset)?, &value)?;
            }
        }
        for offset in MAP_OFFSETS {
            let rows = source
                .get(offset..offset + 0xc04)
                .ok_or_else(|| layout("missing source aggregate storage"))?;
            image.set(add(root("source")?, offset as u64)?, rows)?;
        }
    }
    let aad = root("attack_data")?;
    image.pointer(add(root("packet")?, 0x190)?, aad)?;
    if let Some(input) = controller {
        input.install(&mut image, r.selected_action.controller_link, aad, row)?;
    } else {
        r.selected_action.install(&mut image, aad, row)?;
    }
    for projectile in &r.projectiles {
        projectile.install(&mut image, root("source_actor")?)?;
    }
    let skill_key = controller.and_then(|c| c.actor_skill_key);
    if projectile.is_some() || skill_key.is_some_and(|key| key != 0) {
        install_skill_table(&mut image, root("module")?)?;
    }
    if let Some(PacketRow::OwnedProjectile {
        object,
        descriptor_head,
        action,
        selector,
        global_key,
        descriptor_5f8,
        descriptor_5fc,
        packet_flags,
    }) = projectile
    {
        // 624518 returns this module-owned fallback when the entity has no D.
        // MAIN reads a different scalar for each reached damage-row branch.
        const DEFAULT_DAMAGE: u64 = 0x4ba66b0;
        let fallback = add(root("module")?, DEFAULT_DAMAGE)?;
        let observed = reader.bytes(add(s.module_base, DEFAULT_DAMAGE)?, 0xb00);
        match observed {
            Ok(data) => image.set(fallback, &data)?,
            Err(RuntimeError::RegionNotReadable { .. }) => {
                let guard = u(memory, add(s.module_base, 0x4ba66a0)?)? as i32;
                if guard < -1 {
                    // 872770 calls 8731E4. The complete 2,672-byte native
                    // constructor write mask equals these runs. Gaps stay unknown.
                    let mut defaults = [0; 0xb00];
                    for &(start, end, byte) in super::reset::DAMAGE {
                        defaults[start..end].fill(byte);
                        image.set(fallback + start as u64, &defaults[start..end])?;
                    }
                }
            }
            Err(error) => return Err(error),
        }
        install_projectile(
            &mut image,
            object,
            root("source_actor")?,
            descriptor_head,
            action,
            selector,
        )?;
        image.set(object + 0x5f8, &descriptor_5f8.to_le_bytes())?;
        image.set(object + 0x600, &global_key.to_le_bytes())?;
        image.set(object + 0x5fc, &descriptor_5fc.to_le_bytes())?;
        image.set(object + 0x864, &category.to_le_bytes())?;
        // PC202 326C94 distinguishes the spawned object from its owner, and
        // reads bit 9 of the constructed packet for non-type-2 descriptors.
        image.set(
            add(root("packet")?, 0x288)?,
            &[
                0,
                u8::from(packet_flags & 0x200 != 0 && descriptor_head[3] != 2),
            ],
        )?;
        image.set(add(root("packet")?, 0x13c)?, &packet_flags.to_le_bytes())?;
        // The scene enters MAIN at 3277E6 (return 3277EB). Its caller writes
        // zero at 3277DF. The alternative call site uses a different value.
        image.set(add(root("packet")?, 0x1e4)?, &0u32.to_le_bytes())?;
        for offset in [0x28, 0x198] {
            image.pointer(add(root("packet")?, offset)?, object)?;
        }
    }
    let mut bindings = BTreeMap::new();
    let mut live_objects = BTreeMap::new();
    for (name, address) in [
        ("module", s.module_base),
        ("source_actor", s.player),
        ("source_owner", s.owner),
        ("stat", s.stat),
        ("source", s.source),
        ("d", s.materializer),
    ] {
        let canonical = root(name)?;
        bindings.insert(canonical, address);
        live_objects.insert(address, canonical);
    }
    let hell = u32::from(s.hell_weapon);
    let flags = hell << 16;
    let module = root("module")?;
    let membership = if source.is_some() {
        let holder = reader.pointer(add(s.module_base, MEMBERSHIP_GLOBAL)?)?;
        match super::progression::graph(memory, add(holder, MEMBERSHIP_OFFSET)?, 24, 2) {
            Ok(keys) => Some(keys),
            Err(RuntimeError::RegionNotReadable { .. }) => None,
            Err(error) => return Err(error),
        }
    } else {
        None
    };
    if let Some(keys) = &membership {
        let holder = r
            .fields
            .iter()
            .find_map(|f| {
                if let ValueKind::Pointer { to } = f.value {
                    if f.root == Root::Module && f.path == [MEMBERSHIP_GLOBAL as i64] {
                        return Some(to);
                    }
                }
                None
            })
            .ok_or_else(|| layout("membership manager missing from recipe"))?;
        install_membership(&mut image, add(holder, MEMBERSHIP_OFFSET)?, keys)?;
    }
    if let Some(teb) = r.roots.get("teb") {
        image.page(*teb);
    }
    for key in ["stat", "d", "packet", "stack"] {
        image.page(root(key)?);
    }
    let replacement = controller.and_then(|c| c.reference_target);
    // PC202 2B4E74 writes a u16. Bytes 134..135 have different owners.
    if let (Some(c), Some(key)) = (controller, skill_key) {
        let link = u32_at(c.parameter, 0xdc)?;
        if link & 0x8000_0000 != 0 || link as u16 != key {
            return Err(layout("witnessed skill key differs from selected action"));
        }
        image.set(add(root("packet")?, 0x136)?, &key.to_le_bytes())?;
    }
    let replacement_actor = replacement
        .map(|data| install_reference_target(&mut image, data, root("packet")?, module))
        .transpose()?;
    for field in fields.values() {
        if projectile.is_some()
            && field.root == Root::Packet
            && (((field.path == [0x28] || field.path == [0x198]) && field.n == 8)
                || (matches!(field.path.as_slice(), [0x288] | [0x289]) && field.n == 1)
                || (field.path == [0x13c] && field.n == 4))
        {
            continue;
        }
        if skill_key.is_some()
            && field.root == Root::Packet
            && field.path == [0x136]
            && field.n == 2
        {
            continue;
        }
        if replacement.is_some() && field.root == Root::Packet && field.path == [0x1c8] {
            continue;
        }
        if membership.is_some()
            && field.root == Root::Module
            && matches!(field.path.as_slice(), [first,offset,..] if *first==MEMBERSHIP_GLOBAL as i64 && (0x2820..0x2860).contains(offset))
        {
            continue;
        }
        let live = field.root.live(s);
        let address = match live {
            Some(base) => match reader.address(base, &field.path) {
                Ok(Some(at)) => Some(at),
                Ok(None) => continue,
                Err(RuntimeError::RegionNotReadable { .. }) if shared.is_some() => continue,
                Err(error) => return Err(error),
            },
            None => None,
        };
        let live_address = || address.ok_or_else(|| layout("invalid encounter operand owner"));
        let word = |v: u64| -> R<Vec<u8>> {
            v.to_le_bytes()
                .get(..field.n)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| layout("invalid encounter operand width"))
        };
        let field_bytes = (|| -> R<Vec<u8>> {
            Ok(match &field.value {
                ValueKind::Read => read(memory, live_address()?, field.n)?,
                ValueKind::InitializedThread { guard_rva } => {
                    // 624535 compares the current static guard with the thread's
                    // initialization epoch (signed JG). Model a scoring thread
                    // already synchronized to this initialized PC202 parameter.
                    let guard = u(memory, add(s.module_base, *guard_rva)?)?;
                    if guard as i32 >= -1 {
                        return Err(layout("reference parameter is not initialized"));
                    }
                    word(u64::from(guard))?
                }
                ValueKind::Zero => vec![0; field.n],
                ValueKind::Benchmark { bits } => {
                    if live.is_some() || bits.len() != 2 * field.n {
                        return Err(layout("invalid fixed opponent or action field"));
                    }
                    (0..bits.len())
                        .step_by(2)
                        .map(|i| {
                            u8::from_str_radix(&bits[i..i + 2], 16)
                                .map_err(|_| layout("invalid benchmark byte"))
                        })
                        .collect::<R<Vec<_>>>()?
                }
                ValueKind::Weapon => {
                    let value = match field.path.as_slice() {
                        [8] => u32_at(stat, 0x24)?.wrapping_add(u32_at(stat, 0x2c)?),
                        [12] => u32_at(stat, 0x30)?,
                        [16] => u32_at(stat, 0x34)?,
                        [20] => u32_at(stat, 0x3c)?,
                        // PC202 2B42C6/2B444C initialize packet+18 to 100.
                        // An unimbued weapon leaves that constructor value intact.
                        [24] if u32_at(stat, 0x3c)? == u32::MAX => 100,
                        [24] => u32_at(stat, 0x40)?,
                        [0x144] => category,
                        [0x298] => flags,
                        [0x29a] => hell,
                        _ => return Err(layout("unknown direct weapon field")),
                    };
                    word(u64::from(value))?
                }
                ValueKind::Pointer { to } => {
                    let mut dest = *to;
                    if let Some(at) = address {
                        let actual = reader.pointer(at)?;
                        if actual == 0 {
                            dest = 0;
                        } else if bindings
                            .insert(*to, actual)
                            .is_some_and(|prev| prev != actual)
                            || live_objects
                                .insert(actual, *to)
                                .is_some_and(|prev| prev != *to)
                        {
                            return Err(layout("virtual object has conflicting current bindings"));
                        }
                    }
                    word(dest)?
                }
                ValueKind::ModulePointer { rva } => {
                    if let Some(at) = address {
                        if reader.pointer(at)? != add(s.module_base, *rva)? {
                            return Err(layout(
                                "encounter object has another PC202 type or function",
                            ));
                        }
                    }
                    word(add(module, *rva)?)?
                }
                ValueKind::VectorEnd {
                    begin_path,
                    virtual_begin,
                    extent,
                } => {
                    let Some(at) = reader.address(
                        live.ok_or_else(|| layout("vector endpoint has no owner"))?,
                        begin_path,
                    )?
                    else {
                        return Ok(Vec::new());
                    };
                    let begin = reader.pointer(at)?;
                    let end = reader.pointer(live_address()?)?;
                    if end.checked_sub(begin) != Some(*extent) {
                        return Err(layout("shared parameter vector extent changed"));
                    }
                    word(add(*virtual_begin, *extent)?)?
                }
            })
        })();
        match field_bytes {
            Ok(bytes) => image.set(field.at, &bytes)?,
            Err(RuntimeError::RegionNotReadable { .. }) if shared.is_some() => {}
            Err(error) => return Err(error),
        }
    }
    for t in &r.tables {
        t.install(&mut reader, s, &mut image)?;
    }
    let condition = controller.map_or(TargetCondition::Benchmark, |c| c.target_condition);
    if !matches!(condition, TargetCondition::Benchmark) {
        let owner = match replacement_actor {
            Some(actor) => encounter_peak_target::saturated_opponent_owner(&image, actor)?,
            None => root("target_owner")?,
        };
        match condition {
            TargetCondition::Saturated => encounter_peak_target::install_saturated_opponent(&mut image, owner)?,
            TargetCondition::TheoreticalPeak => {
                encounter_peak_target::install_theoretical_peak_opponent(&mut image, owner, module)?;
            }
            TargetCondition::Peak { ailments, weakness } => {
                encounter_peak_target::install_peak_opponent(&mut image, owner, module, ailments, weakness)?;
            }
            TargetCondition::Benchmark => {}
        }
    }
    let mut entry = vec![0; 416];
    entry[..8].copy_from_slice(b"NKENTRY1");
    entry[8..12].copy_from_slice(&1u32.to_le_bytes());
    entry[12..16].copy_from_slice(&400u32.to_le_bytes());
    entry[24..32].copy_from_slice(&root("packet")?.to_le_bytes());
    entry[48..56].copy_from_slice(&root("stack")?.to_le_bytes());
    entry[408..412].copy_from_slice(&0x1f80u32.to_le_bytes());
    let (image, known) = image.encode();
    Ok(Encounter {
        image,
        known,
        entry,
        flags,
    })
}
