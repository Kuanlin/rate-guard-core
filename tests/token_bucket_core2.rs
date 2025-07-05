use rate_guard_core::{SimpleRateLimitError};
use rate_guard_core::rate_limiters::TokenBucketCore;

#[test]
fn test_contention_failure() {
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;
    use std::sync::atomic::{AtomicBool, Ordering};
    
    let bucket = Arc::new(TokenBucketCore::new(100, 10, 5));
    let bucket_clone = bucket.clone();
    let should_stop = Arc::new(AtomicBool::new(false));
    let should_stop_clone = should_stop.clone();
    
    // Thread 1: Continuously call try_acquire_at to monopolize the lock
    let handle = thread::spawn(move || {
        while !should_stop_clone.load(Ordering::Relaxed) {
            let _ = bucket_clone.try_acquire_at(0, 1);
        }
    });
    
    // Let thread 1 run for a while to establish lock contention
    thread::sleep(Duration::from_millis(10));
    
    // Try to acquire from main thread - should encounter contention failures
    // Due to try_lock() usage, we expect some ContentionFailure errors
    let mut contention_count = 0;
    for _ in 0..1000 {
        if let Err(SimpleRateLimitError::ContentionFailure) = bucket.try_acquire_at(0, 1) {
            contention_count += 1;
        }
    }
    
    should_stop.store(true, Ordering::Relaxed);
    handle.join().unwrap();
    
    assert!(contention_count > 0, "Should observe some contention failures");
}

#[test]
fn test_refill_amount_equals_capacity() {
    let bucket = TokenBucketCore::new(100, 10, 100); // refill_amount = capacity
    
    // Use all tokens
    assert_eq!(bucket.try_acquire_at(0, 100), Ok(()));
    
    // One refill should completely fill the bucket
    // available = min(0 + 100, 100) = 100
    assert_eq!(bucket.try_acquire_at(10, 100), Ok(()));
}

#[test]
fn test_refill_amount_exceeds_capacity() {
    let bucket = TokenBucketCore::new(50, 10, 100); // refill_amount > capacity
    
    // Use all tokens
    assert_eq!(bucket.try_acquire_at(0, 50), Ok(()));
    
    // Refill should be capped at capacity, not exceed it
    // available = min(0 + 100, 50) = 50
    assert_eq!(bucket.try_acquire_at(10, 50), Ok(()));
    assert_eq!(bucket.try_acquire_at(10, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}

#[test]
fn test_high_frequency_refill() {
    let bucket = TokenBucketCore::new(100, 1, 1); // Refill 1 token every tick
    
    // Use all tokens
    assert_eq!(bucket.try_acquire_at(0, 100), Ok(()));
    
    // tick 10: refill_times = 10, total_refilled = 10
    // available = min(0 + 10, 100) = 10, consume 10: available = 0
    assert_eq!(bucket.try_acquire_at(10, 10), Ok(()));
    
    // Bucket should be empty again
    assert_eq!(bucket.try_acquire_at(10, 1), Err(SimpleRateLimitError::InsufficientCapacity));
    
    // tick 20: refill_times from last_refill_tick(10) = (20-10)/1 = 10
    // available = min(0 + 10, 100) = 10
    assert_eq!(bucket.try_acquire_at(20, 10), Ok(()));
}

#[test]
fn test_minimum_capacity_bucket() {
    let bucket = TokenBucketCore::new(1, 10, 1);
    
    // Bucket starts with 1 token
    assert_eq!(bucket.try_acquire_at(0, 1), Ok(()));
    
    // Cannot acquire more - bucket is empty
    assert_eq!(bucket.try_acquire_at(0, 1), Err(SimpleRateLimitError::InsufficientCapacity));
    
    // After refill interval, 1 token is added
    assert_eq!(bucket.try_acquire_at(10, 1), Ok(()));
}

#[test]
fn test_refill_boundary_timing() {
    let bucket = TokenBucketCore::new(100, 10, 5);
    
    // Use all tokens
    assert_eq!(bucket.try_acquire_at(5, 100), Ok(()));
    
    // tick 9: elapsed = 9 - 0 = 9, no refill yet (9 < 10)
    assert_eq!(bucket.try_acquire_at(9, 1), Err(SimpleRateLimitError::InsufficientCapacity));
    
    // tick 10: elapsed = 10 - 0 = 10, exactly one refill interval
    // available = min(0 + 5, 100) = 5, consume 5: available = 0
    assert_eq!(bucket.try_acquire_at(10, 5), Ok(()));
    
    // Bucket should be empty again
    assert_eq!(bucket.try_acquire_at(10, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}

#[test]
fn test_last_refill_tick_alignment() {
    let bucket = TokenBucketCore::new(100, 10, 5);
    
    // Use some tokens at tick 3
    assert_eq!(bucket.try_acquire_at(3, 30), Ok(())); // available = 100 - 30 = 70
    
    // tick 17: elapsed = 17 - 0 = 17, refill_times = 1, refilled = 5
    // last_refill_tick updates to 0 + 1*10 = 10
    // available = min(70 + 5, 100) = 75, consume 20: available = 55
    assert_eq!(bucket.try_acquire_at(17, 20), Ok(()));
    
    // tick 19: elapsed = 19 - 10 = 9, no refill yet (9 < 10)
    // available = 55, consume 40: available = 15
    assert_eq!(bucket.try_acquire_at(19, 40), Ok(()));
    
    // tick 20: elapsed = 20 - 10 = 10, another refill occurs
    // last_refill_tick updates to 10 + 1*10 = 20
    // available = min(15 + 5, 100) = 20, consume 20: available = 0
    assert_eq!(bucket.try_acquire_at(20, 20), Ok(()));
    
    // Bucket should be empty
    assert_eq!(bucket.try_acquire_at(20, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}

#[test]
fn test_same_tick_multiple_calls() {
    let bucket = TokenBucketCore::new(100, 10, 5);
    
    // Multiple calls at the same tick should consume from available tokens
    assert_eq!(bucket.try_acquire_at(5, 20), Ok(())); // available = 100 - 20 = 80
    assert_eq!(bucket.try_acquire_at(5, 30), Ok(())); // available = 80 - 30 = 50
    assert_eq!(bucket.try_acquire_at(5, 40), Ok(())); // available = 50 - 40 = 10
    assert_eq!(bucket.try_acquire_at(5, 10), Ok(())); // available = 10 - 10 = 0
    
    // All tokens consumed, should fail
    assert_eq!(bucket.try_acquire_at(5, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}

#[test]
fn test_refill_calculation_precision() {
    let bucket = TokenBucketCore::new(1000, 7, 13); // Irregular interval and amount
    
    // Use some tokens
    assert_eq!(bucket.try_acquire_at(0, 500), Ok(())); // available = 1000 - 500 = 500
    
    // tick 21: elapsed = 21, refill_times = 21/7 = 3, total_refilled = 3*13 = 39
    // last_refill_tick updates to 0 + 3*7 = 21
    // available = min(500 + 39, 1000) = 539, consume 400: available = 139
    assert_eq!(bucket.try_acquire_at(21, 400), Ok(()));
    
    // tick 28: elapsed = 28 - 21 = 7, refill_times = 1, total_refilled = 13
    // last_refill_tick updates to 21 + 1*7 = 28
    // available = min(139 + 13, 1000) = 152, consume 100: available = 52
    assert_eq!(bucket.try_acquire_at(28, 100), Ok(()));
    
    // Verify remaining capacity
    assert_eq!(bucket.try_acquire_at(28, 52), Ok(()));
    assert_eq!(bucket.try_acquire_at(28, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}

#[test]
fn test_capacity_cap_with_large_refill() {
    let bucket = TokenBucketCore::new(50, 5, 100); // refill_amount > capacity
    
    // Use partial tokens
    assert_eq!(bucket.try_acquire_at(0, 30), Ok(())); // available = 50 - 30 = 20
    
    // tick 5: refill 100 tokens, but capacity caps at 50
    // available = min(20 + 100, 50) = 50, consume 50: available = 0
    assert_eq!(bucket.try_acquire_at(5, 50), Ok(()));
    
    // Bucket should be empty
    assert_eq!(bucket.try_acquire_at(5, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}