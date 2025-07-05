use rate_guard_core::{ SimpleRateLimitError};
use rate_guard_core::rate_limiters::SlidingWindowCounterCore;

#[test]
fn test_new_sliding_window_counter() {
    let _ = SlidingWindowCounterCore::new(100, 5, 4);
    // Constructor should succeed without panic
}

#[test]
#[should_panic(expected = "capacity must be greater than 0")]
fn test_new_with_zero_capacity() {
    SlidingWindowCounterCore::new(0, 5, 4);
}

#[test]
#[should_panic(expected = "bucket_ticks must be greater than 0")]
fn test_new_with_zero_bucket_ticks() {
    SlidingWindowCounterCore::new(100, 0, 4);
}

#[test]
#[should_panic(expected = "bucket_count must be greater than 0")]
fn test_new_with_zero_bucket_count() {
    SlidingWindowCounterCore::new(100, 5, 0);
}

#[test]
fn test_acquire_zero_tokens() {
    let counter = SlidingWindowCounterCore::new(100, 5, 4);
    // Zero token requests should always succeed regardless of counter state
    assert_eq!(counter.try_acquire_at(0, 0), Ok(()));
    assert_eq!(counter.try_acquire_at(100, 0), Ok(()));
}

#[test]
fn test_basic_sliding_window() {
    // bucket_ticks=5, bucket_count=4, window_ticks=20
    // Sliding window size: 20 ticks
    let counter = SlidingWindowCounterCore::new(100, 5, 4);
    
    // tick 0: sliding window [0, 0], Bucket 0 [0-4]
    assert_eq!(counter.try_acquire_at(0, 25), Ok(()));
    
    // tick 5: sliding window [0, 5], Bucket 1 [5-9]
    assert_eq!(counter.try_acquire_at(5, 25), Ok(()));
    
    // tick 10: sliding window [0, 10], Bucket 2 [10-14]
    assert_eq!(counter.try_acquire_at(10, 25), Ok(()));
    
    // tick 15: sliding window [0, 15], Bucket 3 [15-19]
    assert_eq!(counter.try_acquire_at(15, 25), Ok(()));
    
    // Total now 100, should reach capacity
    assert_eq!(counter.try_acquire_at(15, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}

#[test]
fn test_sliding_window_expiry() {
    let counter = SlidingWindowCounterCore::new(100, 5, 4); // window_ticks = 20
    
    // tick 0: Bucket 0 [0-4]
    assert_eq!(counter.try_acquire_at(0, 50), Ok(()));
    
    // tick 25: sliding window [5, 25], Bucket 0 [0-4] expires (start_tick=0 < 5)
    // Only current bucket counts toward the limit
    assert_eq!(counter.try_acquire_at(25, 100), Ok(()));
    
    // Total now 100, should reach capacity
    assert_eq!(counter.try_acquire_at(25, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}

#[test]
fn test_bucket_rotation() {
    let counter = SlidingWindowCounterCore::new(100, 5, 4); // window_ticks = 20
    
    // tick 2: Bucket 0 [0-4]
    assert_eq!(counter.try_acquire_at(2, 30), Ok(()));
    
    // tick 7: Bucket 1 [5-9]
    assert_eq!(counter.try_acquire_at(7, 30), Ok(()));
    
    // tick 22: sliding window [2, 22], Bucket 0 [0-4] expires (start_tick=0 < 2)
    // But Bucket 1 [5-9] still within window (start_tick=5 >= 2)
    // So total = 30, adding 70 gives total = 100
    assert_eq!(counter.try_acquire_at(22, 70), Ok(()));
    assert_eq!(counter.try_acquire_at(22, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}

#[test]
fn test_bucket_lazy_reset() {
    let counter = SlidingWindowCounterCore::new(100, 5, 4);
    
    // tick 2: Bucket 0 [0-4]
    assert_eq!(counter.try_acquire_at(2, 30), Ok(()));
    
    // tick 7: Bucket 1 [5-9]
    assert_eq!(counter.try_acquire_at(7, 40), Ok(()));
    
    // tick 42: sliding window [22, 42], all old buckets expire
    // Buckets are lazily reset when accessed
    assert_eq!(counter.try_acquire_at(42, 100), Ok(()));
}

#[test]
fn test_multiple_bucket_cycles() {
    let counter = SlidingWindowCounterCore::new(80, 5, 2); // window_ticks = 10
    
    // tick 1: Bucket 0 [0-4]
    assert_eq!(counter.try_acquire_at(1, 20), Ok(()));
    
    // tick 6: Bucket 1 [5-9]  
    assert_eq!(counter.try_acquire_at(6, 20), Ok(()));
    
    // tick 12: sliding window [2, 12], Bucket 0 [0-4] expires, Bucket 1 [5-9] still valid
    assert_eq!(counter.try_acquire_at(12, 20), Ok(())); // total = 20 + 20 = 40
    
    // tick 17: sliding window [7, 17], Bucket 1 [5-9] expires
    assert_eq!(counter.try_acquire_at(17, 40), Ok(())); // total = 20 + 40 = 60
    
    assert_eq!(counter.try_acquire_at(17, 20), Ok(())); // total = 60 + 20 = 80
    assert_eq!(counter.try_acquire_at(17, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}

#[test]
fn test_expired_tick() {
    let counter = SlidingWindowCounterCore::new(100, 5, 4);
    
    // Perform first operation
    assert_eq!(counter.try_acquire_at(15, 10), Ok(()));
    
    // Time going backwards should fail
    assert_eq!(counter.try_acquire_at(10, 10), Err(SimpleRateLimitError::ExpiredTick));
    assert_eq!(counter.try_acquire_at(5, 10), Err(SimpleRateLimitError::ExpiredTick));
    
    // Same time should be allowed
    assert_eq!(counter.try_acquire_at(15, 10), Ok(()));
    
    // Move time forward
    assert_eq!(counter.try_acquire_at(25, 10), Ok(()));
    
    // Going back to previous time should fail
    assert_eq!(counter.try_acquire_at(20, 10), Err(SimpleRateLimitError::ExpiredTick));
}

#[test]
fn test_sliding_window_boundaries() {
    let counter = SlidingWindowCounterCore::new(60, 10, 3); // window_ticks = 30
    
    // tick 5: Bucket 0 [0-9]
    assert_eq!(counter.try_acquire_at(5, 20), Ok(()));
    
    // tick 15: Bucket 1 [10-19]
    assert_eq!(counter.try_acquire_at(15, 20), Ok(()));
    
    // tick 25: Bucket 2 [20-29]
    assert_eq!(counter.try_acquire_at(25, 20), Ok(()));
    
    // tick 35: sliding window [5, 35], Bucket 0 [0-9] expires (start_tick=0 < 5)
    assert_eq!(counter.try_acquire_at(35, 20), Ok(())); // total = 20 + 20 + 20 = 60
    assert_eq!(counter.try_acquire_at(35, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}

#[test]
fn test_window_size_calculation() {
    let counter = SlidingWindowCounterCore::new(100, 5, 3); // window_ticks = 15
    
    // tick 10: sliding window [0, 10] (due to saturating_sub)
    assert_eq!(counter.try_acquire_at(10, 50), Ok(()));
    
    // tick 20: sliding window [5, 20], previous bucket [0-4] expires
    assert_eq!(counter.try_acquire_at(20, 50), Ok(()));
    
    // Check if capacity is reached
    assert_eq!(counter.try_acquire_at(20, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}

#[test]
fn test_large_time_gap() {
    let counter = SlidingWindowCounterCore::new(100, 5, 4);
    
    // Add some tokens
    assert_eq!(counter.try_acquire_at(5, 50), Ok(()));
    
    // Jump over long time, old data should all expire
    assert_eq!(counter.try_acquire_at(1000, 100), Ok(()));
}

#[test]
fn test_single_bucket() {
    let counter = SlidingWindowCounterCore::new(50, 10, 1);
    
    // All operations cycle through the same bucket slot
    assert_eq!(counter.try_acquire_at(5, 25), Ok(()));
    assert_eq!(counter.try_acquire_at(9, 25), Ok(()));
    
    // Next cycle, bucket resets
    assert_eq!(counter.try_acquire_at(15, 50), Ok(()));
}

#[test]
fn test_saturating_operations() {
    // Use smaller values to avoid overflow
    let counter = SlidingWindowCounterCore::new(1000, 100, 2);
    
    // Test won't overflow
    assert_eq!(counter.try_acquire_at(0, 500), Ok(()));
    assert_eq!(counter.try_acquire_at(150, 500), Ok(()));
}

#[test]
fn test_window_edge_cases() {
    let counter = SlidingWindowCounterCore::new(50, 5, 2); // window_ticks = 10
    
    // tick 0: sliding window [0, 0], Bucket 0 [0-4]
    assert_eq!(counter.try_acquire_at(0, 15), Ok(()));
    
    // tick 10: sliding window [0, 10], Bucket 0 [0-4] still within window
    assert_eq!(counter.try_acquire_at(10, 15), Ok(())); // total = 15 + 15 = 30
    
    // tick 11: sliding window [1, 11], Bucket 0 [0-4] expires (start_tick=0 < 1)
    // Only Bucket 1 [10-14] within window, total = 15
    assert_eq!(counter.try_acquire_at(11, 20), Ok(())); // total = 15 + 20 = 35
    
    // Verify capacity
    assert_eq!(counter.try_acquire_at(11, 15), Ok(())); // total = 35 + 15 = 50
    assert_eq!(counter.try_acquire_at(11, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}

// Add to tests/sliding_window_counter_core.rs

#[test]
fn test_capacity_remaining() {
    let counter = SlidingWindowCounterCore::new(100, 5, 4); // window_ticks = 20
    
    // Initial state should have full capacity
    assert_eq!(counter.capacity_remaining(0).unwrap(), 100);
    
    // Use tokens in bucket 0 [0-4]
    assert_eq!(counter.try_acquire_at(2, 30), Ok(()));
    assert_eq!(counter.capacity_remaining(2).unwrap(), 70);
    
    // Use tokens in bucket 1 [5-9]
    assert_eq!(counter.try_acquire_at(7, 25), Ok(()));
    assert_eq!(counter.capacity_remaining(7).unwrap(), 45); // 100 - 30 - 25 = 45
    
    // Use tokens in bucket 2 [10-14]
    assert_eq!(counter.try_acquire_at(12, 20), Ok(()));
    assert_eq!(counter.capacity_remaining(12).unwrap(), 25); // 100 - 30 - 25 - 20 = 25
}

#[test]
fn test_current_capacity_no_bucket_update() {
    let counter = SlidingWindowCounterCore::new(100, 5, 4);
    
    // Use some tokens
    assert_eq!(counter.try_acquire_at(2, 40), Ok(()));
    
    // current_capacity should not trigger bucket updates
    assert_eq!(counter.current_capacity().unwrap(), 60);
    
    // Even after time passes, current_capacity returns same value
    assert_eq!(counter.current_capacity().unwrap(), 60);
    
    // But capacity_remaining might trigger bucket updates
    assert_eq!(counter.capacity_remaining(10).unwrap(), 60); // Bucket 0 might expire
}

#[test]
fn test_current_capacity_at() {
    let counter = SlidingWindowCounterCore::new(100, 5, 4); // window_ticks = 20
    
    // Use tokens in different buckets
    assert_eq!(counter.try_acquire_at(2, 20), Ok(()));   // bucket 0 [0-4]
    assert_eq!(counter.try_acquire_at(7, 30), Ok(()));   // bucket 1 [5-9]
    
    // Check capacity at different ticks without state updates
    assert_eq!(counter.current_capacity_at(7).unwrap(), 50);  // Both buckets within window
    assert_eq!(counter.current_capacity_at(15).unwrap(), 50); // Both buckets still within window [0, 15]
    assert_eq!(counter.current_capacity_at(25).unwrap(), 70); // Only bucket 1 within window [5, 25]
}

#[test]
fn test_capacity_remaining_expired_tick() {
    let counter = SlidingWindowCounterCore::new(100, 5, 4);
    
    // Establish a time point
    assert_eq!(counter.try_acquire_at(15, 10), Ok(()));
    
    // Going backwards should fail
    assert_eq!(counter.capacity_remaining(10), Err(SimpleRateLimitError::ExpiredTick));
    assert_eq!(counter.capacity_remaining(5), Err(SimpleRateLimitError::ExpiredTick));
}

#[test]
fn test_sliding_window_expiry2() {
    let counter = SlidingWindowCounterCore::new(100, 5, 4); // window_ticks = 20
    
    // Use tokens in bucket 0 [0-4]
    assert_eq!(counter.try_acquire_at(2, 40), Ok(()));
    assert_eq!(counter.capacity_remaining(2).unwrap(), 60);
    
    // tick 25: sliding window [5, 25], bucket 0 [0-4] should expire
    assert_eq!(counter.capacity_remaining(25).unwrap(), 100); // Bucket 0 expired, full capacity available
    
    // Use tokens in current bucket
    assert_eq!(counter.try_acquire_at(25, 30), Ok(()));
    assert_eq!(counter.capacity_remaining(25).unwrap(), 70);
}

#[test]
fn test_bucket_lazy_reset2() {
    let counter = SlidingWindowCounterCore::new(100, 5, 2); // window_ticks = 10
    
    // Use tokens in bucket 0 [0-4]
    assert_eq!(counter.try_acquire_at(2, 30), Ok(()));
    assert_eq!(counter.capacity_remaining(2).unwrap(), 70);
    
    // Use tokens in bucket 1 [5-9]
    assert_eq!(counter.try_acquire_at(7, 25), Ok(()));
    assert_eq!(counter.capacity_remaining(7).unwrap(), 45);
    
    //[10-14] is bucket 0, [15-19] is bucket 1
    // tick 20: bucket 0 new cycle [20-24], should reset bucket 0
    assert_eq!(counter.capacity_remaining(10).unwrap(), 75); // Only bucket 1 [5-9] within window [10, 20]
}

#[test]
fn test_current_vs_remaining_consistency() {
    let counter = SlidingWindowCounterCore::new(100, 5, 4);
    
    // Use some tokens
    assert_eq!(counter.try_acquire_at(2, 40), Ok(()));
    
    // Both should return same value initially
    assert_eq!(counter.current_capacity().unwrap(), 60);
    assert_eq!(counter.capacity_remaining(2).unwrap(), 60);
    
    // After capacity_remaining potentially triggers updates, check consistency
    assert_eq!(counter.capacity_remaining(25).unwrap(), 100); // Bucket might expire
    // current_capacity doesn't account for expiry, so might differ
}

#[test]
fn test_multiple_bucket_cycles2() {
    let counter = SlidingWindowCounterCore::new(80, 5, 2); // window_ticks = 10
    
    // Use tokens in bucket 0 [0-4]
    assert_eq!(counter.try_acquire_at(2, 20), Ok(()));
    assert_eq!(counter.capacity_remaining(2).unwrap(), 60);
    
    // Use tokens in bucket 1 [5-9]
    assert_eq!(counter.try_acquire_at(7, 25), Ok(()));
    assert_eq!(counter.capacity_remaining(7).unwrap(), 35); // 80 - 20 - 25 = 35
    
    // tick 12: sliding window [2, 12], bucket 0 [0-4] expires
    assert_eq!(counter.capacity_remaining(12).unwrap(), 55); // Only bucket 1 within window
    
    // tick 17: sliding window [7, 17], bucket 1 [5-9] expires
    assert_eq!(counter.capacity_remaining(17).unwrap(), 80); // Both buckets expired
}

#[test]
fn test_single_bucket_configuration() {
    let counter = SlidingWindowCounterCore::new(50, 10, 1); // window_ticks = 10
    
    // Only one bucket, all operations use bucket 0
    assert_eq!(counter.capacity_remaining(5).unwrap(), 50);
    assert_eq!(counter.try_acquire_at(5, 25), Ok(()));
    assert_eq!(counter.capacity_remaining(5).unwrap(), 25);
    
    // tick 15: bucket 0 new cycle [10-19], old bucket [0-9] expires
    assert_eq!(counter.capacity_remaining(15).unwrap(), 50); // Reset to full capacity
}

#[test]
fn test_window_boundary_conditions() {
    let counter = SlidingWindowCounterCore::new(60, 10, 3); // window_ticks = 30
    
    // Use tokens in bucket 0 [0-9]
    assert_eq!(counter.try_acquire_at(5, 20), Ok(()));
    
    // tick 35: sliding window [5, 35], bucket 0 [0-9] should expire
    assert_eq!(counter.capacity_remaining(35).unwrap(), 60); // Full capacity
    
    // tick 39: sliding window [9, 39], bucket 0 [0-9] still expires
    assert_eq!(counter.capacity_remaining(39).unwrap(), 60);
    
    // tick 40: sliding window [10, 40], bucket 0 [0-9] definitely expires
    assert_eq!(counter.capacity_remaining(40).unwrap(), 60);
}