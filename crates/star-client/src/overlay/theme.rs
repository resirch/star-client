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

pub fn agent_color(agent_name: &str) -> Color32 {
    // Mirrors the agent-name color mapping used by the VRY project.
    // We normalize for common variants like "KAY/O" vs "KAYO".
    let lower = agent_name.trim().to_lowercase();
    let key = lower.as_str();
    match key {
        "" | "none" | "unknown" => Color32::from_rgb(100, 100, 100),
        "astra" => Color32::from_rgb(113, 42, 232),
        "breach" => Color32::from_rgb(199, 107, 59),
        "brimstone" => Color32::from_rgb(209, 105, 31),
        "cypher" => Color32::from_rgb(230, 217, 197),
        "chamber" => Color32::from_rgb(184, 154, 70),
        "deadlock" => Color32::from_rgb(102, 119, 176),
        "fade" => Color32::from_rgb(92, 92, 94),
        "jett" => Color32::from_rgb(154, 222, 255),
        "kay/o" | "kayo" => Color32::from_rgb(133, 146, 156),
        "killjoy" => Color32::from_rgb(255, 217, 31),
        "omen" => Color32::from_rgb(71, 80, 143),
        "phoenix" => Color32::from_rgb(254, 130, 102),
        "raze" => Color32::from_rgb(255, 164, 0),
        "reyna" => Color32::from_rgb(181, 101, 181),
        "sage" => Color32::from_rgb(38, 200, 175),
        "skye" => Color32::from_rgb(192, 230, 158),
        "sova" => Color32::from_rgb(59, 160, 229),
        "neon" => Color32::from_rgb(0, 207, 255),
        "viper" => Color32::from_rgb(56, 198, 89),
        "yoru" => Color32::from_rgb(40, 70, 200),
        "harbor" => Color32::from_rgb(0, 128, 128),
        "gekko" => Color32::from_rgb(168, 230, 94),
        "vyse" => Color32::from_rgb(101, 107, 139),
        "iso" => Color32::from_rgb(87, 74, 194),
        "clove" => Color32::from_rgb(242, 143, 208),
        "tejo" => Color32::from_rgb(255, 183, 97),
        "veto" => Color32::from_rgb(30, 60, 90),
        "waylay" => Color32::from_rgb(130, 195, 235),
        _ => TEXT_PRIMARY,
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
