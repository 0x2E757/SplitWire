//! `%LOCALAPPDATA%\SplitWire\splitwire.window`: the user's window settings, applied
//! at once, without Apply. The "Start SplitWire at sign-in" switch is the HKCU Run value
//! (`platform::window::startup`), not in this file.

use crate::config::KeyValueFile;

/// "System" follows the Windows app theme as it changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeChoice {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn key(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.key() == text.trim())
    }

    /// Whether the window is dark, given whether the Windows app theme is.
    pub fn dark(self, system_dark: bool) -> bool {
        match self {
            Self::System => system_dark,
            Self::Light => false,
            Self::Dark => true,
        }
    }
}

/// The window's normal frame in DIP and whether it was maximized: `x,y,w,h,maximized`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub maximized: bool,
}

impl Placement {
    pub fn parse(text: &str) -> Option<Self> {
        let parts: Vec<i32> = text
            .split(',')
            .map(|p| p.trim().parse().ok())
            .collect::<Option<_>>()?;
        let [x, y, width, height, maximized] = parts[..] else {
            return None;
        };
        (width > 0 && height > 0).then_some(Self {
            x,
            y,
            width,
            height,
            maximized: maximized != 0,
        })
    }

    pub fn text(&self) -> String {
        format!(
            "{},{},{},{},{}",
            self.x,
            self.y,
            self.width,
            self.height,
            u8::from(self.maximized)
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowSettings {
    pub open_window_at_sign_in: bool,
    pub theme: ThemeChoice,
    pub placement: Option<Placement>,
}

impl WindowSettings {
    pub const FILE: &'static str = "splitwire.window";

    /// Unknown keys and bad values fall back to the defaults: the file is the user's, not the
    /// service's, and a bad line must not keep the window from opening.
    pub fn parse(text: &str) -> Self {
        let file = KeyValueFile::parse(Self::FILE, text);
        let value = |key: &str| file.get(key).map(|e| e.value.as_str());
        Self {
            open_window_at_sign_in: value("open_window_at_sign_in") == Some("true"),
            theme: value("theme")
                .and_then(ThemeChoice::parse)
                .unwrap_or_default(),
            placement: value("placement").and_then(Placement::parse),
        }
    }

    pub fn text(&self) -> String {
        let mut text = format!(
            "open_window_at_sign_in = {}\ntheme = {}\n",
            self.open_window_at_sign_in,
            self.theme.key()
        );
        if let Some(placement) = self.placement {
            text.push_str(&format!("placement = {}\n", placement.text()));
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_defaults() {
        let settings = WindowSettings {
            open_window_at_sign_in: true,
            theme: ThemeChoice::Dark,
            placement: Some(Placement {
                x: -10,
                y: 20,
                width: 1280,
                height: 1024,
                maximized: true,
            }),
        };
        assert_eq!(WindowSettings::parse(&settings.text()), settings);
        assert_eq!(
            WindowSettings::parse("theme = blue\nx=1\n"),
            WindowSettings::default()
        );
        assert_eq!(Placement::parse("1,2,0,4,0"), None);
    }

    #[test]
    fn the_theme_follows_the_system_unless_chosen() {
        assert!(ThemeChoice::System.dark(true));
        assert!(!ThemeChoice::Light.dark(true));
        assert!(ThemeChoice::Dark.dark(false));
        assert_eq!(ThemeChoice::parse("light"), Some(ThemeChoice::Light));
    }
}
