//! Runs the compiled formula paths of one attack. A run either reproduces the game's code on that path or reports
//! that the candidate left every path; it never guesses. Floating point is native binary32, as in the game.

use crate::engine::LoadError;
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(u8)]
#[allow(non_camel_case_types)]
pub enum Op {
    In,
    Mulss,
    Addss,
    Subss,
    Divss,
    Arg,
    Fn,
    Guard,
    Set,
    Cmov,
    Eq,
    Mix,
    Addr,
    Load,
    Comiss,
    Minss,
    Maxss,
    Sqrtss,
    Cvtsi2ss,
    Cvttss2si,
    Cvtss2sd,
    Cvtsd2ss,
    And,
    Or,
    Xor,
    Add,
    Adc,
    Sub,
    Sbb,
    Inc,
    Dec,
    Neg,
    KeepCf,
    Not,
    Sext,
    Sign,
    Shl,
    Shr,
    Sar,
    Rol,
    Ror,
    Imul,
    Mul,
    MulHi,
    ImulLo,
    ImulHi,
    Div,
    DivHi,
    Idiv,
    IdivHi,
    Bt,
}

const OPS: [Op; 51] = [
    Op::In,
    Op::Mulss,
    Op::Addss,
    Op::Subss,
    Op::Divss,
    Op::Arg,
    Op::Fn,
    Op::Guard,
    Op::Set,
    Op::Cmov,
    Op::Eq,
    Op::Mix,
    Op::Addr,
    Op::Load,
    Op::Comiss,
    Op::Minss,
    Op::Maxss,
    Op::Sqrtss,
    Op::Cvtsi2ss,
    Op::Cvttss2si,
    Op::Cvtss2sd,
    Op::Cvtsd2ss,
    Op::And,
    Op::Or,
    Op::Xor,
    Op::Add,
    Op::Adc,
    Op::Sub,
    Op::Sbb,
    Op::Inc,
    Op::Dec,
    Op::Neg,
    Op::KeepCf,
    Op::Not,
    Op::Sext,
    Op::Sign,
    Op::Shl,
    Op::Shr,
    Op::Sar,
    Op::Rol,
    Op::Ror,
    Op::Imul,
    Op::Mul,
    Op::MulHi,
    Op::ImulLo,
    Op::ImulHi,
    Op::Div,
    Op::DivHi,
    Op::Idiv,
    Op::IdivHi,
    Op::Bt,
];

#[derive(Clone, Copy)]
pub struct Node {
    op: Op,
    cond: u8,
    expect: bool,
    w: u16,
    a: i32,
    b: i32,
    c: i32,
    x: u32,
    y: u32,
    extra: u32,
    extra_len: u32,
}

/// Flags as x86 leaves them; 2 means undefined, and consuming an undefined flag leaves the path.
#[derive(Clone, Copy)]
struct Flags {
    cf: u8,
    zf: u8,
    sf: u8,
    of: u8,
    pf: u8,
}

const NO_FLAGS: Flags = Flags {
    cf: 2,
    zf: 2,
    sf: 2,
    of: 2,
    pf: 2,
};

pub struct Program {
    consts: Vec<u128>,
    nodes: Vec<Node>,
    root: Vec<u32>,
    written: Vec<(u64, u64)>,
    extra: Vec<i64>,
    slots: usize,
}

pub struct Formula {
    regions: Vec<(u64, u64, usize)>,
    arena: Vec<u8>,
    domain: Vec<(u64, u64, usize)>,
    pub dom_base: Vec<u8>,
    pub dom_known: Vec<u8>,
    mapped: Vec<u8>,
    tables: Vec<Vec<Program>>,
    mains: Vec<Program>,
}

/// One worker's reusable buffers.
pub struct Worker {
    val: Vec<u128>,
    fl: Vec<Flags>,
    pub dom: Vec<u8>,
    pub known: Vec<u8>,
    /// Failed path attempts, for profiling.
    pub left: u64,
    /// The path that answered last, per main list and per callee table: neighbouring outfits mostly take the same one.
    main_hint: usize,
    table_hint: Vec<usize>,
}

type Parse<T> = Result<T, &'static str>;

struct Lines<'a>(std::str::Lines<'a>);

impl<'a> Lines<'a> {
    fn next(&mut self) -> Parse<&'a str> {
        self.0.next().ok_or("program file ends early")
    }
    fn words(&mut self) -> Parse<Vec<&'a str>> {
        Ok(self.next()?.split_ascii_whitespace().collect())
    }
    fn count(&mut self) -> Parse<usize> {
        self.words()?
            .get(1)
            .and_then(|w| w.parse().ok())
            .ok_or("bad count line")
    }
}

fn hex<T: TryFrom<u128>>(w: &str) -> Parse<T> {
    u128::from_str_radix(w, 16)
        .ok()
        .and_then(|v| T::try_from(v).ok())
        .ok_or("bad hex number")
}

fn dec<T: std::str::FromStr>(w: &str) -> Parse<T> {
    w.parse().map_err(|_| "bad number")
}

fn hex_bytes(s: &str) -> Parse<Vec<u8>> {
    s.as_bytes()
        .chunks(2)
        .map(|p| {
            std::str::from_utf8(p)
                .ok()
                .and_then(|t| u8::from_str_radix(t, 16).ok())
                .ok_or("bad hex byte")
        })
        .collect()
}

fn read_program(l: &mut Lines) -> Parse<Program> {
    let h = l.words()?;
    if h.len() < 5 || h[0] != "program" {
        return Err("expected a program header");
    }
    let (n_nodes, n_ranges): (usize, usize) = (dec(h[2])?, dec(h[4])?);
    let consts = l
        .words()?
        .into_iter()
        .map(hex)
        .collect::<Parse<Vec<u128>>>()?;
    let root = l
        .words()?
        .into_iter()
        .map(dec)
        .collect::<Parse<Vec<u32>>>()?;
    let r = l.words()?;
    if r.len() < 2 * n_ranges {
        return Err("short written-range line");
    }
    let written = (0..n_ranges)
        .map(|i| Ok((hex(r[2 * i])?, hex(r[2 * i + 1])?)))
        .collect::<Parse<Vec<(u64, u64)>>>()?;
    let (mut nodes, mut extra) = (Vec::with_capacity(n_nodes), Vec::new());
    let mut slots = consts.len();
    for _ in 0..n_nodes {
        let t = l
            .words()?
            .into_iter()
            .map(dec)
            .collect::<Parse<Vec<i64>>>()?;
        if t.len() < 9 {
            return Err("short node");
        }
        let op = *OPS.get(t[0] as usize).ok_or("unknown operation")?;
        let start = extra.len() as u32;
        extra.extend_from_slice(&t[9..]);
        let node = Node {
            op,
            cond: t[5] as u8,
            expect: t[6] != 0,
            w: t[1] as u16,
            a: t[2] as i32,
            b: t[3] as i32,
            c: t[4] as i32,
            x: t[7] as u32,
            y: t[8] as u32,
            extra: start,
            extra_len: extra.len() as u32 - start,
        };
        slots += if op == Op::Fn { node.y as usize } else { 1 };
        nodes.push(node);
    }
    Ok(Program {
        consts,
        nodes,
        root,
        written,
        extra,
        slots,
    })
}

impl Formula {
    #[cfg(test)]
    pub(crate) fn audit_constant(bits: u32) -> Self {
        Self::parse(&format!(
            "regions 0\ndomain 0\n\n\ntables 0\nmains 1\nprogram 0 0 1 0\n{bits:x}\n0\n\n"
        )).unwrap()
    }

    #[cfg(test)]
    pub(crate) fn audit_word(address: u64, bits: u32) -> Self {
        let bytes = bits.to_le_bytes().iter().map(|b| format!("{b:02x}")).collect::<String>();
        Self::parse(&format!(
            "regions 0\ndomain 1\n{address:x} {:x}\n{bytes}\nffffffff\ntables 0\nmains 1\nprogram 0 1 0 0\n\n0\n\n0 32 -1 -1 -1 0 0 0 0\n",
            address + 4
        )).unwrap()
    }

    pub fn load(dir: &Path) -> Result<Formula, LoadError> {
        let fail = |name: &str, reason: String| LoadError {
            file: dir.join(name),
            reason,
        };
        let text = fs::read(dir.join("programs.txt"))
            .map_err(|e| fail("programs.txt", e.to_string()))?;
        let arena =
            fs::read(dir.join("arena.bin")).map_err(|e| fail("arena.bin", e.to_string()))?;
        let mapped = fs::read(dir.join("mapped.bin")).ok();
        Self::load_buffers(dir, &text, &arena, mapped.as_deref())
    }

    pub(crate) fn load_buffers(dir: &Path, text: &[u8], arena: &[u8], mapped: Option<&[u8]>) -> Result<Formula, LoadError> {
        let fail = |name: &str, reason: String| LoadError { file: dir.join(name), reason };
        let text = std::str::from_utf8(text).map_err(|e| fail("programs.txt", e.to_string()))?;
        let mut formula = Formula::parse(text).map_err(|r| fail("programs.txt", r.to_string()))?;
        formula.arena = arena.to_vec();
        if let Some(mapped) = mapped {
            if mapped.len() != formula.dom_base.len() {
                return Err(fail("mapped.bin", "size differs from the domain".into()));
            }
            formula.mapped = mapped.to_vec();
        }
        if formula
            .regions
            .iter()
            .any(|&(_, len, offset)| offset + 2 * len as usize > formula.arena.len())
        {
            return Err(fail("arena.bin", "region past the end".into()));
        }
        Ok(formula)
    }

    fn parse(text: &str) -> Parse<Formula> {
        let mut l = Lines(text.lines());
        let n = l.count()?;
        let mut regions = Vec::with_capacity(n);
        for _ in 0..n {
            let t = l.words()?;
            if t.len() < 3 {
                return Err("short region line");
            }
            regions.push((hex(t[0])?, dec(t[1])?, dec(t[2])?));
        }
        let mut domain = Vec::new();
        let mut at = 0usize;
        for _ in 0..l.count()? {
            let t = l.words()?;
            if t.len() < 2 {
                return Err("short domain line");
            }
            let (s, e): (u64, u64) = (hex(t[0])?, hex(t[1])?);
            domain.push((s, e, at));
            at += e.checked_sub(s).ok_or("empty domain span")? as usize;
        }
        let dom_base = hex_bytes(l.next()?)?;
        let dom_known = hex_bytes(l.next()?)?;
        if dom_base.len() != at || dom_known.len() != at {
            return Err("domain bytes do not match the spans");
        }
        let count = l.count()?;
        let mut tables = Vec::with_capacity(count);
        for _ in 0..count {
            let c = l.count()?;
            tables.push(
                (0..c)
                    .map(|_| read_program(&mut l))
                    .collect::<Parse<Vec<_>>>()?,
            );
        }
        let c = l.count()?;
        let mains = (0..c)
            .map(|_| read_program(&mut l))
            .collect::<Parse<Vec<_>>>()?;
        let mapped = vec![1; dom_base.len()];
        Ok(Formula {
            regions,
            arena: Vec::new(),
            domain,
            dom_base,
            dom_known,
            mapped,
            tables,
            mains,
        })
    }

    /// Start a candidate from the baseline bytes and lay these words over them. Bytes no captured region holds stay out, as in the VM.
    pub fn apply(&self, w: &mut Worker, writes: &[(u64, u32)]) {
        w.dom.copy_from_slice(&self.dom_base);
        w.known.copy_from_slice(&self.dom_known);
        self.overlay(w, writes);
    }

    /// Lay more words over the candidate already applied.
    pub fn overlay(&self, w: &mut Worker, writes: &[(u64, u32)]) {
        for &(address, value) in writes {
            let bytes = value.to_le_bytes();
            let Some(&(start, _, at)) =
                self.domain.iter().find(|d| d.0 <= address && address < d.1)
            else {
                continue;
            };
            let o = at + (address - start) as usize;
            for (i, byte) in bytes.into_iter().enumerate() {
                if self.mapped[o + i] != 0 {
                    w.dom[o + i] = byte;
                    w.known[o + i] = 255;
                }
            }
        }
    }

    pub fn worker(&self) -> Worker {
        let widest = self
            .mains
            .iter()
            .chain(self.tables.iter().flatten())
            .map(|p| p.slots)
            .sum::<usize>()
            * 3;
        Worker {
            val: vec![0; widest],
            fl: vec![NO_FLAGS; widest],
            dom: self.dom_base.clone(),
            known: self.dom_known.clone(),
            left: 0,
            main_hint: 0,
            table_hint: vec![0; self.tables.len()],
        }
    }

    /// Profiling only: per main program, (nodes, nodes that read a varying domain byte directly or through operands).
    /// Callees count once per call site, as tainted when any argument is.
    #[doc(hidden)]
    pub fn taint(&self, varying: &[bool]) -> Vec<(usize, usize, usize)> {
        fn walk(f: &Formula, p: &Program, varying: &[bool], args: &[bool]) -> (usize, usize, bool) {
            let mut t = vec![false; p.slots + 1];
            let (mut total, mut tainted) = (0usize, 0usize);
            let mut i = p.consts.len();
            for n in &p.nodes {
                let at = |k: i32| k >= 0 && t.get(k as usize).copied().unwrap_or(false);
                let mut d = at(n.a) || at(n.b) || at(n.c);
                match n.op {
                    Op::In => {
                        d = (n.x as usize..n.x as usize + (n.w as usize / 8))
                            .any(|b| varying.get(b).copied().unwrap_or(true))
                    }
                    Op::Arg => d = args.get(n.x as usize).copied().unwrap_or(true),
                    Op::Mix | Op::Addr => {
                        let list = &p.extra[n.extra as usize + 1..(n.extra + n.extra_len) as usize];
                        let step = if n.op == Op::Mix { 3 } else { 2 };
                        let pos = if n.op == Op::Mix { 1 } else { 0 };
                        d |= list.chunks(step).any(|q| at(q[pos] as i32));
                    }
                    Op::Load => d = d || true && at(n.a),
                    _ => {}
                }
                if n.op == Op::Fn {
                    let list = &p.extra[n.extra as usize..(n.extra + n.extra_len) as usize];
                    let a: Vec<bool> = list.iter().map(|&k| at(k as i32)).collect();
                    let sub = &f.tables[n.x as usize][0];
                    let (st, sd, out) = walk(f, sub, varying, &a);
                    total += st;
                    tainted += sd;
                    for k in 0..n.y as usize {
                        if let Some(x) = t.get_mut(i + k) {
                            *x = out;
                        }
                    }
                    i += n.y as usize;
                    continue;
                }
                total += 1;
                tainted += d as usize;
                if let Some(x) = t.get_mut(i) {
                    *x = d;
                }
                i += 1;
            }
            let out = p
                .root
                .iter()
                .any(|&r| t.get(r as usize).copied().unwrap_or(true));
            (total, tainted, out)
        }
        self.mains
            .iter()
            .map(|p| {
                let (a, b, _) = walk(self, p, varying, &[]);
                (p.nodes.len(), a, b)
            })
            .collect()
    }

    /// Profiling only: the domain offset of each written address byte.
    #[doc(hidden)]
    pub fn domain_offsets(&self, address: u64) -> Option<usize> {
        self.domain
            .iter()
            .find(|d| d.0 <= address && address < d.1)
            .map(|&(s, _, at)| at + (address - s) as usize)
    }

    #[doc(hidden)]
    pub fn domain_len(&self) -> usize {
        self.dom_base.len()
    }

    /// Output bits of the candidate held in `w.dom`, or None when no program covers it.
    pub fn run(&self, w: &mut Worker) -> Option<u32> {
        let first = w.main_hint;
        for k in std::iter::once(first).chain((0..self.mains.len()).filter(|&k| k != first)) {
            let Some(p) = self.mains.get(k) else { continue };
            if self.exec(p, w, 1, 0, 0).is_some() {
                w.main_hint = k;
                return Some(w.val[0] as u32);
            }
            w.left += 1;
        }
        None
    }

    #[inline]
    fn read(&self, w: &Worker, address: u64, size: usize) -> Option<u128> {
        for &(s, e, o) in &self.domain {
            if address >= s && address + size as u64 <= e {
                let at = o + (address - s) as usize;
                if w.known[at..at + size].iter().any(|&k| k != 255) {
                    return None;
                }
                let mut v = [0u8; 16];
                v[..size].copy_from_slice(&w.dom[at..at + size]);
                return Some(u128::from_le_bytes(v));
            }
        }
        let k = self
            .regions
            .partition_point(|r| r.0 <= address)
            .checked_sub(1)?;
        let (start, len, offset) = self.regions[k];
        let o = (address - start) as usize;
        if o + size > len as usize {
            return None;
        }
        let at = offset + o;
        if self.arena[at + len as usize..at + len as usize + size]
            .iter()
            .any(|&k| k != 255)
        {
            return None;
        }
        let mut v = [0u8; 16];
        v[..size].copy_from_slice(&self.arena[at..at + size]);
        Some(u128::from_le_bytes(v))
    }

    fn exec(
        &self,
        p: &Program,
        w: &mut Worker,
        base: usize,
        args: usize,
        out: usize,
    ) -> Option<()> {
        let frame_end = base + p.slots + p.root.len() + 1;
        w.val[base..base + p.consts.len()].copy_from_slice(&p.consts);
        let mut i = base + p.consts.len();
        for n in &p.nodes {
            let a = if n.a >= 0 {
                w.val[base + n.a as usize]
            } else {
                0
            };
            let b = if n.b >= 0 {
                w.val[base + n.b as usize]
            } else {
                0
            };
            let width = n.w as u32;
            let mask = if width >= 128 {
                u128::MAX
            } else {
                (1u128 << width) - 1
            };
            let result: u128 = match n.op {
                Op::In => {
                    let (at, size) = (n.x as usize, (width / 8) as usize);
                    if w.known[at..at + size].iter().any(|&k| k != 255) {
                        return None;
                    }
                    let mut v = [0u8; 16];
                    v[..size].copy_from_slice(&w.dom[at..at + size]);
                    u128::from_le_bytes(v)
                }
                Op::Mulss | Op::Addss | Op::Subss | Op::Divss => {
                    let (x, y) = (f(a), f(b));
                    if !x.is_finite() || !y.is_finite() || (n.op == Op::Divss && y == 0.0) {
                        return None;
                    }
                    let r = match n.op {
                        Op::Mulss => x * y,
                        Op::Addss => x + y,
                        Op::Subss => x - y,
                        _ => x / y,
                    };
                    if !r.is_finite() {
                        return None;
                    }
                    r.to_bits() as u128
                }
                Op::Arg => w.val[args + n.x as usize],
                Op::Fn => {
                    let list = &p.extra[n.extra as usize..(n.extra + n.extra_len) as usize];
                    for (k, &s) in list.iter().enumerate() {
                        w.val[frame_end + k] = w.val[base + s as usize];
                    }
                    let chunks = n.y as usize;
                    let mut done = false;
                    let table = &self.tables[n.x as usize];
                    let first = w.table_hint[n.x as usize];
                    for k in std::iter::once(first).chain((0..table.len()).filter(|&k| k != first))
                    {
                        let Some(sub) = table.get(k) else { continue };
                        if self
                            .exec(sub, w, frame_end + list.len(), frame_end, i)
                            .is_some()
                        {
                            w.table_hint[n.x as usize] = k;
                            done = true;
                            break;
                        }
                    }
                    if !done {
                        return None;
                    }
                    i += chunks;
                    continue;
                }
                Op::Guard | Op::Set | Op::Cmov => {
                    let c = condition(n.cond, w.fl[base + n.a as usize])?;
                    match n.op {
                        Op::Guard => {
                            if c != n.expect {
                                return None;
                            }
                            0
                        }
                        Op::Set => c as u128,
                        _ => (if c { b } else { w.val[base + n.c as usize] }) & mask,
                    }
                }
                Op::Eq => {
                    if a != b {
                        return None;
                    }
                    0
                }
                Op::Mix => {
                    let mut bytes = b.to_le_bytes();
                    let list = &p.extra[n.extra as usize + 1..(n.extra + n.extra_len) as usize];
                    for q in list.as_chunks::<3>().0 {
                        bytes[q[0] as usize] =
                            w.val[base + q[1] as usize].to_le_bytes()[q[2] as usize];
                    }
                    u128::from_le_bytes(bytes)
                }
                Op::Addr => {
                    let mut r = b as u64;
                    let list = &p.extra[n.extra as usize + 1..(n.extra + n.extra_len) as usize];
                    for q in list.as_chunks::<2>().0 {
                        r = r.wrapping_add(
                            (w.val[base + q[0] as usize] as u64).wrapping_mul(q[1] as u64),
                        );
                    }
                    r as u128
                }
                Op::Load => {
                    let (address, size) = (a as u64, n.x as usize);
                    let end = address + size as u64;
                    let k = p.written.partition_point(|r| r.1 <= address);
                    if k < p.written.len() && p.written[k].0 < end {
                        return None;
                    }
                    self.read(w, address, size)?
                }
                Op::Comiss => {
                    let (x, y) = (f(a), f(b));
                    if !x.is_finite() || !y.is_finite() {
                        return None;
                    }
                    w.fl[i] = Flags {
                        cf: (x < y) as u8,
                        zf: (x == y) as u8,
                        sf: 0,
                        of: 0,
                        pf: 0,
                    };
                    0
                }
                Op::Minss | Op::Maxss => {
                    let (x, y) = (f(a), f(b));
                    if !x.is_finite() || !y.is_finite() {
                        return None;
                    }
                    if n.op == Op::Minss {
                        if x < y {
                            a
                        } else {
                            b
                        }
                    } else if x > y {
                        a
                    } else {
                        b
                    }
                }
                Op::Sqrtss => {
                    let x = f(a);
                    if !x.is_finite() || x < 0.0 {
                        return None;
                    }
                    x.sqrt().to_bits() as u128
                }
                Op::Cvtsi2ss => {
                    let v = signed(a, n.x);
                    if v.unsigned_abs() >= 1 << 53 {
                        return None;
                    }
                    (v as f32).to_bits() as u128
                }
                Op::Cvttss2si => {
                    let x = f(a);
                    if !x.is_finite() || x.abs() as f64 >= 2f64.powi(width as i32 - 1) {
                        return None;
                    }
                    (x as i64 as u128) & mask
                }
                Op::Cvtss2sd => (f(a) as f64).to_bits() as u128,
                Op::Cvtsd2ss => (f64::from_bits(a as u64) as f32).to_bits() as u128,
                Op::And | Op::Or | Op::Xor => {
                    let y = b & mask;
                    let v = match n.op {
                        Op::And => a & y,
                        Op::Or => a | y,
                        _ => a ^ y,
                    };
                    let (z, s, p_) = zsp(v, width);
                    w.fl[i] = Flags {
                        cf: 0,
                        zf: z,
                        sf: s,
                        of: 0,
                        pf: p_,
                    };
                    v
                }
                Op::Add | Op::Adc | Op::Sub | Op::Sbb => {
                    let carry = if n.op == Op::Adc || n.op == Op::Sbb {
                        if n.c >= 0 {
                            let f = w.fl[base + n.c as usize].cf;
                            if f == 2 {
                                return None;
                            }
                            f as u128
                        } else {
                            n.y as u128
                        }
                    } else {
                        0
                    };
                    let (x, y) = (a & mask, b & mask);
                    let (v, cf, of);
                    if n.op == Op::Add || n.op == Op::Adc {
                        let raw = (x as u64 as u128)
                            .wrapping_add(y as u64 as u128)
                            .wrapping_add(carry);
                        v = raw & mask;
                        cf = raw > mask;
                        of = (!(x ^ y) & (x ^ v)) >> (width - 1) & 1 != 0;
                    } else {
                        v = x.wrapping_sub(y).wrapping_sub(carry) & mask;
                        cf = x < y + carry;
                        of = ((x ^ y) & (x ^ v)) >> (width - 1) & 1 != 0;
                    }
                    let (z, s, p_) = zsp(v, width);
                    w.fl[i] = Flags {
                        cf: cf as u8,
                        zf: z,
                        sf: s,
                        of: of as u8,
                        pf: p_,
                    };
                    v
                }
                Op::Inc | Op::Dec => {
                    let x = a & mask;
                    let v = (if n.op == Op::Inc {
                        x + 1
                    } else {
                        x.wrapping_sub(1)
                    }) & mask;
                    let cf = if n.c >= 0 {
                        w.fl[base + n.c as usize].cf
                    } else {
                        n.y as u8
                    };
                    let of = if n.op == Op::Inc {
                        (!(x ^ 1) & (x ^ v)) >> (width - 1) & 1
                    } else {
                        ((x ^ 1) & (x ^ v)) >> (width - 1) & 1
                    };
                    let (z, s, p_) = zsp(v, width);
                    w.fl[i] = Flags {
                        cf,
                        zf: z,
                        sf: s,
                        of: of as u8,
                        pf: p_,
                    };
                    v
                }
                Op::Neg => {
                    let x = a & mask;
                    let v = x.wrapping_neg() & mask;
                    let (z, s, p_) = zsp(v, width);
                    w.fl[i] = Flags {
                        cf: (x != 0) as u8,
                        zf: z,
                        sf: s,
                        of: (((x & v) >> (width - 1)) & 1) as u8,
                        pf: p_,
                    };
                    v
                }
                Op::KeepCf => {
                    w.fl[i] = Flags {
                        cf: w.fl[base + n.a as usize].cf,
                        ..NO_FLAGS
                    };
                    0
                }
                Op::Not => a ^ mask,
                Op::Sext => (signed(a, n.x) as u128) & mask,
                Op::Sign => {
                    if signed(a, width) < 0 {
                        mask
                    } else {
                        0
                    }
                }
                Op::Shl | Op::Shr | Op::Sar => {
                    let count = n.x;
                    let x = a & mask;
                    let (v, cf);
                    if n.op == Op::Shl {
                        v = (x << count) & mask;
                        cf = x >> (width - count) & 1 != 0;
                    } else {
                        v = (if n.op == Op::Sar {
                            (signed(x, width) >> count) as u128
                        } else {
                            x >> count
                        }) & mask;
                        cf = x >> (count - 1) & 1 != 0;
                    }
                    let (z, s, p_) = zsp(v, width);
                    let of = if count != 1 {
                        2
                    } else if n.op == Op::Shl {
                        ((v >> (width - 1) & 1 != 0) != cf) as u8
                    } else if n.op == Op::Shr {
                        (x >> (width - 1) & 1) as u8
                    } else {
                        0
                    };
                    w.fl[i] = Flags {
                        cf: cf as u8,
                        zf: z,
                        sf: s,
                        of,
                        pf: p_,
                    };
                    v
                }
                Op::Rol | Op::Ror => {
                    let count = n.x % width;
                    let x = a & mask;
                    let v = if count == 0 {
                        x
                    } else if n.op == Op::Rol {
                        ((x << count) | (x >> (width - count))) & mask
                    } else {
                        ((x >> count) | (x << (width - count))) & mask
                    };
                    let cf = if n.op == Op::Rol {
                        v & 1
                    } else {
                        v >> (width - 1) & 1
                    };
                    w.fl[i] = Flags {
                        cf: cf as u8,
                        ..NO_FLAGS
                    };
                    v
                }
                Op::Imul => {
                    let prod = signed(a & mask, width).wrapping_mul(signed(b & mask, width));
                    let v = prod as u128 & mask;
                    let over = prod != signed(v, width);
                    w.fl[i] = Flags {
                        cf: over as u8,
                        of: over as u8,
                        ..NO_FLAGS
                    };
                    v
                }
                Op::Mul | Op::MulHi | Op::ImulLo | Op::ImulHi => {
                    let third = w.val[base + n.c as usize];
                    let prod = if n.op == Op::Mul || n.op == Op::MulHi {
                        a.wrapping_mul(third)
                    } else {
                        signed(a, width).wrapping_mul(signed(third, width)) as u128
                    };
                    (if n.op == Op::MulHi || n.op == Op::ImulHi {
                        prod >> width
                    } else {
                        prod
                    }) & mask
                }
                Op::Div | Op::DivHi | Op::Idiv | Op::IdivHi => {
                    let third = w.val[base + n.c as usize];
                    if third == 0 || width == 64 && b != 0 {
                        return None; // a 128-bit dividend stays with the VM
                    }
                    let top = (b << width) | a;
                    let (q, r);
                    if n.op == Op::Div || n.op == Op::DivHi {
                        q = top / third;
                        r = top % third;
                        if q > mask {
                            return None;
                        }
                    } else {
                        let (t, d) = (signed(top, 2 * width), signed(third, width));
                        let (qq, rr) = (t / d, t % d);
                        if qq < -(1i128 << (width - 1)) || qq >= 1i128 << (width - 1) {
                            return None;
                        }
                        q = qq as u128;
                        r = rr as u128;
                    }
                    (if n.op == Op::DivHi || n.op == Op::IdivHi {
                        r
                    } else {
                        q
                    }) & mask
                }
                Op::Bt => {
                    let bit = n.x;
                    w.fl[i] = Flags {
                        cf: (a >> bit & 1) as u8,
                        ..NO_FLAGS
                    };
                    match n.y {
                        1 => a | 1 << bit,
                        0 => a & !(1 << bit),
                        _ => a,
                    }
                }
            };
            w.val[i] = result;
            i += 1;
        }
        for (k, &r) in p.root.iter().enumerate() {
            w.val[out + k] = w.val[base + r as usize];
        }
        Some(())
    }
}

#[inline(always)]
fn f(u: u128) -> f32 {
    f32::from_bits(u as u32)
}

#[inline(always)]
fn signed(v: u128, w: u32) -> i128 {
    if w >= 128 {
        return v as i128;
    }
    let shift = 128 - w;
    ((v << shift) as i128) >> shift
}

#[inline(always)]
fn zsp(v: u128, w: u32) -> (u8, u8, u8) {
    (
        (v == 0) as u8,
        (v >> (w - 1) & 1) as u8,
        (v as u8).count_ones().is_multiple_of(2) as u8,
    )
}

#[inline(always)]
fn condition(code: u8, f: Flags) -> Option<bool> {
    let g = |x: u8| if x == 2 { None } else { Some(x != 0) };
    Some(match code {
        0 => g(f.zf)?,
        1 => !g(f.zf)?,
        2 => g(f.cf)?,
        3 => !g(f.cf)?,
        4 => g(f.cf)? || g(f.zf)?,
        5 => !g(f.cf)? && !g(f.zf)?,
        6 => g(f.sf)?,
        7 => !g(f.sf)?,
        8 => g(f.sf)? != g(f.of)?,
        9 => g(f.sf)? == g(f.of)?,
        10 => g(f.zf)? || g(f.sf)? != g(f.of)?,
        11 => !g(f.zf)? && g(f.sf)? == g(f.of)?,
        12 => g(f.pf)?,
        _ => !g(f.pf)?,
    })
}
