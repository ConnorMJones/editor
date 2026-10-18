//! Keeping background work to one job of a kind at a time.

/// Requests made while a job runs collapse into one more run after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InFlight {
    Idle,
    Running,
    Queued,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    Now,
    Later,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Then {
    /// Requests waited; ask again.
    Again,
    Done,
}

impl InFlight {
    pub fn request(&mut self) -> Start {
        match self {
            InFlight::Idle => {
                *self = InFlight::Running;
                Start::Now
            }
            InFlight::Running | InFlight::Queued => {
                *self = InFlight::Queued;
                Start::Later
            }
        }
    }

    pub fn finish(&mut self) -> Then {
        match std::mem::replace(self, InFlight::Idle) {
            InFlight::Queued => Then::Again,
            InFlight::Running | InFlight::Idle => Then::Done,
        }
    }
}

/// One job at a time, where the next request is worked out again once it finishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Busy {
    Idle,
    Running,
}
