//! A comprehensive rate limiting library for Rust applications.
//!
//! This library provides multiple rate limiting algorithms with a focus on performance,
//! accuracy, and ease of use. All implementations are thread-safe and designed for
//! high-concurrency scenarios.
//!
//! # Quick Start
//!
//! ```rust
//! use rate_limiter_core::rate_limiters::TokenBucketCore;
//!
//! // Create a token bucket with capacity 100, refilling 10 tokens every 5 ticks
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
//! ```rust
//! # use rate_limiter_core::rate_limiters::LeakyBucketCore;
//! let limiter = LeakyBucketCore::new(100, 10, 5); // leak 5 tokens every 10 ticks
//! ```
//!
//! ## [Token Bucket](rate_limiters::TokenBucketCore)
//! Allows bursts up to capacity while maintaining average rate:
//! ```rust
//! # use rate_limiter_core::rate_limiters::TokenBucketCore;
//! let limiter = TokenBucketCore::new(100, 10, 5); // add 5 tokens every 10 ticks
//! ```
//!
//! ## [Fixed Window Counter](rate_limiters::FixedWindowCounterCore)
//! Simple time-window based counting:
//! ```rust
//! # use rate_limiter_core::rate_limiters::FixedWindowCounterCore;
//! let limiter = FixedWindowCounterCore::new(100, 60); // 100 requests per 60 ticks
//! ```
//!
//! ## [Sliding Window Counter](rate_limiters::SlidingWindowCounterCore)
//! Accurate sliding window using multiple time buckets:
//! ```rust
//! # use rate_limiter_core::rate_limiters::SlidingWindowCounterCore;
//! let limiter = SlidingWindowCounterCore::new(100, 10, 6); // 100 requests per 60 ticks
//! ```
//!
//! ## [Approximate Sliding Window](rate_limiters::ApproximateSlidingWindowCore)
//! Memory-efficient approximation using only two windows:
//! ```rust
//! # use rate_limiter_core::rate_limiters::ApproximateSlidingWindowCore;
//! let limiter = ApproximateSlidingWindowCore::new(100, 60); // ~100 requests per 60 ticks
//! ```
//!
//! # Core Concepts
//!
//! ## Time Representation
//! All algorithms use abstract "ticks" to represent time. This allows the library
//! to work with any time unit (milliseconds, seconds, etc.) by mapping your time
//! source to tick values.
//!
//! ## Error Handling
//! All rate limiters return [`AcquireResult`] which can indicate:
//! - **Success** - Request was allowed
//! - **[`ExceedsCapacity`](RateLimitError::ExceedsCapacity)** - Rate limit exceeded
//! - **[`ContentionFailure`](RateLimitError::ContentionFailure)** - Lock contention
//! - **[`ExpiredTick`](RateLimitError::ExpiredTick)** - Time went backwards
//!
//! ## Thread Safety
//! All rate limiters are thread-safe and use non-blocking locks. If a lock cannot
//! be acquired immediately, `ContentionFailure` is returned rather than blocking.
//!
//! # Algorithm Selection Guide
//!
//! Choose your algorithm based on your requirements:
//!
//! - **Strict constant rate**: Use [`LeakyBucketCore`](rate_limiters::LeakyBucketCore)
//! - **Allow controlled bursts**: Use [`TokenBucketCore`](rate_limiters::TokenBucketCore)
//! - **Simple implementation**: Use [`FixedWindowCounterCore`](rate_limiters::FixedWindowCounterCore)
//! - **Accurate sliding window**: Use [`SlidingWindowCounterCore`](rate_limiters::SlidingWindowCounterCore)
//! - **Memory-efficient sliding**: Use [`ApproximateSlidingWindowCore`](rate_limiters::ApproximateSlidingWindowCore)

use std::sync::atomic::AtomicU64;

pub mod rate_limiters;

/// Alias for the atomic counter type used in rate limiter internals.
///
/// Currently maps to [`AtomicU64`] but may change in future versions
/// to support different architectures or requirements.
pub type AtomicUint = AtomicU64;

/// Alias for the basic unsigned integer type used for capacities and ticks.
///
/// Currently maps to [`u64`] providing a large range for tick counts
/// and token capacities. This allows for:
/// - Tick counts up to ~584 billion years at nanosecond resolution
/// - Token capacities suitable for high-throughput applications
pub type Uint = u64;

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
    
    /// The provided tick is too old/expired.
    ///
    /// This happens when:
    /// - Time appears to go backwards
    /// - An old tick value is used after newer operations
    /// - System clock adjustments occur
    /// 
    /// The caller should ensure monotonic time progression.
    ExpiredTick,
}

impl std::fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RateLimitError::ExceedsCapacity => write!(f, "Request exceeds available capacity"),
            RateLimitError::ContentionFailure => write!(f, "Failed due to thread contention"),
            RateLimitError::ExpiredTick => write!(f, "The provided tick is too old/expired"),
        }
    }
}

impl std::error::Error for RateLimitError {}

/// Result type for acquire operations.
///
/// This is a convenience type alias for operations that either succeed
/// with no return value or fail with a [`RateLimitError`].
///
/// # Example
///
/// ```rust
/// use rate_limiter_core::{AcquireResult, RateLimitError};
/// use rate_limiter_core::rate_limiters::TokenBucketCore;
///
/// let limiter = TokenBucketCore::new(10, 1, 1);
/// 
/// let result: AcquireResult = limiter.try_acquire_at(5, 0);
/// match result {
///     Ok(()) => println!("Acquired 5 tokens"),
///     Err(RateLimitError::ExceedsCapacity) => println!("Not enough tokens"),
///     Err(e) => println!("Other error: {}", e),
/// }
/// ```
pub type AcquireResult = Result<(), RateLimitError>;