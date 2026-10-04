fn main() {
    println!("cargo:rerun-if-changed=RustyMCU-icon.png");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
        let ico_path = std::path::Path::new(&out_dir).join("app.ico");
        png_to_ico("RustyMCU-icon.png", &ico_path);

        let mut res = winresource::WindowsResource::new();
        res.set_icon(ico_path.to_str().unwrap());
        res.compile().expect("winresource compile failed");
    }
}

/// Resize the source PNG to 256×256, wrap it in a minimal ICO container,
/// and write it to `dest`.  Windows accepts PNG payloads inside ICO since Vista.
fn png_to_ico(src: &str, dest: &std::path::Path) {
    let img = image::open(src)
        .expect("cannot open RustyMCU-icon.png")
        .resize_exact(256, 256, image::imageops::FilterType::Lanczos3);

    let mut png_buf: Vec<u8> = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut png_buf),
        image::ImageFormat::Png,
    )
    .expect("cannot re-encode icon as PNG");

    // ICO header (6 bytes) + one entry (16 bytes) + PNG payload
    let data_size: u32 = png_buf.len() as u32;
    let data_offset: u32 = 6 + 16;

    let mut ico: Vec<u8> = Vec::with_capacity(data_offset as usize + png_buf.len());
    ico.extend_from_slice(&0u16.to_le_bytes()); // reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // type = ICO
    ico.extend_from_slice(&1u16.to_le_bytes()); // image count = 1
    ico.push(0); // width  (0 → 256)
    ico.push(0); // height (0 → 256)
    ico.push(0); // colour count
    ico.push(0); // reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // colour planes
    ico.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
    ico.extend_from_slice(&data_size.to_le_bytes());
    ico.extend_from_slice(&data_offset.to_le_bytes());
    ico.extend_from_slice(&png_buf);

    std::fs::write(dest, &ico).expect("cannot write app.ico");
}
