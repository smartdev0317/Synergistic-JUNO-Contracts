use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
pub const PASSED_STATUS: &str = "passed";
pub const FAILED_STATUS: &str = "failed";
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub enum Status {
    /// The proposal is open for voting.
    Open,
    /// The proposal has been rejected.
    VotingClosed,
    /// The proposal has been passed but has not been executed.
    Passed,
    /// The proposal has been passed and executed.
    Executed,
    /// The proposal has failed or expired and has been closed. A
    /// proposal deposit refund has been issued if applicable.
    Closed,
    /// The proposal's execution failed.
    ExecutionFailed,
    Rejected,
    TimeOut,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Open => write!(f, "open"),
            Status::VotingClosed => write!(f, "voting closed"),
            Status::Passed => write!(f, "passed"),
            Status::Executed => write!(f, "executed"),
            Status::Rejected => write!(f, "rejected"),
            Status::TimeOut => write!(f, "timeout"),
            Status::Closed => write!(f, "closed"),
            Status::ExecutionFailed => write!(f, "execution failed"),
        }
    }
}
