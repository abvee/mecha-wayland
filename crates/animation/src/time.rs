//! Module holding the Time resource

use std::time::{Duration, Instant};
use app::prelude::*;

/// A sampled monotonic clock, independent of wall-clock time.
///
/// Starts at zero. The first update establishes the origin, with zero
/// delta, so application setup time is not counted. Later updates add the
/// actual interval, without clamping long gaps. Reads never sample the
/// clock or consume the delta.
///
/// Durations retain integer seconds and nanoseconds. Convert at the point
/// of interpolation with [`Duration::as_secs_f32`] or
/// [`Duration::as_secs_f64`], rather than accumulating floating-point time.
/// A duration-based animation can record `elapsed()` at its start and
/// subtract that from later samples to measure its own progress.
#[derive(Debug, Default)]
pub struct Time {
    last_update: Option<Instant>,
    elapsed: Duration,
    delta: Duration,
}

impl Resource for Time {}

impl Time {
    /// Time from the first sample to the latest sample.
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Time between the two most recent samples, zero before the second.
    /// This is an update interval, not a guaranteed frame interval.
    pub fn delta(&self) -> Duration {
        self.delta
    }

    /// Sample the system's monotonic clock.
    pub fn update(&mut self) {
        self.update_at(Instant::now());
    }

    /// Supply a sample explicitly, for deterministic tests or an external
    /// clock driver. Equal consecutive samples produce a zero delta.
    ///
    /// Use one clock driver: advancing to synthetic future instants and
    /// then calling [`Time::update`] can make the next sample go backwards.
    ///
    /// # Panics
    ///
    /// If `now` is earlier than the previous sample. The clock is unchanged.
    pub fn update_at(&mut self, now: Instant) {
        let delta = self.last_update.map_or(Duration::ZERO, |last| {
            now.checked_duration_since(last)
                .expect("Time cannot move backwards")
        });
        self.elapsed += delta;
        self.delta = delta;
        self.last_update = Some(now);
    }
}
