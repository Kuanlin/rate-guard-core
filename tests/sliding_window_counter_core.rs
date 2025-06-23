use rate_limiter_core::{ RateLimitError};
use rate_limiter_core::rate_limiters::SlidingWindowCounterCore;

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
    assert_eq!(counter.try_acquire_at(0, 100), Ok(()));
}

#[test]
fn test_basic_sliding_window() {
    // bucket_ticks=5, bucket_count=4, window_ticks=20
    // Sliding window size: 20 ticks
    let counter = SlidingWindowCounterCore::new(100, 5, 4);
    
    // tick 0: sliding window [0, 0], Bucket 0 [0-4]
    assert_eq!(counter.try_acquire_at(25, 0), Ok(()));
    
    // tick 5: sliding window [0, 5], Bucket 1 [5-9]
    assert_eq!(counter.try_acquire_at(25, 5), Ok(()));
    
    // tick 10: sliding window [0, 10], Bucket 2 [10-14]
    assert_eq!(counter.try_acquire_at(25, 10), Ok(()));
    
    // tick 15: sliding window [0, 15], Bucket 3 [15-19]
    assert_eq!(counter.try_acquire_at(25, 15), Ok(()));
    
    // Total now 100, should reach capacity
    assert_eq!(counter.try_acquire_at(1, 15), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_sliding_window_expiry() {
    let counter = SlidingWindowCounterCore::new(100, 5, 4); // window_ticks = 20
    
    // tick 0: Bucket 0 [0-4]
    assert_eq!(counter.try_acquire_at(50, 0), Ok(()));
    
    // tick 25: sliding window [5, 25], Bucket 0 [0-4] expires (start_tick=0 < 5)
    // Only current bucket counts toward the limit
    assert_eq!(counter.try_acquire_at(100, 25), Ok(()));
    
    // Total now 100, should reach capacity
    assert_eq!(counter.try_acquire_at(1, 25), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_bucket_rotation() {
    let counter = SlidingWindowCounterCore::new(100, 5, 4); // window_ticks = 20
    
    // tick 2: Bucket 0 [0-4]
    assert_eq!(counter.try_acquire_at(30, 2), Ok(()));
    
    // tick 7: Bucket 1 [5-9]
    assert_eq!(counter.try_acquire_at(30, 7), Ok(()));
    
    // tick 22: sliding window [2, 22], Bucket 0 [0-4] expires (start_tick=0 < 2)
    // But Bucket 1 [5-9] still within window (start_tick=5 >= 2)
    // So total = 30, adding 70 gives total = 100
    assert_eq!(counter.try_acquire_at(70, 22), Ok(()));
    assert_eq!(counter.try_acquire_at(1, 22), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_bucket_lazy_reset() {
    let counter = SlidingWindowCounterCore::new(100, 5, 4);
    
    // tick 2: Bucket 0 [0-4]
    assert_eq!(counter.try_acquire_at(30, 2), Ok(()));
    
    // tick 7: Bucket 1 [5-9]
    assert_eq!(counter.try_acquire_at(40, 7), Ok(()));
    
    // tick 42: sliding window [22, 42], all old buckets expire
    // Buckets are lazily reset when accessed
    assert_eq!(counter.try_acquire_at(100, 42), Ok(()));
}

#[test]
fn test_multiple_bucket_cycles() {
    let counter = SlidingWindowCounterCore::new(80, 5, 2); // window_ticks = 10
    
    // tick 1: Bucket 0 [0-4]
    assert_eq!(counter.try_acquire_at(20, 1), Ok(()));
    
    // tick 6: Bucket 1 [5-9]  
    assert_eq!(counter.try_acquire_at(20, 6), Ok(()));
    
    // tick 12: sliding window [2, 12], Bucket 0 [0-4] expires, Bucket 1 [5-9] still valid
    assert_eq!(counter.try_acquire_at(20, 12), Ok(())); // total = 20 + 20 = 40
    
    // tick 17: sliding window [7, 17], Bucket 1 [5-9] expires
    assert_eq!(counter.try_acquire_at(40, 17), Ok(())); // total = 20 + 40 = 60
    
    assert_eq!(counter.try_acquire_at(20, 17), Ok(())); // total = 60 + 20 = 80
    assert_eq!(counter.try_acquire_at(1, 17), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_expired_tick() {
    let counter = SlidingWindowCounterCore::new(100, 5, 4);
    
    // Perform first operation
    assert_eq!(counter.try_acquire_at(10, 15), Ok(()));
    
    // Time going backwards should fail
    assert_eq!(counter.try_acquire_at(10, 10), Err(RateLimitError::ExpiredTick));
    assert_eq!(counter.try_acquire_at(10, 5), Err(RateLimitError::ExpiredTick));
    
    // Same time should be allowed
    assert_eq!(counter.try_acquire_at(10, 15), Ok(()));
    
    // Move time forward
    assert_eq!(counter.try_acquire_at(10, 25), Ok(()));
    
    // Going back to previous time should fail
    assert_eq!(counter.try_acquire_at(10, 20), Err(RateLimitError::ExpiredTick));
}

#[test]
fn test_sliding_window_boundaries() {
    let counter = SlidingWindowCounterCore::new(60, 10, 3); // window_ticks = 30
    
    // tick 5: Bucket 0 [0-9]
    assert_eq!(counter.try_acquire_at(20, 5), Ok(()));
    
    // tick 15: Bucket 1 [10-19]
    assert_eq!(counter.try_acquire_at(20, 15), Ok(()));
    
    // tick 25: Bucket 2 [20-29]
    assert_eq!(counter.try_acquire_at(20, 25), Ok(()));
    
    // tick 35: sliding window [5, 35], Bucket 0 [0-9] expires (start_tick=0 < 5)
    assert_eq!(counter.try_acquire_at(20, 35), Ok(())); // total = 20 + 20 + 20 = 60
    assert_eq!(counter.try_acquire_at(1, 35), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_window_size_calculation() {
    let counter = SlidingWindowCounterCore::new(100, 5, 3); // window_ticks = 15
    
    // tick 10: sliding window [0, 10] (due to saturating_sub)
    assert_eq!(counter.try_acquire_at(50, 10), Ok(()));
    
    // tick 20: sliding window [5, 20], previous bucket [0-4] expires
    assert_eq!(counter.try_acquire_at(50, 20), Ok(()));
    
    // Check if capacity is reached
    assert_eq!(counter.try_acquire_at(1, 20), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_large_time_gap() {
    let counter = SlidingWindowCounterCore::new(100, 5, 4);
    
    // Add some tokens
    assert_eq!(counter.try_acquire_at(50, 5), Ok(()));
    
    // Jump over long time, old data should all expire
    assert_eq!(counter.try_acquire_at(100, 1000), Ok(()));
}

#[test]
fn test_single_bucket() {
    let counter = SlidingWindowCounterCore::new(50, 10, 1);
    
    // All operations cycle through the same bucket slot
    assert_eq!(counter.try_acquire_at(25, 5), Ok(()));
    assert_eq!(counter.try_acquire_at(25, 9), Ok(()));
    
    // Next cycle, bucket resets
    assert_eq!(counter.try_acquire_at(50, 15), Ok(()));
}

#[test]
fn test_saturating_operations() {
    // Use smaller values to avoid overflow
    let counter = SlidingWindowCounterCore::new(1000, 100, 2);
    
    // Test won't overflow
    assert_eq!(counter.try_acquire_at(500, 0), Ok(()));
    assert_eq!(counter.try_acquire_at(500, 150), Ok(()));
}

#[test]
fn test_window_edge_cases() {
    let counter = SlidingWindowCounterCore::new(50, 5, 2); // window_ticks = 10
    
    // tick 0: sliding window [0, 0], Bucket 0 [0-4]
    assert_eq!(counter.try_acquire_at(15, 0), Ok(()));
    
    // tick 10: sliding window [0, 10], Bucket 0 [0-4] still within window
    assert_eq!(counter.try_acquire_at(15, 10), Ok(())); // total = 15 + 15 = 30
    
    // tick 11: sliding window [1, 11], Bucket 0 [0-4] expires (start_tick=0 < 1)
    // Only Bucket 1 [10-14] within window, total = 15
    assert_eq!(counter.try_acquire_at(20, 11), Ok(())); // total = 15 + 20 = 35
    
    // Verify capacity
    assert_eq!(counter.try_acquire_at(15, 11), Ok(())); // total = 35 + 15 = 50
    assert_eq!(counter.try_acquire_at(1, 11), Err(RateLimitError::ExceedsCapacity));
}