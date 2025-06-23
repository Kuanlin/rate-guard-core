use rate_limiter_core::{RateLimitError};
use rate_limiter_core::rate_limiters::FixedWindowCounterCore;

#[test]
fn test_new_fixed_window_counter() {
    let _ = FixedWindowCounterCore::new(100, 10);
    // Constructor should succeed without panic
}

#[test]
#[should_panic(expected = "capacity must be greater than 0")]
fn test_new_with_zero_capacity() {
    FixedWindowCounterCore::new(0, 10);
}

#[test]
#[should_panic(expected = "window_ticks must be greater than 0")]
fn test_new_with_zero_window_ticks() {
    FixedWindowCounterCore::new(100, 0);
}

#[test]
fn test_acquire_zero_tokens() {
    let counter = FixedWindowCounterCore::new(100, 10);
    // Zero token requests should always succeed regardless of counter state
    assert_eq!(counter.try_acquire_at(0, 0), Ok(()));
    assert_eq!(counter.try_acquire_at(0, 100), Ok(()));
}

#[test]
fn test_basic_acquire_single_window() {
    let counter = FixedWindowCounterCore::new(100, 10); // Windows: [0-9], [10-19], [20-29]...
    
    // Consume tokens within the first window [0-9]
    assert_eq!(counter.try_acquire_at(30, 0), Ok(())); // count = 30
    assert_eq!(counter.try_acquire_at(20, 5), Ok(())); // count = 50
    assert_eq!(counter.try_acquire_at(50, 9), Ok(())); // count = 100
    
    // Now at capacity limit
    assert_eq!(counter.try_acquire_at(1, 9), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_window_transition() {
    let counter = FixedWindowCounterCore::new(100, 10); // Window size 10 ticks
    
    // Use all capacity in first window [0-9]
    assert_eq!(counter.try_acquire_at(100, 5), Ok(()));
    assert_eq!(counter.try_acquire_at(1, 9), Err(RateLimitError::ExceedsCapacity));
    
    // Transition to second window [10-19] - counter resets
    assert_eq!(counter.try_acquire_at(50, 10), Ok(())); // count = 50 in new window
    assert_eq!(counter.try_acquire_at(50, 15), Ok(())); // count = 100 in new window
    
    // Second window is also full
    assert_eq!(counter.try_acquire_at(1, 19), Err(RateLimitError::ExceedsCapacity));
    
    // Transition to third window [20-29] - counter resets again
    assert_eq!(counter.try_acquire_at(100, 20), Ok(()));
}

#[test]
fn test_window_boundaries() {
    let counter = FixedWindowCounterCore::new(50, 10);
    
    // Window 0: [0-9]
    assert_eq!(counter.try_acquire_at(25, 0), Ok(())); // count = 25
    assert_eq!(counter.try_acquire_at(25, 9), Ok(())); // count = 50
    
    // tick 10 starts new window [10-19], counter resets to 0
    assert_eq!(counter.try_acquire_at(50, 10), Ok(())); // count = 50 in new window
    
    // tick 20 starts new window [20-29], counter resets to 0
    assert_eq!(counter.try_acquire_at(50, 20), Ok(())); // count = 50 in new window
}

#[test]
fn test_skip_windows() {
    let counter = FixedWindowCounterCore::new(100, 10);
    
    // Use some capacity in window 0 [0-9]
    assert_eq!(counter.try_acquire_at(50, 5), Ok(()));
    
    // Jump multiple windows to tick 35 (window 3: [30-39])
    // Windows 1 and 2 are skipped entirely
    assert_eq!(counter.try_acquire_at(100, 35), Ok(()));
    
    // Within same window, should not be able to add more
    assert_eq!(counter.try_acquire_at(1, 39), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_window_calculation() {
    let counter = FixedWindowCounterCore::new(10, 5); // Window size 5 ticks
    
    // Window 0: [0-4]
    assert_eq!(counter.try_acquire_at(5, 0), Ok(()));
    assert_eq!(counter.try_acquire_at(5, 4), Ok(()));
    
    // Window 1: [5-9] 
    assert_eq!(counter.try_acquire_at(10, 5), Ok(()));
    
    // Window 2: [10-14]
    assert_eq!(counter.try_acquire_at(10, 10), Ok(()));
    
    // Window 3: [15-19]
    assert_eq!(counter.try_acquire_at(10, 15), Ok(()));
}

#[test]
fn test_expired_tick() {
    let counter = FixedWindowCounterCore::new(100, 10);
    
    // Transition to window 1 [10-19], start_tick updates to 10
    assert_eq!(counter.try_acquire_at(10, 15), Ok(()));
    
    // Time going backwards below start_tick(10) should fail
    assert_eq!(counter.try_acquire_at(10, 9), Err(RateLimitError::ExpiredTick));
    assert_eq!(counter.try_acquire_at(10, 5), Err(RateLimitError::ExpiredTick));
    assert_eq!(counter.try_acquire_at(10, 0), Err(RateLimitError::ExpiredTick));
    
    // Equal to current start_tick should be allowed
    assert_eq!(counter.try_acquire_at(10, 10), Ok(()));
    
    // Within current window should work
    assert_eq!(counter.try_acquire_at(10, 15), Ok(()));
    assert_eq!(counter.try_acquire_at(10, 19), Ok(()));
    
    // Transition to newer window
    assert_eq!(counter.try_acquire_at(10, 25), Ok(())); // Window 2 [20-29], start_tick = 20
    
    // Going back to previous window should fail
    assert_eq!(counter.try_acquire_at(10, 19), Err(RateLimitError::ExpiredTick));
    assert_eq!(counter.try_acquire_at(10, 15), Err(RateLimitError::ExpiredTick));
}

#[test]
fn test_window_start_alignment() {
    let counter = FixedWindowCounterCore::new(20, 10);
    
    // Start at tick 7, should be in window 0 [0-9]
    assert_eq!(counter.try_acquire_at(10, 7), Ok(()));
    
    // tick 12 should be in window 1 [10-19], start_tick should update to 10
    assert_eq!(counter.try_acquire_at(15, 12), Ok(()));
    
    // Cannot go back to tick 9 (less than current start_tick 10)
    assert_eq!(counter.try_acquire_at(5, 9), Err(RateLimitError::ExpiredTick));
    
    // tick 25 should be in window 2 [20-29]  
    assert_eq!(counter.try_acquire_at(20, 25), Ok(()));
}

#[test]
fn test_capacity_edge_cases() {
    let counter = FixedWindowCounterCore::new(1, 10); // Capacity of only 1
    
    // Use the single token
    assert_eq!(counter.try_acquire_at(1, 5), Ok(()));
    
    // Cannot add more
    assert_eq!(counter.try_acquire_at(1, 8), Err(RateLimitError::ExceedsCapacity));
    
    // New window resets counter
    assert_eq!(counter.try_acquire_at(1, 10), Ok(()));
}

#[test]
fn test_large_window_size() {
    let counter = FixedWindowCounterCore::new(1000, 100);
    
    // Multiple operations within large window
    for i in 0..10 {
        assert_eq!(counter.try_acquire_at(100, i * 5), Ok(()));
    }
    
    // Window transition at tick 100
    assert_eq!(counter.try_acquire_at(1000, 100), Ok(()));
}

#[test]
fn test_saturating_operations() {
    let counter = FixedWindowCounterCore::new(u64::MAX, u64::MAX);
    
    // Test that large values don't overflow
    assert_eq!(counter.try_acquire_at(u64::MAX, 0), Ok(()));
    
    // Large time jumps should work
    assert_eq!(counter.try_acquire_at(u64::MAX, u64::MAX), Ok(()));
}

#[test]
fn test_consecutive_windows() {
    let counter = FixedWindowCounterCore::new(30, 10);
    
    // Window 0 [0-9]: use 20
    assert_eq!(counter.try_acquire_at(20, 5), Ok(()));
    
    // Window 1 [10-19]: use 30 (full)
    assert_eq!(counter.try_acquire_at(30, 15), Ok(()));
    
    // Window 2 [20-29]: use 10  
    assert_eq!(counter.try_acquire_at(10, 25), Ok(()));
    
    // Window 3 [30-39]: use 30 (full)
    assert_eq!(counter.try_acquire_at(30, 35), Ok(()));
    
    // Check independence of each window
    assert_eq!(counter.try_acquire_at(1, 35), Err(RateLimitError::ExceedsCapacity));
}