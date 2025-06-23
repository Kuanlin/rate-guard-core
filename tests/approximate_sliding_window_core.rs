use rate_limiter_core::{RateLimitError};
use rate_limiter_core::rate_limiters::ApproximateSlidingWindowCore;

#[test]
fn test_new_approximate_sliding_window() {
    let _ = ApproximateSlidingWindowCore::new(100, 10);
    // Constructor should succeed without panic
}

#[test]
#[should_panic(expected = "capacity must be greater than 0")]
fn test_new_with_zero_capacity() {
    ApproximateSlidingWindowCore::new(0, 10);
}

#[test]
#[should_panic(expected = "window_ticks must be greater than 0")]
fn test_new_with_zero_window_ticks() {
    ApproximateSlidingWindowCore::new(100, 0);
}

#[test]
fn test_acquire_zero_tokens() {
    let counter = ApproximateSlidingWindowCore::new(100, 10);
    // Zero token requests should always succeed regardless of counter state
    assert_eq!(counter.try_acquire_at(0, 0), Ok(()));
    assert_eq!(counter.try_acquire_at(0, 100), Ok(()));
}

#[test]
fn test_basic_acquire() {
    let counter = ApproximateSlidingWindowCore::new(10, 5);
    
    // At tick 0, should be able to acquire 5 tokens
    assert_eq!(counter.try_acquire_at(5, 0), Ok(()));
    
    // Should be able to acquire 5 more (capacity is 10)
    assert_eq!(counter.try_acquire_at(5, 0), Ok(()));
    
    // Exceeds capacity
    assert_eq!(counter.try_acquire_at(1, 0), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_window_switching() {
    let counter = ApproximateSlidingWindowCore::new(10, 5);
    
    // tick 4: Window 0 [0, 4], use 3 tokens
    assert_eq!(counter.try_acquire_at(3, 4), Ok(()));
    
    // tick 7: Still in Window 1 [5, 9], sliding window [3, 7]
    // Window 0 [0, 4] overlaps with sliding window [3, 7] at [3, 4] = 2 ticks
    // Weighted calculation will impose partial restrictions
    assert_eq!(counter.try_acquire_at(5, 7), Ok(()));
    
    // Try to exceed total capacity
    assert_eq!(counter.try_acquire_at(5, 7), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_sliding_window_overlap() {
    let counter = ApproximateSlidingWindowCore::new(10, 5);
    
    // Use 3 tokens in first window
    assert_eq!(counter.try_acquire_at(3, 5), Ok(()));
    
    // tick 15: sliding window [11, 15], active window [15, 19], inactive window [0, 4]
    // Inactive window has no overlap with sliding window, so doesn't count
    assert_eq!(counter.try_acquire_at(7, 15), Ok(()));
}

#[test]
fn test_window_expiration() {
    let counter = ApproximateSlidingWindowCore::new(10, 5);
    
    // Use tokens in first window
    assert_eq!(counter.try_acquire_at(5, 5), Ok(()));
    
    // tick 25: sliding window [21, 25], inactive window [0, 4] has expired
    // Since there's no overlap, can use full capacity
    assert_eq!(counter.try_acquire_at(10, 25), Ok(()));
}

#[test]
fn test_approximation_accuracy() {
    let counter = ApproximateSlidingWindowCore::new(10, 10);
    
    // Use full capacity at tick 9
    assert_eq!(counter.try_acquire_at(10, 9), Ok(()));
    
    // tick 15: sliding window [6, 15]
    // Inactive window [0, 9] overlaps with sliding window at [6, 9] = 4 ticks
    // According to weighted calculation, effective tokens are reduced
    assert_eq!(counter.try_acquire_at(6, 15), Ok(()));
    
    // Exceeds limit
    assert_eq!(counter.try_acquire_at(1, 15), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_macro_safety() {
    // Test other_window! macro
    use rate_limiter_core::other_window;
    assert_eq!(other_window!(0), 1);
    assert_eq!(other_window!(1), 0);
}

#[test]
fn test_expired_tick() {
    let counter = ApproximateSlidingWindowCore::new(100, 10);
    
    // Establish state of two windows
    assert_eq!(counter.try_acquire_at(10, 5), Ok(()));   // Window 0 [0-9]
    assert_eq!(counter.try_acquire_at(10, 15), Ok(()));  // Window 1 [10-19]
    
    // Advance to newer window, updating max_window_start
    assert_eq!(counter.try_acquire_at(10, 25), Ok(()));  // Window 0 [20-29]
    
    // Now max_window_start should be 20, going back should fail
    assert_eq!(counter.try_acquire_at(10, 19), Err(RateLimitError::ExpiredTick));
    assert_eq!(counter.try_acquire_at(10, 15), Err(RateLimitError::ExpiredTick));
    assert_eq!(counter.try_acquire_at(10, 10), Err(RateLimitError::ExpiredTick));
    
    // Equal to or greater than current max_window_start should work
    assert_eq!(counter.try_acquire_at(10, 20), Ok(()));
    assert_eq!(counter.try_acquire_at(10, 25), Ok(()));
    
    // Continue advancing
    assert_eq!(counter.try_acquire_at(10, 35), Ok(()));  // Window 1 [30-39]
    
    // Now going back should fail
    assert_eq!(counter.try_acquire_at(10, 29), Err(RateLimitError::ExpiredTick));
}

#[test]
fn test_window_boundary_precise() {
    let counter = ApproximateSlidingWindowCore::new(20, 10);
    
    // tick 5: Window 0 [0-9]
    assert_eq!(counter.try_acquire_at(8, 5), Ok(()));
    
    // tick 10: Window 1 [10-19], sliding window [1, 10]
    // Window 0 overlap portion [1, 9] = 9 ticks
    assert_eq!(counter.try_acquire_at(10, 10), Ok(()));
    
    // tick 15: sliding window [6, 15]
    // Window 0 overlap portion [6, 9] = 4 ticks  
    assert_eq!(counter.try_acquire_at(2, 15), Ok(()));
}

#[test]
fn test_large_time_gap() {
    let counter = ApproximateSlidingWindowCore::new(100, 10);
    
    // Add some tokens
    assert_eq!(counter.try_acquire_at(50, 5), Ok(()));
    
    // Jump over long time, old data should all expire
    assert_eq!(counter.try_acquire_at(100, 1000), Ok(()));
}

#[test]
fn test_same_window_operations() {
    let counter = ApproximateSlidingWindowCore::new(100, 20);
    
    // Multiple operations within same window
    assert_eq!(counter.try_acquire_at(20, 5), Ok(()));
    assert_eq!(counter.try_acquire_at(30, 10), Ok(()));
    assert_eq!(counter.try_acquire_at(25, 15), Ok(()));
    assert_eq!(counter.try_acquire_at(20, 19), Ok(()));
    
    // Check if there's still space
    assert_eq!(counter.try_acquire_at(5, 19), Ok(()));
}

#[test]
fn test_saturating_operations() {
    // Use smaller values to avoid overflow
    let counter = ApproximateSlidingWindowCore::new(1000, 100);
    
    // Test won't overflow
    assert_eq!(counter.try_acquire_at(500, 0), Ok(()));
    assert_eq!(counter.try_acquire_at(400, 150), Ok(()));
}