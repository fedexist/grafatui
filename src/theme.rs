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
            "solarized-dark" => Self {
                background: Color::Rgb(0, 43, 54),
                text: Color::Rgb(131, 148, 150),
                title: Color::Rgb(38, 139, 210), // Blue
                border: Color::Rgb(88, 110, 117),
                border_selected: Color::Rgb(181, 137, 0), // Yellow
                legend_text: Color::Rgb(131, 148, 150),
                legend_dim: Color::Rgb(88, 110, 117),
                palette: vec![
                    Color::Rgb(181, 137, 0),   // Yellow
                    Color::Rgb(203, 75, 22),   // Orange
                    Color::Rgb(220, 50, 47),   // Red
                    Color::Rgb(211, 54, 130),  // Magenta
                    Color::Rgb(108, 113, 196), // Violet
                    Color::Rgb(38, 139, 210),  // Blue
                    Color::Rgb(42, 161, 152),  // Cyan
                    Color::Rgb(133, 153, 0),   // Green
                ],
            },
            "solarized-light" => Self {
                background: Color::Rgb(253, 246, 227),
                text: Color::Rgb(101, 123, 131),
                title: Color::Rgb(38, 139, 210), // Blue
                border: Color::Rgb(147, 161, 161),
                border_selected: Color::Rgb(181, 137, 0), // Yellow
                legend_text: Color::Rgb(101, 123, 131),
                legend_dim: Color::Rgb(147, 161, 161),
                palette: vec![
                    Color::Rgb(181, 137, 0),   // Yellow
                    Color::Rgb(203, 75, 22),   // Orange
                    Color::Rgb(220, 50, 47),   // Red
                    Color::Rgb(211, 54, 130),  // Magenta
                    Color::Rgb(108, 113, 196), // Violet
                    Color::Rgb(38, 139, 210),  // Blue
                    Color::Rgb(42, 161, 152),  // Cyan
                    Color::Rgb(133, 153, 0),   // Green
                ],
            },
            "gruvbox" => Self {
                background: Color::Rgb(40, 40, 40),
                text: Color::Rgb(235, 219, 178),
                title: Color::Rgb(215, 153, 33), // Yellow
                border: Color::Rgb(146, 131, 116),
                border_selected: Color::Rgb(254, 128, 25), // Orange
                legend_text: Color::Rgb(235, 219, 178),
                legend_dim: Color::Rgb(146, 131, 116),
                palette: vec![
                    Color::Rgb(204, 36, 29),   // Red
                    Color::Rgb(152, 151, 26),  // Green
                    Color::Rgb(215, 153, 33),  // Yellow
                    Color::Rgb(69, 133, 136),  // Blue
                    Color::Rgb(177, 98, 134),  // Purple
                    Color::Rgb(104, 157, 106), // Aqua
                    Color::Rgb(254, 128, 25),  // Orange
                ],
            },
            "tokyo-night" => Self {
                background: Color::Rgb(26, 27, 38),
                text: Color::Rgb(169, 177, 214),
                title: Color::Rgb(122, 162, 247), // Blue
                border: Color::Rgb(86, 95, 137),
                border_selected: Color::Rgb(255, 158, 100), // Orange
                legend_text: Color::Rgb(169, 177, 214),
                legend_dim: Color::Rgb(86, 95, 137),
                palette: vec![
                    Color::Rgb(247, 118, 142), // Red
                    Color::Rgb(158, 206, 106), // Green
                    Color::Rgb(224, 175, 104), // Yellow
                    Color::Rgb(122, 162, 247), // Blue
                    Color::Rgb(187, 154, 247), // Magenta
                    Color::Rgb(125, 207, 255), // Cyan
                    Color::Rgb(255, 158, 100), // Orange
                ],
            },
            "catppuccin" => Self {
                background: Color::Rgb(30, 30, 46),
                text: Color::Rgb(205, 214, 244),
                title: Color::Rgb(137, 180, 250), // Blue
                border: Color::Rgb(88, 91, 112),
                border_selected: Color::Rgb(249, 226, 175), // Yellow
                legend_text: Color::Rgb(205, 214, 244),
                legend_dim: Color::Rgb(88, 91, 112),
                palette: vec![
                    Color::Rgb(243, 139, 168), // Red
                    Color::Rgb(166, 227, 161), // Green
                    Color::Rgb(249, 226, 175), // Yellow
                    Color::Rgb(137, 180, 250), // Blue
                    Color::Rgb(203, 166, 247), // Mauve
                    Color::Rgb(148, 226, 213), // Teal
                    Color::Rgb(250, 179, 135), // Peach
                ],
            },
            _ => Self::default(),
        }
    }
}
