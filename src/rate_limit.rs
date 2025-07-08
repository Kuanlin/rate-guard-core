//! Core trait for rate limiter algorithms.
//!
//! This module defines the unified trait used by all rate limiter implementations.
//! It allows consistent use and interchangeability across token bucket, leaky bucket, window counter, and other algorithms.

pub use crate::types::Uint;
use crate::{SimpleRateLimitError, SimpleRateLimitResult, VerboseRateLimitResult};

/// The core trait implemented by all rate limiter algorithms.
///
/// This trait defines the essential operations available on any rate limiter,
/// supporting both simple and verbose (diagnostic) usage patterns.
pub trait RateLimitCore: Send + Sync {
    /// Attempts to acquire the specified number of tokens at the given tick (fast-path).
    ///
    /// Returns immediately with a minimal error type (`SimpleAcquireError`) for best performance.
    ///
    /// # Arguments
    /// * `tick` – Current time tick (from the application)
    /// * `tokens` – Number of tokens to acquire
    ///
    /// # Returns
    /// * `Ok(())` if the request is allowed
    /// * `Err(SimpleAcquireError)` if denied or failed
    fn try_acquire_at(&self, tick: Uint, tokens: Uint) -> SimpleRateLimitResult;

    /// Attempts to acquire tokens at the given tick, returning detailed diagnostics (verbose-path).
    ///
    /// Returns a verbose error type (`VerboseAcquireError`) that includes additional context, such as
    /// current available tokens, required wait time, and more. This is useful for async backoff,
    /// logging, or advanced handling.
    ///
    /// # Arguments
    /// * `tick` – Current time tick (from the application)
    /// * `tokens` – Number of tokens to acquire
    ///
    /// # Returns
    /// * `Ok(())` if the request is allowed
    /// * `Err(VerboseAcquireError)` with detailed info if denied or failed
    fn try_acquire_verbose_at(&self, tick: Uint, tokens: Uint) -> VerboseRateLimitResult;

    /// Returns the number of tokens currently available at the given tick.
    ///
    /// # Arguments
    /// * `tick` – Current time tick (from the application)
    ///
    /// # Returns
    /// The number of tokens currently available for acquisition.
    fn capacity_remaining(&self, tick: Uint) -> Result<Uint, SimpleRateLimitError>;
    fn capacity_remaining_or_0(&self, tick: Uint) -> Uint {
        self.capacity_remaining(tick).unwrap_or(0)
    }
}
