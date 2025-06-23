use std::sync::Mutex;
use crate::{Uint, RateLimitError, AcquireResult};

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
/// use rate_limiter_core::rate_limiters::SlidingWindowCounterCore;
///
/// // Create counter with capacity 100, bucket size 5 ticks, 4 buckets total
/// // Total window size = 5 * 4 = 20 ticks
/// let counter = SlidingWindowCounterCore::new(100, 5, 4);
///
/// // Tick 2: bucket 0 [0-4], sliding window [0, 2]
/// assert_eq!(counter.try_acquire_at(30, 2), Ok(()));
///
/// // Tick 7: bucket 1 [5-9], sliding window [0, 7] 
/// assert_eq!(counter.try_acquire_at(40, 7), Ok(()));
///
/// // Tick 25: sliding window [6, 25], bucket 0 [0-4] expires
/// // Only bucket 1 [5-9] (40 tokens) counts toward limit
/// assert_eq!(counter.try_acquire_at(60, 25), Ok(()));
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

impl SlidingWindowCounterCore {
    /// Creates a new sliding window counter with the specified parameters.
    ///
    /// # Parameters
    ///
    /// * `capacity` - Maximum number of tokens allowed within the sliding window
    /// * `bucket_ticks` - Duration of each bucket in ticks
    /// * `bucket_count` - Number of buckets in the sliding window
    ///
    /// # Panics
    ///
    /// Panics if any parameter is zero, as this would create an invalid configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rate_limiter_core::rate_limiters::SlidingWindowCounterCore;
    ///
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
    ///
    /// The total duration of the sliding window (bucket_ticks * bucket_count).
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
    ///
    /// * `tokens` - Number of tokens to acquire
    /// * `tick` - Current time tick for the operation
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the tokens were successfully acquired
    /// * `Err(RateLimitError::ExceedsCapacity)` - If acquiring would exceed window capacity
    /// * `Err(RateLimitError::ContentionFailure)` - If unable to acquire the internal lock
    /// * `Err(RateLimitError::ExpiredTick)` - If the tick is older than the last recorded operation
    ///
    /// # Bucket Management
    ///
    /// - Buckets are organized in a circular array indexed by `(tick / bucket_ticks) % bucket_count`
    /// - When accessing a bucket, if its start time doesn't match the expected time, it's reset (lazy reset)
    /// - Only buckets whose start time falls within the sliding window contribute to the total
    ///
    /// # Example
    ///
    /// ```rust
    /// use rate_limiter_core::rate_limiters::SlidingWindowCounterCore;
    /// use rate_limiter_core::RateLimitError;
    ///
    /// let counter = SlidingWindowCounterCore::new(50, 10, 3); // 30-tick window
    ///
    /// // Fill different buckets
    /// assert_eq!(counter.try_acquire_at(20, 5), Ok(()));   // bucket 0 [0-9]
    /// assert_eq!(counter.try_acquire_at(20, 15), Ok(()));  // bucket 1 [10-19]
    /// 
    /// // Tick 35: window [6, 35], bucket 0 [0-9] expires
    /// assert_eq!(counter.try_acquire_at(30, 35), Ok(()));  // Only bucket 1 counts
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

        // Prevent time from going backwards (only check if we have previous data)
        if state.bucket_start_ticks[state.last_bucket_index] > 0 && 
           tick < state.bucket_start_ticks[state.last_bucket_index] {
            return Err(RateLimitError::ExpiredTick);
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
        let mut total = 0;
        for i in 0..(self.bucket_count as usize) {
            // Only count buckets that fall within the sliding window
            if state.bucket_start_ticks[i] >= window_start_tick && state.bucket_start_ticks[i] <= tick {
                total += state.buckets[i];
            }
        }

        // Check if we can accommodate the requested tokens
        if total <= self.capacity.saturating_sub(tokens) {
            state.buckets[current_bucket_index] += tokens;
            state.last_bucket_index = current_bucket_index;
            Ok(())
        } else {
            Err(RateLimitError::ExceedsCapacity)
        }
    }
}