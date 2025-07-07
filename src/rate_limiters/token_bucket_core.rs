use std::sync::Mutex;
use crate::{SimpleAcquireResult, SimpleRateLimitError, Uint, VerboseAcquireResult, VerboseRateLimitError};
use crate::rate_limiter_core::RateLimiterCore;

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
/// use rate_guard_core::rate_limiters::TokenBucketCore;
///
/// // Create a bucket with capacity 100, refilling 5 tokens every 10 ticks
/// let bucket = TokenBucketCore::new(100, 10, 5);
///
/// // Use all initial tokens
/// assert_eq!(bucket.try_acquire_at(0, 100), Ok(()));
///
/// // Should fail - no tokens left
/// assert!(bucket.try_acquire_at(0, 1).is_err());
///
/// // After refill interval, 5 tokens are added
/// assert_eq!(bucket.try_acquire_at(10, 5), Ok(()));
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

impl RateLimiterCore for TokenBucketCore {
    /// Attempts to acquire the specified number of tokens at the given tick.
    ///
    /// This method is a wrapper that calls the main `try_acquire_at` logic.
    ///
    /// # Arguments
    /// * `tokens` - Number of tokens to acquire
    /// * `tick` - Current time tick
    ///
    /// # Returns
    /// Returns [`SimpleAcquireResult`] indicating success or error type.
    #[inline(always)]
    fn try_acquire_at(&self, tick: Uint,tokens: Uint) -> SimpleAcquireResult {
        self.try_acquire_at(tick, tokens)
    }
    /// Attempts to acquire tokens at the given tick, returning detailed diagnostics.
    /// 
    /// This method is a wrapper that calls the main `try_acquire_verbose_at` logic.
    ///    
    /// # Arguments
    /// * `tick` - Current time tick
    /// * `tokens` - Number of tokens to acquire
    /// # Returns
    /// Returns [`VerboseAcquireResult`] indicating success or error type.
    /// 
    /// # Example
    /// ```rust
    /// use rate_guard_core::rate_limiters::TokenBucketCore;
    /// use rate_guard_core::VerboseRateLimitError;
    /// let bucket = TokenBucketCore::new(100, 10, 5);
    /// let tick = 20;
    /// match bucket.try_acquire_verbose_at(tick, 30) {
    ///     Ok(()) => println!("Request allowed!"),
    ///     Err(VerboseRateLimitError::InsufficientCapacity { available, retry_after_ticks, .. }) => {
    ///         println!("Please retry in {} ticks ({} tokens available)", retry_after_ticks, available);
    ///     },
    ///     Err(e) => println!("Denied: {}", e),
    /// }
    ///```
    #[inline(always)]
    fn try_acquire_verbose_at(&self, tick: Uint, tokens: Uint) -> VerboseAcquireResult {
        self.try_acquire_verbose_at(tick, tokens)
    }
    /// Returns the number of tokens that can still be acquired without exceeding capacity.
    ///
    /// # Arguments
    /// * `tick` - Current time tick for refill calculation
    ///
    /// # Returns
    /// Number of available tokens or 0 if error.
    #[inline(always)]
    fn capacity_remaining(&self, tick: Uint) -> Uint {
        self.capacity_remaining(tick).unwrap_or(0)
    }
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
    /// Panics if any parameter is zero.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rate_guard_core::rate_limiters::TokenBucketCore;
    ///
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
    /// * `tokens` - Number of tokens to acquire
    /// * `tick` - Current time tick for the operation
    ///
    /// # Returns
    /// * `Ok(())` - If the tokens were successfully acquired
    /// * `Err(SimpleRateLimitError::InsufficientCapacity)` - If insufficient tokens are available
    /// * `Err(SimpleRateLimitError::ContentionFailure)` - If unable to acquire the internal lock
    /// * `Err(SimpleRateLimitError::ExpiredTick)` - If the tick is older than the last operation
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
        if tick < state.last_refill_tick {
            return Err(SimpleRateLimitError::ExpiredTick);
        }

        // Calculate how many tokens should be added based on elapsed time
        let elapsed_ticks = tick - state.last_refill_tick;
        let refill_times = elapsed_ticks / self.refill_interval;
        let total_refilled = refill_times.saturating_mul(self.refill_amount);
        
        // Apply the refill, capped at bucket capacity
        state.available = (state.available.saturating_add(total_refilled)).min(self.capacity);
        
        // Update last refill tick to align with actual refill timing
        if refill_times > 0 {
            state.last_refill_tick = state.last_refill_tick + (refill_times * self.refill_interval);
        }

        // Check if we have sufficient tokens available
        if tokens <= state.available {
            state.available -= tokens;
            Ok(())
        } else {
            Err(SimpleRateLimitError::InsufficientCapacity)
        }
    }

    /// Attempts to acquire the specified number of tokens at the given tick,
    /// returning detailed diagnostics on failure.
    ///
    /// This verbose version of the rate limiter provides additional context when a request
    /// fails due to rate limiting constraints. Unlike the fast-path `try_acquire_at`,
    /// which returns a minimal error enum, this method returns rich information
    /// that can be used for logging, backoff timing, or user feedback.
    ///
    /// # Behavior
    /// - Tokens are added to the bucket based on the elapsed time since the last refill.
    /// - If sufficient tokens are available, the request is allowed and tokens are deducted.
    /// - If not enough tokens are available, an error is returned with a recommended
    ///   number of ticks to wait before retrying.
    /// - If the requested tokens permanently exceed the configured capacity,
    ///   a `BeyondCapacity` error is returned.
    ///
    /// # Arguments
    /// * `tick` – The current logical time tick (e.g., milliseconds since app start)
    /// * `tokens` – The number of tokens to acquire
    ///
    /// # Returns
    /// * `Ok(())` – If the tokens were successfully acquired
    /// * `Err(VerboseRateLimitError::ContentionFailure)` – If lock acquisition failed
    /// * `Err(VerboseRateLimitError::ExpiredTick)` – If the provided tick is older than the last refill
    /// * `Err(VerboseRateLimitError::BeyondCapacity)` – If the requested amount exceeds the bucket's max capacity
    /// * `Err(VerboseRateLimitError::InsufficientCapacity)` – If not enough tokens are currently available,
    ///     includes how many are available and how long to wait (in ticks) before retrying.
    ///
    /// # Example
    /// ```
    /// use rate_guard_core::rate_limiters::TokenBucketCore;
    /// use rate_guard_core::VerboseRateLimitError;
    ///
    /// let bucket = TokenBucketCore::new(100, 10, 5);
    /// let tick = 20;
    ///
    /// match bucket.try_acquire_verbose_at(tick, 30) {
    ///     Ok(()) => println!("Request allowed!"),
    ///     Err(VerboseRateLimitError::InsufficientCapacity { available, retry_after_ticks, .. }) => {
    ///         println!("Please retry in {} ticks ({} tokens available)", retry_after_ticks, available);
    ///     },
    ///     Err(e) => println!("Denied: {}", e),
    /// }
    /// ```
    #[inline(always)]
    pub fn try_acquire_verbose_at(&self, tick: Uint, tokens: Uint) -> VerboseAcquireResult {
        if tokens == 0 {
            return Ok(());
        }

        let mut state = self.state.try_lock()
            .map_err(|_| VerboseRateLimitError::ContentionFailure)?;

        if tick < state.last_refill_tick {
            return Err(VerboseRateLimitError::ExpiredTick {
                min_acceptable_tick: state.last_refill_tick,
            });
        }

        if tokens > self.capacity {
            return Err(VerboseRateLimitError::BeyondCapacity {
                acquiring: tokens,
                capacity: self.capacity,
            });
        }

        let elapsed_ticks = tick - state.last_refill_tick;
        let refill_times = elapsed_ticks / self.refill_interval;
        let total_refilled = refill_times.saturating_mul(self.refill_amount);

        state.available = (state.available + total_refilled).min(self.capacity);

        if refill_times > 0 {
            state.last_refill_tick += refill_times * self.refill_interval;
        }

        if tokens <= state.available {
            state.available -= tokens;
            Ok(())
        } else {
            let needed_tokens = tokens - state.available;
            let refill_per_tick = self.refill_amount;
            let retry_after_ticks = self.refill_interval
                .saturating_mul((needed_tokens + refill_per_tick - 1) / refill_per_tick);

            Err(VerboseRateLimitError::InsufficientCapacity {
                acquiring: tokens,
                available: state.available,
                retry_after_ticks,
            })
        }
    }

    /// Gets the current number of tokens remaining in the bucket.
    /// This method updates the bucket state based on elapsed time (performs refill),
    /// then returns the current number of available tokens.
    #[inline]
    pub fn tokens_in_bucket(&self, tick: Uint) -> Result<Uint, SimpleRateLimitError> {
        self.capacity_remaining(tick)
    }

    /// Gets the current remaining token capacity.
    ///
    /// This method updates the bucket state based on elapsed time (performs refill),
    /// then returns the current number of available tokens.
    ///
    /// # Parameters
    /// * `tick` - Current time tick for refill calculation
    ///
    /// # Returns
    /// * `Ok(available_tokens)` - Current number of available tokens
    /// * `Err(SimpleRateLimitError::ContentionFailure)` - Unable to acquire internal lock
    /// * `Err(SimpleRateLimitError::ExpiredTick)` - Time went backwards
    #[inline(always)]
    pub fn capacity_remaining(&self, tick: Uint) -> Result<Uint, SimpleRateLimitError> {
        // Attempt to acquire the lock, return contention error if unavailable
        let mut state = match self.state.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Err(SimpleRateLimitError::ContentionFailure),
        };

        // Prevent time from going backwards
        if tick < state.last_refill_tick {
            return Err(SimpleRateLimitError::ExpiredTick);
        }

        // Calculate how many tokens should be added based on elapsed time
        let elapsed_ticks = tick - state.last_refill_tick;
        let refill_times = elapsed_ticks / self.refill_interval;
        let total_refilled = refill_times.saturating_mul(self.refill_amount);
        
        // Apply the refill, capped at bucket capacity
        state.available = (state.available.saturating_add(total_refilled)).min(self.capacity);
        
        // Update last refill tick to align with actual refill timing
        if refill_times > 0 {
            state.last_refill_tick = state.last_refill_tick + (refill_times * self.refill_interval);
        }

        // Return current available token count
        Ok(state.available)
    }

    /// Gets the current token capacity without updating refill state.
    ///
    /// This method returns the current number of tokens in the bucket without
    /// performing any refill calculations based on elapsed time. Suitable for
    /// quick queries when you don't want to modify the bucket state.
    ///
    /// # Returns
    /// * `Ok(available_tokens)` - Current tokens in bucket (without refill update)
    /// * `Err(SimpleRateLimitError::ContentionFailure)` - Unable to acquire internal lock
    #[inline(always)]
    pub fn current_capacity(&self) -> Result<Uint, SimpleRateLimitError> {
        let state = match self.state.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Err(SimpleRateLimitError::ContentionFailure),
        };

        Ok(state.available)
    }
}

/// Configuration structure for creating a `TokenBucketCore` limiter.
#[derive(Debug, Clone)]
pub struct TokenBucketCoreConfig {
    /// Maximum number of tokens the bucket can hold.
    pub capacity: Uint,
    /// Number of ticks between each refill event.
    pub refill_interval: Uint,
    /// Number of tokens added per interval.
    pub refill_amount: Uint,
}

impl TokenBucketCoreConfig {
    /// Creates a new configuration instance.
    pub fn new(capacity: Uint, refill_interval: Uint, refill_amount: Uint) -> Self {
        Self {
            capacity,
            refill_interval,
            refill_amount,
        }
    }
}

impl From<TokenBucketCoreConfig> for TokenBucketCore {
    /// Converts a `TokenBucketCoreConfig` into a `TokenBucketCore` instance.
    ///
    /// # Panics
    /// This method will panic if any field in the config is zero.
    /// It is intended for use with validated or hardcoded input.
    ///
    /// # Examples
    ///
    /// Using [`From::from`] explicitly:
    ///
    /// ```
    /// use rate_guard_core::rate_limiters::{TokenBucketCore, TokenBucketCoreConfig};
    ///
    /// let config = TokenBucketCoreConfig {
    ///     capacity: 100,
    ///     refill_interval: 10,
    ///     refill_amount: 5,
    /// };
    ///
    /// let limiter = TokenBucketCore::from(config);
    /// ```
    ///
    /// Using `.into()` with type inference:
    ///
    /// ```
    /// use rate_guard_core::rate_limiters::{TokenBucketCore, TokenBucketCoreConfig};
    ///
    /// let limiter: TokenBucketCore = TokenBucketCoreConfig {
    ///     capacity: 100,
    ///     refill_interval: 10,
    ///     refill_amount: 5,
    /// }.into();
    /// ```
    #[inline(always)]
    fn from(config: TokenBucketCoreConfig) -> Self {
        TokenBucketCore::new(config.capacity, config.refill_interval, config.refill_amount)
    }
}
