/*
 * Copyright 2025 Federico D'Ambrosio
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use ratatui::style::Color;

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct DisplayFormat {
    pub(crate) unit: Option<String>,
    pub(crate) decimals: Option<usize>,
    pub(crate) no_value: Option<String>,
}

impl DisplayFormat {
    pub(crate) fn format_value(&self, value: Option<f64>) -> String {
        value
            .map(|value| self.format_number(value))
            .unwrap_or_else(|| self.no_value.clone().unwrap_or_else(|| "-".to_string()))
    }

    pub(crate) fn format_number(&self, value: f64) -> String {
        // Keep this first pass intentionally small: these common Grafana units
        // unlock most imported dashboard readability while unknown units retain
        // Grafatui's previous compact SI behavior instead of rendering worse.
        match self.unit_key().as_deref() {
            Some("bytes" | "decbytes") => format_scaled(
                value,
                &["B", "KB", "MB", "GB", "TB", "PB"],
                self.decimals,
                "",
            ),
            Some("bits" | "decbits") => format_scaled(
                value,
                &["b", "Kb", "Mb", "Gb", "Tb", "Pb"],
                self.decimals,
                "",
            ),
            Some("s" | "seconds") => format_duration_seconds(value, self.decimals),
            Some("ms") => format_suffix(value, self.decimals, "ms"),
            Some("percent") => format_suffix(value, self.decimals, "%"),
            // Grafana's percentunit stores ratios as 0.0-1.0, so scale to the
            // user-facing percent text dashboards expect.
            Some("percentunit") => format_suffix(value * 100.0, self.decimals, "%"),
            Some("ops") => format_rate(value, self.decimals, " ops/s"),
            Some("reqps" | "rps") => format_rate(value, self.decimals, " req/s"),
            Some("bps" | "bytes/sec" | "bytes/s") => format_scaled(
                value,
                &["B/s", "KB/s", "MB/s", "GB/s", "TB/s", "PB/s"],
                self.decimals,
                "",
            ),
            Some("short" | "none") | None => format_si_with_decimals(value, self.decimals),
            Some(_) => format_si_with_decimals(value, self.decimals),
        }
    }

    fn unit_key(&self) -> Option<String> {
        self.unit
            .as_deref()
            .map(str::trim)
            .filter(|unit| !unit.is_empty())
            .map(|unit| unit.to_ascii_lowercase())
    }
}

/// Maps a normalized value (0.0-1.0) to a heatmap color (blue -> green -> yellow -> red)
pub(crate) fn value_to_heatmap_color(normalized: f64) -> Color {
    // Use a simple color gradient for heatmap
    // 0.0 = Blue (cold), 0.5 = Yellow, 1.0 = Red (hot)
    if normalized < 0.33 {
        // Blue to Cyan
        Color::Cyan
    } else if normalized < 0.66 {
        // Yellow/Green
        Color::Yellow
    } else {
        // Red/Magenta for hot values
        Color::Red
    }
}

fn format_si_with_decimals(val: f64, decimals: Option<usize>) -> String {
    let abs = val.abs();
    let decimals = decimals.unwrap_or(2);
    if abs >= 1e9 {
        format!("{:.*}G", decimals, val / 1e9)
    } else if abs >= 1e6 {
        format!("{:.*}M", decimals, val / 1e6)
    } else if abs >= 1e3 {
        format!("{:.*}k", decimals, val / 1e3)
    } else {
        format!("{:.*}", decimals, val)
    }
}

fn format_suffix(value: f64, decimals: Option<usize>, suffix: &str) -> String {
    format!("{:.*}{suffix}", decimals.unwrap_or(2), value)
}

fn format_rate(value: f64, decimals: Option<usize>, suffix: &str) -> String {
    format!("{}{suffix}", format_si_with_decimals(value, decimals))
}

fn format_scaled(
    value: f64,
    suffixes: &[&str],
    decimals: Option<usize>,
    separator: &str,
) -> String {
    let mut scaled = value;
    let mut suffix_index = 0;

    while scaled.abs() >= 1000.0 && suffix_index + 1 < suffixes.len() {
        scaled /= 1000.0;
        suffix_index += 1;
    }

    format!(
        "{:.*}{separator}{}",
        decimals.unwrap_or(2),
        scaled,
        suffixes[suffix_index]
    )
}

fn format_duration_seconds(value: f64, decimals: Option<usize>) -> String {
    let abs = value.abs();
    if abs >= 86_400.0 {
        format_suffix(value / 86_400.0, decimals, "d")
    } else if abs >= 3_600.0 {
        format_suffix(value / 3_600.0, decimals, "h")
    } else if abs >= 60.0 {
        format_suffix(value / 60.0, decimals, "m")
    } else {
        format_suffix(value, decimals, "s")
    }
}

pub(crate) fn format_time(ts: f64) -> String {
    use chrono::TimeZone;
    if let Some(dt) = chrono::Utc.timestamp_opt(ts as i64, 0).single() {
        dt.format("%H:%M:%S").to_string()
    } else {
        format!("{}", ts)
    }
}

pub(crate) fn format_axis_time(ts: f64, range_secs: f64) -> String {
    use chrono::{TimeZone, Timelike};

    const DAY: f64 = 24.0 * 60.0 * 60.0;
    if range_secs < DAY {
        return format_time(ts);
    }

    let Some(dt) = chrono::Utc.timestamp_opt(ts as i64, 0).single() else {
        return format!("{}", ts);
    };

    if range_secs < 7.0 * DAY {
        if dt.hour() == 0 && dt.minute() == 0 && dt.second() == 0 {
            dt.format("%b %d").to_string()
        } else {
            dt.format("%b %d %Hh").to_string()
        }
    } else if range_secs < 90.0 * DAY {
        dt.format("%b %d").to_string()
    } else if range_secs < 730.0 * DAY {
        dt.format("%Y-%m").to_string()
    } else {
        dt.format("%Y").to_string()
    }
}

/// Generate a color from a string using hash-based approach.
/// Uses HSL color space to ensure visually distinct, vibrant colors.
pub(crate) fn get_hash_color(name: &str) -> Color {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    let hash = hasher.finish();

    // Use HSL color space for better color distribution
    // Hue: use the hash to get different hues (0-360 degrees)
    let hue = (hash % 360) as f32;

    // Saturation: keep high for vibrant colors (60-90%)
    let saturation = 60.0 + ((hash >> 8) % 30) as f32;

    // Lightness: keep in a range that's visible on both light and dark backgrounds (45-65%)
    let lightness = 45.0 + ((hash >> 16) % 20) as f32;

    hsl_to_rgb(hue, saturation, lightness)
}

/// Convert HSL to RGB color for ratatui.
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> Color {
    let s = s / 100.0;
    let l = l / 100.0;

    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    Color::Rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_format_uses_si_for_missing_and_unknown_units() {
        let default = DisplayFormat::default();
        assert_eq!(default.format_number(1_500.0), "1.50k");

        let unknown = DisplayFormat {
            unit: Some("widgets".to_string()),
            decimals: None,
            no_value: None,
        };
        assert_eq!(unknown.format_number(1_500.0), "1.50k");
    }

    #[test]
    fn test_display_format_scales_bytes_and_bits() {
        let bytes = DisplayFormat {
            unit: Some("bytes".to_string()),
            decimals: None,
            no_value: None,
        };
        assert_eq!(bytes.format_number(1_536.0), "1.54KB");

        let bits = DisplayFormat {
            unit: Some("bits".to_string()),
            decimals: Some(1),
            no_value: None,
        };
        assert_eq!(bits.format_number(1_536.0), "1.5Kb");
    }

    #[test]
    fn test_display_format_percent_and_percentunit() {
        let percent = DisplayFormat {
            unit: Some("percent".to_string()),
            decimals: Some(1),
            no_value: None,
        };
        assert_eq!(percent.format_number(42.42), "42.4%");

        let percent_unit = DisplayFormat {
            unit: Some("percentunit".to_string()),
            decimals: Some(0),
            no_value: None,
        };
        assert_eq!(percent_unit.format_number(0.4242), "42%");
    }

    #[test]
    fn test_display_format_decimals_no_value_and_rates() {
        let rate = DisplayFormat {
            unit: Some("reqps".to_string()),
            decimals: Some(0),
            no_value: Some("n/a".to_string()),
        };
        assert_eq!(rate.format_number(1234.56), "1k req/s");
        assert_eq!(rate.format_value(None), "n/a");

        let default = DisplayFormat::default();
        assert_eq!(default.format_value(None), "-");
    }

    #[test]
    fn test_format_axis_time_uses_time_for_short_ranges() {
        use chrono::TimeZone;

        let ts = chrono::Utc
            .with_ymd_and_hms(2026, 4, 30, 12, 34, 56)
            .single()
            .unwrap()
            .timestamp() as f64;

        assert_eq!(format_axis_time(ts, 60.0 * 60.0), "12:34:56");
    }

    #[test]
    fn test_format_axis_time_uses_date_for_multi_day_midnight_ticks() {
        use chrono::TimeZone;

        let ts = chrono::Utc
            .with_ymd_and_hms(2026, 4, 30, 0, 0, 0)
            .single()
            .unwrap()
            .timestamp() as f64;

        assert_eq!(format_axis_time(ts, 2.0 * 24.0 * 60.0 * 60.0), "Apr 30");
    }

    #[test]
    fn test_format_axis_time_keeps_hour_for_multi_day_non_midnight_ticks() {
        use chrono::TimeZone;

        let ts = chrono::Utc
            .with_ymd_and_hms(2026, 4, 30, 12, 0, 0)
            .single()
            .unwrap()
            .timestamp() as f64;

        assert_eq!(format_axis_time(ts, 2.0 * 24.0 * 60.0 * 60.0), "Apr 30 12h");
    }

    #[test]
    fn test_format_axis_time_scales_to_wider_ranges() {
        use chrono::TimeZone;

        let ts = chrono::Utc
            .with_ymd_and_hms(2026, 4, 30, 12, 0, 0)
            .single()
            .unwrap()
            .timestamp() as f64;
        let day = 24.0 * 60.0 * 60.0;

        assert_eq!(format_axis_time(ts, 14.0 * day), "Apr 30");
        assert_eq!(format_axis_time(ts, 120.0 * day), "2026-04");
        assert_eq!(format_axis_time(ts, 800.0 * day), "2026");
    }
}
