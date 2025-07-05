// tests/rate_limiter_core_token_bucket.rs

use std::sync::Arc;

use rate_guard_core::{SimpleRateLimitError, SimpleAcquireResult};
use rate_guard_core::rate_limiter_core::RateLimiterCore;
use rate_guard_core::rate_limiters::TokenBucketCore;

/// Helper function to create a TokenBucketCore as RateLimiterCore
fn create_token_bucket_limiter(capacity: u64, refill_interval: u64, refill_amount: u64) -> Box<dyn RateLimiterCore> {
    Box::new(TokenBucketCore::new(capacity, refill_interval, refill_amount))
}

#[test]
fn test_rate_limiter_core_initial_capacity() {
    let limiter: Box<dyn RateLimiterCore> = create_token_bucket_limiter(100, 10, 5);
    
    // Token bucket starts full
    assert_eq!(limiter.capacity_remaining(0), 100);
    
    // Can immediately use all tokens
    assert_eq!(limiter.try_acquire_at(0, 100), Ok(()));
    assert_eq!(limiter.capacity_remaining(0), 0);
    
    // Bucket is now empty
    assert_eq!(limiter.try_acquire_at(0, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}

#[test]
fn test_rate_limiter_core_refill_mechanism() {
    let limiter: Box<dyn RateLimiterCore> = create_token_bucket_limiter(50, 10, 10);
    
    // Use all tokens
    assert_eq!(limiter.try_acquire_at(0, 50), Ok(()));
    assert_eq!(limiter.capacity_remaining(0), 0);
    
    // No refill before interval
    assert_eq!(limiter.capacity_remaining(5), 0);
    
    // After one refill interval
    assert_eq!(limiter.capacity_remaining(10), 10);
    
    // After multiple intervals
    assert_eq!(limiter.capacity_remaining(30), 30); // 3 intervals = 30 tokens
    
    // Capacity cap test
    assert_eq!(limiter.capacity_remaining(60), 50); // Should cap at 50
}

#[test]
fn test_rate_limiter_core_gradual_consumption() {
    let limiter: Box<dyn RateLimiterCore> = create_token_bucket_limiter(100, 10, 20);
    
    // Gradual consumption
    assert_eq!(limiter.try_acquire_at(0, 30), Ok(()));
    assert_eq!(limiter.capacity_remaining(0), 70);
    
    assert_eq!(limiter.try_acquire_at(5, 40), Ok(()));
    assert_eq!(limiter.capacity_remaining(5), 30);
    
    // Wait for refill
    assert_eq!(limiter.capacity_remaining(10), 50); // 30 + 20 = 50
    
    assert_eq!(limiter.try_acquire_at(10, 50), Ok(()));
    assert_eq!(limiter.capacity_remaining(10), 0);
}

#[test]
fn test_rate_limiter_core_time_regression() {
    let limiter: Box<dyn RateLimiterCore> = create_token_bucket_limiter(100, 10, 5);
    
    // Establish time at tick 20
    assert_eq!(limiter.try_acquire_at(20, 10), Ok(()));
    
    // Going backwards should fail
    let result: SimpleAcquireResult = limiter.try_acquire_at(15, 10);
    assert_eq!(result, Err(SimpleRateLimitError::ExpiredTick));
    
    // Same time should work
    assert_eq!(limiter.try_acquire_at(20, 10), Ok(()));
}

#[test]
fn test_rate_limiter_core_burst_capacity() {
    let limiter: Box<dyn RateLimiterCore> = create_token_bucket_limiter(200, 10, 10);
    
    // Can burst up to full capacity
    assert_eq!(limiter.try_acquire_at(0, 200), Ok(()));
    
    // After refill, can only get refill amount
    assert_eq!(limiter.capacity_remaining(10), 10);
    assert_eq!(limiter.try_acquire_at(10, 10), Ok(()));
    
    // Multiple refills accumulate
    assert_eq!(limiter.capacity_remaining(50), 40); // 4 intervals = 40 tokens
}

#[test]
fn test_rate_limiter_core_zero_operations() {
    let limiter: Box<dyn RateLimiterCore> = create_token_bucket_limiter(100, 10, 5);
    
    // Zero token requests always succeed
    assert_eq!(limiter.try_acquire_at(0, 0), Ok(()));
    assert_eq!(limiter.try_acquire_at(1000, 0), Ok(()));
    
    // Capacity unchanged
    assert_eq!(limiter.capacity_remaining(0), 100);
}

#[test]
fn test_rate_limiter_core_concurrent_refill() {
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;
    
    let bucket = Arc::new(TokenBucketCore::new(100, 10, 10));
    let limiter: Arc<dyn RateLimiterCore> = bucket;
    
    // Use all tokens
    assert_eq!(limiter.try_acquire_at(0, 100), Ok(()));
    
    let mut handles = vec![];
    
    // Multiple threads trying to acquire after refill
    for i in 1..=5 {
        let limiter_clone = limiter.clone();
        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(i * 2));
            let tick = 10; // All at same refill interval
            limiter_clone.try_acquire_at(tick, 5)
        });
        handles.push(handle);
    }
    
    let results: Vec<_> = handles.into_iter()
        .map(|h| h.join().unwrap())
        .collect();
    
    // Only 2 threads should succeed (10 tokens / 5 tokens each)
    let success_count = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(success_count, 2);
}

#[test]
fn test_rate_limiter_core_interface_consistency() {
    let limiter: Box<dyn RateLimiterCore> = create_token_bucket_limiter(75, 5, 15);
    
    // Check initial state
    assert_eq!(limiter.capacity_remaining(0), 75);
    
    // Use some capacity
    assert_eq!(limiter.try_acquire_at(0, 25), Ok(()));
    assert_eq!(limiter.capacity_remaining(0), 50);
    
    // Wait for refill
    assert_eq!(limiter.capacity_remaining(5), 65); // 50 + 15 = 65
    
    // Use exactly remaining
    assert_eq!(limiter.try_acquire_at(5, 65), Ok(()));
    assert_eq!(limiter.capacity_remaining(5), 0);
    
    // Verify empty
    assert_eq!(limiter.try_acquire_at(5, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}

#[test]
fn test_rate_limiter_core_large_refill() {
    let limiter: Box<dyn RateLimiterCore> = create_token_bucket_limiter(50, 10, 100);
    
    // Use some tokens
    assert_eq!(limiter.try_acquire_at(0, 30), Ok(()));
    assert_eq!(limiter.capacity_remaining(0), 20);
    
    // Large refill should cap at capacity
    assert_eq!(limiter.capacity_remaining(10), 50); // 20 + 100 = 120, capped at 50
    
    // Verify capped
    assert_eq!(limiter.try_acquire_at(10, 50), Ok(()));
    assert_eq!(limiter.try_acquire_at(10, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}

#[test]
fn test_rate_limiter_core_as_trait_object() {
    let limiter: Box<dyn RateLimiterCore> = Box::new(TokenBucketCore::new(100, 20, 25));
    
    // Complex scenario through trait
    assert_eq!(limiter.try_acquire_at(0, 80), Ok(()));
    assert_eq!(limiter.capacity_remaining(0), 20);
    
    // Partial refill period
    assert_eq!(limiter.capacity_remaining(15), 20); // No refill yet
    
    // One refill
    assert_eq!(limiter.capacity_remaining(20), 45); // 20 + 25 = 45
    
    // Multiple refills
    assert_eq!(limiter.capacity_remaining(60), 95); // 20 + 3*25 = 95
    
    // Fill to capacity
    assert_eq!(limiter.try_acquire_at(60, 95), Ok(()));
    assert_eq!(limiter.capacity_remaining(60), 0);
}

#[test]
fn test_rate_limiter_core_polymorphic_comparison() {
    // Compare token bucket with different configurations
    let limiters: Vec<(Box<dyn RateLimiterCore>, &str)> = vec![
        (create_token_bucket_limiter(100, 10, 10), "balanced"),
        (create_token_bucket_limiter(100, 1, 1), "high frequency"),
        (create_token_bucket_limiter(100, 100, 100), "burst refill"),
    ];
    
    for (limiter, config) in limiters.iter() {
        // All start at full capacity
        assert_eq!(
            limiter.capacity_remaining(0), 
            100, 
            "Config '{}' should start at full capacity", 
            config
        );
        
        // All can burst full capacity
        assert_eq!(
            limiter.try_acquire_at(0, 100), 
            Ok(()), 
            "Config '{}' should allow full burst", 
            config
        );
        
        // All are empty after burst
        assert_eq!(
            limiter.capacity_remaining(0), 
            0, 
            "Config '{}' should be empty after burst", 
            config
        );
    }
}

#[test]
fn test_rate_limiter_core_trait_boundary() {
    // Verify Send + Sync bounds work correctly
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Box<dyn RateLimiterCore>>();
    assert_send_sync::<TokenBucketCore>();
    
    // Can use in Arc for thread sharing
    let _shared: Arc<dyn RateLimiterCore> = Arc::new(TokenBucketCore::new(100, 10, 10));
}