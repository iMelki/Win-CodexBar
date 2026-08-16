//! DeepSeek peak/off-peak pricing schedule.
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use serde::Serialize;

pub const EFFECTIVE_AT: DateTime<Utc> = DateTime::<Utc>::from_naive_utc_and_offset(
    NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2026, 8, 16).unwrap(),
        NaiveTime::from_hms_opt(16, 0, 0).unwrap(),
    ),
    Utc,
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PricingPeriod {
    Standard,
    Peak,
    OffPeak,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PricingScheduleStatus {
    pub period: PricingPeriod,
    pub next_transition: Option<DateTime<Utc>>,
}

/// Evaluate the official UTC schedule. Intervals are half-open: [start, end).
pub fn status_at(now: DateTime<Utc>) -> PricingScheduleStatus {
    if now < EFFECTIVE_AT {
        return PricingScheduleStatus {
            period: PricingPeriod::Standard,
            next_transition: Some(EFFECTIVE_AT),
        };
    }
    let today = [
        (
            NaiveTime::from_hms_opt(1, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(4, 0, 0).unwrap(),
            PricingPeriod::Peak,
        ),
        (
            NaiveTime::from_hms_opt(4, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(6, 0, 0).unwrap(),
            PricingPeriod::OffPeak,
        ),
        (
            NaiveTime::from_hms_opt(6, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(10, 0, 0).unwrap(),
            PricingPeriod::Peak,
        ),
    ];
    for (start, end, period) in today {
        let start =
            DateTime::<Utc>::from_naive_utc_and_offset(now.date_naive().and_time(start), Utc);
        let end = DateTime::<Utc>::from_naive_utc_and_offset(now.date_naive().and_time(end), Utc);
        if now >= start && now < end {
            return PricingScheduleStatus {
                period,
                next_transition: Some(end),
            };
        }
    }
    // 00:00-01:00 and 10:00-24:00 are off-peak. The next transition is the
    // upcoming 01:00 (the start of the peak window). Before 01:00 that is
    // today's 01:00; from 10:00 onward it is tomorrow's 01:00.
    let today_start = DateTime::<Utc>::from_naive_utc_and_offset(
        now.date_naive()
            .and_time(NaiveTime::from_hms_opt(1, 0, 0).unwrap()),
        Utc,
    );
    let next = if now < today_start {
        today_start
    } else {
        DateTime::<Utc>::from_naive_utc_and_offset(
            (now.date_naive() + chrono::Days::new(1))
                .and_time(NaiveTime::from_hms_opt(1, 0, 0).unwrap()),
            Utc,
        )
    };
    PricingScheduleStatus {
        period: PricingPeriod::OffPeak,
        next_transition: Some(next),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn at(h: u32, m: u32) -> DateTime<Utc> {
        DateTime::<Utc>::from_naive_utc_and_offset(
            NaiveDate::from_ymd_opt(2026, 8, 17)
                .unwrap()
                .and_hms_opt(h, m, 0)
                .unwrap(),
            Utc,
        )
    }
    #[test]
    fn pre_effective_is_standard() {
        let s = status_at(EFFECTIVE_AT - chrono::Duration::seconds(1));
        assert_eq!(s.period, PricingPeriod::Standard);
        assert_eq!(s.next_transition, Some(EFFECTIVE_AT));
    }
    #[test]
    fn peak_and_off_peak_boundaries_are_half_open() {
        assert_eq!(status_at(at(1, 0)).period, PricingPeriod::Peak);
        assert_eq!(status_at(at(4, 0)).period, PricingPeriod::OffPeak);
        assert_eq!(status_at(at(6, 0)).period, PricingPeriod::Peak);
        assert_eq!(status_at(at(10, 0)).period, PricingPeriod::OffPeak);
    }
    #[test]
    fn peak_transitions_returned() {
        assert_eq!(status_at(at(2, 0)).next_transition, Some(at(4, 0)));
        assert_eq!(status_at(at(5, 0)).next_transition, Some(at(6, 0)));
    }
    #[test]
    fn midnight_transition_is_today_not_tomorrow() {
        // 00:30 UTC is off-peak; the next transition is today's 01:00,
        // ~30 minutes away — not tomorrow's 01:00 (~24.5 hours away).
        let s = status_at(at(0, 30));
        assert_eq!(s.period, PricingPeriod::OffPeak);
        let next = s.next_transition.expect("next transition");
        assert_eq!(next, at(1, 0));
        let delta = next - at(0, 30);
        assert_eq!(delta.num_minutes(), 30);
    }
    #[test]
    fn late_off_peak_transitions_to_tomorrow_peak() {
        // 23:59 UTC is off-peak; the next transition is tomorrow's 01:00.
        let s = status_at(at(23, 59));
        assert_eq!(s.period, PricingPeriod::OffPeak);
        let next = s.next_transition.expect("next transition");
        assert_eq!(next, at(1, 0) + chrono::Days::new(1));
        let delta = next - at(23, 59);
        assert_eq!(delta.num_minutes(), 61);
    }
}
