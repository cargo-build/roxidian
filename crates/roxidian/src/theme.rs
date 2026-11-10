//! Window-local selection; CSS owns the palettes.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum Theme {
    #[default]
    Synthwave,
    Light,
}

impl Theme {
    pub fn toggle(self) -> Self {
        match self {
            Self::Synthwave => Self::Light,
            Self::Light => Self::Synthwave,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Synthwave => "SynthWave '84",
            Self::Light => "Obsidian Light",
        }
    }
    pub fn class(self) -> &'static str {
        match self {
            Self::Synthwave => "app dark",
            Self::Light => "app light",
        }
    }
}
