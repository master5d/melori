use crate::coach_progress::{
    best_streak, current_streak, period_summary, rolling_baseline, trend_series, Baseline,
    CoachDashboard, TrendWindow,
};
use crate::managers::history::HistoryManager;
use chrono::Local;
use std::sync::Arc;
use tauri::State;

const WINDOW_DAYS: i64 = 7;
const BASELINE_LOOKBACK_DAYS: i64 = 30;
const BASELINE_MIN_SESSIONS: usize = 3;

fn local_tz_offset() -> i64 {
    use chrono::Offset;
    Local::now().offset().fix().local_minus_utc() as i64
}

#[tauri::command]
#[specta::specta]
pub async fn get_coach_dashboard(
    history_manager: State<'_, Arc<HistoryManager>>,
    window: TrendWindow,
) -> Result<CoachDashboard, String> {
    let rows = history_manager
        .get_coach_sessions(None)
        .map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().timestamp();
    let tz = local_tz_offset();
    Ok(CoachDashboard {
        summary: period_summary(&rows, now, tz, WINDOW_DAYS),
        trend: trend_series(&rows, now, tz, window),
        current_streak: current_streak(&rows, now, tz),
        best_streak: best_streak(&rows, tz),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn get_coach_baseline(
    history_manager: State<'_, Arc<HistoryManager>>,
) -> Result<Option<Baseline>, String> {
    let rows = history_manager
        .get_coach_sessions(None)
        .map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().timestamp();
    let tz = local_tz_offset();
    Ok(rolling_baseline(
        &rows,
        now,
        tz,
        BASELINE_LOOKBACK_DAYS,
        BASELINE_MIN_SESSIONS,
    ))
}
