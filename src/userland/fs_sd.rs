//! `fs` shell commands — persistent files on the boot SD card (FAT32 root dir).
//!
//! Mounts the SD per operation (no cached handle) via
//! [`crate::board::zynq7000::sd`] + [`crate::board::zynq7000::fat`]. 8.3 names
//! only, root directory only — enough to make the Golden Phase shell a
//! *usable* utility OS: files survive a power-cycle.

use crate::board::zynq7000::fat::{Fat32, RootEntry};
use crate::board::zynq7000::sd::SdCard;
use crate::platform::String;

/// Convert a shell filename (`hello.txt`) to the padded 8.3 form (`HELLO   TXT`).
pub fn name_to_83(name: &str) -> [u8; 11] {
    let mut out = [b' '; 11];
    let (base, ext) = match name.rsplit_once('.') {
        Some((b, e)) => (b, e),
        None => (name, ""),
    };
    for (i, c) in base.bytes().take(8).enumerate() {
        out[i] = c.to_ascii_uppercase();
    }
    for (i, c) in ext.bytes().take(3).enumerate() {
        out[8 + i] = c.to_ascii_uppercase();
    }
    out
}

fn name_from_83(n: &[u8; 11]) -> String {
    let base = core::str::from_utf8(&n[0..8]).unwrap_or("").trim_end();
    let ext = core::str::from_utf8(&n[8..11]).unwrap_or("").trim_end();
    if ext.is_empty() {
        base.into()
    } else {
        alloc::format!("{base}.{ext}")
    }
}

fn mount() -> Result<(SdCard, Fat32), &'static str> {
    let mut sd = SdCard::init().map_err(|_| "fs: SD init failed")?;
    let fat = Fat32::mount(&mut sd).map_err(|_| "fs: no FAT32 volume")?;
    Ok((sd, fat))
}

/// `fs save <name> <text>` — write `data` to the SD file `name`.
pub fn save(name: &str, data: &[u8]) -> String {
    let (mut sd, fat) = match mount() {
        Ok(v) => v,
        Err(e) => return e.into(),
    };
    match fat.write_root_file(&mut sd, &name_to_83(name), data) {
        Ok(()) => alloc::format!("fs: saved {} ({} bytes) to SD", name, data.len()),
        Err(e) => alloc::format!("fs: save failed ({e:?})"),
    }
}

/// `fs cat <name>` — read the SD file `name` and return its contents.
pub fn cat(name: &str) -> String {
    let (mut sd, fat) = match mount() {
        Ok(v) => v,
        Err(e) => return e.into(),
    };
    let n83 = name_to_83(name);
    let size = match fat.find_in_root(&mut sd, &n83) {
        Ok((_, sz)) => sz as usize,
        Err(_) => return alloc::format!("fs: {name}: not found"),
    };
    let mut buf = alloc::vec![0u8; size];
    match fat.read_root_file(&mut sd, &n83, &mut buf) {
        Ok(n) => String::from_utf8_lossy(&buf[..n]).into_owned(),
        Err(e) => alloc::format!("fs: read failed ({e:?})"),
    }
}

/// `fs ls` — list root-directory files on the SD.
pub fn list() -> String {
    let (mut sd, fat) = match mount() {
        Ok(v) => v,
        Err(e) => return e.into(),
    };
    let mut entries = [RootEntry::default(); 64];
    match fat.list_root(&mut sd, &mut entries) {
        Ok(0) => "fs: (empty)".into(),
        Ok(n) => {
            let mut out = alloc::format!("fs: {n} file(s) on SD:\n");
            for ent in entries.iter().take(n) {
                out.push_str(&alloc::format!(
                    "  {:<12} {} bytes\n",
                    name_from_83(&ent.name),
                    ent.size
                ));
            }
            out
        }
        Err(e) => alloc::format!("fs: list failed ({e:?})"),
    }
}

/// Read a root-directory SD file as raw bytes (GPA staging).
pub fn read_bytes(name: &str) -> Result<crate::platform::Vec<u8>, &'static str> {
    let (mut sd, fat) = mount()?;
    let n83 = name_to_83(name);
    let size = fat
        .find_in_root(&mut sd, &n83)
        .map_err(|_| "fs: not found")?
        .1 as usize;
    let mut buf = crate::platform::Vec::new();
    buf.resize(size, 0);
    fat.read_root_file(&mut sd, &n83, &mut buf)
        .map_err(|_| "fs: read failed")?;
    Ok(buf)
}
pub fn remove(name: &str) -> String {
    let (mut sd, fat) = match mount() {
        Ok(v) => v,
        Err(e) => return e.into(),
    };
    match fat.delete_root_file(&mut sd, &name_to_83(name)) {
        Ok(()) => alloc::format!("fs: removed {name}"),
        Err(e) => alloc::format!("fs: remove failed ({e:?})"),
    }
}
