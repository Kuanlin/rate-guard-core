// tests/rate_limiter_core_leaky_bucket.rs

use rate_guard_core::{SimpleRateLimitError, SimpleAcquireResult};
use rate_guard_core::rate_limiter_core::RateLimiterCore;
use rate_guard_core::rate_limiters::LeakyBucketCore;

/// Helper function to create a LeakyBucketCore as RateLimiterCore
fn create_leaky_bucket_limiter(capacity: u64, leak_interval: u64, leak_amount: u64) -> Box<dyn RateLimiterCore> {
    Box::new(LeakyBucketCore::new(capacity, leak_interval, leak_amount))
}

#[test]
fn test_rate_limiter_core_basic_acquire() {
    let limiter: Box<dyn RateLimiterCore> = create_leaky_bucket_limiter(100, 10, 5);
    
    // Test basic acquisition through trait
    assert_eq!(limiter.try_acquire_at(0, 30), Ok(()));
    assert_eq!(limiter.try_acquire_at(0, 50), Ok(()));
    assert_eq!(limiter.try_acquire_at(0, 20), Ok(()));
    
    // Should be at capacity now
    assert_eq!(limiter.try_acquire_at(0, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}

#[test]
fn test_rate_limiter_core_capacity_remaining() {
    let limiter: Box<dyn RateLimiterCore> = create_leaky_bucket_limiter(100, 10, 5);
    
    // Initial capacity should be 0 (leaky bucket starts empty)
    assert_eq!(limiter.capacity_remaining(0), 0);
    
    // Add some tokens
    assert_eq!(limiter.try_acquire_at(0, 40), Ok(()));
    assert_eq!(limiter.capacity_remaining(0), 40);
    
    // After leak interval, capacity should decrease
    assert_eq!(limiter.capacity_remaining(10), 35); // 40 - 5 = 35
}

#[test]
fn test_rate_limiter_core_zero_tokens() {
    let limiter: Box<dyn RateLimiterCore> = create_leaky_bucket_limiter(100, 10, 5);
    
    // Zero token requests should always succeed
    assert_eq!(limiter.try_acquire_at(0, 0), Ok(()));
    assert_eq!(limiter.try_acquire_at(100, 0), Ok(()));
}

#[test]
fn test_rate_limiter_core_leak_behavior() {
    let limiter: Box<dyn RateLimiterCore> = create_leaky_bucket_limiter(50, 10, 10);
    
    // Fill the bucket
    assert_eq!(limiter.try_acquire_at(0, 50), Ok(()));
    assert_eq!(limiter.capacity_remaining(0), 50);
    
    // After one leak interval, 10 tokens should leak out
    assert_eq!(limiter.capacity_remaining(10), 40);
    
    // Multiple leak intervals
    assert_eq!(limiter.capacity_remaining(30), 20); // 50 - 3*10 = 20
    
    // Can acquire more tokens after leak
    assert_eq!(limiter.try_acquire_at(30, 30), Ok(()));
    assert_eq!(limiter.capacity_remaining(30), 50); // 20 + 30 = 50
}

#[test]
fn test_rate_limiter_core_time_consistency() {
    let limiter: Box<dyn RateLimiterCore> = create_leaky_bucket_limiter(100, 10, 5);
    
    // Establish a time point
    assert_eq!(limiter.try_acquire_at(20, 20), Ok(()));
    
    // Going backwards in time should fail
    let result: SimpleAcquireResult = limiter.try_acquire_at(15, 10);
    assert_eq!(result, Err(SimpleRateLimitError::ExpiredTick));
}

#[test]
fn test_rate_limiter_core_complete_drain() {
    let limiter: Box<dyn RateLimiterCore> = create_leaky_bucket_limiter(30, 10, 15);
    
    // Fill the bucket
    assert_eq!(limiter.try_acquire_at(0, 30), Ok(()));
    
    // After two leak intervals, bucket should be empty
    // First interval: 30 - 15 = 15
    // Second interval: 15 - 15 = 0
    assert_eq!(limiter.capacity_remaining(20), 0);
    
    // Can fill it again
    assert_eq!(limiter.try_acquire_at(20, 30), Ok(()));
}

#[test]
fn test_rate_limiter_core_concurrent_behavior() {
    use std::sync::Arc;
    use std::thread;
    
    // Create limiter wrapped in Arc for sharing
    let bucket = Arc::new(LeakyBucketCore::new(100, 10, 5));
    let limiter: Arc<dyn RateLimiterCore> = bucket;
    
    let mut handles = vec![];
    
    // Spawn multiple threads trying to acquire tokens
    for i in 0..5 {
        let limiter_clone = limiter.clone();
        let handle = thread::spawn(move || {
            let tick = i * 5;
            limiter_clone.try_acquire_at(tick, 10)
        });
        handles.push(handle);
    }
    
    // Collect results
    let results: Vec<_> = handles.into_iter()
        .map(|h| h.join().unwrap())
        .collect();
    
    // At least some should succeed
    let success_count = results.iter().filter(|r| r.is_ok()).count();
    assert!(success_count > 0);
}

#[test]
fn test_rate_limiter_core_interface_consistency() {
    let limiter: Box<dyn RateLimiterCore> = create_leaky_bucket_limiter(50, 5, 5);
    
    // Test that capacity_remaining and try_acquire_at are consistent
    assert_eq!(limiter.capacity_remaining(0), 0); // Starts empty
    
    // Acquire some tokens
    assert_eq!(limiter.try_acquire_at(0, 30), Ok(()));
    assert_eq!(limiter.capacity_remaining(0), 30);
    
    // Try to acquire exactly the remaining capacity
    assert_eq!(limiter.try_acquire_at(0, 20), Ok(()));
    assert_eq!(limiter.capacity_remaining(0), 50);
    
    // Should be at capacity
    assert_eq!(limiter.try_acquire_at(0, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}

#[test]
fn test_rate_limiter_core_large_time_jump() {
    let limiter: Box<dyn RateLimiterCore> = create_leaky_bucket_limiter(100, 10, 5);
    
    // Fill the bucket
    assert_eq!(limiter.try_acquire_at(0, 100), Ok(()));
    
    // Large time jump should leak everything
    assert_eq!(limiter.capacity_remaining(1000), 0);
    
    // Can fill again
    assert_eq!(limiter.try_acquire_at(1000, 100), Ok(()));
}

#[test]
fn test_rate_limiter_core_as_trait_object() {
    // Test that we can use LeakyBucketCore as a trait object
    let limiter: Box<dyn RateLimiterCore> = Box::new(LeakyBucketCore::new(75, 15, 15));
    
    // Perform operations through trait
    assert_eq!(limiter.try_acquire_at(10, 50), Ok(()));
    assert_eq!(limiter.capacity_remaining(10), 50);
    
    // Wait for leak
    assert_eq!(limiter.capacity_remaining(25), 35); // 50 - 15 = 35
    
    // Fill to capacity
    assert_eq!(limiter.try_acquire_at(25, 40), Ok(()));
    assert_eq!(limiter.capacity_remaining(25), 75); // 35 + 40 = 75
    
    // At capacity
    assert_eq!(limiter.try_acquire_at(25, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}

#[test]
fn test_rate_limiter_core_polymorphic_usage() {
    // Test using the trait with different configurations
    let limiters: Vec<Box<dyn RateLimiterCore>> = vec![
        create_leaky_bucket_limiter(100, 10, 10),
        create_leaky_bucket_limiter(50, 5, 5),
        create_leaky_bucket_limiter(200, 20, 40),
    ];
    
    for (i, limiter) in limiters.iter().enumerate() {
        let tick = i as u64 * 100;
        let result = limiter.try_acquire_at(tick, 10);
        assert_eq!(result, Ok(()), "Limiter {} should allow acquiring 10 tokens", i);
        
        let remaining = limiter.capacity_remaining(tick);
        assert_eq!(remaining, 10, "Limiter {} should have 10 tokens remaining", i);
    }
}