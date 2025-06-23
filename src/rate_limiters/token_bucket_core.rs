use std::sync::Mutex;
use crate::{Uint, RateLimitError, AcquireResult};
/// Core implementation of the token bucket rate limiting algorithm.
///
/// The token bucket algorithm maintains a bucket that is periodically refilled with tokens
/// at a constant rate. Each request consumes tokens from the bucket, and if insufficient
/// tokens are available, the request is rejected. This allows for burst traffic up to
/// the bucket capacity while maintaining an average rate equal to the refill rate.
///
/// # Algorithm Behavior
///
/// - The bucket starts full with `capacity` tokens
/// - Tokens are added to the bucket at regular intervals up to the capacity limit
/// - Requests consume tokens from the available pool
/// - If insufficient tokens are available, the request is rejected
/// - Unused tokens accumulate up to the bucket capacity, allowing bursts
///
/// # Example
///
/// ```rust
/// use rate_limiter_core::rate_limiters::TokenBucketCore;
///
/// // Create a bucket with capacity 100, refilling 5 tokens every 10 ticks
/// let bucket = TokenBucketCore::new(100, 10, 5);
///
/// // Use all initial tokens
/// assert_eq!(bucket.try_acquire_at(100, 0), Ok(()));
///
/// // Should fail - no tokens left
/// assert!(bucket.try_acquire_at(1, 0).is_err());
///
/// // After refill interval, 5 tokens are added
/// assert_eq!(bucket.try_acquire_at(5, 10), Ok(()));
/// ```
pub struct TokenBucketCore {
    /// Maximum number of tokens the bucket can hold
    capacity: Uint,
    /// Number of ticks between each refill event
    refill_interval: Uint,
    /// Number of tokens added in each refill event
    refill_amount: Uint,
    /// Internal state protected by mutex for thread safety
    state: Mutex<TokenBucketCoreState>,
}

/// Internal state of the token bucket
struct TokenBucketCoreState {
    /// Current number of tokens available in the bucket
    available: Uint,
    /// Tick when the last refill occurred (used for calculating elapsed time)
    last_refill_tick: Uint,
}

impl TokenBucketCore {
    /// Creates a new token bucket with the specified parameters.
    ///
    /// # Parameters
    ///
    /// * `capacity` - Maximum number of tokens the bucket can hold
    /// * `refill_interval` - Number of ticks between refill events
    /// * `refill_amount` - Number of tokens added per refill interval
    ///
    /// # Panics
    ///
    /// Panics if any parameter is zero, as this would create an invalid configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rate_limiter_core::rate_limiters::TokenBucketCore;
    ///
    /// // Bucket that holds 100 tokens, adds 10 tokens every 5 ticks
    /// let bucket = TokenBucketCore::new(100, 5, 10);
    /// ```
    pub fn new(capacity: Uint, refill_interval: Uint, refill_amount: Uint) -> Self {
        assert!(capacity > 0, "capacity must be greater than 0");
        assert!(refill_interval > 0, "refill_interval must be greater than 0");
        assert!(refill_amount > 0, "refill_amount must be greater than 0");

        TokenBucketCore {
            capacity,
            refill_interval,
            refill_amount,
            state: Mutex::new(TokenBucketCoreState {
                available: capacity, // Bucket starts full
                last_refill_tick: 0,
            }),
        }
    }

    /// Attempts to acquire the specified number of tokens at the given tick.
    ///
    /// This method first calculates how many tokens should have been added since the
    /// last operation, updates the bucket state accordingly, then checks if sufficient
    /// tokens are available for the request.
    ///
    /// # Parameters
    ///
    /// * `tokens` - Number of tokens to acquire
    /// * `tick` - Current time tick for the operation
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the tokens were successfully acquired
    /// * `Err(RateLimitError::ExceedsCapacity)` - If insufficient tokens are available
    /// * `Err(RateLimitError::ContentionFailure)` - If unable to acquire the internal lock
    /// * `Err(RateLimitError::ExpiredTick)` - If the tick is older than the last operation
    ///
    /// # Time Behavior
    ///
    /// The method automatically handles time progression by calculating elapsed ticks
    /// and applying the appropriate number of refill events. The bucket capacity acts
    /// as an upper limit - excess tokens from refills are discarded.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rate_limiter_core::rate_limiters::TokenBucketCore;
    /// use rate_limiter_core::RateLimitError;
    ///
    /// let bucket = TokenBucketCore::new(50, 10, 5);
    ///
    /// // Use all tokens
    /// assert_eq!(bucket.try_acquire_at(50, 0), Ok(()));
    ///
    /// // Should fail - no tokens left
    /// assert_eq!(bucket.try_acquire_at(1, 0), Err(RateLimitError::ExceedsCapacity));
    ///
    /// // After refill interval, 5 tokens are added
    /// assert_eq!(bucket.try_acquire_at(5, 10), Ok(()));
    /// ```
    #[inline]
    pub fn try_acquire_at(&self, tokens: Uint, tick: Uint) -> AcquireResult {
        // Early return for zero tokens - always succeeds
        if tokens == 0 {
            return Ok(());
        }

        // Attempt to acquire the lock, return contention error if unavailable
        let mut state = match self.state.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Err(RateLimitError::ContentionFailure),
        };

        // Prevent time from going backwards
        if tick < state.last_refill_tick {
            return Err(RateLimitError::ExpiredTick);
        }

        // Calculate how many tokens should be added based on elapsed time
        let elapsed_ticks = tick - state.last_refill_tick;
        let refill_times = elapsed_ticks / self.refill_interval;
        let total_refilled = refill_times.saturating_mul(self.refill_amount);
        
        // Apply the refill, capped at bucket capacity
        state.available = (state.available.saturating_add(total_refilled)).min(self.capacity);
        
        // Update last refill tick to align with actual refill timing
        // This ensures consistent refill intervals regardless of when operations occur
        if refill_times > 0 {
            state.last_refill_tick = state.last_refill_tick + (refill_times * self.refill_interval);
        }

        // Check if we have sufficient tokens available
        if tokens <= state.available {
            state.available -= tokens;
            Ok(())
        } else {
            Err(RateLimitError::ExceedsCapacity)
        }
    }
}