//! Background observer for DeepSeek peak/off-peak pricing transitions.
//!
//! A single Rust-owned task sleeps until the next pricing transition, then
//! emits a typed Tauri event with the fresh status and fires the notification
//! once. This replaces the previous per-webview polling hook so the four
//! surfaces (main, settings, float-bar, detached flyout) no longer each own a
//! minute timer or duplicate the backend read.

use std::sync::Mutex;
use std::time::Duration;

use chrono::Utc;
use tauri::Manager;

use crate::commands::build_deepseek_pricing_status;
use crate::events;
use crate::state::AppState;

/// Cap the sleep so a long off-peak stretch (e.g. 10:00→01:00) still wakes
/// often enough to stay responsive to system clock jumps and to re-check
/// whether DeepSeek was enabled/disabled in settings.
const MAX_SLEEP: Duration = Duration::from_secs(15 * 60);

/// Install the DeepSeek pricing observer. The task runs for the lifetime of
/// the app; it emits `codexbar:deepseek-pricing` on each transition and fires
/// the advisory toast once per observed period change.
pub fn install(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let now = Utc::now();
            let schedule = codexbar::providers::deepseek::pricing::status_at(now);

            // Fire the notification once on the period that just started.
            let settings = codexbar::settings::Settings::load();
            if settings.enabled_providers.contains("deepseek") {
                if let Some(state) = app.try_state::<Mutex<AppState>>()
                    && let Ok(mut guard) = state.lock()
                {
                    guard
                        .notification_manager
                        .notify_pricing_transition(schedule.period, &settings);
                }
                // Emit the fresh status to every webview.
                let payload = build_deepseek_pricing_status(now);
                events::emit_deepseek_pricing_changed(&app, &payload);
            } else {
                // DeepSeek disabled — reset the tracked period so the next
                // enablement fires the notification as a real transition.
                if let Some(state) = app.try_state::<Mutex<AppState>>()
                    && let Ok(mut guard) = state.lock()
                {
                    guard.notification_manager.reset_pricing_period();
                }
            }

            // Sleep until the next transition (capped) so we wake right at the
            // boundary and re-emit.
            let sleep = match schedule.next_transition {
                Some(next) => {
                    let delta = (next - now).to_std().unwrap_or(Duration::from_secs(60));
                    if delta > MAX_SLEEP {
                        MAX_SLEEP
                    } else if delta.is_zero() {
                        Duration::from_secs(60)
                    } else {
                        delta
                    }
                }
                None => Duration::from_secs(60),
            };
            tokio::time::sleep(sleep).await;
        }
    });
}
