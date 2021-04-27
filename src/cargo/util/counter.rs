use std::time::Instant;

/// A metrics counter storing only latest `N` records.
pub struct MetricsCounter<const N: usize> {
    /// Slots to store metrics.
    slots: [(usize, Instant); N],
    /// The slot of the oldest record.
    /// Also the next slot to store the new record.
    index: usize,
}

impl<const N: usize> MetricsCounter<N> {
    /// Creates a new counter with an initial value.
    pub fn new(init: usize) -> Self {
        debug_assert!(N > 0, "number of slots must be greater than zero");
        Self {
            slots: [(init, Instant::now()); N],
            index: 0,
        }
    }

    /// Adds record to the counter.
    pub fn add(&mut self, data: usize) {
        self.slots[self.index] = (data, Instant::now());
        self.index = (self.index + 1) % N;
    }

    /// Calculates per-second average rate of all slots.
    pub fn rate(&self) -> f32 {
        let latest = self.slots[self.index.checked_sub(1).unwrap_or(N - 1)];
        let oldest = self.slots[self.index];
        let duration = (latest.1 - oldest.1).as_secs_f32();
        let avg = (latest.0 - oldest.0) as f32 / duration;
        if f32::is_nan(avg) {
            0f32
        } else {
            avg
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MetricsCounter;

    #[test]
    fn counter() {
        let mut counter = MetricsCounter::<3>::new(0);
        assert_eq!(counter.rate(), 0f32);
        for i in 1..=5 {
            counter.add(i);
            assert!(counter.rate() > 0f32);
        }
    }

    #[test]
    #[should_panic(expected = "number of slots must be greater than zero")]
    fn counter_zero_slot() {
        let _counter = MetricsCounter::<0>::new(0);
    }
}
