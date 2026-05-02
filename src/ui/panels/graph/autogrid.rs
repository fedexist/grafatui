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

pub(super) fn calculate_value_grid_ticks(y_bounds: [f64; 2], chart_height: u16) -> Vec<f64> {
    let min = y_bounds[0];
    let max = y_bounds[1];
    if !min.is_finite() || !max.is_finite() || max <= min || chart_height < 4 {
        return Vec::new();
    }

    let target_lines = (usize::from(chart_height) / 6).clamp(2, 4);
    let step = nice_grid_step(max - min, target_lines);
    if step <= 0.0 || !step.is_finite() {
        return Vec::new();
    }

    let mut ticks = Vec::new();
    let mut tick = (min / step).ceil() * step;
    while tick < max {
        if tick > min {
            ticks.push(tick);
        }
        tick += step;
    }
    ticks
}

fn nice_grid_step(range: f64, target_lines: usize) -> f64 {
    if range <= 0.0 || !range.is_finite() || target_lines == 0 {
        return 0.0;
    }

    let raw_step = range / target_lines as f64;
    let magnitude = 10_f64.powf(raw_step.log10().floor());
    let fraction = raw_step / magnitude;
    let nice_fraction = if fraction <= 1.0 {
        1.0
    } else if fraction <= 2.0 {
        2.0
    } else if fraction <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice_fraction * magnitude
}

pub(super) fn calculate_time_grid_ticks(start: f64, end: f64, chart_width: u16) -> Vec<f64> {
    if !start.is_finite() || !end.is_finite() || end <= start || chart_width < 8 {
        return Vec::new();
    }

    let range = end - start;
    let mut step = base_time_grid_step(range);
    let max_ticks = (usize::from(chart_width) / 20).clamp(3, 8);
    while count_interior_ticks(start, end, step) > max_ticks {
        step = next_time_grid_step(step);
    }

    let mut ticks = Vec::new();
    let mut tick = (start / step).ceil() * step;
    while tick < end {
        if tick > start {
            ticks.push(tick);
        }
        tick += step;
    }
    ticks
}

fn base_time_grid_step(range: f64) -> f64 {
    const MINUTE: f64 = 60.0;
    const HOUR: f64 = 60.0 * MINUTE;
    const DAY: f64 = 24.0 * HOUR;

    if range <= 10.0 * MINUTE {
        MINUTE
    } else if range <= 30.0 * MINUTE {
        5.0 * MINUTE
    } else if range <= 90.0 * MINUTE {
        30.0 * MINUTE
    } else if range <= 3.0 * HOUR {
        HOUR
    } else if range <= 6.0 * HOUR {
        2.0 * HOUR
    } else if range <= 12.0 * HOUR {
        3.0 * HOUR
    } else if range <= DAY {
        6.0 * HOUR
    } else if range <= 2.0 * DAY {
        12.0 * HOUR
    } else {
        DAY
    }
}

fn next_time_grid_step(step: f64) -> f64 {
    const STEPS: [f64; 10] = [
        60.0, 300.0, 600.0, 1800.0, 3600.0, 7200.0, 10800.0, 21600.0, 43200.0, 86400.0,
    ];

    STEPS
        .iter()
        .copied()
        .find(|candidate| *candidate > step)
        .unwrap_or(step * 2.0)
}

fn count_interior_ticks(start: f64, end: f64, step: f64) -> usize {
    if step <= 0.0 {
        return 0;
    }

    let mut count = 0;
    let mut tick = (start / step).ceil() * step;
    while tick < end {
        if tick > start {
            count += 1;
        }
        tick += step;
    }
    count
}

pub(super) fn build_autogrid_datasets(
    x_bounds: [f64; 2],
    y_bounds: [f64; 2],
    time_ticks: &[f64],
    value_ticks: &[f64],
    plot_width: u16,
    plot_height: u16,
) -> Vec<Vec<(f64, f64)>> {
    if x_bounds[1] <= x_bounds[0] || y_bounds[1] <= y_bounds[0] {
        return Vec::new();
    }

    let mut datasets = Vec::new();
    let vertical_samples = usize::from(plot_height).saturating_mul(4).max(2);
    let horizontal_samples = usize::from(plot_width).saturating_mul(2).max(2);

    for tick in time_ticks {
        if *tick <= x_bounds[0] || *tick >= x_bounds[1] {
            continue;
        }
        datasets.push(
            (0..=vertical_samples)
                .map(|i| {
                    let y = interpolate(y_bounds[0], y_bounds[1], i, vertical_samples);
                    (*tick, y)
                })
                .collect(),
        );
    }

    for tick in value_ticks {
        if *tick <= y_bounds[0] || *tick >= y_bounds[1] {
            continue;
        }
        datasets.push(
            (0..=horizontal_samples)
                .map(|i| {
                    let x = interpolate(x_bounds[0], x_bounds[1], i, horizontal_samples);
                    (x, *tick)
                })
                .collect(),
        );
    }
    datasets
}

fn interpolate(start: f64, end: f64, index: usize, total: usize) -> f64 {
    start + (end - start) * index as f64 / total as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_value_grid_ticks_round_values() {
        let ticks = calculate_value_grid_ticks([329.0, 1287.0], 20);
        assert_eq!(ticks, vec![500.0, 1000.0]);
    }

    #[test]
    fn test_calculate_value_grid_ticks_excludes_boundaries() {
        let ticks = calculate_value_grid_ticks([0.0, 100.0], 20);
        assert!(!ticks.contains(&0.0));
        assert!(!ticks.contains(&100.0));
    }

    #[test]
    fn test_calculate_value_grid_ticks_invalid_ranges() {
        assert!(calculate_value_grid_ticks([1.0, 1.0], 20).is_empty());
        assert!(calculate_value_grid_ticks([2.0, 1.0], 20).is_empty());
        assert!(calculate_value_grid_ticks([f64::NAN, 1.0], 20).is_empty());
        assert!(calculate_value_grid_ticks([0.0, 1.0], 3).is_empty());
    }

    #[test]
    fn test_calculate_time_grid_ticks_two_hour_window() {
        let start = 41_820.0; // 11:37 UTC
        let end = start + 2.0 * 60.0 * 60.0;

        let ticks = calculate_time_grid_ticks(start, end, 80);

        assert_eq!(ticks, vec![43_200.0, 46_800.0]); // 12:00, 13:00 UTC
    }

    #[test]
    fn test_calculate_time_grid_ticks_one_hour_window() {
        let start = 44_520.0; // 12:22 UTC
        let end = start + 60.0 * 60.0;

        let ticks = calculate_time_grid_ticks(start, end, 80);

        assert_eq!(ticks, vec![45_000.0, 46_800.0]); // 12:30, 13:00 UTC
    }

    #[test]
    fn test_calculate_time_grid_ticks_five_minute_window() {
        let start = 43_335.0; // 12:02:15 UTC
        let end = start + 5.0 * 60.0;

        let ticks = calculate_time_grid_ticks(start, end, 120);

        assert_eq!(
            ticks,
            vec![43_380.0, 43_440.0, 43_500.0, 43_560.0, 43_620.0]
        );
    }

    #[test]
    fn test_build_autogrid_datasets() {
        let datasets = build_autogrid_datasets([0.0, 10.0], [0.0, 10.0], &[5.0], &[5.0], 10, 5);

        assert_eq!(datasets.len(), 2);
        assert_eq!(datasets[0].first(), Some(&(5.0, 0.0)));
        assert_eq!(datasets[0].last(), Some(&(5.0, 10.0)));
        assert_eq!(datasets[0].len(), 21);
        assert_eq!(datasets[1].first(), Some(&(0.0, 5.0)));
        assert_eq!(datasets[1].last(), Some(&(10.0, 5.0)));
        assert_eq!(datasets[1].len(), 21);
    }
}
