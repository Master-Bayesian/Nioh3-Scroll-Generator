//! Write the decrypted body of a save for offline inspection.
//!
//! Usage: `cargo run --example save_plain -- <SAVEDATA.BIN> <out.bin>`

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let save = args
        .next()
        .ok_or("usage: save_plain <SAVEDATA.BIN> <out.bin>")?;
    let out = args.next().ok_or("missing output path")?;
    let plain = nioh3_save::crypto::decrypt_container(&std::fs::read(save)?)?;
    std::fs::write(out, plain)?;
    Ok(())
}
