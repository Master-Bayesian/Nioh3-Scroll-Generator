//! Compare the Rust pipeline with the Python model's answers. argv: <dir>

use path_runner::pipeline::{Frame, Scratch, Tables};
use std::path::Path;
use std::time::Instant;

fn main() {
    let dir = std::env::args().nth(1).unwrap();
    let dir = Path::new(&dir);
    let t = Instant::now();
    let tables = Tables::load(&dir.join("tables.bin"));
    let frame = Frame::load(&dir.join("frame.bin"));
    let characters = Frame::characters(&dir.join("characters.bin"));
    println!("loaded in {:.2?}, characters {}", t.elapsed(), characters.len());
    let text = std::fs::read_to_string(dir.join("vectors.txt")).unwrap();
    let expect = std::fs::read(dir.join("expect.bin")).unwrap();
    let mut at = 0usize;
    let mut s = Scratch::new();
    let (mut same, mut wrong, mut both_refuse, mut only_python, mut only_rust) = (0, 0, 0, 0, 0);
    let mut spent = std::time::Duration::ZERO;
    let mut runs = 0u32;
    for (k, line) in text.lines().enumerate() {
        let mut w = line.split_ascii_whitespace();
        let c: usize = w.next().unwrap().parse().unwrap();
        let mut attrs = [0i32; 7];
        for a in &mut attrs {
            *a = w.next().unwrap().parse().unwrap();
        }
        let gain: u32 = w.next().unwrap().parse().unwrap();
        let defense: u32 = w.next().unwrap().parse().unwrap();
        let ops: Vec<(u16, i64)> = w
            .map(|p| {
                let (g, v) = p.split_once(':').unwrap();
                (g.parse().unwrap(), v.parse().unwrap())
            })
            .collect();
        let status = expect[at];
        at += 1;
        let mut ch = characters[c].clone();
        ch.inv.copy_from_slice(&expect[at..at + 0x2018]);
        at += 0x2018;
        let t = Instant::now();
        let got = tables.pipeline(&frame, &ch, &attrs, &ops, &mut s).and_then(|_| tables.frame_writes(&frame, &ch, &attrs, gain, defense, &mut s));
        spent += t.elapsed();
        runs += 1;
        if status != 0 {
            match got {
                Err(_) => both_refuse += 1,
                Ok(_) => {
                    only_python += 1;
                    if only_python < 6 {
                        println!("vector {k}: python refused, rust answered");
                    }
                }
            }
            continue;
        }
        let (st, d) = (&expect[at..at + 0x900], &expect[at + 0x900..at + 0x1400]);
        at += 0x1400;
        let n = u32::from_le_bytes(expect[at..at + 4].try_into().unwrap()) as usize;
        at += 4;
        let mut want: Vec<(u64, u32)> = expect[at..at + 12 * n]
            .chunks_exact(12)
            .map(|r| (u64::from_le_bytes(r[..8].try_into().unwrap()), u32::from_le_bytes(r[8..].try_into().unwrap())))
            .collect();
        at += 12 * n;
        if let Err(e) = got {
            only_rust += 1;
            if only_rust < 6 {
                println!("vector {k}: rust refused: {e}");
            }
            continue;
        }
        let mut bad = false;
        for (name, a, b) in [("stat", &s.stat[..], st), ("d", &s.d[..], d)] {
            if a != b {
                bad = true;
                if wrong < 6 {
                    let first = (0..a.len()).step_by(4).filter(|&i| a[i..i + 4] != b[i..i + 4]).take(6).collect::<Vec<_>>();
                    print!("vector {k} {name} differs at");
                    for i in first {
                        print!(" {i:#x}: {:02x?} want {:02x?}", &a[i..i + 4], &b[i..i + 4]);
                    }
                    println!();
                }
            }
        }
        want.sort_unstable();
        let mut mine = s.writes.clone();
        mine.sort_unstable();
        mine.dedup(); // the current-player STAT can be the source STAT, so one weight word is written twice
        if mine != want {
            bad = true;
            if wrong < 6 {
                let only: Vec<_> = mine.iter().filter(|x| !want.contains(x)).take(5).collect();
                let miss: Vec<_> = want.iter().filter(|x| !mine.contains(x)).take(5).collect();
                println!("vector {k} writes differ: rust {} python {} only-rust {only:x?} only-python {miss:x?}", mine.len(), want.len());
            }
        }
        if bad {
            wrong += 1;
        } else {
            same += 1;
        }
    }
    println!(
        "same {same} wrong {wrong} both refused {both_refuse} refused only by python {only_python} refused only by rust {only_rust}; {:.1} us per candidate",
        spent.as_secs_f64() / runs as f64 * 1e6
    );
}
