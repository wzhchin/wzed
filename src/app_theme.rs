use gpui::*;
use theme::{ThemeSettingsProvider, UiDensity};

pub(crate) struct WzedThemeSettings {
    ui_font: Font,
    buffer_font: Font,
}

impl WzedThemeSettings {
    pub(crate) fn new() -> Self {
        Self {
            ui_font: Font {
                family: "Helvetica".into(),
                weight: FontWeight::NORMAL,
                style: FontStyle::Normal,
                features: FontFeatures::default(),
                fallbacks: None,
            },
            buffer_font: Font {
                family: "Monospace".into(),
                weight: FontWeight::NORMAL,
                style: FontStyle::Normal,
                features: FontFeatures::default(),
                fallbacks: None,
            },
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
        px(14.0)
    }

    fn buffer_font_size(&self, _cx: &App) -> Pixels {
        px(14.0)
    }

    fn ui_density(&self, _cx: &App) -> UiDensity {
        UiDensity::Default
    }
}
