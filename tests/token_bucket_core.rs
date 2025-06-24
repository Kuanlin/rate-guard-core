use rate_limiter_core::{Uint, RateLimitError};
use rate_limiter_core::rate_limiters::TokenBucketCore;

#[test]
fn test_new_token_bucket() {
    let _ = TokenBucketCore::new(100, 10, 5);
    // Constructor should succeed without panic
}

#[test]
#[should_panic(expected = "capacity must be greater than 0")]
fn test_new_with_zero_capacity() {
    TokenBucketCore::new(0, 10, 5);
}

#[test]
#[should_panic(expected = "refill_interval must be greater than 0")]
fn test_new_with_zero_refill_interval() {
    TokenBucketCore::new(100, 0, 5);
}

#[test]
#[should_panic(expected = "refill_amount must be greater than 0")]
fn test_new_with_zero_refill_amount() {
    TokenBucketCore::new(100, 10, 0);
}

#[test]
fn test_acquire_zero_tokens() {
    let bucket = TokenBucketCore::new(100, 10, 5);
    // Zero token requests should always succeed regardless of bucket state
    assert_eq!(bucket.try_acquire_at(0, 0), Ok(()));
    assert_eq!(bucket.try_acquire_at(0, 100), Ok(()));
}

#[test]
fn test_initial_full_bucket() {
    let bucket = TokenBucketCore::new(100, 10, 5);
    
    // Token bucket starts full, can immediately use all tokens
    assert_eq!(bucket.try_acquire_at(100, 0), Ok(()));
    
    // Now bucket is empty
    assert_eq!(bucket.try_acquire_at(1, 0), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_basic_acquire() {
    let bucket = TokenBucketCore::new(100, 10, 5);
    
    // Consume tokens gradually (bucket starts with 100 tokens)
    assert_eq!(bucket.try_acquire_at(30, 0), Ok(())); // available = 100 - 30 = 70
    assert_eq!(bucket.try_acquire_at(20, 0), Ok(())); // available = 70 - 20 = 50
    assert_eq!(bucket.try_acquire_at(50, 0), Ok(())); // available = 50 - 50 = 0
    
    // Now bucket should be empty
    assert_eq!(bucket.try_acquire_at(1, 0), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_refill_mechanism() {
    let bucket = TokenBucketCore::new(100, 10, 5); // Refill 5 tokens every 10 ticks
    
    // Use all tokens
    assert_eq!(bucket.try_acquire_at(100, 0), Ok(()));
    
    // Within refill interval, bucket should still be empty
    assert_eq!(bucket.try_acquire_at(1, 5), Err(RateLimitError::ExceedsCapacity));
    
    // At refill interval, 5 tokens should be added
    // available = 0 + 5 = 5, consume 5: available = 0
    assert_eq!(bucket.try_acquire_at(5, 10), Ok(()));
    
    // Bucket is empty again
    assert_eq!(bucket.try_acquire_at(1, 10), Err(RateLimitError::ExceedsCapacity));
    
    // Another refill interval adds 5 more tokens
    assert_eq!(bucket.try_acquire_at(5, 20), Ok(()));
}

#[test]
fn test_multiple_refill_intervals() {
    let bucket = TokenBucketCore::new(100, 10, 5); // Refill 5 tokens every 10 ticks
    
    // Use all tokens
    assert_eq!(bucket.try_acquire_at(100, 0), Ok(()));
    
    // Skip multiple refill intervals: 30 ticks = 3 intervals = 15 tokens refilled
    // available = 0 + 15 = 15, consume 15: available = 0
    assert_eq!(bucket.try_acquire_at(15, 30), Ok(()));
    
    // Bucket is empty again
    assert_eq!(bucket.try_acquire_at(1, 30), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_capacity_cap() {
    let bucket = TokenBucketCore::new(100, 10, 20); // Refill 20 tokens, but capacity is only 100
    
    // Use some tokens
    assert_eq!(bucket.try_acquire_at(30, 0), Ok(())); // available = 100 - 30 = 70
    
    // Wait for refill, but shouldn't exceed capacity
    // available = min(70 + 20, 100) = 90, consume 90: available = 0
    assert_eq!(bucket.try_acquire_at(90, 10), Ok(()));
    
    // Another refill, should only reach capacity limit
    // available = min(0 + 20, 100) = 20, consume 20: available = 0
    assert_eq!(bucket.try_acquire_at(20, 20), Ok(()));
    
    // Cannot exceed refill amount
    assert_eq!(bucket.try_acquire_at(1, 20), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_time_alignment() {
    let bucket = TokenBucketCore::new(100, 10, 5);
    
    // Use all tokens at tick 5
    assert_eq!(bucket.try_acquire_at(100, 5), Ok(()));
    
    // At tick 12: elapsed from last_refill_tick(0) = 12, refill_times = 1, refilled = 5
    // last_refill_tick updates to 0 + 1*10 = 10
    // available = 0 + 5 = 5, consume 5: available = 0
    assert_eq!(bucket.try_acquire_at(5, 12), Ok(()));
    
    // At tick 22: elapsed from last_refill_tick(10) = 12, refill_times = 1, refilled = 5
    // last_refill_tick updates to 10 + 1*10 = 20
    // available = 0 + 5 = 5, consume 5: available = 0
    assert_eq!(bucket.try_acquire_at(5, 22), Ok(()));
    
    // Bucket should be empty
    assert_eq!(bucket.try_acquire_at(1, 22), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_expired_tick() {
    let bucket = TokenBucketCore::new(100, 10, 5);
    
    // Normal operation establishes last_refill_tick = 0
    assert_eq!(bucket.try_acquire_at(10, 20), Ok(()));
    
    // Time going backwards should fail
    assert_eq!(bucket.try_acquire_at(10, 15), Err(RateLimitError::ExpiredTick));
    assert_eq!(bucket.try_acquire_at(10, 10), Err(RateLimitError::ExpiredTick));
    
    // Same time should be allowed
    assert_eq!(bucket.try_acquire_at(10, 20), Ok(()));
}

#[test]
fn test_large_time_gap() {
    let bucket = TokenBucketCore::new(100, 10, 5);
    
    // Use all tokens
    assert_eq!(bucket.try_acquire_at(100, 0), Ok(()));
    
    // Jump to much later time - should refill to capacity
    // After 1000 ticks: refill_times = 1000/10 = 100, total_refilled = 100*5 = 500
    // available = min(0 + 500, 100) = 100
    assert_eq!(bucket.try_acquire_at(100, 1000), Ok(()));
}

#[test]
fn test_partial_consumption() {
    let bucket = TokenBucketCore::new(100, 10, 10);
    
    // Consume partial tokens
    assert_eq!(bucket.try_acquire_at(60, 0), Ok(())); // available = 100 - 60 = 40
    
    // Refill once: available = min(40 + 10, 100) = 50, consume 30: available = 20
    assert_eq!(bucket.try_acquire_at(30, 10), Ok(()));
    
    // Refill again: available = min(20 + 10, 100) = 30, consume 20: available = 10
    assert_eq!(bucket.try_acquire_at(20, 20), Ok(()));
    
    // Check remaining tokens
    assert_eq!(bucket.try_acquire_at(10, 20), Ok(()));
    assert_eq!(bucket.try_acquire_at(1, 20), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_saturating_operations() {
    let bucket = TokenBucketCore::new(Uint::MAX, 1, Uint::MAX);
    
    // Test that large values don't overflow, bucket starts full
    assert_eq!(bucket.try_acquire_at(Uint::MAX - 1, 0), Ok(()));
    
    // Large time jumps should refill to capacity without overflow
    assert_eq!(bucket.try_acquire_at(Uint::MAX, Uint::MAX), Ok(()));
}