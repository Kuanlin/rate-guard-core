use std::sync::Mutex;
use crate::{Uint, RateLimitError, AcquireResult};
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
/// use rate_limiter_core::rate_limiters::FixedWindowCounterCore;
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
    /// use rate_limiter_core::rate_limiters::FixedWindowCounterCore;
    ///
    /// // Allow 50 requests per window of 20 ticks
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
    ///
    /// * `tokens` - Number of tokens to acquire
    /// * `tick` - Current time tick for the operation
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the tokens were successfully acquired
    /// * `Err(RateLimitError::ExceedsCapacity)` - If acquiring would exceed window capacity
    /// * `Err(RateLimitError::ContentionFailure)` - If unable to acquire the internal lock
    /// * `Err(RateLimitError::ExpiredTick)` - If the tick is older than the current window start
    ///
    /// # Window Transitions
    ///
    /// When a tick falls into a new window (tick >= current_window_start + window_ticks),
    /// the counter automatically resets to zero and the window start time is updated.
    /// This allows for immediate full capacity usage in the new window.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rate_limiter_core::rate_limiters::FixedWindowCounterCore;
    /// use rate_limiter_core::RateLimitError;
    ///
    /// let counter = FixedWindowCounterCore::new(30, 10);
    ///
    /// // Window 0 [0-9]: Use capacity
    /// assert_eq!(counter.try_acquire_at(30, 5), Ok(()));
    /// assert_eq!(counter.try_acquire_at(1, 9), Err(RateLimitError::ExceedsCapacity));
    ///
    /// // Window 1 [10-19]: Counter resets
    /// assert_eq!(counter.try_acquire_at(30, 10), Ok(()));
    /// ```
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
}