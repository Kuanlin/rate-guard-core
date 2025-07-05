//! error.rs
//! Defines both simple and verbose rate limiting error/result types.

use crate::types::Uint;
use core::fmt;

/// Error type for fast-path rate limiting. No extra diagnostic information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimpleRateLimitError {
    InsufficientCapacity,
    BeyondCapacity,
    ExpiredTick,
    ContentionFailure,
}

/// Result type for fast-path rate limiting.
pub type SimpleAcquireResult = Result<(), SimpleRateLimitError>;

/// Error type for verbose rate limiting. Contains diagnostic information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerboseRateLimitError {
    /// Not enough tokens available.
    InsufficientCapacity {
        acquiring: Uint,
        available: Uint,
        retry_after_ticks: Uint,
    },
    /// Request permanently exceeds the configured capacity.
    BeyondCapacity {
        acquiring: Uint,
        capacity: Uint,
    },
    /// Provided tick is too old.
    ExpiredTick {
        min_acceptable_tick: Uint,
    },
    /// Failed due to lock contention.
    ContentionFailure,
}

/// Result type for verbose rate limiting.
pub type VerboseAcquireResult = Result<(), VerboseRateLimitError>;

// Display trait for SimpleRateLimitError
impl fmt::Display for SimpleRateLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use SimpleRateLimitError::*;
        match self {
            InsufficientCapacity => write!(f, "Insufficient capacity (fast path)."),
            BeyondCapacity => write!(f, "Request exceeds maximum capacity (fast path)."),
            ExpiredTick => write!(f, "Expired tick (fast path)."),
            ContentionFailure => write!(f, "Contention failure (fast path)."),
        }
    }
}

// Display trait for VerboseRateLimitError
impl fmt::Display for VerboseRateLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use VerboseRateLimitError::*;
        match self {
            InsufficientCapacity { acquiring, available, retry_after_ticks } => {
                write!(
                    f,
                    "Insufficient capacity: tried to acquire {}, available {}, retry after {} tick(s).",
                    acquiring, available, retry_after_ticks
                )
            }
            BeyondCapacity { acquiring, capacity } => {
                write!(
                    f,
                    "Request exceeds maximum capacity: tried to acquire {}, capacity {}. This request cannot succeed.",
                    acquiring, capacity
                )
            }
            ExpiredTick { min_acceptable_tick } => {
                write!(
                    f,
                    "Expired tick: minimum acceptable tick is {}.",
                    min_acceptable_tick
                )
            }
            ContentionFailure => {
                write!(f, "Contention failure: resource is locked by another operation. Please retry.")
            }
        }
    }
}

impl std::error::Error for SimpleRateLimitError {}
impl std::error::Error for VerboseRateLimitError {}
