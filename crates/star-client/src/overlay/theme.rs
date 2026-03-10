use egui::{Color32, FontFamily, FontId, Rounding, Stroke, Vec2};

pub const BG_COLOR: Color32 = Color32::from_rgba_premultiplied(15, 15, 20, 220);
pub const HEADER_BG: Color32 = Color32::from_rgba_premultiplied(25, 25, 35, 240);
pub const ROW_BG_ALLY: Color32 = Color32::from_rgba_premultiplied(20, 30, 20, 200);
pub const ROW_BG_ENEMY: Color32 = Color32::from_rgba_premultiplied(30, 20, 20, 200);
pub const ROW_BG_HOVER: Color32 = Color32::from_rgba_premultiplied(40, 40, 55, 220);
pub const BORDER_COLOR: Color32 = Color32::from_rgba_premultiplied(60, 60, 80, 180);
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(230, 230, 240);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(160, 160, 175);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(100, 100, 115);
pub const STAR_COLOR: Color32 = Color32::from_rgb(255, 215, 0);
pub const PARTY_COLORS: &[Color32] = &[
    Color32::from_rgb(100, 200, 255),
    Color32::from_rgb(255, 180, 100),
    Color32::from_rgb(180, 130, 255),
    Color32::from_rgb(100, 255, 180),
    Color32::from_rgb(255, 130, 180),
];

pub const WIN_COLOR: Color32 = Color32::from_rgb(100, 220, 100);
pub const LOSS_COLOR: Color32 = Color32::from_rgb(220, 100, 100);

pub fn header_font() -> FontId {
    FontId::new(13.0, FontFamily::Proportional)
}

pub fn body_font() -> FontId {
    FontId::new(12.5, FontFamily::Proportional)
}

pub fn small_font() -> FontId {
    FontId::new(10.0, FontFamily::Proportional)
}

pub fn star_font() -> FontId {
    FontId::new(14.0, FontFamily::Proportional)
}

pub fn table_rounding() -> Rounding {
    Rounding::same(6.0)
}

pub fn table_stroke() -> Stroke {
    Stroke::new(1.0, BORDER_COLOR)
}

pub fn row_padding() -> Vec2 {
    Vec2::new(8.0, 4.0)
}

pub fn winrate_color(pct: f64) -> Color32 {
    if pct >= 55.0 {
        WIN_COLOR
    } else if pct <= 45.0 {
        LOSS_COLOR
    } else {
        TEXT_PRIMARY
    }
}

pub fn kd_color(kd: f64) -> Color32 {
    if kd >= 1.2 {
        WIN_COLOR
    } else if kd <= 0.8 {
        LOSS_COLOR
    } else {
        TEXT_PRIMARY
    }
}

pub fn hs_color(pct: f64) -> Color32 {
    if pct >= 25.0 {
        WIN_COLOR
    } else if pct <= 15.0 {
        LOSS_COLOR
    } else {
        TEXT_PRIMARY
    }
}

pub fn rr_change_color(rr: i32) -> Color32 {
    if rr > 0 {
        WIN_COLOR
    } else if rr < 0 {
        LOSS_COLOR
    } else {
        TEXT_MUTED
    }
}

pub fn party_color(party_number: i32) -> Color32 {
    if party_number <= 0 {
        Color32::TRANSPARENT
    } else {
        let idx = ((party_number - 1) as usize) % PARTY_COLORS.len();
        PARTY_COLORS[idx]
    }
}
