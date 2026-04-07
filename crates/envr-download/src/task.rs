use envr_error::{EnvrError, EnvrResult};
use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Queued,
    Running,
    Failed,
    Cancelled,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(250),
            max_delay: Duration::from_secs(10),
        }
    }
}

impl RetryPolicy {
    pub fn backoff_delay(&self, attempt: u32) -> Duration {
        // attempt: 1..=max_retries
        let shift = attempt.saturating_sub(1).min(31);
        let factor = 1u32.checked_shl(shift).unwrap_or(u32::MAX);
        let delay = self.base_delay.saturating_mul(factor);
        delay.min(self.max_delay)
    }
}

#[derive(Clone)]
pub struct CancelToken {
    cancelled: Arc<AtomicBool>,
}

impl fmt::Debug for CancelToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CancelToken")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

impl CancelToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Same flag as [`Self::cancel`] / [`Self::is_cancelled`], for code paths that only accept an `Arc<AtomicBool>`.
    pub fn shared_atomic(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

impl Default for CancelToken {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskError {
    pub message: String,
    pub code: envr_error::ErrorCode,
}

impl From<EnvrError> for TaskError {
    fn from(value: EnvrError) -> Self {
        Self {
            message: value.to_string(),
            code: value.code(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub id: String,
    pub state: TaskState,
    pub attempts: u32,
    pub retry_policy: RetryPolicy,
    pub last_error: Option<TaskError>,
    pub cancel: CancelToken,
}

impl DownloadTask {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            state: TaskState::Queued,
            attempts: 0,
            retry_policy: RetryPolicy::default(),
            last_error: None,
            cancel: CancelToken::new(),
        }
    }

    pub fn start(&mut self) -> EnvrResult<()> {
        match self.state {
            TaskState::Queued | TaskState::Failed => {
                if self.cancel.is_cancelled() {
                    self.state = TaskState::Cancelled;
                } else {
                    self.state = TaskState::Running;
                }
                Ok(())
            }
            _ => Err(EnvrError::Validation(format!(
                "cannot start task {} from state {:?}",
                self.id, self.state
            ))),
        }
    }

    pub fn mark_done(&mut self) -> EnvrResult<()> {
        match self.state {
            TaskState::Running => {
                if self.cancel.is_cancelled() {
                    self.state = TaskState::Cancelled;
                } else {
                    self.state = TaskState::Done;
                }
                Ok(())
            }
            _ => Err(EnvrError::Validation(format!(
                "cannot complete task {} from state {:?}",
                self.id, self.state
            ))),
        }
    }

    pub fn fail(&mut self, err: impl Into<TaskError>) -> EnvrResult<Option<Duration>> {
        let err = err.into();
        match self.state {
            TaskState::Running => {
                self.attempts = self.attempts.saturating_add(1);
                self.last_error = Some(err);

                if self.cancel.is_cancelled() {
                    self.state = TaskState::Cancelled;
                    return Ok(None);
                }

                if self.attempts <= self.retry_policy.max_retries {
                    self.state = TaskState::Queued;
                    Ok(Some(self.retry_policy.backoff_delay(self.attempts)))
                } else {
                    self.state = TaskState::Failed;
                    Ok(None)
                }
            }
            _ => Err(EnvrError::Validation(format!(
                "cannot fail task {} from state {:?}",
                self.id, self.state
            ))),
        }
    }

    pub fn cancel(&mut self) -> EnvrResult<()> {
        self.cancel.cancel();
        match self.state {
            TaskState::Done => Err(EnvrError::Validation(format!(
                "cannot cancel task {} from state {:?}",
                self.id, self.state
            ))),
            _ => {
                self.state = TaskState::Cancelled;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_queued_running_done() {
        let mut t = DownloadTask::new("t1");
        assert_eq!(t.state, TaskState::Queued);
        t.start().unwrap();
        assert_eq!(t.state, TaskState::Running);
        t.mark_done().unwrap();
        assert_eq!(t.state, TaskState::Done);
    }

    #[test]
    fn fail_retries_then_failed() {
        let mut t = DownloadTask::new("t1");
        t.retry_policy.max_retries = 2;

        t.start().unwrap();
        let d1 = t.fail(EnvrError::Download("x".to_string())).unwrap();
        assert!(d1.is_some());
        assert_eq!(t.state, TaskState::Queued);

        t.start().unwrap();
        let d2 = t.fail(EnvrError::Download("x".to_string())).unwrap();
        assert!(d2.is_some());
        assert_eq!(t.state, TaskState::Queued);

        t.start().unwrap();
        let d3 = t.fail(EnvrError::Download("x".to_string())).unwrap();
        assert!(d3.is_none());
        assert_eq!(t.state, TaskState::Failed);
    }

    #[test]
    fn cancel_moves_to_cancelled() {
        let mut t = DownloadTask::new("t1");
        t.cancel().unwrap();
        assert_eq!(t.state, TaskState::Cancelled);
        assert!(t.cancel.is_cancelled());
    }

    #[test]
    fn invalid_state_transitions_return_validation_error() {
        let mut t = DownloadTask::new("t1");
        let e1 = t.mark_done().expect_err("queued cannot complete");
        assert!(e1.to_string().contains("cannot complete task"));

        t.start().expect("start");
        let e2 = t.start().expect_err("running cannot start");
        assert!(e2.to_string().contains("cannot start task"));
    }

    #[test]
    fn cancelled_task_cannot_be_started_again() {
        let mut t = DownloadTask::new("t1");
        t.cancel().expect("cancel");
        let err = t.start().expect_err("cancelled task should not start");
        assert!(err.to_string().contains("cannot start task"));
        assert_eq!(t.state, TaskState::Cancelled);
    }
}

#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    #[derive(Debug, Clone)]
    enum Op {
        Start,
        Done,
        Cancel,
        Fail,
    }

    proptest! {
        #[test]
        fn state_is_always_valid(ops in prop::collection::vec(0u8..=3u8, 0..64)) {
            let mut t = DownloadTask::new("t1");
            for b in ops {
                let op = match b {
                    0 => Op::Start,
                    1 => Op::Done,
                    2 => Op::Cancel,
                    _ => Op::Fail,
                };
                match op {
                    Op::Start => { let _ = t.start(); }
                    Op::Done => { let _ = t.mark_done(); }
                    Op::Cancel => { let _ = t.cancel(); }
                    Op::Fail => {
                        let _ = t.fail(EnvrError::Download("x".to_string()));
                    }
                }
                // invariant: Done is terminal; Cancelled is terminal (except we already reject cancelling done)
                if t.state == TaskState::Done {
                    // subsequent operations shouldn't move it away from Done
                    let s = t.state;
                    let _ = t.start();
                    let _ = t.fail(EnvrError::Download("x".to_string()));
                    prop_assert_eq!(t.state, s);
                }
            }
        }
    }
}
