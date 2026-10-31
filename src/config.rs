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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutputPalette {
    #[default]
    Standard,
    HighContrast,
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub theme: Theme,
    pub font_size: u8,
    pub colorize_output: bool,
    pub output_palette: OutputPalette,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            font_size: 13,
            colorize_output: true,
            output_palette: OutputPalette::Standard,
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
                "colorize_output" => {
                    if let Ok(enabled) = value.trim().parse::<bool>() {
                        settings.colorize_output = enabled;
                    }
                }
                "output_palette" => {
                    settings.output_palette = match value.trim().trim_matches('"') {
                        "high_contrast" => OutputPalette::HighContrast,
                        _ => OutputPalette::Standard,
                    };
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
            format!(
                "theme = \"{theme}\"\nfont_size = {}\ncolorize_output = {}\noutput_palette = \"{}\"\n",
                self.font_size,
                self.colorize_output,
                match self.output_palette {
                    OutputPalette::Standard => "standard",
                    OutputPalette::HighContrast => "high_contrast",
                }
            ),
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
        assert!(settings.colorize_output);
        assert_eq!(settings.output_palette, OutputPalette::Standard);
    }

    #[test]
    fn malformed_values_do_not_replace_defaults() {
        let settings = Settings::parse("theme = \"nonsense\"\nfont_size = invalid\n");
        assert_eq!(settings.theme, Theme::System);
        assert_eq!(settings.font_size, 13);
        assert!(settings.colorize_output);
    }

    #[test]
    fn valid_settings_are_loaded_and_font_size_is_clamped() {
        let settings = Settings::parse(
            "theme = \"dark\"\nfont_size = 100\ncolorize_output = false\noutput_palette = \"high_contrast\"\n",
        );
        assert_eq!(settings.theme, Theme::Dark);
        assert_eq!(settings.font_size, 32);
        assert!(!settings.colorize_output);
        assert_eq!(settings.output_palette, OutputPalette::HighContrast);
    }
}
