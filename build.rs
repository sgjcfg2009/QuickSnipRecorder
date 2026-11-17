use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn push_res_entry(
    out: &mut Vec<u8>,
    type_id: u16,
    name_id: u16,
    mem_flags: u16,
    lang_id: u16,
    data: &[u8],
) {
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&32u32.to_le_bytes());
    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&type_id.to_le_bytes());
    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&name_id.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&mem_flags.to_le_bytes());
    out.extend_from_slice(&lang_id.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(data);
    while (out.len() & 3) != 0 {
        out.push(0);
    }
}

fn embed_ico_resource(ico_path: &Path) {
    let Ok(bytes) = fs::read(ico_path) else {
        return;
    };
    if bytes.len() < 6 {
        return;
    }
    let reserved = u16::from_le_bytes([bytes[0], bytes[1]]);
    let ico_type = u16::from_le_bytes([bytes[2], bytes[3]]);
    let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    if reserved != 0 || ico_type != 1 || count == 0 || bytes.len() < 6 + count * 16 {
        return;
    }

    let mut res = Vec::with_capacity(bytes.len() + 512);
    // 32-byte empty header required at the start of every Win32 .res file.
    push_res_entry(&mut res, 0, 0, 0, 0, &[]);

    let mut grp_dir = Vec::with_capacity(6 + count * 14);
    grp_dir.extend_from_slice(&bytes[0..6]);

    for i in 0..count {
        let e = &bytes[6 + i * 16..6 + (i + 1) * 16];
        let img_size = u32::from_le_bytes([e[8], e[9], e[10], e[11]]) as usize;
        let img_off = u32::from_le_bytes([e[12], e[13], e[14], e[15]]) as usize;
        if img_off.saturating_add(img_size) > bytes.len() {
            return;
        }
        let icon_id = (i + 1) as u16;
        push_res_entry(
            &mut res,
            3, // RT_ICON
            icon_id,
            0x1030,
            0x0409,
            &bytes[img_off..img_off + img_size],
        );
        grp_dir.extend_from_slice(&e[0..12]);
        grp_dir.extend_from_slice(&icon_id.to_le_bytes());
    }

    push_res_entry(&mut res, 14, 1, 0x1030, 0x0409, &grp_dir); // RT_GROUP_ICON

    if let Some(out_dir) = env::var_os("OUT_DIR") {
        let res_path = PathBuf::from(out_dir).join("app.res");
        if fs::write(&res_path, &res).is_ok() {
            println!("cargo:rustc-link-arg={}", res_path.display());
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=ffmpeg.exe");
    println!("cargo:rerun-if-changed=app.ico");
    println!("cargo:rustc-check-cfg=cfg(has_embedded_ffmpeg)");
    if Path::new("ffmpeg.exe").exists() {
        println!("cargo:rustc-cfg=has_embedded_ffmpeg");
    }
    let ico = Path::new("app.ico");
    if ico.exists() {
        embed_ico_resource(ico);
    }
}
