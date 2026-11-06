//! Replay exported test vectors bit for bit, then measure throughput. argv: <export dir> [threads]

use path_runner::Bundle;
use std::path::Path;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let threads: usize = args.get(2).map_or(16, |s| s.parse().unwrap());
    let dir = Path::new(&args[1]);
    let t = Instant::now();
    let bundle = Bundle::load(dir);
    println!("loaded in {:.2?}: {:?}", t.elapsed(), bundle.sizes());
    let text = std::fs::read_to_string(dir.join("vectors.txt")).unwrap();
    let vectors: Vec<(u32, Vec<(u32, u8)>)> = text
        .lines()
        .map(|l| {
            let mut w = l.split_ascii_whitespace();
            let expected = u32::from_str_radix(w.next().unwrap(), 16).unwrap();
            let writes = w
                .map(|p| {
                    let (a, b) = p.split_once(':').unwrap();
                    (a.parse().unwrap(), b.parse().unwrap())
                })
                .collect();
            (expected, writes)
        })
        .collect();

    let mut worker = bundle.worker();
    let (mut same, mut wrong, mut uncovered) = (0, 0, 0);
    for (expected, writes) in &vectors {
        worker.dom.copy_from_slice(&bundle.dom_base);
        worker.known.copy_from_slice(&bundle.dom_known);
        for &(a, b) in writes {
            worker.dom[a as usize] = b;
            worker.known[a as usize] = 255;
        }
        match bundle.run(&mut worker) {
            Some(bits) if bits == *expected => same += 1,
            Some(bits) => {
                wrong += 1;
                if wrong < 6 {
                    println!("DIFF {bits:08x} expected {expected:08x}");
                }
            }
            None => uncovered += 1,
        }
    }
    println!("vectors {} same {same} wrong {wrong} uncovered {uncovered} failed paths {}", vectors.len(), worker.left);

    for &count in &[1usize, threads] {
        let rounds = 40_000usize;
        let t = Instant::now();
        std::thread::scope(|s| {
            for k in 0..count {
                let (bundle, vectors) = (&bundle, &vectors);
                s.spawn(move || {
                    let mut w = bundle.worker();
                    for i in 0..rounds {
                        let (_, writes) = &vectors[(i + k * 97) % vectors.len()];
                        w.dom.copy_from_slice(&bundle.dom_base);
                        w.known.copy_from_slice(&bundle.dom_known);
                        for &(a, b) in writes {
                            w.dom[a as usize] = b;
                            w.known[a as usize] = 255;
                        }
                        std::hint::black_box(bundle.run(&mut w));
                    }
                });
            }
        });
        let total = (rounds * count) as f64;
        let s = t.elapsed().as_secs_f64();
        println!("{count} thread(s): {:.1} us per candidate, {:.0} candidates per second", s / rounds as f64 * 1e6, total / s);
    }
}
