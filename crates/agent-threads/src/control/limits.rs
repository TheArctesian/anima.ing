//! The soft limits of the threads of this computer (THR-114).

use serde::{Deserialize, Serialize};

/// The soft limits of the threads of this computer (THR-114): guidance, raised by the person.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    pub depth: u32,
    pub children: u32,
    pub running: u32,
}

/// The words of the operator at a limit (THR-114, R-308).
pub const OVER_LIMIT: &str = "You are over the recommended guidance and at risk of cyberpsychosis from running this many sessions. Do you want to raise the limit?";

impl Limits {
    pub const FIRST: Self = Self {
        depth: 3,
        children: 5,
        running: 10,
    };

    /// The next step: 5, 10, 15 after the first; then each step doubles the one before it, with
    /// no end: 10, 20, 30, then 20, 40, 60 (THR-114, the answer of the operator "Keep doubling").
    #[must_use]
    pub const fn raised(self) -> Self {
        if self.depth == Self::FIRST.depth
            && self.children == Self::FIRST.children
            && self.running == Self::FIRST.running
        {
            return Self {
                depth: 5,
                children: 10,
                running: 15,
            };
        }
        Self {
            depth: self.depth.saturating_mul(2),
            children: self.children.saturating_mul(2),
            running: self.running.saturating_mul(2),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Limits;

    #[test]
    fn thr_114_the_soft_limits_go_to_5_10_15_then_double_with_no_end() {
        let steps: Vec<(u32, u32, u32)> =
            std::iter::successors(Some(Limits::FIRST), |limits| Some(limits.raised()))
                .take(5)
                .map(|limits| (limits.depth, limits.children, limits.running))
                .collect();
        assert_eq!(
            steps,
            [
                (3, 5, 10),
                (5, 10, 15),
                (10, 20, 30),
                (20, 40, 60),
                (40, 80, 120)
            ]
        );
        let top = Limits {
            depth: u32::MAX,
            children: u32::MAX,
            running: u32::MAX,
        };
        assert_eq!(top.raised(), top, "no overflow at the end of the numbers");
    }
}
