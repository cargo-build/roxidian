//! Window-local pane geometry; pointer coordinates are logical CSS pixels.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Mode {
    #[default]
    Preview,
    Source,
    Split,
}

impl Mode {
    pub fn for_path(path: &std::path::Path) -> Self {
        match crate::workspace::is_markdown(path) {
            true => Self::Preview,
            false => Self::Source,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Layout {
    pub sidebar: f64,
    pub split: f64,
    pub sidebar_visible: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Boundary {
    Sidebar,
    Split,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            sidebar: 280.,
            split: 0.5,
            sidebar_visible: true,
        }
    }
}

impl Layout {
    pub fn resize(&mut self, boundary: Boundary, x: f64, window_width: f64) {
        match boundary {
            Boundary::Sidebar => {
                self.sidebar = x.clamp(180., (window_width - 320.).clamp(180., 600.))
            }
            Boundary::Split => {
                let sidebar = if self.sidebar_visible {
                    self.sidebar + 6.
                } else {
                    0.
                };
                self.split = ((x - sidebar) / (window_width - sidebar - 6.).max(1.)).clamp(0.2, 0.8)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dragging_keeps_both_panes_usable() {
        let mut layout = Layout::default();
        layout.resize(Boundary::Sidebar, 350., 1000.);
        assert_eq!(layout.sidebar, 350.);
        layout.resize(Boundary::Sidebar, 900., 820.);
        assert_eq!(layout.sidebar, 500.);
        layout.resize(Boundary::Split, 100., 1000.);
        assert_eq!(layout.split, 0.2);
        layout.resize(Boundary::Split, 990., 1000.);
        assert_eq!(layout.split, 0.8);
    }

    #[test]
    fn markdown_opens_preview_and_other_text_opens_source() {
        assert_eq!(
            Mode::for_path(std::path::Path::new("README.md")),
            Mode::Preview
        );
        assert_eq!(
            Mode::for_path(std::path::Path::new("NOTES.MD")),
            Mode::Preview
        );
        assert_eq!(
            Mode::for_path(std::path::Path::new("settings.toml")),
            Mode::Source
        );
    }
}
