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
        let object = self.object;
        let tag = self.descriptor_head[0].wrapping_shl(4) | 4;
        image.set(object, &tag.to_le_bytes())?;
        image.pointer(object + 0x260, actor)?;
        image.set(object + 0x590, &1.0f32.to_le_bytes())?;
        image.set(object + 0x594, &u64::from(1.0f32.to_bits()).to_le_bytes())?;
        image.set(object + 0x59c, &u32::MAX.to_le_bytes())?;
        image.set(object + 0x5a0, &1u32.to_le_bytes())?;
        for (i, word) in self.descriptor_head.iter().enumerate() {
            image.set(object + 0x5d0 + i as u64 * 4, &word.to_le_bytes())?;
        }
        image.set(object + 0x758, &u32::from(self.action).to_le_bytes())?;
        image.set(object + 0x75c, &[self.selector])?;
        image.set(self.row_address, &static_bytes(&self.damage_row, 192)?)
    }
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

impl SelectedAction {
    fn install(&self, image: &mut Image, packet_row_address: u64, packet_row: &[u8; 192]) -> R<()> {
        image.pointer(self.controller_link, ACTION_STATE)?;
        image.pointer(ACTION_STATE + 0x20, ACTION_PARAM)?;
        image.set(ACTION_PARAM, &static_bytes(&self.parameter, 336)?)?;
        image.pointer(ACTION_STATE + 0x40, ACTION_TABLE)?;
        image.set(ACTION_STATE + 0x48, &self.first_index.to_le_bytes())?;
        let count = u16::try_from(self.rows.len())
            .map_err(|_| layout("invalid static action row count"))?;
        image.set(ACTION_STATE + 0x4a, &count.to_le_bytes())?;
        if self.packet_row >= self.rows.len() {
            return Err(layout("invalid first action row"));
        }
        for (i, raw) in self.rows.iter().enumerate() {
            let bytes = static_bytes(raw, 192)?;
            let address = if i == self.packet_row {
                if bytes != packet_row {
                    return Err(layout("attack parameter and packet use different rows"));
                }
                packet_row_address
            } else {
                ACTION_ROWS + 192 * i as u64
            };
            image.set(address, &bytes)?;
            image.pointer(
                ACTION_TABLE + 8 * (u64::from(self.first_index) + i as u64),
                address,
            )?;
        }
        Ok(())
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
#[derive(Deserialize)]
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

fn recipe(key: &[u8; 32]) -> R<&'static Recipe> {
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
        .map_err(|_| layout("invalid built-in PC202 encounters"))?
        .iter()
        .find(|recipe| &recipe.action_row_key == key)
        .ok_or_else(|| layout("selected attack has no encounter with verified inputs"))
}
pub fn direct_encounter(
    memory: &dyn InventoryMemory,
    s: &CombatSnapshot,
    stat: &[u8],
    category: u32,
    row_key: &[u8; 32],
    row: &[u8; 192],
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
    materialize_encounter(memory, &role, stat, category, row_key, row)
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
    let r = recipe(row_key)?;
    let root = |key: &str| {
        r.roots
            .get(key)
            .copied()
            .ok_or_else(|| layout("missing virtual encounter anchor"))
    };
    if stat.len() != 0x900
        || s.form != r.form
        || s.held_slot != r.held_slot
        || category != r.item_category
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
    let aad = root("attack_data")?;
    image.pointer(add(root("packet")?, 0x190)?, aad)?;
    r.selected_action.install(&mut image, aad, row)?;
    for projectile in &r.projectiles {
        projectile.install(&mut image, root("source_actor")?)?;
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
    if let Some(teb) = r.roots.get("teb") {
        image.page(*teb);
    }
    for key in ["stat", "d", "packet", "stack"] {
        image.page(root(key)?);
    }
    for field in &r.fields {
        let live = field.root.live(s);
        let address = match live {
            Some(base) => match reader.address(base, &field.path)? {
                Some(at) => Some(at),
                None => continue,
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
        let bytes = match &field.value {
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
                    continue;
                };
                let begin = reader.pointer(at)?;
                let end = reader.pointer(live_address()?)?;
                if end.checked_sub(begin) != Some(*extent) {
                    return Err(layout("shared parameter vector extent changed"));
                }
                word(add(*virtual_begin, *extent)?)?
            }
        };
        image.set(field.at, &bytes)?;
    }
    for t in &r.tables {
        t.install(&mut reader, s, &mut image)?;
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
