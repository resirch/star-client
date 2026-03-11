use egui::{Color32, FontFamily, FontId, Rounding, Stroke, Vec2};
use std::path::Path;

const REGULAR_FONT_FAMILY: &str = "overlay-regular";

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
pub const TEAM_RED: Color32 = Color32::from_rgb(238, 77, 77);
pub const TEAM_BLUE: Color32 = Color32::from_rgb(76, 151, 237);
pub const STATUS_INGAME: Color32 = Color32::from_rgb(241, 39, 39);
pub const STATUS_PREGAME: Color32 = Color32::from_rgb(103, 237, 76);
pub const STATUS_MENU: Color32 = Color32::from_rgb(238, 241, 54);
pub const STATUS_WAITING: Color32 = Color32::from_rgb(255, 165, 0);
pub const VRY_DARK_RED: Color32 = Color32::from_rgb(64, 15, 10);
pub const VRY_YELLOW: Color32 = Color32::from_rgb(140, 119, 11);
pub const VRY_GREEN: Color32 = Color32::from_rgb(18, 204, 25);
pub const VRY_WHITE: Color32 = Color32::from_rgb(255, 255, 255);
pub const RR_PENALTY_NONE: Color32 = Color32::from_rgb(200, 200, 200);
pub const RR_PENALTY_LOW: Color32 = Color32::from_rgb(255, 165, 0);
pub const RR_PENALTY_HIGH: Color32 = Color32::from_rgb(255, 0, 0);
pub const PARTY_COLORS: &[Color32] = &[
    Color32::from_rgb(100, 200, 255),
    Color32::from_rgb(255, 180, 100),
    Color32::from_rgb(180, 130, 255),
    Color32::from_rgb(100, 255, 180),
    Color32::from_rgb(255, 130, 180),
];

pub fn header_font() -> FontId {
    FontId::new(13.0, FontFamily::Proportional)
}

pub fn body_font() -> FontId {
    FontId::new(12.5, FontFamily::Proportional)
}

pub fn small_font() -> FontId {
    FontId::new(10.0, FontFamily::Proportional)
}

pub fn small_regular_font() -> FontId {
    FontId::new(10.0, FontFamily::Name(REGULAR_FONT_FAMILY.into()))
}

pub fn star_font() -> FontId {
    FontId::new(14.0, FontFamily::Proportional)
}

pub fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut loaded_fonts = Vec::new();
    let default_proportional = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    for source in system_font_fallbacks() {
        let path = Path::new(source.path);
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };

        let mut font_data = egui::FontData::from_owned(bytes);
        font_data.index = source.index;
        fonts.font_data.insert(source.name.into(), font_data);
        loaded_fonts.push(source.name.to_string());
    }

    let regular_family = if loaded_fonts.is_empty() {
        default_proportional.clone()
    } else {
        loaded_fonts
            .iter()
            .filter(|name| !name.ends_with("-bold"))
            .cloned()
            .chain(
                loaded_fonts
                    .iter()
                    .filter(|name| name.ends_with("-bold"))
                    .cloned(),
            )
            .chain(default_proportional.clone())
            .collect()
    };
    fonts
        .families
        .insert(FontFamily::Name(REGULAR_FONT_FAMILY.into()), regular_family);

    if !loaded_fonts.is_empty() {
        if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
            let existing = family.clone();
            *family = loaded_fonts.iter().cloned().chain(existing).collect();
        }
        if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
            let existing = family.clone();
            *family = loaded_fonts.iter().cloned().chain(existing).collect();
        }

        tracing::info!(
            "Loaded multilingual font fallbacks: {}",
            loaded_fonts.join(", ")
        );
    } else {
        tracing::warn!(
            "No system font fallbacks were loaded; non-Latin glyph coverage may be limited"
        );
    }

    ctx.set_fonts(fonts);
}

struct SystemFontSource {
    name: &'static str,
    path: &'static str,
    index: u32,
}

#[cfg(target_os = "windows")]
fn system_font_fallbacks() -> &'static [SystemFontSource] {
    &[
        SystemFontSource {
            name: "system-segoe-ui-bold",
            path: r"C:\Windows\Fonts\segoeuib.ttf",
            index: 0,
        },
        SystemFontSource {
            name: "system-segoe-ui",
            path: r"C:\Windows\Fonts\segoeui.ttf",
            index: 0,
        },
        SystemFontSource {
            name: "system-malgun-gothic",
            path: r"C:\Windows\Fonts\malgun.ttf",
            index: 0,
        },
        SystemFontSource {
            name: "system-microsoft-yahei",
            path: r"C:\Windows\Fonts\msyh.ttc",
            index: 0,
        },
        SystemFontSource {
            name: "system-microsoft-jhenghei",
            path: r"C:\Windows\Fonts\msjh.ttc",
            index: 0,
        },
        SystemFontSource {
            name: "system-yu-gothic",
            path: r"C:\Windows\Fonts\YuGothR.ttc",
            index: 0,
        },
        SystemFontSource {
            name: "system-ms-gothic",
            path: r"C:\Windows\Fonts\msgothic.ttc",
            index: 0,
        },
        SystemFontSource {
            name: "system-simsun",
            path: r"C:\Windows\Fonts\simsun.ttc",
            index: 0,
        },
        SystemFontSource {
            name: "system-nirmala-ui",
            path: r"C:\Windows\Fonts\Nirmala.ttc",
            index: 0,
        },
        SystemFontSource {
            name: "system-leelawadee-ui",
            path: r"C:\Windows\Fonts\LeelawUI.ttf",
            index: 0,
        },
    ]
}

#[cfg(not(target_os = "windows"))]
fn system_font_fallbacks() -> &'static [SystemFontSource] {
    &[]
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

pub fn team_text_color(is_ally: bool) -> Color32 {
    if is_ally {
        TEAM_BLUE
    } else {
        TEAM_RED
    }
}

pub fn team_id_color(team_id: &str) -> Color32 {
    if team_id.eq_ignore_ascii_case("red") {
        TEAM_RED
    } else if team_id.eq_ignore_ascii_case("blue") {
        TEAM_BLUE
    } else {
        TEXT_PRIMARY
    }
}

pub fn winrate_color(pct: f64) -> Color32 {
    gradient_color(
        pct,
        &[
            (0.0, 45.0, VRY_DARK_RED, VRY_YELLOW),
            (45.0, 55.0, VRY_YELLOW, VRY_GREEN),
            (55.0, 100.0, VRY_GREEN, VRY_WHITE),
        ],
    )
}

pub fn kd_color(kd: f64) -> Color32 {
    if kd >= 1.2 {
        VRY_GREEN
    } else if kd <= 0.8 {
        STATUS_INGAME
    } else {
        TEXT_PRIMARY
    }
}

pub fn hs_color(pct: f64) -> Color32 {
    gradient_color(
        pct,
        &[
            (0.0, 25.0, VRY_DARK_RED, VRY_YELLOW),
            (25.0, 50.0, VRY_YELLOW, VRY_GREEN),
            (50.0, 100.0, VRY_GREEN, VRY_WHITE),
        ],
    )
}

pub fn rr_change_color(rr: i32) -> Color32 {
    if rr > 0 {
        VRY_GREEN
    } else if rr < 0 {
        STATUS_INGAME
    } else {
        VRY_WHITE
    }
}

pub fn rr_penalty_color(afk_penalty: i32) -> Color32 {
    if afk_penalty == 0 {
        RR_PENALTY_NONE
    } else if afk_penalty <= 5 {
        RR_PENALTY_LOW
    } else {
        RR_PENALTY_HIGH
    }
}

pub fn level_color(level: i32) -> Color32 {
    if level >= 400 {
        Color32::from_rgb(102, 212, 212)
    } else if level >= 300 {
        Color32::from_rgb(207, 207, 76)
    } else if level >= 200 {
        Color32::from_rgb(71, 71, 204)
    } else if level >= 100 {
        Color32::from_rgb(241, 144, 54)
    } else {
        Color32::from_rgb(211, 211, 211)
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

fn gradient_color(value: f64, ranges: &[(f64, f64, Color32, Color32)]) -> Color32 {
    let value = value.clamp(0.0, 100.0);

    for (start, end, from, to) in ranges {
        if value >= *start && value <= *end {
            let span = (*end - *start).max(f64::EPSILON);
            let t = ((value - *start) / span) as f32;
            return lerp_color(*from, *to, t);
        }
    }

    TEXT_PRIMARY
}

fn lerp_color(from: Color32, to: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |a: u8, b: u8| a as f32 + (b as f32 - a as f32) * t;

    Color32::from_rgb(
        lerp(from.r(), to.r()).round() as u8,
        lerp(from.g(), to.g()).round() as u8,
        lerp(from.b(), to.b()).round() as u8,
    )
}
