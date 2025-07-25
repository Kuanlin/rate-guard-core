use std::sync::Mutex;
use crate::{rate_limit::RateLimitCore, SimpleRateLimitResult, SimpleRateLimitError, Uint, VerboseRateLimitResult, VerboseRateLimitError};

/// Core implementation of the sliding window counter rate limiting algorithm.
///
/// The sliding window counter algorithm divides time into fixed-size buckets and
/// tracks token usage across a sliding window of multiple buckets. This provides
/// more accurate rate limiting than fixed windows by smoothing out traffic patterns
/// and preventing burst accumulation at window boundaries.
///
/// # Algorithm Behavior
///
/// - Time is divided into buckets of `bucket_ticks` duration each
/// - A sliding window spans `bucket_count` buckets (total window size = bucket_ticks * bucket_count)
/// - Only buckets within the current sliding window are counted toward the capacity limit
/// - Buckets outside the window are considered expired and don't count
/// - Each bucket is lazily reset when accessed after expiration
///
/// # Sliding Window Calculation
///
/// For a given tick, the sliding window spans from:
/// `[tick - window_size + 1, tick]` where `window_size = bucket_ticks * bucket_count`
///
/// Only buckets whose start time falls within this range contribute to the total count.
///
/// # Example
///
/// ```rust
/// use rate_guard_core::cores::SlidingWindowCounterCore;
///
/// // Create counter with capacity 100, bucket size 5 ticks, 4 buckets total
/// // Total window size = 5 * 4 = 20 ticks
/// let counter = SlidingWindowCounterCore::new(100, 5, 4);
///
/// // Tick 2: bucket 0 [0-4], sliding window [0, 2]
/// assert_eq!(counter.try_acquire_at(2, 30), Ok(()));
///
/// // Tick 7: bucket 1 [5-9], sliding window [0, 7] 
/// assert_eq!(counter.try_acquire_at(7, 40), Ok(()));
///
/// // Tick 25: sliding window [6, 25], bucket 0 [0-4] expires
/// // Only bucket 1 [5-9] (40 tokens) counts toward limit
/// assert_eq!(counter.try_acquire_at(25, 60), Ok(()));
/// ```
pub struct SlidingWindowCounterCore {
    /// Maximum number of tokens allowed within the sliding window
    capacity: Uint,
    /// Duration of each bucket in ticks
    bucket_ticks: Uint,
    /// Number of buckets in the sliding window
    bucket_count: Uint,
    /// Internal state protected by mutex for thread safety
    state: Mutex<SlidingWindowCounterCoreState>,
}

/// Internal state of the sliding window counter
struct SlidingWindowCounterCoreState {
    /// Token counts for each bucket (circular array)
    buckets: Vec<Uint>,
    /// Start tick for each bucket (used to determine if bucket is valid)
    bucket_start_ticks: Vec<Uint>,
    /// Index of the most recently used bucket
    last_bucket_index: usize,
}


/// Core trait implementation for the fixed window counter.
/// This provides the basic operations needed by the rate limiter core trait.
impl RateLimitCore for SlidingWindowCounterCore {
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
    /// Returns [`SimpleRateLimitResult`] indicating success or specific failure reason. 
    fn try_acquire_at(&self, tick: Uint,tokens: Uint) -> SimpleRateLimitResult {
        self.try_acquire_at(tick, tokens)
    }

    /// Attempts to acquire tokens at the given tick, returning detailed diagnostics.
    /// This method is a wrapper around `try_acquire_verbose_at` for convenience.
    /// # Arguments
    ///
    /// * `tokens` - Number of tokens to acquire.
    /// * `tick` - Current time tick.
    /// # Returns
    /// 
    /// Returns [`VerboseRateLimitResult`] with detailed diagnostics or error.
    /// This includes information like available tokens, retry time, and more.
    fn try_acquire_verbose_at(&self, tick: Uint, tokens: Uint) -> VerboseRateLimitResult {
        self.try_acquire_verbose_at(tick, tokens)
    }


    /// Returns the number of tokens that can still be acquired without exceeding capacity.
    /// 
    /// # Arguments
    /// * `tick` - Current time tick for leak calculation.
    /// # Returns
    /// The number of tokens currently available for acquisition, or an error if unable to acquire lock or if tick is expired.
    fn capacity_remaining(&self, tick: Uint) -> Result<Uint, SimpleRateLimitError> {
        self.capacity_remaining(tick)
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
    fn capacity_remaining_or_0(&self, tick: Uint) -> Uint {
        self.capacity_remaining_or_0(tick)
    }
}

impl SlidingWindowCounterCore {
    /// Creates a new sliding window counter with the specified parameters.
    ///
    /// # Parameters
    /// * `capacity` - Maximum number of tokens allowed within the sliding window
    /// * `bucket_ticks` - Duration of each bucket in ticks
    /// * `bucket_count` - Number of buckets in the sliding window
    ///
    /// # Panics
    /// Panics if any parameter is zero, as this would create an invalid configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rate_guard_core::cores::SlidingWindowCounterCore;
    /// // Window of 100 tokens across 5 buckets of 10 ticks each (50 tick window)
    /// let counter = SlidingWindowCounterCore::new(100, 10, 5);
    /// ```
    pub fn new(capacity: Uint, bucket_ticks: Uint, bucket_count: Uint) -> Self {
        assert!(capacity > 0, "capacity must be greater than 0");
        assert!(bucket_ticks > 0, "bucket_ticks must be greater than 0");
        assert!(bucket_count > 0, "bucket_count must be greater than 0");
        
        SlidingWindowCounterCore {
            capacity,
            bucket_ticks,
            bucket_count,
            state: Mutex::new(SlidingWindowCounterCoreState {
                buckets: vec![0; bucket_count as usize],
                bucket_start_ticks: vec![0; bucket_count as usize],
                last_bucket_index: 0,
            }),
        }
    }

    /// Calculates the total window size in ticks.
    ///
    /// # Returns
    /// Returns the total duration of the sliding window (bucket_ticks * bucket_count).
    #[inline]
    fn window_ticks(&self) -> Uint {
        self.bucket_ticks.saturating_mul(self.bucket_count)
    }

    /// Attempts to acquire the specified number of tokens at the given tick.
    ///
    /// This method determines which bucket the current tick belongs to, performs
    /// lazy reset of expired buckets, calculates the total tokens used within
    /// the current sliding window, and checks if the request can be accommodated.
    ///
    /// # Parameters
    /// * `tokens` - Number of tokens to acquire
    /// * `tick` - Current time tick for the operation
    ///
    /// # Returns
    /// * `Ok(())` - If the tokens were successfully acquired
    /// * `Err(SimpleRateLimitError::InsufficientCapacity)` - If acquiring would exceed window capacity
    /// * `Err(SimpleRateLimitError::ContentionFailure)` - If unable to acquire the internal lock
    /// * `Err(SimpleRateLimitError::ExpiredTick)` - If the tick is older than the last recorded operation
    ///
    /// # Bucket Management
    ///
    /// - Buckets are organized in a circular array indexed by `(tick / bucket_ticks) % bucket_count`
    /// - When accessing a bucket, if its start time doesn't match the expected time, it's reset (lazy reset)
    /// - Only buckets whose start time falls within the sliding window contribute to the total
    #[inline(always)]
    pub fn try_acquire_at(&self, tick: Uint,tokens: Uint) -> SimpleRateLimitResult {
        // Early return for zero tokens - always succeeds
        if tokens == 0 {
            return Ok(());
        }
        
        if tokens > self.capacity {
            return Err(SimpleRateLimitError::BeyondCapacity);
        }

        // Attempt to acquire the lock, return contention error if unavailable
        let mut state = match self.state.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Err(SimpleRateLimitError::ContentionFailure),
        };

        // Prevent time from going backwards (only check if we have previous data)
        if state.bucket_start_ticks[state.last_bucket_index] > 0 && 
           tick < state.bucket_start_ticks[state.last_bucket_index] {
            return Err(SimpleRateLimitError::ExpiredTick);
        }

        // Determine which bucket this tick belongs to
        let current_bucket_index = ((tick / self.bucket_ticks) as usize) % (self.bucket_count as usize);
        let current_bucket_start_tick = (tick / self.bucket_ticks) * self.bucket_ticks;

        // Lazy reset: if this bucket's start time is different, it's a new bucket cycle
        if state.bucket_start_ticks[current_bucket_index] != current_bucket_start_tick {
            state.buckets[current_bucket_index] = 0;
            state.bucket_start_ticks[current_bucket_index] = current_bucket_start_tick;
        }

        // Calculate the sliding window range
        let window_start_tick = tick.saturating_sub(self.window_ticks());

        // Count tokens in all valid buckets within the sliding window
        let total = self.count_tokens_in_valid_buckets_within_sliding_window(&state, tick, window_start_tick);

        // Check if we can accommodate the requested tokens
        if total <= self.capacity.saturating_sub(tokens) {
            state.buckets[current_bucket_index] += tokens;
            state.last_bucket_index = current_bucket_index;
            Ok(())
        } else {
            Err(SimpleRateLimitError::InsufficientCapacity)
        }
    }

    /// Attempts to acquire the specified number of tokens at the given tick,
    /// returning detailed diagnostics if the request is denied.
    /// This method behaves similarly to `try_acquire_at`, but provides
    /// richer error information for better diagnostics.
    /// # Parameters
    /// * `tokens` - Number of tokens to acquire
    /// * `tick` - Current time tick for the operation
    /// # Returns
    /// * `Ok(())` - If the tokens were successfully acquired
    /// * `Err(VerboseRateLimitError::ContentionFailure)` - If unable to acquire the internal lock
    /// * `Err(VerboseRateLimitError::ExpiredTick { min_acceptable_tick })` - If the tick is older than the last recorded operation
    /// * `Err(VerboseRateLimitError::BeyondCapacity { acquiring, capacity })` - If the requested tokens exceed the maximum capacity
    /// * `Err(VerboseRateLimitError::InsufficientCapacity { acquiring, available, retry_after_ticks })` - If there are not enough tokens available, but suggests how long to wait before retrying
    #[inline(always)]
    pub fn try_acquire_verbose_at(&self, tick: Uint, tokens: Uint) -> VerboseRateLimitResult {
        if tokens == 0 {
            return Ok(());
        }

        let mut state = self.state.try_lock()
            .map_err(|_| VerboseRateLimitError::ContentionFailure)?;

        let window_start_tick = tick.saturating_sub(self.window_ticks());

        // Reject if time has gone backwards
        if state.bucket_start_ticks[state.last_bucket_index] > 0 &&
            tick < state.bucket_start_ticks[state.last_bucket_index]
        {
            return Err(VerboseRateLimitError::ExpiredTick {
                min_acceptable_tick: state.bucket_start_ticks[state.last_bucket_index],
            });
        }

        // Reject if acquiring more than capacity
        if tokens > self.capacity {
            return Err(VerboseRateLimitError::BeyondCapacity {
                acquiring: tokens,
                capacity: self.capacity,
            });
        }

        // Determine current bucket index
        let bucket_ticks = self.bucket_ticks;
        let bucket_count = self.bucket_count as usize;
        let current_bucket_idx = ((tick / bucket_ticks) % self.bucket_count as Uint) as usize;
        let current_bucket_start_tick = (tick / bucket_ticks) * bucket_ticks;

        // Reset current bucket if entering new time slot
        if state.bucket_start_ticks[current_bucket_idx] != current_bucket_start_tick {
            state.buckets[current_bucket_idx] = 0;
            state.bucket_start_ticks[current_bucket_idx] = current_bucket_start_tick;
        }

        state.last_bucket_index = current_bucket_idx;

        // ----- Phase 1: calculate total used tokens in current window -----
        let mut total_used = 0;
        let mut valid_indices = Vec::with_capacity(bucket_count);

        for i in 0..bucket_count {
            let idx = i;
            let ts = state.bucket_start_ticks[idx];
            if ts >= window_start_tick && ts <= tick {
                total_used += state.buckets[idx];
                valid_indices.push(idx);
            }
        }

        let available = self.capacity.saturating_sub(total_used);

        if tokens <= available {
            state.buckets[current_bucket_idx] += tokens;
            return Ok(());
        }

        // ----- Phase 2: simulate expiration to estimate retry -----
        let mut released = 0;
        let mut retry_after_ticks = self.window_ticks(); // fallback to full window

        for (i, &idx) in valid_indices.iter().enumerate() {
            released += state.buckets[idx];

            let remaining = available + released;
            if remaining >= tokens {
                retry_after_ticks = ((i + 1) as Uint) * bucket_ticks;
                break;
            }
        }

        Err(VerboseRateLimitError::InsufficientCapacity {
            acquiring: tokens,
            available,
            retry_after_ticks,
        })
    }


    /// Counts the total number of tokens currently present in valid buckets
    /// within the sliding window defined by `window_start_tick` and `tick`.
    ///
    /// Only buckets whose start time falls within the inclusive range
    /// `[window_start_tick, tick]` are considered valid and included in the total.
    /// This ensures that expired or future buckets are excluded from the calculation.
    ///
    /// # Parameters
    /// * `state` - A reference to the internal bucket state
    /// * `tick` - The current tick (inclusive upper bound of the sliding window)
    /// * `window_start_tick` - The oldest tick included in the window (inclusive lower bound)
    ///
    /// # Returns
    /// Returns the total number of tokens in all buckets that fall within the current sliding window.
    #[inline(always)]
    fn count_tokens_in_valid_buckets_within_sliding_window(
        &self,
        state: &SlidingWindowCounterCoreState,
        tick: Uint,
        window_start_tick: Uint,
    ) -> Uint {
        let mut total = 0;
        for i in 0..(self.bucket_count as usize) {
            let start_tick = state.bucket_start_ticks[i];
            if start_tick >= window_start_tick && start_tick <= tick {
                total += state.buckets[i];
            }
        }
        total
    }

    /// Gets the current remaining token capacity in the sliding window.
    ///
    /// This method updates bucket states based on current tick (performs lazy reset
    /// of expired buckets), calculates total tokens used within the current sliding
    /// window, then returns the remaining capacity.
    ///
    /// # Parameters
    /// * `tick` - Current time tick for sliding window calculation
    ///
    /// # Returns
    /// * `Ok(remaining_capacity)` - Remaining tokens available in sliding window
    /// * `Err(SimpleRateLimitError::ContentionFailure)` - Unable to acquire internal lock
    /// * `Err(SimpleRateLimitError::ExpiredTick)` - Time went backwards
    #[inline(always)]
    pub fn capacity_remaining(&self, tick: Uint) -> Result<Uint, SimpleRateLimitError> {
        // Attempt to acquire the lock, return contention error if unavailable
        let mut state = match self.state.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Err(SimpleRateLimitError::ContentionFailure),
        };

        // Prevent time from going backwards (only check if we have previous data)
        if state.bucket_start_ticks[state.last_bucket_index] > 0 && 
           tick < state.bucket_start_ticks[state.last_bucket_index] {
            return Err(SimpleRateLimitError::ExpiredTick);
        }

        // Determine which bucket this tick belongs to
        let current_bucket_index = ((tick / self.bucket_ticks) as usize) % (self.bucket_count as usize);
        let current_bucket_start_tick = (tick / self.bucket_ticks) * self.bucket_ticks;

        // Lazy reset: if this bucket's start time is different, it's a new bucket cycle
        if state.bucket_start_ticks[current_bucket_index] != current_bucket_start_tick {
            state.buckets[current_bucket_index] = 0;
            state.bucket_start_ticks[current_bucket_index] = current_bucket_start_tick;
        }

        // Calculate the sliding window range
        let window_start_tick = tick.saturating_sub(self.window_ticks());

        // Count tokens in all valid buckets within the sliding window
        let total_used = self.count_tokens_in_valid_buckets_within_sliding_window(&state, tick, window_start_tick);

        // Update last bucket index for future ExpiredTick checks
        state.last_bucket_index = current_bucket_index;

        // Return remaining capacity
        Ok(self.capacity.saturating_sub(total_used))
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
    #[inline(always)]
    pub fn capacity_remaining_or_0(&self, tick: Uint) -> Uint {
        self.capacity_remaining(tick).unwrap_or(0)
    }

    /// Gets the current remaining capacity without updating bucket states.
    ///
    /// This method returns the remaining capacity in the current sliding window
    /// without performing any bucket lazy reset or state updates. Suitable for
    /// quick queries when you don't want to modify the bucket states.
    ///
    /// # Returns
    /// * `Ok(remaining_capacity)` - Remaining capacity in sliding window (without state update)
    /// * `Err(SimpleRateLimitError::ContentionFailure)` - Unable to acquire internal lock
    #[inline(always)]
    pub fn current_capacity(&self) -> Result<Uint, SimpleRateLimitError> {
        let state = match self.state.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Err(SimpleRateLimitError::ContentionFailure),
        };

        // Calculate total tokens used in all buckets (without window filtering)
        // Note: This is a simplified approach that counts all tokens in all buckets
        // For a more accurate current sliding window, we'd need the current tick
        let total_used: Uint = state.buckets.iter().sum();

        Ok(self.capacity.saturating_sub(total_used))
    }

    /// Gets the current remaining capacity for a specific tick without updating bucket states.
    ///
    /// This method calculates the remaining capacity for a specific sliding window
    /// without performing bucket lazy reset or state updates.
    ///
    /// # Parameters
    /// * `tick` - Time tick for sliding window calculation
    ///
    /// # Returns
    /// * `Ok(remaining_capacity)` - Remaining capacity in sliding window at given tick
    /// * `Err(SimpleRateLimitError::ContentionFailure)` - Unable to acquire internal lock
    #[inline(always)]
    pub fn current_capacity_at(&self, tick: Uint) -> Result<Uint, SimpleRateLimitError> {
        let state = match self.state.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Err(SimpleRateLimitError::ContentionFailure),
        };

        // Calculate the sliding window range
        let window_start_tick = tick.saturating_sub(self.window_ticks());

        // Count tokens in all valid buckets within the sliding window (without updates)
        let total_used = self.count_tokens_in_valid_buckets_within_sliding_window(&state, tick, window_start_tick);

        Ok(self.capacity.saturating_sub(total_used))
    }


    /// Returns the current remaining capacity
    /// This method is a convenience wrapper around `current_capacity`
    /// that returns 0 if the capacity is not available.
    #[inline(always)]
    pub fn current_capacity_or_0(&self) -> Uint {
        self.current_capacity().unwrap_or(0)
    }
}

/// Configuration structure for creating a `SlidingWindowCounterCore` limiter.
#[derive(Debug, Clone)]
pub struct SlidingWindowCounterCoreConfig {
    /// Maximum number of tokens allowed in the sliding window.
    pub capacity: Uint,
    /// Number of ticks per bucket in the sliding window.
    pub bucket_ticks: Uint,
    /// Total number of buckets in the sliding window.
    pub bucket_count: Uint,
}

impl SlidingWindowCounterCoreConfig {
    /// Creates a new configuration instance.
    pub fn new(capacity: Uint, bucket_ticks: Uint, bucket_count: Uint) -> Self {
        Self {
            capacity,
            bucket_ticks,
            bucket_count,
        }
    }
}

impl From<SlidingWindowCounterCoreConfig> for SlidingWindowCounterCore {
    /// Converts a `SlidingWindowCounterCoreConfig` into a `SlidingWindowCounterCore` instance.
    ///
    /// # Panics
    /// This method will panic if any of the configuration fields are zero.
    /// It assumes that the configuration is already validated or hardcoded.
    ///
    /// # Examples
    ///
    /// Using [`From::from`] explicitly:
    ///
    /// ```
    /// use rate_guard_core::cores::{SlidingWindowCounterCore, SlidingWindowCounterCoreConfig};
    ///
    /// let config = SlidingWindowCounterCoreConfig {
    ///     capacity: 100,
    ///     bucket_ticks: 5,
    ///     bucket_count: 12,
    /// };
    ///
    /// let limiter = SlidingWindowCounterCore::from(config);
    /// ```
    ///
    /// Using `.into()` with type inference:
    ///
    /// ```
    /// use rate_guard_core::cores::{SlidingWindowCounterCore, SlidingWindowCounterCoreConfig};
    ///
    /// let limiter: SlidingWindowCounterCore = SlidingWindowCounterCoreConfig {
    ///     capacity: 100,
    ///     bucket_ticks: 5,
    ///     bucket_count: 12,
    /// }.into();
    /// ```
    #[inline(always)]
    fn from(config: SlidingWindowCounterCoreConfig) -> Self {
        SlidingWindowCounterCore::new(config.capacity, config.bucket_ticks, config.bucket_count)
    }
}
