//! A comprehensive rate limiting library for Rust applications.
//!
//! This library provides multiple rate limiting algorithms with a focus on performance,
//! accuracy, and ease of use. All implementations are thread-safe and designed for
//! high-concurrency scenarios.
//!
//! Time is represented using abstract "ticks" — unit-less integers that typically map
//! to nanoseconds, but can represent any monotonic unit you choose.
//!
//! # Quick Start
//!
//! ```rust
//! use rate_guard_core::rate_limiters::TokenBucketCore;
//!
//! // Capacity: 100 tokens
//! // Refill: 10 tokens every 5 ticks
//! let limiter = TokenBucketCore::new(100, 5, 10);
//!
//! // Try to acquire 20 tokens at tick 0
//! match limiter.try_acquire_at(20, 0) {
//!     Ok(()) => println!("Request allowed"),
//!     Err(e) => println!("Request denied: {}", e),
//! }
//! ```
//!
//! # Available Rate Limiting Algorithms
//!
//! ## [Leaky Bucket](rate_limiters::LeakyBucketCore)
//! Tokens leak out at a constant rate, providing smooth traffic shaping:
//!
//! ```rust
//! # use rate_guard_core::rate_limiters::LeakyBucketCore;
//! let limiter = LeakyBucketCore::new(100, 10, 5); // leak 5 tokens every 10 ticks
//! ```
//!
//! ## [Token Bucket](rate_limiters::TokenBucketCore)
//! Allows bursts up to capacity while maintaining average rate:
//!
//! ```rust
//! # use rate_guard_core::rate_limiters::TokenBucketCore;
//! let limiter = TokenBucketCore::new(100, 10, 5); // add 5 tokens every 10 ticks
//! ```
//!
//! ## [Fixed Window Counter](rate_limiters::FixedWindowCounterCore)
//! Simple time-window based counting:
//!
//! ```rust
//! # use rate_guard_core::rate_limiters::FixedWindowCounterCore;
//! let limiter = FixedWindowCounterCore::new(100, 60); // 100 requests per 60 ticks
//! ```
//!
//! ## [Sliding Window Counter](rate_limiters::SlidingWindowCounterCore)
//! Accurate sliding window using multiple time buckets:
//!
//! ```rust
//! # use rate_guard_core::rate_limiters::SlidingWindowCounterCore;
//! let limiter = SlidingWindowCounterCore::new(100, 10, 6); // 100 requests per 60 ticks
//! ```
//!
//! ## [Approximate Sliding Window](rate_limiters::ApproximateSlidingWindowCore)
//! Memory-efficient approximation using only two windows:
//!
//! ```rust
//! # use rate_guard_core::rate_limiters::ApproximateSlidingWindowCore;
//! let limiter = ApproximateSlidingWindowCore::new(100, 60); // ~100 requests per 60 ticks
//! ```
//!
//! # Core Concepts
//!
//! ## Time Representation
//! All algorithms use abstract "ticks" to represent time. You can map ticks to any unit
//! (e.g., milliseconds, nanoseconds). Internally, `Tick` is an unsigned integer (`u64` or `u128`)
//! based on crate features.
//!
//! ## Error Handling
//! All rate limiters return [`AcquireResult`] which can indicate:
//! - **Success** — Request was allowed
//! - **[`ExceedsCapacity`](RateLimitError::ExceedsCapacity)** — Rate limit exceeded
//! - **[`ContentionFailure`](RateLimitError::ContentionFailure)** — Lock contention
//! - **[`ExpiredTick`](RateLimitError::ExpiredTick)** — Time went backwards or was reused
//!
//! ## Thread Safety
//! All rate limiters are thread-safe and use non-blocking locks. If a lock cannot
//! be acquired immediately, `ContentionFailure` is returned rather than blocking.
//!
//! # Feature Flags
//!
//! This crate supports selecting the internal tick precision:
//!
//! - `tick_u64` *(default)* — `Tick = u64`, supports ~584 years of nanosecond ticks
//! - `tick_u128` — `Tick = u128`, supports extremely long durations or ultra-high precision
//!
//! To use `u128`, compile with:
//! ```sh
//! cargo build --no-default-features --features tick_u128
//! ```

pub mod rate_limiters;

/// Alias for the basic unsigned integer type used for capacities and ticks.
///
/// This type is selected at compile time using feature flags:
/// - `tick_u64` (default): uses [`u64`]
/// - `tick_u128`: uses [`u128`]
#[cfg(all(feature = "tick_u64", feature = "tick_u128"))]
compile_error!("You cannot enable both `tick_u64` and `tick_u128` features at the same time");

#[cfg(feature = "tick_u64")]
pub type Uint = u64;

#[cfg(feature = "tick_u128")]
pub type Uint = u128;

/// Error types for rate limiter operations.
///
/// These errors indicate different failure modes when attempting to acquire
/// tokens from a rate limiter.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RateLimitError {
    /// Request exceeds available capacity.
    ///
    /// This indicates that allowing the request would violate the rate limit.
    /// The caller should either:
    /// - Reject the request
    /// - Wait and retry later
    /// - Reduce the number of tokens requested
    ExceedsCapacity,

    /// Failed due to contention with other threads.
    ///
    /// This occurs when the internal lock cannot be acquired immediately.
    /// The caller should typically:
    /// - Retry the operation
    /// - Implement backoff strategy
    /// - Consider the request as temporarily failed
    ContentionFailure,

    /// The provided tick is too old or regressed.
    ///
    /// This occurs when:
    /// - Time appears to go backwards
    /// - An older tick is reused
    /// - System clock adjustments occur
    ExpiredTick,
}

impl std::fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RateLimitError::ExceedsCapacity => write!(f, "Request exceeds available capacity"),
            RateLimitError::ContentionFailure => write!(f, "Failed due to thread contention"),
            RateLimitError::ExpiredTick => write!(f, "The provided tick is too old or regressed"),
        }
    }
}

impl std::error::Error for RateLimitError {}

/// Result type for acquire operations.
///
/// Convenience alias for operations that either succeed (`()`) or fail
/// with a [`RateLimitError`].
///
/// # Example
/// ```rust
/// use rate_guard_core::{AcquireResult, RateLimitError};
/// use rate_guard_core::rate_limiters::TokenBucketCore;
///
/// let limiter = TokenBucketCore::new(10, 1, 1);
/// let result: AcquireResult = limiter.try_acquire_at(5, 0);
///
/// match result {
///     Ok(()) => println!("Acquired 5 tokens"),
///     Err(RateLimitError::ExceedsCapacity) => println!("Not enough tokens"),
///     Err(e) => println!("Other error: {}", e),
/// }
/// ```
pub type AcquireResult = Result<(), RateLimitError>;
