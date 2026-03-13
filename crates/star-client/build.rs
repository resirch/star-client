#[cfg(target_os = "windows")]
fn main() {
    if let Err(error) = configure_windows_icon() {
        panic!("failed to configure Windows executable icon: {error}");
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {}

#[cfg(target_os = "windows")]
fn configure_windows_icon() -> Result<(), Box<dyn std::error::Error>> {
    use image::imageops::FilterType;
    use image::ImageFormat;
    use std::path::PathBuf;

    println!("cargo:rerun-if-changed=assets/star.jpg");

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let source = manifest_dir.join("assets").join("star.jpg");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    let icon_path = out_dir.join("star-app-icon.ico");

    let icon = image::open(&source)?.resize(256, 256, FilterType::Lanczos3);
    icon.save_with_format(&icon_path, ImageFormat::Ico)?;

    winres::WindowsResource::new()
        .set_icon(icon_path.to_string_lossy().as_ref())
        .compile()?;

    Ok(())
}
