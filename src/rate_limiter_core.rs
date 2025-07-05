//! Core trait for rate limiter algorithms.
//!
//! This module defines the unified trait used by all rate limiter implementations.

pub use crate::types::Uint;
use crate::SimpleAcquireResult;

/// The core trait for all rate limiter algorithms.
///
/// Implementors of this trait provide the basic operations needed by any rate limiter.
/// This allows for consistent use across leaky bucket, token bucket, window counter, and other algorithms.
pub trait RateLimiterCore: Send + Sync {
    /// Attempts to acquire the specified number of tokens at the given tick.
    ///
    /// # Arguments
    /// * `tokens` - Number of tokens to acquire
    /// * `tick` - Current time tick (from the application)
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(RateLimitError)` if denied or failed
    fn try_acquire_at(&self, tick: Uint,tokens: Uint) -> SimpleAcquireResult;

    /// Returns the number of tokens that can still be acquired at the given tick.
    ///
    /// # Arguments
    /// * `tick` - Current time tick (from the application)
    ///
    /// # Returns
    /// The number of tokens currently available for acquisition.
    fn capacity_remaining(&self, tick: Uint) -> Uint;
}
