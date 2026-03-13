use image::imageops::FilterType;

const APP_ICON_BYTES: &[u8] = include_bytes!("../assets/star.jpg");
const OVERLAY_STAR_BYTES: &[u8] = include_bytes!("../assets/star-nobg.png");

pub fn tray_icon_rgba(size: u32) -> anyhow::Result<(Vec<u8>, u32, u32)> {
    let image = image::load_from_memory(APP_ICON_BYTES)?
        .resize(size, size, FilterType::Lanczos3)
        .into_rgba8();
    let (width, height) = image.dimensions();
    Ok((image.into_raw(), width, height))
}

pub fn overlay_star_image(size: u32) -> anyhow::Result<egui::ColorImage> {
    let image = image::load_from_memory(OVERLAY_STAR_BYTES)?
        .resize(size, size, FilterType::Lanczos3)
        .into_rgba8();
    let (width, height) = image.dimensions();
    let rgba = image.into_raw();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [width as usize, height as usize],
        &rgba,
    ))
}
