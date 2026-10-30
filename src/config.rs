use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub theme: Theme,
    pub font_size: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            font_size: 13,
        }
    }
}

impl Settings {
    pub fn path() -> Option<PathBuf> {
        let base = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
        Some(base.join("editshell").join("settings.toml"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        let Ok(contents) = fs::read_to_string(path) else {
            return Self::default();
        };
        Self::parse(&contents)
    }

    fn parse(contents: &str) -> Self {
        let mut settings = Self::default();
        for line in contents.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "theme" => {
                    settings.theme = match value.trim().trim_matches('"') {
                        "light" => Theme::Light,
                        "dark" => Theme::Dark,
                        _ => Theme::System,
                    };
                }
                "font_size" => {
                    if let Ok(size) = value.trim().parse::<u8>() {
                        settings.font_size = size.clamp(8, 32);
                    }
                }
                _ => {}
            }
        }
        settings
    }

    pub fn save(&self) -> io::Result<()> {
        let Some(path) = Self::path() else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "neither XDG_CONFIG_HOME nor HOME is set",
            ));
        };
        let parent = path.parent().expect("settings path has a parent");
        fs::create_dir_all(parent)?;
        let theme = match self.theme {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        };
        fs::write(
            path,
            format!("theme = \"{theme}\"\nfont_size = {}\n", self.font_size),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_reasonable() {
        let settings = Settings::default();
        assert_eq!(settings.theme, Theme::System);
        assert_eq!(settings.font_size, 13);
    }

    #[test]
    fn malformed_values_do_not_replace_defaults() {
        let settings = Settings::parse("theme = \"nonsense\"\nfont_size = invalid\n");
        assert_eq!(settings.theme, Theme::System);
        assert_eq!(settings.font_size, 13);
    }

    #[test]
    fn valid_settings_are_loaded_and_font_size_is_clamped() {
        let settings = Settings::parse("theme = \"dark\"\nfont_size = 100\n");
        assert_eq!(settings.theme, Theme::Dark);
        assert_eq!(settings.font_size, 32);
    }
}
