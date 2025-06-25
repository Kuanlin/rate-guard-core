use std::sync::Mutex;
use crate::{Uint, RateLimitError, AcquireResult};

/// Toggles between window indices 0 and 1.
///
/// This macro is used to switch between the two alternating windows
/// in the approximate sliding window algorithm.
///
/// # Parameters
///
/// * `$current` - Current window index (must be 0 or 1)
///
/// # Returns
///
/// The opposite window index (0 becomes 1, 1 becomes 0)
///
/// # Example
///
/// ```rust
/// use rate_guard_core::other_window;
///
/// assert_eq!(other_window!(0), 1);
/// assert_eq!(other_window!(1), 0);
/// ```
#[macro_export]
macro_rules! other_window {
    ($current:expr) => {{
        debug_assert!($current <= 1, "Window index must be 0 or 1, got {}", $current);
        (!($current) & 1)
    }};
}
/// Core implementation of the approximate sliding window rate limiting algorithm.
///
/// The approximate sliding window algorithm uses only two windows to estimate
/// token usage within a sliding window. This provides a good balance between
/// accuracy and memory efficiency compared to the full sliding window counter.
/// It uses weighted interpolation to approximate the contribution of the
/// previous window to the current sliding window.
///
/// # Algorithm Behavior
///
/// - Maintains exactly two windows that alternate as time progresses
/// - Each window covers `window_ticks` duration
/// - Uses weighted contribution from the previous window based on overlap
/// - More memory efficient than full sliding window counter
/// - Provides reasonable approximation of true sliding window behavior
///
/// # Window Management
///
/// Windows are assigned based on `(tick / window_ticks) % 2`:
/// - Window 0: [0, window_ticks-1], [2*window_ticks, 3*window_ticks-1], ...
/// - Window 1: [window_ticks, 2*window_ticks-1], [3*window_ticks, 4*window_ticks-1], ...
///
/// # Weighted Contribution Calculation
///
/// For a sliding window [sw_head, tick], the total contribution is calculated as:
/// - Current window: `tokens * window_ticks` (full weight)
/// - Previous window: `tokens * overlap_length` (partial weight based on overlap)
///
/// # Example
///
/// ```rust
/// use rate_guard_core::rate_limiters::ApproximateSlidingWindowCore;
///
/// // Create counter with capacity 100, window size 10 ticks
/// let counter = ApproximateSlidingWindowCore::new(100, 10);
///
/// // Tick 5: Window 0 [0-9], sliding window [0, 5]
/// assert_eq!(counter.try_acquire_at(30, 5), Ok(()));
///
/// // Tick 15: Window 1 [10-19], sliding window [6, 15]
/// // Window 0 contributes partially based on overlap [6, 9] = 4 ticks
/// assert_eq!(counter.try_acquire_at(40, 15), Ok(()));
/// ```
pub struct ApproximateSlidingWindowCore {
    /// Maximum number of tokens allowed within the sliding window
    capacity: Uint,
    /// Duration of each window in ticks
    window_ticks: Uint,
    /// Internal state protected by mutex for thread safety
    state: Mutex<ApproximateSlidingWindowCoreState>,
}

/// Internal state of the approximate sliding window counter
struct ApproximateSlidingWindowCoreState {
    /// Token counts for the two alternating windows
    windows: [Uint; 2],
    /// Start ticks for each window (used for overlap calculation)
    window_starts: [Uint; 2],
    /// Index (0 or 1) of the currently active window
    current_index: usize,
}

impl ApproximateSlidingWindowCore {
    /// Creates a new approximate sliding window counter with the specified parameters.
    ///
    /// # Parameters
    ///
    /// * `capacity` - Maximum number of tokens allowed within the sliding window
    /// * `window_ticks` - Duration of each window in ticks
    ///
    /// # Panics
    ///
    /// Panics if any parameter is zero, as this would create an invalid configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rate_guard_core::rate_limiters::ApproximateSlidingWindowCore;
    ///
    /// // Allow 200 tokens within a sliding window of 20 ticks
    /// let counter = ApproximateSlidingWindowCore::new(200, 20);
    /// ```
    pub fn new(capacity: Uint, window_ticks: Uint) -> Self {
        assert!(capacity > 0, "capacity must be greater than 0");
        assert!(window_ticks > 0, "window_ticks must be greater than 0");
        
        ApproximateSlidingWindowCore {
            capacity,
            window_ticks,
            state: Mutex::new(ApproximateSlidingWindowCoreState {
                windows: [0, 0],
                window_starts: [0, 0], // Initialize two consecutive windows
                current_index: 0,
            }),
        }
    }

    /// Attempts to acquire the specified number of tokens at the given tick.
    ///
    /// This method updates the window state, calculates the weighted contribution
    /// from both windows based on their overlap with the current sliding window,
    /// and checks if the request can be accommodated within the capacity limit.
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
    /// * `Err(RateLimitError::ExpiredTick)` - If the tick is older than any window start
    ///
    /// # Approximation Method
    ///
    /// The algorithm uses weighted contributions to approximate sliding window behavior:
    /// 1. Calculate sliding window range: [tick - window_ticks + 1, tick]
    /// 2. Current window contributes: `tokens * window_ticks`
    /// 3. Previous window contributes: `tokens * overlap_length`
    /// 4. Total must not exceed: `capacity * window_ticks`
    ///
    /// # Example
    ///
    /// ```rust
    /// use rate_guard_core::rate_limiters::ApproximateSlidingWindowCore;
    /// use rate_guard_core::RateLimitError;
    ///
    /// let counter = ApproximateSlidingWindowCore::new(60, 10);
    ///
    /// // Window 0: Add tokens
    /// assert_eq!(counter.try_acquire_at(30, 5), Ok(()));
    ///
    /// // Window 1: Previous window partially contributes
    /// assert_eq!(counter.try_acquire_at(40, 15), Ok(()));
    ///
    /// // Check capacity limit
    /// assert_eq!(counter.try_acquire_at(10, 15), Err(RateLimitError::ExceedsCapacity));
    /// ```
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

        // Prevent time from going backwards - check against the latest window start
        let max_window_start = state.window_starts[0].max(state.window_starts[1]);
        if tick < max_window_start {
            return Err(RateLimitError::ExpiredTick);
        }

        // Update window state based on current tick
        self.update_windows(&mut state, tick);

        // Calculate sliding window range [sw_head, tick]
        let sw_head = tick.saturating_sub(self.window_ticks - 1);
        
        // Calculate weighted contributions and check capacity
        let total_contribution = self.calculate_weighted_contribution(&state, sw_head, tick);
        let required_contribution = tokens * self.window_ticks;
        let capacity_contribution = self.capacity * self.window_ticks;
        let current_index = state.current_index; // Store current index

        // Check if request can be accommodated
        if total_contribution <= capacity_contribution.saturating_sub(required_contribution) {
            state.windows[current_index] += tokens; // Use stored index
            Ok(())
        } else {
            Err(RateLimitError::ExceedsCapacity)
        }
    }

    /// Updates the window state based on the current tick.
    ///
    /// This method determines which window should be active and resets
    /// windows when transitioning to a new time period.
    ///
    /// # Parameters
    ///
    /// * `state` - Mutable reference to the internal state
    /// * `tick` - Current time tick
    #[inline(always)]
    fn update_windows(&self, state: &mut ApproximateSlidingWindowCoreState, tick: Uint) {
        let expected_index = ((tick / self.window_ticks) % 2) as usize;
        let expected_start = (tick / self.window_ticks) * self.window_ticks;

        if expected_index != state.current_index || state.window_starts[expected_index] != expected_start {
            // Switch to new window
            state.current_index = expected_index;
            if state.window_starts[expected_index] != expected_start {
                state.windows[expected_index] = 0;
                state.window_starts[expected_index] = expected_start;
            }
        }
    }

    /// Calculates the weighted contribution from both windows to the sliding window.
    ///
    /// This method determines how much each window contributes to the current
    /// sliding window based on their overlap with the sliding window range.
    ///
    /// # Parameters
    ///
    /// * `state` - Reference to the internal state
    /// * `sw_head` - Start of the sliding window
    /// * `sw_end` - End of the sliding window
    ///
    /// # Returns
    ///
    /// Total weighted contribution from both windows
    #[inline(always)]
    fn calculate_weighted_contribution(&self, state: &ApproximateSlidingWindowCoreState, sw_head: Uint, sw_end: Uint) -> Uint {
        let current_idx = state.current_index;
        let other_idx = crate::other_window!(current_idx);

        // Current window always contributes with full weight
        let current_contribution = state.windows[current_idx] * self.window_ticks;

        // Check if the other window overlaps with the sliding window
        let other_window_start = state.window_starts[other_idx];
        let other_window_end = other_window_start + self.window_ticks - 1;

        if sw_head > other_window_end {
            // Other window completely expired - no contribution
            current_contribution
        } else {
            // Calculate overlap length between other window and sliding window
            let overlap_start = sw_head.max(other_window_start);
            let overlap_end = sw_end.min(other_window_end);
            let overlap = if overlap_start <= overlap_end {
                overlap_end - overlap_start + 1
            } else {
                0
            };

            // Other window contributes based on overlap length
            let other_contribution = state.windows[other_idx] * overlap;
            current_contribution + other_contribution
        }
    }
}