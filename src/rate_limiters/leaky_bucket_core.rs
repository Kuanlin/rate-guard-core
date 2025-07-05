use std::sync::Mutex;
use crate::{rate_limiter_core::RateLimiterCore, SimpleAcquireResult, SimpleRateLimitError, Uint};

/// Core implementation of the leaky bucket rate limiting algorithm.
///
/// The leaky bucket algorithm maintains a bucket with a fixed capacity that "leaks"
/// tokens at a constant rate. New requests add tokens to the bucket, and if the bucket
/// would overflow, the request is rejected. This creates a smooth rate limiting behavior
/// that prevents bursts while allowing sustained traffic up to the leak rate.
///
/// # Algorithm Behavior
///
/// - Tokens are added to the bucket when requests are made.
/// - Tokens "leak" out of the bucket at regular intervals.
/// - If adding tokens would exceed capacity, the request is rejected.
/// - The bucket starts empty and can hold up to `capacity` tokens.
///
/// # Example
///
/// ```rust
/// use rate_guard_core::rate_limiters::LeakyBucketCore;
///
/// // Create a bucket with capacity 100, leaking 5 tokens every 10 ticks
/// let bucket = LeakyBucketCore::new(100, 10, 5);
///
/// // Try to acquire 30 tokens at tick 0
/// assert_eq!(bucket.try_acquire_at(0, 30), Ok(()));
///
/// // Fill the bucket completely
/// assert_eq!(bucket.try_acquire_at(0, 70), Ok(()));
///
/// // This should fail as bucket is full
/// assert!(bucket.try_acquire_at(0, 1).is_err());
///
/// // Wait for leak interval and try again
/// assert_eq!(bucket.try_acquire_at(10, 5), Ok(())); // 5 tokens leaked out
/// ```
pub struct LeakyBucketCore {
    /// Maximum number of tokens the bucket can hold.
    capacity: Uint,
    /// Number of ticks between each leak event.
    leak_interval: Uint,
    /// Number of tokens that leak out in each leak event.
    leak_amount: Uint,
    /// Internal state protected by mutex for thread safety.
    state: Mutex<LeakyBucketCoreState>,
}

/// Internal state of the leaky bucket.
struct LeakyBucketCoreState {
    /// Current number of tokens in the bucket.
    remaining: Uint,
    /// Tick when the last leak occurred (used for calculating elapsed time).
    last_leak_tick: Uint,
}

impl RateLimiterCore for LeakyBucketCore {
    /// Attempts to acquire the specified number of tokens at the given tick.
    ///
    /// This method is a wrapper around `try_acquire_at` for convenience.
    ///
    /// # Arguments
    ///
    /// * `tokens` - Number of tokens to acquire.
    /// * `tick` - Current time tick.
    ///
    /// # Returns
    ///
    /// Returns [`SimpleAcquireResult`] indicating success or specific failure reason.
    fn try_acquire_at(&self, tick: Uint,tokens: Uint) -> SimpleAcquireResult {
        self.try_acquire_at(tick, tokens)
    }

    /// Returns the number of tokens that can still be acquired without exceeding capacity.
    ///
    /// # Arguments
    ///
    /// * `tick` - Current time tick for leak calculation.
    ///
    /// # Returns
    ///
    /// The number of tokens currently available for acquisition, or 0 if error.
    fn capacity_remaining(&self, tick: Uint) -> Uint {
        self.capacity_remaining(tick).unwrap_or(0)
    }
}

impl LeakyBucketCore {
    /// Creates a new leaky bucket with the specified parameters.
    ///
    /// # Parameters
    ///
    /// * `capacity` - Maximum number of tokens the bucket can hold.
    /// * `leak_interval` - Number of ticks between leak events.
    /// * `leak_amount` - Number of tokens that leak out per interval.
    ///
    /// # Panics
    ///
    /// Panics if any parameter is zero, as this would create an invalid configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rate_guard_core::rate_limiters::LeakyBucketCore;
    ///
    /// // Bucket that holds 100 tokens, leaks 10 tokens every 5 ticks
    /// let bucket = LeakyBucketCore::new(100, 5, 10);
    /// ```
    pub fn new(capacity: Uint, leak_interval: Uint, leak_amount: Uint) -> Self {
        assert!(capacity > 0, "capacity must be greater than 0");
        assert!(leak_interval > 0, "leak_interval must be greater than 0");
        assert!(leak_amount > 0, "leak_amount must be greater than 0");
        
        LeakyBucketCore {
            capacity,
            leak_interval,
            leak_amount,
            state: Mutex::new(LeakyBucketCoreState {
                remaining: 0,
                last_leak_tick: 0,
            }),
        }
    }

    /// Attempts to acquire the specified number of tokens at the given tick.
    ///
    /// This method first calculates how many tokens should have leaked since the
    /// last operation, updates the bucket state accordingly, then checks if the
    /// requested tokens can be accommodated without exceeding capacity.
    ///
    /// # Parameters
    ///
    /// * `tokens` - Number of tokens to acquire.
    /// * `tick` - Current time tick for the operation.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the tokens were successfully acquired.
    /// * `Err(SimpleRateLimitError::InsufficientCapacity)` - If acquiring would exceed bucket capacity.
    /// * `Err(SimpleRateLimitError::ContentionFailure)` - If unable to acquire the internal lock.
    /// * `Err(SimpleRateLimitError::ExpiredTick)` - If the tick is older than the last operation.
    #[inline(always)]
    pub fn try_acquire_at(&self, tick: Uint,tokens: Uint) -> SimpleAcquireResult {
        // Early return for zero tokens - always succeeds
        if tokens == 0 {
            return Ok(());
        }

        // Attempt to acquire the lock, return contention error if unavailable
        let mut state = match self.state.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Err(SimpleRateLimitError::ContentionFailure),
        };

        // Prevent time from going backwards
        if tick < state.last_leak_tick {
            return Err(SimpleRateLimitError::ExpiredTick);
        }

        // Calculate how much should leak based on elapsed time
        let elapsed_ticks = tick - state.last_leak_tick;
        let leak_times = elapsed_ticks / self.leak_interval;
        let total_leaked = leak_times.saturating_mul(self.leak_amount);
        
        // Apply the leak (remove tokens from bucket)
        state.remaining = state.remaining.saturating_sub(total_leaked);
        
        // Update last leak tick to align with actual leak timing
        // This ensures consistent leak intervals regardless of when operations occur
        if leak_times > 0 {
            state.last_leak_tick = state.last_leak_tick + (leak_times * self.leak_interval);
        }

        // Check if we can accommodate the requested tokens
        if tokens <= self.capacity.saturating_sub(state.remaining) {
            state.remaining += tokens;
            Ok(())
        } else {
            Err(SimpleRateLimitError::InsufficientCapacity)
        }
    }

    /// Gets the current remaining token capacity.
    ///
    /// This method updates the bucket state based on elapsed time (performs leak),
    /// then returns the current number of tokens in the bucket.
    ///
    /// # Parameters
    ///
    /// * `tick` - Current time tick for leak calculation.
    ///
    /// # Returns
    ///
    /// * `Ok(remaining_tokens)` - Current number of tokens in bucket.
    /// * `Err(SimpleRateLimitError::ContentionFailure)` - Unable to acquire internal lock.
    /// * `Err(SimpleRateLimitError::ExpiredTick)` - Time went backwards.
    #[inline(always)]
    pub fn capacity_remaining(&self, tick: Uint) -> Result<Uint, SimpleRateLimitError> {
        let mut state = match self.state.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Err(SimpleRateLimitError::ContentionFailure),
        };

        if tick < state.last_leak_tick {
            return Err(SimpleRateLimitError::ExpiredTick);
        }

        let elapsed_ticks = tick - state.last_leak_tick;
        let leak_times = elapsed_ticks / self.leak_interval;
        let total_leaked = leak_times.saturating_mul(self.leak_amount);
        
        state.remaining = state.remaining.saturating_sub(total_leaked);
        
        if leak_times > 0 {
            state.last_leak_tick = state.last_leak_tick + (leak_times * self.leak_interval);
        }

        Ok(state.remaining)
    }

    /// Gets the current token count without updating leak state.
    ///
    /// This method returns the current number of tokens in the bucket without
    /// performing any leak calculations based on elapsed time. Suitable for
    /// quick queries when you don't want to modify the bucket state.
    ///
    /// # Returns
    ///
    /// * `Ok(remaining_tokens)` - Current tokens in bucket (without leak update).
    /// * `Err(SimpleRateLimitError::ContentionFailure)` - Unable to acquire internal lock.
    #[inline(always)]
    pub fn current_capacity(&self) -> Result<Uint, SimpleRateLimitError> {
        let state = match self.state.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Err(SimpleRateLimitError::ContentionFailure),
        };

        Ok(state.remaining)
    }
}
