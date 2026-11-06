//! Attributes in, damage out, entirely in Rust: pipeline, frame writes, compiled formula. argv: <export dir> [threads]

use path_runner::pipeline::{Frame, Scratch, Tables};
use path_runner::Bundle;
use std::path::Path;
use std::time::Instant;

struct Case {
    character: usize,
    attrs: [i32; 7],
    gain: u32,
    defense: u32,
    expected: u32,
    ops: Vec<(u16, i64)>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let threads: usize = args.get(2).map_or(16, |s| s.parse().unwrap());
    let dir = Path::new(&args[1]);
    let t = Instant::now();
    let bundle = Bundle::load(dir);
    let tables = Tables::load(&dir.join("tables.bin"));
    let frame = Frame::load(&dir.join("frame.bin"));
    let characters = Frame::characters(&dir.join("characters.bin"));
    println!("loaded in {:.2?}", t.elapsed());
    let text = std::fs::read_to_string(dir.join("end_to_end.txt")).unwrap();
    let cases: Vec<Case> = text
        .lines()
        .map(|l| {
            let mut w = l.split_ascii_whitespace();
            let character = w.next().unwrap().parse().unwrap();
            let mut attrs = [0; 7];
            for a in &mut attrs {
                *a = w.next().unwrap().parse().unwrap();
            }
            let gain = w.next().unwrap().parse().unwrap();
            let defense = w.next().unwrap().parse().unwrap();
            let expected = u32::from_str_radix(w.next().unwrap(), 16).unwrap();
            let ops = w
                .map(|p| {
                    let (g, v) = p.split_once(':').unwrap();
                    (g.parse().unwrap(), v.parse().unwrap())
                })
                .collect();
            Case { character, attrs, gain, defense, expected, ops }
        })
        .collect();

    let evaluate = |c: &Case, s: &mut Scratch, w: &mut path_runner::Worker| -> Option<u32> {
        let ch = &characters[c.character];
        tables.pipeline(&frame, ch, &c.attrs, &c.ops, s).ok()?;
        tables.frame_writes(&frame, ch, &c.attrs, c.gain, c.defense, s).ok()?;
        bundle.apply(w, &s.writes);
        bundle.run(w)
    };
    let (mut s, mut w) = (Scratch::new(), bundle.worker());
    let (mut same, mut wrong, mut uncovered) = (0, 0, 0);
    for c in &cases {
        match evaluate(c, &mut s, &mut w) {
            Some(bits) if bits == c.expected => same += 1,
            Some(bits) => {
                wrong += 1;
                if wrong < 6 {
                    println!("DIFF {:?} {bits:08x} expected {:08x}", c.attrs, c.expected);
                }
            }
            None => uncovered += 1,
        }
    }
    println!("cases {} same {same} wrong {wrong} uncovered {uncovered}", cases.len());
    for count in [1usize, threads] {
        let rounds = 40_000usize;
        let t = Instant::now();
        std::thread::scope(|scope| {
            for k in 0..count {
                let (cases, evaluate) = (&cases, &evaluate);
                let bundle = &bundle;
                scope.spawn(move || {
                    let (mut s, mut w) = (Scratch::new(), bundle.worker());
                    for i in 0..rounds {
                        std::hint::black_box(evaluate(&cases[(i + k * 97) % cases.len()], &mut s, &mut w));
                    }
                });
            }
        });
        let seconds = t.elapsed().as_secs_f64();
        println!("{count} thread(s): {:.1} us per candidate, {:.0} candidates per second", seconds / rounds as f64 * 1e6, (rounds * count) as f64 / seconds);
    }
}
