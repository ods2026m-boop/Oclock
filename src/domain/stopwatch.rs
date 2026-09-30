//! The stopwatch: high-resolution elapsed time from a monotonic clock.
//!
//! Structurally the same idea as the timer, with two additions that a
//! stopwatch needs: a **lap list** recording where the press happened, and
//! the derived per-lap statistics the UI ranks laps by.
//!
//! Elapsed time is never accumulated from callbacks. It is
//! `banked + (now - running_since)`, where `now` is a monotonic reading
//! supplied by the caller. A stalled render, a busy event loop or a
//! background job cannot make the stopwatch drift, because none of them
//! participate in the measurement.

use std::time::Duration;

/// One recorded lap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lap {
    /// 1-based lap number.
    pub index: usize,
    /// Total elapsed time when the lap was taken.
    pub total: Duration,
    /// The time this lap alone took.
    pub split: Duration,
}

/// Whether a lap was the fastest or slowest recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LapRank {
    Fastest,
    Slowest,
}

/// A stopwatch with a lap list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stopwatch {
    /// Elapsed time banked from completed run segments.
    banked: Duration,
    /// Monotonic reading when the current segment started.
    running_since: Option<Duration>,
    /// Laps, oldest first.
    laps: Vec<Lap>,
}

impl Stopwatch {
    /// A stopwatch at zero, not running.
    pub fn new() -> Stopwatch {
        Stopwatch::default()
    }

    /// Elapsed time as of the monotonic reading `now`.
    pub fn elapsed(&self, now: Duration) -> Duration {
        match self.running_since {
            Some(since) => self.banked + now.saturating_sub(since),
            None => self.banked,
        }
    }

    /// Whether the stopwatch is counting.
    pub fn is_running(&self) -> bool {
        self.running_since.is_some()
    }

    /// The recorded laps, oldest first.
    pub fn laps(&self) -> &[Lap] {
        &self.laps
    }

    /// How many laps are recorded.
    pub fn lap_count(&self) -> usize {
        self.laps.len()
    }

    /// The duration of the most recent lap, or the whole run when there are
    /// no laps.
    pub fn current_lap(&self, now: Duration) -> Duration {
        match self.laps.last() {
            Some(lap) => self.elapsed(now).saturating_sub(lap.total),
            None => self.elapsed(now),
        }
    }

    /// Starts, or resumes after a pause.
    pub fn start(&mut self, now: Duration) {
        if self.running_since.is_some() {
            return;
        }
        self.running_since = Some(now);
    }

    /// Stops counting, keeping the elapsed time.
    pub fn pause(&mut self, now: Duration) {
        if let Some(since) = self.running_since.take() {
            self.banked += now.saturating_sub(since);
        }
    }

    /// Starts again after a pause.
    pub fn resume(&mut self, now: Duration) {
        self.start(now);
    }

    /// Returns to zero and clears the lap list.
    pub fn reset(&mut self) {
        self.banked = Duration::ZERO;
        self.running_since = None;
        self.laps.clear();
    }

    /// Returns to zero, keeping the laps that were recorded.
    ///
    /// The other half of [`Stopwatch::reset`], and the one the stored
    /// `keep_laps` preference selects: the recorded marks are a record of the
    /// session rather than of one run, so somebody timing a sequence of laps
    /// wants them to outlive the run that produced them.
    ///
    /// The laps' own marks are left exactly as they were recorded — when the
    /// press happened has not changed — along with the numbering and the splits
    /// between them, which are already consistent with the marks that remain.
    pub fn restart(&mut self) {
        self.banked = Duration::ZERO;
        self.running_since = None;
    }

    /// Clears the lap list without touching the elapsed time.
    pub fn clear_laps(&mut self) {
        self.laps.clear();
    }

    /// Drops the lap at `index`, or reports that there was none.
    ///
    /// The marks that remain are a run of their own: they are renumbered from
    /// one, and each split is recomputed as the interval between it and the
    /// mark before it, which is the relationship every recorded split has.
    /// Removing a lap therefore leaves a list that still reads correctly —
    /// absolute totals untouched, splits describing the gaps that remain —
    /// rather than one whose third lap is still called the third and whose
    /// second split covers a lap that is no longer there.
    pub fn remove_lap(&mut self, index: usize) -> bool {
        if index >= self.laps.len() {
            return false;
        }
        self.laps.remove(index);
        self.renumber();
        true
    }

    /// Restores the numbering and the split invariant of the lap list.
    fn renumber(&mut self) {
        let mut previous = Duration::ZERO;
        for (index, lap) in self.laps.iter_mut().enumerate() {
            lap.index = index + 1;
            lap.split = lap.total.saturating_sub(previous);
            previous = lap.total;
        }
    }

    /// Records a lap at `now`. Does nothing when the stopwatch is not running.
    pub fn lap(&mut self, now: Duration) -> Option<Lap> {
        self.running_since?;
        let total = self.elapsed(now);
        let split = total.saturating_sub(self.laps.last().map_or(Duration::ZERO, |l| l.total));

        let lap = Lap {
            index: self.laps.len() + 1,
            total,
            split,
        };
        self.laps.push(lap);
        Some(lap)
    }

    /// The rank of a lap, or `None` when there are too few laps to tell.
    pub fn rank_of(&self, index: usize) -> Option<LapRank> {
        if self.laps.len() < 2 {
            return None;
        }

        let lap = *self.laps.get(index)?;
        let fastest = self.laps.iter().map(|l| l.split).min()?;
        let slowest = self.laps.iter().map(|l| l.split).max()?;

        // With only two laps, one of them is always both fastest and slowest;
        // fastest wins, which is the more useful signal to highlight.
        if lap.split == fastest {
            Some(LapRank::Fastest)
        } else if lap.split == slowest {
            Some(LapRank::Slowest)
        } else {
            None
        }
    }

    /// Folds in `elapsed` that the process did not observe, for example after
    /// a suspend/resume cycle. A paused stopwatch is unaffected.
    pub fn catch_up(&mut self, elapsed: Duration) {
        if self.running_since.is_some() {
            // While running, `elapsed` is `banked + now - running_since`, so
            // adding to `banked` moves the whole reading forward by exactly
            // the unobserved stretch.
            self.banked += elapsed;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(value: u64) -> Duration {
        Duration::from_secs(value)
    }

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn a_new_stopwatch_is_zeroed_and_stopped() {
        let stopwatch = Stopwatch::new();
        assert!(!stopwatch.is_running());
        assert_eq!(stopwatch.elapsed(ms(5_000)), Duration::ZERO);
        assert_eq!(stopwatch.lap_count(), 0);
    }

    #[test]
    fn start_measures_from_the_monotonic_reading() {
        let mut stopwatch = Stopwatch::new();
        stopwatch.start(ms(1_000));
        assert!(stopwatch.is_running());
        assert_eq!(stopwatch.elapsed(ms(1_000)), Duration::ZERO);
        assert_eq!(stopwatch.elapsed(ms(4_250)), ms(3_250));
    }

    #[test]
    fn elapsed_is_independent_of_how_often_it_is_read() {
        let mut stopwatch = Stopwatch::new();
        stopwatch.start(ms(0));

        // A hundred reads versus a single one must agree exactly.
        let mut accumulated = Duration::ZERO;
        for step in 1..=100 {
            accumulated +=
                stopwatch.elapsed(ms(step * 500)) - stopwatch.elapsed(ms((step - 1) * 500));
        }
        assert_eq!(accumulated, ms(50_000));
        assert_eq!(stopwatch.elapsed(ms(50_000)), ms(50_000));
    }

    #[test]
    fn a_render_stall_does_not_lose_time() {
        let mut stopwatch = Stopwatch::new();
        stopwatch.start(ms(0));
        // Nothing was drawn for two seconds.
        assert_eq!(stopwatch.elapsed(ms(2_000)), ms(2_000));
    }

    #[test]
    fn pause_banks_time_and_resume_continues() {
        let mut stopwatch = Stopwatch::new();
        stopwatch.start(ms(0));
        stopwatch.pause(ms(10_000));

        assert!(!stopwatch.is_running());
        assert_eq!(stopwatch.elapsed(ms(10_000)), ms(10_000));
        // Time passing while stopped must not accumulate.
        assert_eq!(stopwatch.elapsed(ms(60_000)), ms(10_000));

        stopwatch.resume(ms(60_000));
        assert!(stopwatch.is_running());
        assert_eq!(stopwatch.elapsed(ms(65_000)), ms(15_000));
    }

    #[test]
    fn pausing_twice_is_harmless() {
        let mut stopwatch = Stopwatch::new();
        stopwatch.start(ms(0));
        stopwatch.pause(ms(5_000));
        stopwatch.pause(ms(9_000));
        assert_eq!(stopwatch.elapsed(ms(9_000)), ms(5_000));
    }

    #[test]
    fn starting_twice_does_not_restart() {
        let mut stopwatch = Stopwatch::new();
        stopwatch.start(ms(0));
        stopwatch.start(ms(5_000));
        assert_eq!(stopwatch.elapsed(ms(6_000)), ms(6_000));
    }

    #[test]
    fn reset_clears_everything() {
        let mut stopwatch = Stopwatch::new();
        stopwatch.start(ms(0));
        stopwatch.lap(ms(4_000));
        stopwatch.reset();

        assert_eq!(stopwatch.elapsed(ms(90_000)), Duration::ZERO);
        assert!(!stopwatch.is_running());
        assert_eq!(stopwatch.lap_count(), 0);
    }

    #[test]
    fn laps_record_total_and_split() {
        let mut stopwatch = Stopwatch::new();
        stopwatch.start(ms(0));

        let first = stopwatch.lap(ms(4_000)).expect("lap");
        let second = stopwatch.lap(ms(9_000)).expect("lap");
        let third = stopwatch.lap(ms(10_500)).expect("lap");

        assert_eq!(first.index, 1);
        assert_eq!(first.total, ms(4_000));
        assert_eq!(first.split, ms(4_000));

        assert_eq!(second.index, 2);
        assert_eq!(second.total, ms(9_000));
        assert_eq!(second.split, ms(5_000));

        assert_eq!(third.index, 3);
        assert_eq!(third.total, ms(10_500));
        assert_eq!(third.split, ms(1_500));

        assert_eq!(stopwatch.laps(), &[first, second, third]);
    }

    #[test]
    fn current_lap_is_the_time_since_the_last_mark() {
        let mut stopwatch = Stopwatch::new();
        stopwatch.start(ms(0));

        assert_eq!(stopwatch.current_lap(ms(3_000)), ms(3_000));
        stopwatch.lap(ms(3_000));
        assert_eq!(stopwatch.current_lap(ms(5_000)), ms(2_000));
        stopwatch.lap(ms(5_000));
        assert_eq!(stopwatch.current_lap(ms(5_500)), ms(500));
    }

    #[test]
    fn laps_are_ignored_when_stopped() {
        let mut stopwatch = Stopwatch::new();
        assert!(stopwatch.lap(ms(1_000)).is_none());
        stopwatch.start(ms(0));
        stopwatch.pause(ms(1_000));
        assert!(stopwatch.lap(ms(2_000)).is_none());
    }

    #[test]
    fn laps_continue_across_pauses() {
        let mut stopwatch = Stopwatch::new();
        stopwatch.start(ms(0));
        stopwatch.pause(ms(4_000));
        stopwatch.resume(ms(10_000));
        let lap = stopwatch.lap(ms(12_000)).expect("lap");

        // The first lap's split equals its total, paused stretch included: a
        // lap is the interval between two lap marks, and there has only been
        // one.
        assert_eq!(lap.total, ms(6_000));
        assert_eq!(lap.split, ms(6_000));

        // The second lap is the interval since the first mark: running time
        // only, because the stopwatch was not paused in between.
        let second = stopwatch.lap(ms(15_000)).expect("lap");
        assert_eq!(second.total, ms(9_000));
        assert_eq!(second.split, ms(3_000));
    }

    #[test]
    fn fastest_and_slowest_laps_are_identified() {
        let mut stopwatch = Stopwatch::new();
        stopwatch.start(ms(0));
        stopwatch.lap(ms(4_000)); // split 4.000
        stopwatch.lap(ms(9_000)); // split 5.000
        stopwatch.lap(ms(13_500)); // split 4.500

        assert_eq!(stopwatch.rank_of(0), Some(LapRank::Fastest));
        assert_eq!(stopwatch.rank_of(1), Some(LapRank::Slowest));
        assert_eq!(stopwatch.rank_of(2), None);
    }

    #[test]
    fn a_single_lap_has_no_rank() {
        let mut stopwatch = Stopwatch::new();
        stopwatch.start(ms(0));
        stopwatch.lap(ms(1_000));
        assert_eq!(stopwatch.rank_of(0), None);
    }

    #[test]
    fn two_laps_label_the_faster_one() {
        let mut stopwatch = Stopwatch::new();
        stopwatch.start(ms(0));
        stopwatch.lap(ms(2_000)); // split 2.000
        stopwatch.lap(ms(6_000)); // split 4.000

        assert_eq!(stopwatch.rank_of(0), Some(LapRank::Fastest));
        assert_eq!(stopwatch.rank_of(1), Some(LapRank::Slowest));
    }

    #[test]
    fn a_removed_lap_leaves_the_rest_a_coherent_list() {
        let mut clock = Stopwatch::new();
        clock.start(secs(0));
        clock.lap(secs(2));
        clock.lap(secs(5));
        clock.lap(secs(9));
        assert_eq!(clock.lap_count(), 3);

        assert!(clock.remove_lap(1), "the middle lap is the one named");

        let laps = clock.laps();
        assert_eq!(laps.len(), 2);
        assert_eq!(
            laps.iter().map(|lap| lap.index).collect::<Vec<_>>(),
            vec![1, 2],
            "the numbering has no hole in it"
        );
        assert_eq!(
            laps.iter().map(|lap| lap.split).collect::<Vec<_>>(),
            vec![secs(2), secs(7)],
            "and each split is the gap between the marks that remain"
        );
        assert_eq!(
            laps.iter().map(|lap| lap.total).collect::<Vec<_>>(),
            vec![secs(2), secs(9)],
            "while the marks themselves are what they always were"
        );

        // The ranks are of the list as it now stands, not of one with a hole in
        // it.
        assert_eq!(clock.rank_of(1), Some(LapRank::Slowest));
        assert_eq!(clock.rank_of(0), Some(LapRank::Fastest));
    }

    #[test]
    fn removing_a_lap_that_is_not_there_changes_nothing() {
        let mut clock = Stopwatch::new();
        clock.start(secs(0));
        clock.lap(secs(1));
        clock.lap(secs(2));

        for index in [2, 3, 99, usize::MAX] {
            assert!(!clock.remove_lap(index), "there is no lap {index}");
        }
        assert_eq!(clock.lap_count(), 2, "and the list is untouched");

        // Removing the last one leaves a list that is still a list.
        assert!(clock.remove_lap(1));
        assert_eq!(clock.laps()[0].split, secs(1));
        assert!(clock.remove_lap(0));
        assert!(clock.laps().is_empty());
        assert!(!clock.remove_lap(0), "an empty list has nothing to remove");
    }

    #[test]
    fn restarting_keeps_the_laps_and_resets_the_run() {
        let mut clock = Stopwatch::new();
        clock.start(secs(0));
        clock.lap(secs(2));
        clock.lap(secs(5));
        clock.lap(secs(9));

        clock.restart();

        assert_eq!(clock.lap_count(), 3, "the record of the run survives");
        assert_eq!(
            clock.laps().iter().map(|lap| lap.index).collect::<Vec<_>>(),
            vec![1, 2, 3],
            "numbered as they were"
        );
        assert_eq!(
            clock.laps().iter().map(|lap| lap.split).collect::<Vec<_>>(),
            vec![secs(2), secs(3), secs(4)],
            "still describing the gaps between them"
        );
        assert_eq!(
            clock.elapsed(secs(20)),
            Duration::ZERO,
            "while the run is new"
        );
        assert!(!clock.is_running(), "and waiting to be started");
    }

    #[test]
    fn clearing_laps_keeps_the_elapsed_time() {
        let mut stopwatch = Stopwatch::new();
        stopwatch.start(ms(0));
        stopwatch.lap(ms(2_000));
        stopwatch.clear_laps();

        assert_eq!(stopwatch.lap_count(), 0);
        assert_eq!(stopwatch.elapsed(ms(3_000)), ms(3_000));
        assert_eq!(stopwatch.current_lap(ms(3_000)), ms(3_000));
    }

    #[test]
    fn catch_up_folds_in_a_suspend() {
        let mut stopwatch = Stopwatch::new();
        stopwatch.start(ms(0));
        assert_eq!(stopwatch.elapsed(ms(5_000)), ms(5_000));

        // The machine slept for ten minutes.
        stopwatch.catch_up(Duration::from_secs(600));
        assert_eq!(stopwatch.elapsed(ms(5_000)), ms(605_000));
    }

    #[test]
    fn catch_up_does_nothing_while_paused() {
        let mut stopwatch = Stopwatch::new();
        stopwatch.start(ms(0));
        stopwatch.pause(ms(5_000));
        stopwatch.catch_up(Duration::from_secs(600));
        assert_eq!(stopwatch.elapsed(ms(5_000)), ms(5_000));
    }

    #[test]
    fn a_long_run_keeps_counting_past_the_hour() {
        let mut stopwatch = Stopwatch::new();
        stopwatch.start(ms(0));
        assert_eq!(
            stopwatch.elapsed(ms(3_723_440)),
            Duration::from_secs(3723) + ms(440)
        );
    }
}
