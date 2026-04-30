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

pub(crate) fn format_si(val: f64) -> String {
    let abs = val.abs();
    if abs >= 1e9 {
        format!("{:.2}G", val / 1e9)
    } else if abs >= 1e6 {
        format!("{:.2}M", val / 1e6)
    } else if abs >= 1e3 {
        format!("{:.2}k", val / 1e3)
    } else {
        format!("{:.2}", val)
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
