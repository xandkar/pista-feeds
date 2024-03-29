// TODO Clock is not a good match for the semantics.
//      We sleep cooperatively with the caller, so it is more like pushback.
use std::time::Duration;

pub struct Tick;

/// First tick is immediate, subsequent ones after the given interval.
pub fn new(interval: Duration) -> impl Iterator<Item = Tick> {
    std::iter::once(Tick).chain(Clock { interval })
}

struct Clock {
    interval: Duration,
}

impl Iterator for Clock {
    type Item = Tick;

    fn next(&mut self) -> Option<Self::Item> {
        std::thread::sleep(self.interval);
        Some(Tick)
    }
}
