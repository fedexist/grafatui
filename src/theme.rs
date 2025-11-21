use ratatui::style::Color;

#[derive(Debug, Clone)]
pub struct Theme {
    #[allow(dead_code)]
    pub background: Color,
    pub text: Color,
    pub title: Color,
    pub border: Color,
    pub border_selected: Color,
    #[allow(dead_code)]
    pub legend_text: Color,
    #[allow(dead_code)]
    pub legend_dim: Color,
    pub palette: Vec<Color>,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::Reset,
            text: Color::Reset,
            title: Color::Cyan,
            border: Color::DarkGray,
            border_selected: Color::Yellow,
            legend_text: Color::White,
            legend_dim: Color::DarkGray,
            palette: vec![
                Color::Green,
                Color::Yellow,
                Color::Blue,
                Color::Magenta,
                Color::Cyan,
                Color::Red,
                Color::LightGreen,
                Color::LightYellow,
                Color::LightBlue,
                Color::LightMagenta,
                Color::LightCyan,
                Color::LightRed,
            ],
        }
    }
}

impl Theme {
    pub fn from_str(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "dracula" => Self {
                background: Color::Rgb(40, 42, 54),
                text: Color::Rgb(248, 248, 242),
                title: Color::Rgb(189, 147, 249),           // Purple
                border: Color::Rgb(98, 114, 164),           // Comment
                border_selected: Color::Rgb(255, 121, 198), // Pink
                legend_text: Color::Rgb(248, 248, 242),
                legend_dim: Color::Rgb(98, 114, 164),
                palette: vec![
                    Color::Rgb(139, 233, 253), // Cyan
                    Color::Rgb(80, 250, 123),  // Green
                    Color::Rgb(255, 184, 108), // Orange
                    Color::Rgb(255, 121, 198), // Pink
                    Color::Rgb(189, 147, 249), // Purple
                    Color::Rgb(255, 85, 85),   // Red
                ],
            },
            "monokai" => Self {
                background: Color::Rgb(39, 40, 34),
                text: Color::Rgb(248, 248, 242),
                title: Color::Rgb(102, 217, 239), // Blue
                border: Color::Rgb(117, 113, 94),
                border_selected: Color::Rgb(253, 151, 31), // Orange
                legend_text: Color::Rgb(248, 248, 242),
                legend_dim: Color::Rgb(117, 113, 94),
                palette: vec![
                    Color::Rgb(166, 226, 46),  // Green
                    Color::Rgb(102, 217, 239), // Blue
                    Color::Rgb(249, 38, 114),  // Pink
                    Color::Rgb(253, 151, 31),  // Orange
                    Color::Rgb(174, 129, 255), // Purple
                ],
            },
            _ => Self::default(),
        }
    }
}
