//! Runs the guarded programs exported by path_export.py. A run either reproduces the VM on that path or reports
//! that the candidate left it; it never guesses. Floating point is native binary32, as in the game.

pub mod pipeline;

use std::fs;
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(u8)]
#[allow(non_camel_case_types)]
pub enum Op {
    In, Mulss, Addss, Subss, Divss, Arg, Fn, Guard, Set, Cmov, Eq, Mix, Addr, Load, Comiss, Minss, Maxss, Sqrtss, Cvtsi2ss, Cvttss2si,
    Cvtss2sd, Cvtsd2ss, And, Or, Xor, Add, Adc, Sub, Sbb, Inc, Dec, Neg, KeepCf, Not, Sext, Sign, Shl, Shr, Sar, Rol, Ror, Imul, Mul,
    MulHi, ImulLo, ImulHi, Div, DivHi, Idiv, IdivHi, Bt,
}

const OPS: [Op; 51] = [
    Op::In, Op::Mulss, Op::Addss, Op::Subss, Op::Divss, Op::Arg, Op::Fn, Op::Guard, Op::Set, Op::Cmov, Op::Eq, Op::Mix, Op::Addr, Op::Load,
    Op::Comiss, Op::Minss, Op::Maxss, Op::Sqrtss, Op::Cvtsi2ss, Op::Cvttss2si, Op::Cvtss2sd, Op::Cvtsd2ss, Op::And, Op::Or, Op::Xor, Op::Add,
    Op::Adc, Op::Sub, Op::Sbb, Op::Inc, Op::Dec, Op::Neg, Op::KeepCf, Op::Not, Op::Sext, Op::Sign, Op::Shl, Op::Shr, Op::Sar, Op::Rol, Op::Ror,
    Op::Imul, Op::Mul, Op::MulHi, Op::ImulLo, Op::ImulHi, Op::Div, Op::DivHi, Op::Idiv, Op::IdivHi, Op::Bt,
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

const NO_FLAGS: Flags = Flags { cf: 2, zf: 2, sf: 2, of: 2, pf: 2 };

pub struct Program {
    consts: Vec<u128>,
    nodes: Vec<Node>,
    root: Vec<u32>,
    written: Vec<(u64, u64)>,
    extra: Vec<i64>,
    slots: usize,
}

pub struct Bundle {
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
    pub left: u64,
}

struct Lines<'a>(std::str::Lines<'a>);

impl<'a> Lines<'a> {
    fn next(&mut self) -> &'a str {
        self.0.next().expect("truncated program file")
    }
    fn words(&mut self) -> Vec<&'a str> {
        self.next().split_ascii_whitespace().collect()
    }
}

fn hex_bytes(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let v = |c: u8| (c as char).to_digit(16).unwrap() as u8;
    b.chunks(2).map(|p| v(p[0]) << 4 | v(p[1])).collect()
}

fn read_program(l: &mut Lines) -> Program {
    let h = l.words();
    assert_eq!(h[0], "program");
    let (n_nodes, n_ranges): (usize, usize) = (h[2].parse().unwrap(), h[4].parse().unwrap());
    let consts: Vec<u128> = l.words().iter().map(|w| u128::from_str_radix(w, 16).unwrap()).collect();
    let root: Vec<u32> = l.words().iter().map(|w| w.parse().unwrap()).collect();
    let r = l.words();
    let written = (0..n_ranges).map(|i| (u64::from_str_radix(r[2 * i], 16).unwrap(), u64::from_str_radix(r[2 * i + 1], 16).unwrap())).collect();
    let (mut nodes, mut extra) = (Vec::with_capacity(n_nodes), Vec::new());
    let mut slots = consts.len();
    for _ in 0..n_nodes {
        let t = l.words();
        let n = |i: usize| t[i].parse::<i64>().unwrap();
        let op = OPS[n(0) as usize];
        let start = extra.len() as u32;
        extra.extend(t[9..].iter().map(|w| w.parse::<i64>().unwrap()));
        let node = Node {
            op,
            cond: n(5) as u8,
            expect: n(6) != 0,
            w: n(1) as u16,
            a: n(2) as i32,
            b: n(3) as i32,
            c: n(4) as i32,
            x: n(7) as u32,
            y: n(8) as u32,
            extra: start,
            extra_len: extra.len() as u32 - start,
        };
        slots += if op == Op::Fn { node.y as usize } else { 1 };
        nodes.push(node);
    }
    Program { consts, nodes, root, written, extra, slots }
}

impl Bundle {
    pub fn load(dir: &Path) -> Bundle {
        let text = fs::read_to_string(dir.join("programs.txt")).unwrap();
        let mut l = Lines(text.lines());
        let n: usize = l.words()[1].parse().unwrap();
        let mut regions = Vec::with_capacity(n);
        for _ in 0..n {
            let t = l.words();
            regions.push((u64::from_str_radix(t[0], 16).unwrap(), t[1].parse().unwrap(), t[2].parse().unwrap()));
        }
        let h = l.words();
        let mut domain = Vec::new();
        let mut at = 0usize;
        for _ in 0..h[1].parse::<usize>().unwrap() {
            let t = l.words();
            let (s, e) = (u64::from_str_radix(t[0], 16).unwrap(), u64::from_str_radix(t[1], 16).unwrap());
            domain.push((s, e, at));
            at += (e - s) as usize;
        }
        let dom_base = hex_bytes(l.next());
        let dom_known = hex_bytes(l.next());
        let count: usize = l.words()[1].parse().unwrap();
        let mut tables = Vec::with_capacity(count);
        for _ in 0..count {
            let c: usize = l.words()[1].parse().unwrap();
            tables.push((0..c).map(|_| read_program(&mut l)).collect());
        }
        let c: usize = l.words()[1].parse().unwrap();
        let mains = (0..c).map(|_| read_program(&mut l)).collect();
        let mapped = fs::read(dir.join("mapped.bin")).unwrap_or_else(|_| vec![1; dom_base.len()]);
        Bundle { regions, arena: fs::read(dir.join("arena.bin")).unwrap(), domain, dom_base, dom_known, mapped, tables, mains }
    }

    /// Start a candidate from the baseline bytes and lay these words over them. Bytes no captured region holds stay out, as in the VM.
    pub fn apply(&self, w: &mut Worker, writes: &[(u64, u32)]) {
        w.dom.copy_from_slice(&self.dom_base);
        w.known.copy_from_slice(&self.dom_known);
        for &(address, value) in writes {
            let bytes = value.to_le_bytes();
            let Some(&(start, _, at)) = self.domain.iter().find(|d| d.0 <= address && address < d.1) else { continue };
            let o = at + (address - start) as usize;
            for i in 0..4 {
                if self.mapped[o + i] != 0 {
                    w.dom[o + i] = bytes[i];
                    w.known[o + i] = 255;
                }
            }
        }
    }

    pub fn worker(&self) -> Worker {
        let widest = self.mains.iter().chain(self.tables.iter().flatten()).map(|p| p.slots).sum::<usize>() * 3;
        Worker { val: vec![0; widest], fl: vec![NO_FLAGS; widest], dom: self.dom_base.clone(), known: self.dom_known.clone(), left: 0 }
    }

    pub fn sizes(&self) -> (usize, Vec<usize>, usize) {
        (self.mains.len(), self.tables.iter().map(Vec::len).collect(), self.mains.first().map_or(0, |p| p.nodes.len()))
    }

    /// Output bits of the candidate held in `w.dom`, or None when no program covers it.
    pub fn run(&self, w: &mut Worker) -> Option<u32> {
        for p in &self.mains {
            if self.exec(p, w, 1, 0, 0).is_some() {
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
        let k = self.regions.partition_point(|r| r.0 <= address).checked_sub(1)?;
        let (start, len, offset) = self.regions[k];
        let o = (address - start) as usize;
        if o + size > len as usize {
            return None;
        }
        let at = offset + o;
        if self.arena[at + len as usize..at + len as usize + size].iter().any(|&k| k != 255) {
            return None;
        }
        let mut v = [0u8; 16];
        v[..size].copy_from_slice(&self.arena[at..at + size]);
        Some(u128::from_le_bytes(v))
    }

    fn exec(&self, p: &Program, w: &mut Worker, base: usize, args: usize, out: usize) -> Option<()> {
        let frame_end = base + p.slots + p.root.len() + 1;
        w.val[base..base + p.consts.len()].copy_from_slice(&p.consts);
        let mut i = base + p.consts.len();
        for n in &p.nodes {
            let a = if n.a >= 0 { w.val[base + n.a as usize] } else { 0 };
            let b = if n.b >= 0 { w.val[base + n.b as usize] } else { 0 };
            let width = n.w as u32;
            let mask = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 };
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
                    for sub in &self.tables[n.x as usize] {
                        if self.exec(sub, w, frame_end + list.len(), frame_end, i).is_some() {
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
                    for q in list.chunks_exact(3) {
                        bytes[q[0] as usize] = w.val[base + q[1] as usize].to_le_bytes()[q[2] as usize];
                    }
                    u128::from_le_bytes(bytes)
                }
                Op::Addr => {
                    let mut r = b as u64;
                    let list = &p.extra[n.extra as usize + 1..(n.extra + n.extra_len) as usize];
                    for q in list.chunks_exact(2) {
                        r = r.wrapping_add((w.val[base + q[0] as usize] as u64).wrapping_mul(q[1] as u64));
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
                    w.fl[i] = Flags { cf: (x < y) as u8, zf: (x == y) as u8, sf: 0, of: 0, pf: 0 };
                    0
                }
                Op::Minss | Op::Maxss => {
                    let (x, y) = (f(a), f(b));
                    if !x.is_finite() || !y.is_finite() {
                        return None;
                    }
                    if n.op == Op::Minss {
                        if x < y { a } else { b }
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
                    w.fl[i] = Flags { cf: 0, zf: z, sf: s, of: 0, pf: p_ };
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
                        let raw = (x as u64 as u128).wrapping_add(y as u64 as u128).wrapping_add(carry);
                        v = raw & mask;
                        cf = raw > mask;
                        of = (!(x ^ y) & (x ^ v)) >> (width - 1) & 1 != 0;
                    } else {
                        v = x.wrapping_sub(y).wrapping_sub(carry) & mask;
                        cf = x < y + carry;
                        of = ((x ^ y) & (x ^ v)) >> (width - 1) & 1 != 0;
                    }
                    let (z, s, p_) = zsp(v, width);
                    w.fl[i] = Flags { cf: cf as u8, zf: z, sf: s, of: of as u8, pf: p_ };
                    v
                }
                Op::Inc | Op::Dec => {
                    let x = a & mask;
                    let v = (if n.op == Op::Inc { x + 1 } else { x.wrapping_sub(1) }) & mask;
                    let cf = if n.c >= 0 { w.fl[base + n.c as usize].cf } else { n.y as u8 };
                    let of = if n.op == Op::Inc { (!(x ^ 1) & (x ^ v)) >> (width - 1) & 1 } else { ((x ^ 1) & (x ^ v)) >> (width - 1) & 1 };
                    let (z, s, p_) = zsp(v, width);
                    w.fl[i] = Flags { cf, zf: z, sf: s, of: of as u8, pf: p_ };
                    v
                }
                Op::Neg => {
                    let x = a & mask;
                    let v = x.wrapping_neg() & mask;
                    let (z, s, p_) = zsp(v, width);
                    w.fl[i] = Flags { cf: (x != 0) as u8, zf: z, sf: s, of: (((x & v) >> (width - 1)) & 1) as u8, pf: p_ };
                    v
                }
                Op::KeepCf => {
                    w.fl[i] = Flags { cf: w.fl[base + n.a as usize].cf, ..NO_FLAGS };
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
                        v = (if n.op == Op::Sar { (signed(x, width) >> count) as u128 } else { x >> count }) & mask;
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
                    w.fl[i] = Flags { cf: cf as u8, zf: z, sf: s, of, pf: p_ };
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
                    let cf = if n.op == Op::Rol { v & 1 } else { v >> (width - 1) & 1 };
                    w.fl[i] = Flags { cf: cf as u8, ..NO_FLAGS };
                    v
                }
                Op::Imul => {
                    let prod = signed(a & mask, width).wrapping_mul(signed(b & mask, width));
                    let v = prod as u128 & mask;
                    let over = prod != signed(v, width);
                    w.fl[i] = Flags { cf: over as u8, of: over as u8, ..NO_FLAGS };
                    v
                }
                Op::Mul | Op::MulHi | Op::ImulLo | Op::ImulHi => {
                    let third = w.val[base + n.c as usize];
                    let prod = if n.op == Op::Mul || n.op == Op::MulHi {
                        a.wrapping_mul(third)
                    } else {
                        signed(a, width).wrapping_mul(signed(third, width)) as u128
                    };
                    (if n.op == Op::MulHi || n.op == Op::ImulHi { prod >> width } else { prod }) & mask
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
                    (if n.op == Op::DivHi || n.op == Op::IdivHi { r } else { q }) & mask
                }
                Op::Bt => {
                    let bit = n.x;
                    w.fl[i] = Flags { cf: (a >> bit & 1) as u8, ..NO_FLAGS };
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
    ((v == 0) as u8, (v >> (w - 1) & 1) as u8, ((v as u8).count_ones() % 2 == 0) as u8)
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
