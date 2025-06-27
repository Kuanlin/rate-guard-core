use rate_guard_core::{Uint, RateLimitError};
use rate_guard_core::rate_limiters::LeakyBucketCore;

#[test]
fn test_new_leaky_bucket() {
    let _ = LeakyBucketCore::new(100, 10, 5);
    // Constructor should succeed without panic
}

#[test]
#[should_panic(expected = "capacity must be greater than 0")]
fn test_new_with_zero_capacity() {
    LeakyBucketCore::new(0, 10, 5);
}

#[test]
#[should_panic(expected = "leak_interval must be greater than 0")]
fn test_new_with_zero_leak_interval() {
    LeakyBucketCore::new(100, 0, 5);
}

#[test]
#[should_panic(expected = "leak_amount must be greater than 0")]
fn test_new_with_zero_leak_amount() {
    LeakyBucketCore::new(100, 10, 0);
}

#[test]
fn test_acquire_zero_tokens() {
    let bucket = LeakyBucketCore::new(100, 10, 5);
    // Zero token requests should always succeed regardless of bucket state
    assert_eq!(bucket.try_acquire_at(0, 0), Ok(()));
    assert_eq!(bucket.try_acquire_at(0, 100), Ok(()));
}

#[test]
fn test_basic_acquire() {
    let bucket = LeakyBucketCore::new(100, 10, 5);
    
    // First acquisition should succeed (bucket starts empty)
    assert_eq!(bucket.try_acquire_at(10, 0), Ok(()));
    
    // Continue acquiring within capacity
    assert_eq!(bucket.try_acquire_at(20, 0), Ok(()));
    assert_eq!(bucket.try_acquire_at(30, 0), Ok(()));
}

#[test]
fn test_capacity_exceeded() {
    let bucket = LeakyBucketCore::new(100, 10, 5);
    
    // Fill the bucket to capacity
    assert_eq!(bucket.try_acquire_at(100, 0), Ok(()));
    
    // Additional requests should fail
    assert_eq!(bucket.try_acquire_at(1, 0), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_leak_mechanism() {
    let bucket = LeakyBucketCore::new(100, 10, 5); // Leaks 5 tokens every 10 ticks
    
    // Fill the bucket completely
    assert_eq!(bucket.try_acquire_at(100, 0), Ok(()));
    
    // Within leak interval, bucket should still be full
    assert_eq!(bucket.try_acquire_at(1, 5), Err(RateLimitError::ExceedsCapacity));
    
    // At leak interval boundary, 5 tokens should have leaked out
    // remaining = 100 - 5 = 95, so we can add 5 more
    assert_eq!(bucket.try_acquire_at(5, 10), Ok(()));
    
    // After another leak interval, 5 more tokens leak out
    // remaining = 100 - 5 = 95, so we can add 5 more
    assert_eq!(bucket.try_acquire_at(5, 20), Ok(()));
}

#[test]
fn test_multiple_leak_intervals() {
    let bucket = LeakyBucketCore::new(100, 10, 5); // Leaks 5 tokens every 10 ticks
    
    // Fill the bucket completely
    assert_eq!(bucket.try_acquire_at(100, 0), Ok(()));
    
    // Skip multiple leak intervals: 30 ticks = 3 intervals = 15 tokens leaked
    // remaining = 100 - 15 = 85, so we can add 15 tokens
    assert_eq!(bucket.try_acquire_at(15, 30), Ok(()));
    
    // Now the bucket is full again (85 + 15 = 100)
    assert_eq!(bucket.try_acquire_at(1, 30), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_time_alignment() {
    let bucket = LeakyBucketCore::new(100, 10, 5);
    
    // Add tokens at tick 5
    assert_eq!(bucket.try_acquire_at(50, 5), Ok(()));
    
    // At tick 12: elapsed from last_leak_tick(0) = 12, leak_times = 1, leaked = 5
    // remaining = 50 - 5 = 45, then add 40: 45 + 40 = 85
    assert_eq!(bucket.try_acquire_at(40, 12), Ok(()));
    
    // At tick 22: elapsed from last_leak_tick(10) = 12, leak_times = 1, leaked = 5  
    // remaining = 85 - 5 = 80, then add 10: 80 + 10 = 90
    assert_eq!(bucket.try_acquire_at(10, 22), Ok(()));
    
    // Now remaining = 90, can add 10 more to reach capacity
    assert_eq!(bucket.try_acquire_at(10, 22), Ok(()));
    
    // Now bucket should be full
    assert_eq!(bucket.try_acquire_at(1, 22), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_expired_tick() {
    let bucket = LeakyBucketCore::new(100, 10, 5);
    
    // Normal operation establishes last_leak_tick = 0
    assert_eq!(bucket.try_acquire_at(10, 20), Ok(()));
    
    // Time going backwards should fail
    assert_eq!(bucket.try_acquire_at(10, 15), Err(RateLimitError::ExpiredTick));
    assert_eq!(bucket.try_acquire_at(10, 10), Err(RateLimitError::ExpiredTick));
    
    // Same time should be allowed
    assert_eq!(bucket.try_acquire_at(10, 20), Ok(()));
}

#[test]
fn test_large_time_gap() {
    let bucket = LeakyBucketCore::new(100, 10, 5);
    
    // Fill the bucket
    assert_eq!(bucket.try_acquire_at(100, 0), Ok(()));
    
    // Jump to much later time - bucket should be completely leaked out
    // After 1000 ticks: leak_times = 1000/10 = 100, total_leaked = 100*5 = 500
    // Since 500 > 100, remaining becomes 0 due to saturating_sub
    assert_eq!(bucket.try_acquire_at(100, 1000), Ok(()));
}

#[test]
fn test_saturating_operations() {
    let bucket = LeakyBucketCore::new(Uint::MAX, 1, Uint::MAX);
    
    // Test that large values don't overflow
    assert_eq!(bucket.try_acquire_at(Uint::MAX, 0), Ok(()));
    
    // Large time jumps should work without overflow
    assert_eq!(bucket.try_acquire_at(1, Uint::MAX), Ok(()));
}

// Add to tests/leaky_bucket_core.rs

#[test]
fn test_capacity_remaining() {
    let bucket = LeakyBucketCore::new(100, 10, 5);
    
    // Initial state should be empty
    assert_eq!(bucket.capacity_remaining(0).unwrap(), 0);
    
    // Add some tokens
    assert_eq!(bucket.try_acquire_at(30, 0), Ok(()));
    assert_eq!(bucket.capacity_remaining(0).unwrap(), 30);
    
    // Add more tokens
    assert_eq!(bucket.try_acquire_at(20, 0), Ok(()));
    assert_eq!(bucket.capacity_remaining(0).unwrap(), 50);
    
    // Time passes, should leak
    assert_eq!(bucket.capacity_remaining(10).unwrap(), 45); // 50 - 5 = 45
}

#[test]
fn test_current_capacity_no_leak() {
    let bucket = LeakyBucketCore::new(100, 10, 5);
    
    // Add some tokens
    assert_eq!(bucket.try_acquire_at(40, 0), Ok(()));
    
    // current_capacity should not trigger leak
    assert_eq!(bucket.current_capacity().unwrap(), 40);
    
    // Even after time passes, current_capacity returns same value
    assert_eq!(bucket.current_capacity().unwrap(), 40);
    
    // But capacity_remaining will trigger leak
    assert_eq!(bucket.capacity_remaining(10).unwrap(), 35); // 40 - 5 = 35
}

#[test]
fn test_capacity_remaining_expired_tick() {
    let bucket = LeakyBucketCore::new(100, 10, 5);
    
    // Establish a time point
    assert_eq!(bucket.try_acquire_at(10, 20), Ok(()));
    
    // Going backwards in time should fail
    assert_eq!(bucket.capacity_remaining(15), Err(RateLimitError::ExpiredTick));
    assert_eq!(bucket.capacity_remaining(10), Err(RateLimitError::ExpiredTick));
}

#[test]
fn test_capacity_remaining_leak_behavior() {
    let bucket = LeakyBucketCore::new(100, 10, 5);
    
    // Fill the bucket
    assert_eq!(bucket.try_acquire_at(100, 0), Ok(()));
    assert_eq!(bucket.capacity_remaining(0).unwrap(), 100);
    
    // After one leak interval
    assert_eq!(bucket.capacity_remaining(10).unwrap(), 95); // 100 - 5 = 95
    
    // After multiple leak intervals
    assert_eq!(bucket.capacity_remaining(30).unwrap(), 85); // 100 - 3*5 = 85
    
    // Should not go below zero
    assert_eq!(bucket.capacity_remaining(1000).unwrap(), 0);
}

#[test]
fn test_current_vs_remaining_consistency() {
    let bucket = LeakyBucketCore::new(100, 10, 5);
    
    // Add some tokens
    assert_eq!(bucket.try_acquire_at(40, 0), Ok(()));
    
    // Both should return same value at same tick
    assert_eq!(bucket.current_capacity().unwrap(), 40);
    assert_eq!(bucket.capacity_remaining(0).unwrap(), 40);
    
    // After capacity_remaining triggers leak, current_capacity should reflect the update
    assert_eq!(bucket.capacity_remaining(10).unwrap(), 35);
    assert_eq!(bucket.current_capacity().unwrap(), 35);
}

#[test]
fn test_leak_boundary_timing() {
    let bucket = LeakyBucketCore::new(100, 10, 5);
    
    // Add tokens at tick 5
    assert_eq!(bucket.try_acquire_at(50, 5), Ok(()));
    
    // tick 9: elapsed = 9 - 0 = 9, no leak yet (9 < 10)
    assert_eq!(bucket.capacity_remaining(9).unwrap(), 50);
    
    // tick 10: elapsed = 10 - 0 = 10, exactly one leak interval
    // remaining = 50 - 5 = 45
    assert_eq!(bucket.capacity_remaining(10).unwrap(), 45);
    
    // tick 19: elapsed = 19 - 10 = 9, no leak yet (9 < 10)
    assert_eq!(bucket.capacity_remaining(19).unwrap(), 45);
    
    // tick 20: elapsed = 20 - 10 = 10, another leak occurs
    // remaining = 45 - 5 = 40
    assert_eq!(bucket.capacity_remaining(20).unwrap(), 40);
}

#[test]
fn test_empty_bucket_leak() {
    let bucket = LeakyBucketCore::new(100, 10, 5);
    
    // Bucket starts empty
    assert_eq!(bucket.capacity_remaining(0).unwrap(), 0);
    
    // Leak from empty bucket should remain empty
    assert_eq!(bucket.capacity_remaining(10).unwrap(), 0);
    assert_eq!(bucket.capacity_remaining(100).unwrap(), 0);
}

#[test]
fn test_large_leak_amount() {
    let bucket = LeakyBucketCore::new(50, 10, 100); // leak_amount > capacity
    
    // Fill the bucket
    assert_eq!(bucket.try_acquire_at(50, 0), Ok(()));
    assert_eq!(bucket.capacity_remaining(0).unwrap(), 50);
    
    // After leak interval, more than capacity would leak (100 > 50)
    // Due to saturating_sub, remaining becomes 0
    assert_eq!(bucket.capacity_remaining(10).unwrap(), 0);
}