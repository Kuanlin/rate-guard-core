//! Approximate sliding window rate limiter implementation.
//!
//! This module provides an approximate sliding window rate limiter that uses
//! a two-window approach to efficiently approximate a true sliding window.

use std::sync::Mutex;
use crate::{rate_limiter_core::RateLimiterCore, AcquireResult, RateLimitError, Uint};

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
/// # use rate_guard_core::rate_limiters::ApproximateSlidingWindowCore;
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

impl RateLimiterCore for ApproximateSlidingWindowCore {
    /// Attempts to acquire tokens at the current tick.
    ///
    /// This is a convenience method that calls `try_acquire_at` with the provided tick.
    ///
    /// # Arguments
    ///
    /// * `tokens` - Number of tokens to acquire
    /// * `tick` - Current time tick
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Tokens successfully acquired
    /// * `Err(RateLimitError)` - Various error conditions (see `try_acquire_at`)
    fn try_acquire_at(&self, tokens: Uint, tick: Uint) -> AcquireResult {
        self.try_acquire_at(tokens, tick)
    }

    /// Gets the current remaining capacity.
    ///
    /// # Arguments
    ///
    /// * `tick` - Current time tick
    ///
    /// # Returns
    ///
    /// Number of tokens currently available for acquisition
    fn capacity_remaining(&self, tick: Uint) -> Uint {
        self.capacity_remaining(tick).unwrap_or(0)
    }
}

/// Internal state of the approximate sliding window counter
#[derive(Debug, Clone)]
struct ApproximateSlidingWindowCoreState {
    /// Token counts for the two alternating windows
    windows: [Uint; 2],
    /// Start ticks for each window (used for overlap calculation)
    window_starts: [Uint; 2],
    /// Index (0 or 1) of the currently active window
    current_index: usize,
}

impl ApproximateSlidingWindowCoreState {
    /// Creates a new state with both windows initialized to start at tick 0.
    fn new() -> Self {
        Self {
            windows: [0, 0],
            window_starts: [0, 0],
            current_index: 0,
        }
    }
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
    /// # use rate_guard_core::rate_limiters::ApproximateSlidingWindowCore;
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
            state: Mutex::new(ApproximateSlidingWindowCoreState::new()),
        }
    }

    /// Performs state transition based on the given tick.
    ///
    /// This function updates the window state to ensure the current window
    /// covers the specified tick. It handles:
    /// - Transitioning to a new window when necessary
    /// - Expiring completely outdated windows
    /// - Initializing new windows with correct start times
    ///
    /// # Arguments
    ///
    /// * `state` - Mutable reference to the window state
    /// * `tick` - The current time tick
    /// * `window_ticks` - Duration of each window in ticks
    fn state_transition_by_tick(
        state: &mut ApproximateSlidingWindowCoreState,
        tick: Uint,
        window_ticks: Uint,
    ) {
        let expected_index = ((tick / window_ticks) % 2) as usize;
        let expected_start = (tick / window_ticks) * window_ticks;

        if expected_index != state.current_index || state.window_starts[expected_index] != expected_start {
            // Switch to new window
            state.current_index = expected_index;

            // Check if we need to reset the window
            if state.window_starts[expected_index] != expected_start {
                // Reset the window for the new time period
                state.windows[expected_index] = 0;
                state.window_starts[expected_index] = expected_start;

                // Check if the other window is completely expired
                let other_idx = crate::other_window!(expected_index);
                if expected_start > state.window_starts[other_idx] + window_ticks {
                    // Other window is completely expired, reset it
                    state.windows[other_idx] = 0;
                    state.window_starts[other_idx] = expected_start;
                }
            }
        }
    }

    /// Calculates the weighted contribution of all windows based on state.
    ///
    /// This function computes how much of the rate limit is currently used by
    /// considering both windows and their overlap with the sliding window.
    ///
    /// The calculation works as follows:
    /// - Current window contributes with full weight (tokens * window_duration)
    /// - Other window contributes proportionally based on its overlap with the sliding window
    /// - Completely expired windows contribute nothing
    ///
    /// # Arguments
    ///
    /// * `state` - Reference to the current window state
    /// * `sw_head` - Start tick of the sliding window (inclusive)
    /// * `sw_end` - End tick of the sliding window (inclusive)
    /// * `window_ticks` - Duration of each window in ticks
    ///
    /// # Returns
    ///
    /// Total weighted contribution from all active windows
    fn calculate_weighted_contribution_by_state(
        state: &ApproximateSlidingWindowCoreState,
        sw_head: Uint,
        sw_end: Uint,
        window_ticks: Uint,
    ) -> Uint {
        let current_idx = state.current_index;
        let other_idx = crate::other_window!(current_idx);

        // Current window always contributes with full weight
        let current_contribution = state.windows[current_idx] * window_ticks;

        // Check if the other window overlaps with the sliding window
        let other_window_start = state.window_starts[other_idx];
        let other_window_end = other_window_start + window_ticks - 1;

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

    /// Updates window state to cover the given tick.
    ///
    /// This method calls the pure state transition function.
    ///
    /// # Arguments
    ///
    /// * `state` - Mutable reference to the window state
    /// * `tick` - The current time tick
    #[inline(always)]
    fn update_windows(&self, state: &mut ApproximateSlidingWindowCoreState, tick: Uint) {
        Self::state_transition_by_tick(state, tick, self.window_ticks);
    }

    /// Calculates weighted contribution using instance state.
    ///
    /// # Arguments
    ///
    /// * `state` - Reference to the window state
    /// * `sw_head` - Start tick of the sliding window (inclusive)
    /// * `sw_end` - End tick of the sliding window (inclusive)
    ///
    /// # Returns
    ///
    /// Total weighted contribution from all active windows
    #[inline(always)]
    fn calculate_weighted_contribution(
        &self,
        state: &ApproximateSlidingWindowCoreState,
        sw_head: Uint,
        sw_end: Uint,
    ) -> Uint {
        Self::calculate_weighted_contribution_by_state(state, sw_head, sw_end, self.window_ticks)
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
        let current_index = state.current_index;

        // Check if request can be accommodated
        if total_contribution <= capacity_contribution.saturating_sub(required_contribution) {
            state.windows[current_index] += tokens;
            Ok(())
        } else {
            Err(RateLimitError::ExceedsCapacity)
        }
    }

    /// Gets the current remaining token capacity using approximate sliding window calculation.
    ///
    /// This method updates the window state and calculates remaining capacity based on
    /// the current usage across all relevant windows.
    ///
    /// # Arguments
    ///
    /// * `tick` - Current time tick
    ///
    /// # Returns
    ///
    /// * `Ok(remaining_capacity)` - Number of tokens that can still be acquired
    /// * `Err(RateLimitError::ExpiredTick)` - If the tick is older than the current state
    /// * `Err(RateLimitError::ContentionFailure)` - If unable to acquire state lock
    #[inline(always)]
    pub fn capacity_remaining(&self, tick: Uint) -> Result<Uint, RateLimitError> {
        let mut state = match self.state.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Err(RateLimitError::ContentionFailure),
        };

        let max_window_start = state.window_starts[0].max(state.window_starts[1]);
        if tick < max_window_start {
            return Err(RateLimitError::ExpiredTick);
        }

        // Update actual state
        Self::state_transition_by_tick(&mut state, tick, self.window_ticks);

        let sw_head = tick.saturating_sub(self.window_ticks - 1);
        let total_contribution = self.calculate_weighted_contribution(&state, sw_head, tick);
        let capacity_contribution = self.capacity * self.window_ticks;
        let remaining_contribution = capacity_contribution.saturating_sub(total_contribution);

        Ok(remaining_contribution / self.window_ticks)
    }

    /// Gets the remaining capacity for a specific tick without updating window state.
    ///
    /// This method provides a read-only view of what the remaining capacity would be
    /// at a given tick, without affecting the current limiter state. It's useful for
    /// planning or checking capacity without committing to token acquisition.
    ///
    /// # Arguments
    ///
    /// * `tick` - The time tick to check capacity for
    ///
    /// # Returns
    ///
    /// * `Ok(remaining_capacity)` - Number of tokens that would be available
    /// * `Err(RateLimitError::ContentionFailure)` - If unable to acquire state lock
    #[inline(always)]
    pub fn current_capacity_at(&self, tick: Uint) -> Result<Uint, RateLimitError> {
        let state = match self.state.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Err(RateLimitError::ContentionFailure),
        };

        // Clone state to do a fake update without affecting the original
        let mut fake_state = ApproximateSlidingWindowCoreState {
            windows: state.windows,
            window_starts: state.window_starts,
            current_index: state.current_index,
        };

        // Do fake update on cloned state
        Self::state_transition_by_tick(&mut fake_state, tick, self.window_ticks);

        // Now use the existing calculation with the updated fake state
        let sw_head = tick.saturating_sub(self.window_ticks - 1);
        let total_contribution = Self::calculate_weighted_contribution_by_state(&fake_state, sw_head, tick, self.window_ticks);
        let capacity_contribution = self.capacity * self.window_ticks;
        let remaining_contribution = capacity_contribution.saturating_sub(total_contribution);

        Ok(remaining_contribution / self.window_ticks)
    }

    /// Gets the current capacity based on the existing window state.
    ///
    /// This method calculates the remaining capacity using the current window state
    /// without any updates or state transitions. It uses the most recent window's
    /// start time as the reference point for the sliding window calculation.
    ///
    /// # Returns
    ///
    /// * `Ok(remaining_capacity)` - Number of tokens currently available based on existing state
    /// * `Err(RateLimitError::ContentionFailure)` - If unable to acquire state lock
    #[inline(always)]
    pub fn current_capacity(&self) -> Result<Uint, RateLimitError> {
        let state = match self.state.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Err(RateLimitError::ContentionFailure),
        };

        // Use the current window's end as the reference tick for sliding window calculation
        let current_window_start = state.window_starts[state.current_index];
        let reference_tick = current_window_start + self.window_ticks - 1;

        // Calculate capacity based on current state without any updates
        let sw_head = reference_tick.saturating_sub(self.window_ticks - 1);
        let total_contribution = Self::calculate_weighted_contribution_by_state(&state, sw_head, reference_tick, self.window_ticks);
        let capacity_contribution = self.capacity * self.window_ticks;
        let remaining_contribution = capacity_contribution.saturating_sub(total_contribution);

        Ok(remaining_contribution / self.window_ticks)
    }
}
