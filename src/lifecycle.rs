//! Canonical execution lifecycle classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    Accepted,
    Running,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}
impl Lifecycle {
    pub fn classify(status: &str) -> Self {
        match status {
            "accepted" => Self::Accepted,
            "running" => Self::Running,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            "interrupted" | "interrupted_non_resumable" | "interrupted_resumable" => {
                Self::Interrupted
            }
            _ => Self::Failed,
        }
    }
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::Interrupted
        )
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeoutDisposition {
    pub completion_unknown: bool,
    pub cancellation_requested: bool,
}
pub fn synchronous_timeout() -> TimeoutDisposition {
    TimeoutDisposition {
        completion_unknown: true,
        cancellation_requested: true,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_states_are_canonical() {
        assert_eq!(Lifecycle::classify("running"), Lifecycle::Running);
        assert!(Lifecycle::classify("interrupted_non_resumable").is_terminal());
        assert_eq!(
            synchronous_timeout(),
            TimeoutDisposition {
                completion_unknown: true,
                cancellation_requested: true
            }
        );
    }
}
