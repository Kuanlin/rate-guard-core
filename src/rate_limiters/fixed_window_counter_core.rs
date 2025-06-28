use std::sync::Mutex;
use crate::{rate_limiter_core::RateLimiterCore, AcquireResult, RateLimitError, Uint};

/// Core implementation of the fixed window counter rate limiting algorithm.
///
/// The fixed window counter algorithm divides time into fixed-size windows and counts
/// requests within each window. When a window boundary is crossed, the counter resets
/// to zero. This provides simple and predictable rate limiting but can allow bursts
/// at window boundaries.
///
/// # Algorithm Behavior
///
/// - Time is divided into fixed windows of `window_ticks` duration
/// - Each window has independent capacity tracking
/// - Counters reset to zero at the start of each new window
/// - Requests are accepted if they don't exceed the window's remaining capacity
///
/// # Window Boundaries
///
/// Windows are aligned to multiples of `window_ticks`:
/// - Window 0: [0, window_ticks-1]
/// - Window 1: [window_ticks, 2*window_ticks-1]
/// - Window 2: [2*window_ticks, 3*window_ticks-1]
/// - And so on...
///
/// # Example
///
/// ```rust
/// use rate_guard_core::rate_limiters::FixedWindowCounterCore;
///
/// // Create a counter with capacity 100 per window of 10 ticks
/// let counter = FixedWindowCounterCore::new(100, 10);
///
/// // Window 0 [0-9]: Use 50 tokens at tick 5
/// assert_eq!(counter.try_acquire_at(50, 5), Ok(()));
/// 
/// // Still in window 0: Use remaining 50 tokens
/// assert_eq!(counter.try_acquire_at(50, 9), Ok(()));
///
/// // Window 1 [10-19]: Counter resets, can use full capacity again
/// assert_eq!(counter.try_acquire_at(100, 10), Ok(()));
/// ```
pub struct FixedWindowCounterCore {
    /// Maximum number of tokens allowed per window
    capacity: Uint,
    /// Duration of each window in ticks
    window_ticks: Uint,
    /// Internal state protected by mutex for thread safety
    state: Mutex<FixedWindowCounterCoreState>,
}

/// Internal state of the fixed window counter
struct FixedWindowCounterCoreState {
    /// Current count of tokens used in the active window
    count: Uint,
    /// Tick when the current window started
    start_tick: Uint,
}

/// Core trait implementation for the fixed window counter.
/// This provides the basic operations needed by the rate limiter core trait.
impl RateLimiterCore for FixedWindowCounterCore {
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
    /// Returns [`AcquireResult`] indicating success or specific failure reason. 
    fn try_acquire_at(&self, tokens: Uint, tick: Uint) -> AcquireResult {
        self.try_acquire_at(tokens, tick)
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


impl FixedWindowCounterCore {
    /// Creates a new fixed window counter with the specified parameters.
    ///
    /// # Parameters
    ///
    /// * `capacity` - Maximum number of tokens allowed per window
    /// * `window_ticks` - Duration of each window in ticks
    ///
    /// # Panics
    ///
    /// Panics if any parameter is zero, as this would create an invalid configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rate_guard_core::rate_limiters::FixedWindowCounterCore;
    /// let counter = FixedWindowCounterCore::new(50, 20);
    /// ```
    pub fn new(capacity: Uint, window_ticks: Uint) -> Self {
        assert!(capacity > 0, "capacity must be greater than 0");
        assert!(window_ticks > 0, "window_ticks must be greater than 0");
        
        FixedWindowCounterCore {
            capacity,
            window_ticks,
            state: Mutex::new(FixedWindowCounterCoreState {
                count: 0,
                start_tick: 0, // First window starts at tick 0
            }),
        }
    }

    /// Attempts to acquire the specified number of tokens at the given tick.
    ///
    /// This method first determines which window the current tick belongs to,
    /// resets the counter if a new window has started, then checks if the
    /// requested tokens can be accommodated within the window's capacity.
    ///
    /// # Parameters
    /// * `tokens` - Number of tokens to acquire
    /// * `tick` - Current time tick for the operation
    ///
    /// # Returns
    /// * `Ok(())` - If the tokens were successfully acquired
    /// * `Err(RateLimitError::ExceedsCapacity)` - If acquiring would exceed window capacity
    /// * `Err(RateLimitError::ContentionFailure)` - If unable to acquire the internal lock
    /// * `Err(RateLimitError::ExpiredTick)` - If the tick is older than the current window start
    ///
    /// # Window Transitions
    ///
    /// When a tick falls into a new window (`tick >= current_window_start + window_ticks`),
    /// the counter automatically resets to zero and the window start time is updated.
    /// This allows for immediate full capacity usage in the new window.
    #[inline(always)]
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

        // Prevent time from going backwards within the current window
        if tick < state.start_tick {
            return Err(RateLimitError::ExpiredTick);
        }

        // Calculate which window the current tick belongs to
        let current_window = tick / self.window_ticks;
        let state_window = state.start_tick / self.window_ticks;

        // Check if we've moved to a new window
        if current_window > state_window {
            // Reset counter and update window start time
            state.count = 0;
            state.start_tick = current_window * self.window_ticks;
        }

        // Check if we can accommodate the requested tokens within capacity
        if tokens <= self.capacity.saturating_sub(state.count) {
            state.count += tokens;
            Ok(())
        } else {
            Err(RateLimitError::ExceedsCapacity)
        }
    }

    /// Gets the current remaining token capacity in the current window.
    /// 
    /// This method updates the window state based on current tick (resets counter
    /// if a new window has started), then returns the remaining capacity in the
    /// current window.
    ///
    /// # Parameters
    /// * `tick` - Current time tick for window calculation
    ///
    /// # Returns
    /// * `Ok(remaining_capacity)` - Remaining tokens available in current window
    /// * `Err(RateLimitError::ContentionFailure)` - Unable to acquire internal lock
    /// * `Err(RateLimitError::ExpiredTick)` - Time went backwards
    #[inline(always)]
    pub fn capacity_remaining(&self, tick: Uint) -> Result<Uint, RateLimitError> {
        // Attempt to acquire the lock, return contention error if unavailable
        let mut state = match self.state.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Err(RateLimitError::ContentionFailure),
        };

        // Prevent time from going backwards within the current window
        if tick < state.start_tick {
            return Err(RateLimitError::ExpiredTick);
        }

        // Calculate which window the current tick belongs to
        let current_window = tick / self.window_ticks;
        let state_window = state.start_tick / self.window_ticks;

        // Check if we've moved to a new window
        if current_window > state_window {
            // Reset counter and update window start time
            state.count = 0;
            state.start_tick = current_window * self.window_ticks;
        }

        // Return remaining capacity in current window
        Ok(self.capacity.saturating_sub(state.count))
    }

    /// Gets the current remaining capacity without updating window state.
    ///
    /// This method simply returns the remaining tokens that can be acquired in
    /// the current window **without** checking or triggering a window change.
    /// Useful for lightweight queries when you do not want to touch state.
    ///
    /// # Returns
    /// * `Ok(remaining_capacity)` - Remaining capacity in current window (without window update)
    /// * `Err(RateLimitError::ContentionFailure)` - Unable to acquire internal lock
    #[inline(always)]
    pub fn current_capacity(&self) -> Result<Uint, RateLimitError> {
        let state = match self.state.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Err(RateLimitError::ContentionFailure),
        };

        Ok(self.capacity.saturating_sub(state.count))
    }
}
