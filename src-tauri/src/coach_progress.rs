use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Debug, PartialEq)]
pub struct SessionRow {
    pub timestamp: i64, // unix seconds
    pub word_count: u32,
    pub duration_ms: u64,
    pub wpm: u32,
    pub filler_total: u32,
    pub weak_total: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Type)]
pub enum TrendWindow {
    Days7,
    Days30,
    All,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct PeriodSummary {
    pub avg_wpm: u32,
    pub avg_filler_rate: f32,
    pub session_count: u32,
    pub prev_avg_wpm: u32,
    pub prev_avg_filler_rate: f32,
    pub prev_session_count: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct TrendPoint {
    pub day: i64, // day index (days since epoch, local)
    pub avg_wpm: u32,
    pub avg_filler_rate: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct Baseline {
    pub avg_wpm: u32,
    pub avg_filler_rate: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct CoachDashboard {
    pub summary: PeriodSummary,
    pub trend: Vec<TrendPoint>,
    pub current_streak: u32,
    pub best_streak: u32,
}

const SECS_PER_DAY: i64 = 86_400;

/// Local-day index for a unix timestamp given the local tz offset (seconds).
pub fn day_index(ts: i64, tz_offset_secs: i64) -> i64 {
    (ts + tz_offset_secs).div_euclid(SECS_PER_DAY)
}

fn filler_rate(filler_total: u32, word_count: u32) -> f32 {
    if word_count == 0 {
        0.0
    } else {
        filler_total as f32 / word_count as f32 * 100.0
    }
}

/// avg wpm + avg filler-rate (per-session mean) + count over a slice.
fn aggregate(rows: &[&SessionRow]) -> (u32, f32, u32) {
    if rows.is_empty() {
        return (0, 0.0, 0);
    }
    let n = rows.len() as f32;
    let wpm = rows.iter().map(|r| r.wpm as f32).sum::<f32>() / n;
    let fr = rows
        .iter()
        .map(|r| filler_rate(r.filler_total, r.word_count))
        .sum::<f32>()
        / n;
    (wpm.round() as u32, fr, rows.len() as u32)
}

pub fn period_summary(
    rows: &[SessionRow],
    now: i64,
    tz_offset: i64,
    window_days: i64,
) -> PeriodSummary {
    let today = day_index(now, tz_offset);
    let cur_start = today - (window_days - 1); // inclusive window of `window_days` days ending today
    let prev_start = cur_start - window_days;
    let cur: Vec<&SessionRow> = rows
        .iter()
        .filter(|r| day_index(r.timestamp, tz_offset) >= cur_start)
        .collect();
    let prev: Vec<&SessionRow> = rows
        .iter()
        .filter(|r| {
            let d = day_index(r.timestamp, tz_offset);
            d >= prev_start && d < cur_start
        })
        .collect();
    let (avg_wpm, avg_filler_rate, session_count) = aggregate(&cur);
    let (prev_avg_wpm, prev_avg_filler_rate, prev_session_count) = aggregate(&prev);
    PeriodSummary {
        avg_wpm,
        avg_filler_rate,
        session_count,
        prev_avg_wpm,
        prev_avg_filler_rate,
        prev_session_count,
    }
}

pub fn trend_series(
    rows: &[SessionRow],
    now: i64,
    tz_offset: i64,
    window: TrendWindow,
) -> Vec<TrendPoint> {
    let today = day_index(now, tz_offset);
    let min_day = match window {
        TrendWindow::Days7 => Some(today - 6),
        TrendWindow::Days30 => Some(today - 29),
        TrendWindow::All => None,
    };
    use std::collections::BTreeMap;
    let mut by_day: BTreeMap<i64, Vec<&SessionRow>> = BTreeMap::new();
    for r in rows {
        let d = day_index(r.timestamp, tz_offset);
        if min_day.map(|m| d >= m).unwrap_or(true) {
            by_day.entry(d).or_default().push(r);
        }
    }
    by_day
        .into_iter()
        .map(|(day, rs)| {
            let (avg_wpm, avg_filler_rate, _) = aggregate(&rs);
            TrendPoint {
                day,
                avg_wpm,
                avg_filler_rate,
            }
        })
        .collect()
}

fn distinct_days_desc(rows: &[SessionRow], tz_offset: i64) -> Vec<i64> {
    use std::collections::BTreeSet;
    let set: BTreeSet<i64> = rows
        .iter()
        .map(|r| day_index(r.timestamp, tz_offset))
        .collect();
    let mut v: Vec<i64> = set.into_iter().collect();
    v.sort_unstable_by(|a, b| b.cmp(a)); // descending
    v
}

/// Consecutive days ending today or yesterday (one-day grace).
pub fn current_streak(rows: &[SessionRow], now: i64, tz_offset: i64) -> u32 {
    let today = day_index(now, tz_offset);
    let days = distinct_days_desc(rows, tz_offset);
    if days.is_empty() {
        return 0;
    }
    let first = days[0];
    if first != today && first != today - 1 {
        return 0; // streak broken (no practice today or yesterday)
    }
    let mut streak = 1u32;
    let mut expected = first - 1;
    for &d in &days[1..] {
        if d == expected {
            streak += 1;
            expected -= 1;
        } else {
            break;
        }
    }
    streak
}

pub fn best_streak(rows: &[SessionRow], tz_offset: i64) -> u32 {
    let mut days = distinct_days_desc(rows, tz_offset);
    days.sort_unstable(); // ascending
    let mut best = 0u32;
    let mut run = 0u32;
    let mut prev: Option<i64> = None;
    for d in days {
        run = match prev {
            Some(p) if d == p + 1 => run + 1,
            _ => 1,
        };
        best = best.max(run);
        prev = Some(d);
    }
    best
}

/// Rolling baseline over the trailing `lookback_days`; falls back to all rows
/// if fewer than `min_sessions` fall in the window; `None` below `min_sessions`.
pub fn rolling_baseline(
    rows: &[SessionRow],
    now: i64,
    tz_offset: i64,
    lookback_days: i64,
    min_sessions: usize,
) -> Option<Baseline> {
    let today = day_index(now, tz_offset);
    let start = today - (lookback_days - 1);
    let in_window: Vec<&SessionRow> = rows
        .iter()
        .filter(|r| day_index(r.timestamp, tz_offset) >= start)
        .collect();
    let chosen: Vec<&SessionRow> = if in_window.len() >= min_sessions {
        in_window
    } else {
        rows.iter().collect()
    };
    if chosen.len() < min_sessions {
        return None;
    }
    let (avg_wpm, avg_filler_rate, _) = aggregate(&chosen);
    Some(Baseline {
        avg_wpm,
        avg_filler_rate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_700_000_000;

    fn row(day_offset_from_now: i64, wpm: u32, fillers: u32, words: u32) -> SessionRow {
        SessionRow {
            timestamp: NOW - day_offset_from_now * SECS_PER_DAY,
            word_count: words,
            duration_ms: 60_000,
            wpm,
            filler_total: fillers,
            weak_total: 0,
        }
    }

    #[test]
    fn period_summary_splits_current_and_previous_windows() {
        let rows = vec![
            row(1, 100, 5, 100), // current
            row(3, 140, 1, 100), // current
            row(8, 80, 10, 100), // previous
        ];
        let s = period_summary(&rows, NOW, 0, 7);
        assert_eq!(s.session_count, 2);
        assert_eq!(s.avg_wpm, 120); // (100+140)/2
        assert_eq!(s.prev_session_count, 1);
        assert_eq!(s.prev_avg_wpm, 80);
    }

    #[test]
    fn trend_buckets_same_day_into_one_point() {
        let rows = vec![
            row(1, 100, 0, 100),
            row(1, 200, 0, 100),
            row(2, 150, 0, 100),
        ];
        let pts = trend_series(&rows, NOW, 0, TrendWindow::Days7);
        assert_eq!(pts.len(), 2);
        let day1 = day_index(NOW, 0) - 1;
        let p = pts.iter().find(|p| p.day == day1).unwrap();
        assert_eq!(p.avg_wpm, 150);
    }

    #[test]
    fn current_streak_counts_consecutive_with_grace() {
        let rows = vec![
            row(0, 100, 0, 100),
            row(1, 100, 0, 100),
            row(2, 100, 0, 100),
        ];
        assert_eq!(current_streak(&rows, NOW, 0), 3);
        let rows2 = vec![row(0, 100, 0, 100), row(3, 100, 0, 100)];
        assert_eq!(current_streak(&rows2, NOW, 0), 1);
        let rows3 = vec![row(2, 100, 0, 100)];
        assert_eq!(current_streak(&rows3, NOW, 0), 0);
        assert_eq!(current_streak(&[], NOW, 0), 0);
    }

    #[test]
    fn best_streak_finds_longest_run() {
        let rows = vec![
            row(12, 1, 0, 1),
            row(11, 1, 0, 1),
            row(10, 1, 0, 1),
            row(5, 1, 0, 1),
            row(2, 1, 0, 1),
            row(1, 1, 0, 1),
        ];
        assert_eq!(best_streak(&rows, 0), 3);
    }

    #[test]
    fn baseline_hidden_below_min_sessions() {
        let rows = vec![row(1, 100, 5, 100), row(2, 100, 5, 100)];
        assert_eq!(rolling_baseline(&rows, NOW, 0, 30, 3), None);
        let rows3 = vec![
            row(1, 120, 6, 100),
            row(2, 120, 6, 100),
            row(3, 120, 6, 100),
        ];
        let b = rolling_baseline(&rows3, NOW, 0, 30, 3).unwrap();
        assert_eq!(b.avg_wpm, 120);
        assert_eq!(b.avg_filler_rate, 6.0);
    }

    #[test]
    fn empty_inputs_are_safe() {
        let s = period_summary(&[], NOW, 0, 7);
        assert_eq!((s.avg_wpm, s.session_count), (0, 0));
        assert!(trend_series(&[], NOW, 0, TrendWindow::All).is_empty());
        assert_eq!(best_streak(&[], 0), 0);
    }
}
