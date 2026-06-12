use gpui::*;
use serde::Deserialize;
use theme::{ThemeSettingsProvider, UiDensity};

// Semantic color constants for the dark theme. All render code should reference
// these names instead of raw hsla() literals so that a future light theme only
// needs to change this one place.
//
// Hsla fields: { h: 0..1, s: 0..1, l: 0..1, a: 0..1 }
// To convert from the hsla(hue_deg, sat, light, alpha) function convention:
//   h = hue_deg / 360.0, s = sat, l = light, a = alpha

pub(crate) mod colors {
    use gpui::Hsla;

    // -- Backgrounds (darkest → lightest) --
    pub const BG_DEEPEST: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.08, a: 1.0 };
    pub const BG_BASE: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.1, a: 1.0 };
    pub const BG_RAISED: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.12, a: 1.0 };
    pub const BG_PANEL: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.13, a: 1.0 };
    pub const BG_BORDER: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.15, a: 1.0 };
    pub const BG_TAB_ACTIVE: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.18, a: 1.0 };
    pub const BG_HOVER: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.2, a: 1.0 };
    pub const BG_HOVER_DEEP: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.22, a: 1.0 };
    pub const BG_BORDER_STRONG: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.25, a: 1.0 };
    pub const BG_DRAG: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.2, a: 0.9 };

    // -- Text (dim → bright) --
    pub const TEXT_DIM: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.4, a: 1.0 };
    pub const TEXT_DIM_ICON: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.45, a: 1.0 };
    pub const TEXT_SECONDARY: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.5, a: 1.0 };
    pub const TEXT_MUTED: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.6, a: 1.0 };
    pub const TEXT_DEFAULT: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.7, a: 1.0 };
    pub const TEXT_BRIGHT: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.8, a: 1.0 };
    pub const TEXT_PRIMARY: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.9, a: 1.0 };
    pub const TEXT_SELECTED: Hsla = Hsla { h: 0.0, s: 0.0, l: 1.0, a: 1.0 };

    // -- Accent: blue (h=220/360≈0.6111) --
    pub const ACCENT: Hsla = Hsla { h: 0.6111, s: 0.8, l: 0.6, a: 1.0 };
    pub const ACCENT_SELECTED: Hsla = Hsla { h: 0.6111, s: 0.6, l: 0.4, a: 1.0 };
    pub const ACCENT_HOVER: Hsla = Hsla { h: 0.6111, s: 0.5, l: 0.35, a: 1.0 };

    // -- Accent: gold (h=40/360≈0.1111, for pinned tabs) --
    pub const GOLD_ACTIVE: Hsla = Hsla { h: 0.1111, s: 0.8, l: 0.55, a: 1.0 };
    pub const GOLD_INACTIVE: Hsla = Hsla { h: 0.1111, s: 0.6, l: 0.4, a: 1.0 };

    // -- Diff (h=120/360≈0.3333 for green) --
    pub const DIFF_ADDED_BG: Hsla = Hsla { h: 0.3333, s: 0.6, l: 0.2, a: 0.3 };
    pub const DIFF_REMOVED_BG: Hsla = Hsla { h: 0.0, s: 0.6, l: 0.15, a: 0.35 };

    // -- Search highlights (h=48/360≈0.1333) --
    pub const SEARCH_MATCH: Hsla = Hsla { h: 0.1333, s: 1.0, l: 0.6, a: 1.0 };
    pub const SEARCH_CURRENT: Hsla = Hsla { h: 0.1333, s: 1.0, l: 0.5, a: 0.4 };

    // -- Misc --
    pub const SHADOW: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.5 };
    pub const TRANSPARENT: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.0 };
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct UserSettings {
    pub ui_font_family: Option<String>,
    pub ui_font_size: Option<f32>,
    pub buffer_font_family: Option<String>,
    pub buffer_font_size: Option<f32>,
    pub tab_size: Option<u32>,
}

pub(crate) fn load_user_settings() -> UserSettings {
    let path = crate::utils::config_dir().join("settings.json");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return UserSettings::default(),
    };
    match serde_json::from_str(&content) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("failed to parse settings.json: {err:#}");
            UserSettings::default()
        }
    }
}

pub(crate) struct WzedThemeSettings {
    ui_font: Font,
    buffer_font: Font,
    ui_font_size: Pixels,
    buffer_font_size: Pixels,
}

impl WzedThemeSettings {
    pub(crate) fn new() -> Self {
        let settings = load_user_settings();
        let default_ui = "Helvetica";
        let default_buffer = "Monospace";
        Self {
            ui_font: Font {
                family: settings
                    .ui_font_family
                    .unwrap_or_else(|| default_ui.into())
                    .into(),
                weight: FontWeight::NORMAL,
                style: FontStyle::Normal,
                features: FontFeatures::default(),
                fallbacks: None,
            },
            buffer_font: Font {
                family: settings
                    .buffer_font_family
                    .unwrap_or_else(|| default_buffer.into())
                    .into(),
                weight: FontWeight::NORMAL,
                style: FontStyle::Normal,
                features: FontFeatures::default(),
                fallbacks: None,
            },
            ui_font_size: px(settings.ui_font_size.unwrap_or(14.0)),
            buffer_font_size: px(settings.buffer_font_size.unwrap_or(14.0)),
        }
    }
}

impl ThemeSettingsProvider for WzedThemeSettings {
    fn ui_font<'a>(&'a self, _cx: &'a App) -> &'a Font {
        &self.ui_font
    }

    fn buffer_font<'a>(&'a self, _cx: &'a App) -> &'a Font {
        &self.buffer_font
    }

    fn ui_font_size(&self, _cx: &App) -> Pixels {
        self.ui_font_size
    }

    fn buffer_font_size(&self, _cx: &App) -> Pixels {
        self.buffer_font_size
    }

    fn ui_density(&self, _cx: &App) -> UiDensity {
        UiDensity::Default
    }
}
