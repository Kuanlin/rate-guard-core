use rate_limiter_core::{RateLimitError};
use rate_limiter_core::rate_limiters::LeakyBucketCore;

#[test]
fn test_contention_failure() {
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;
    use std::sync::atomic::{AtomicBool, Ordering};
    
    let bucket = Arc::new(LeakyBucketCore::new(100, 10, 5));
    let bucket_clone = bucket.clone();
    let should_stop = Arc::new(AtomicBool::new(false));
    let should_stop_clone = should_stop.clone();
    
    // Thread 1: Continuously call try_acquire_at to monopolize the lock
    let handle = thread::spawn(move || {
        while !should_stop_clone.load(Ordering::Relaxed) {
            let _ = bucket_clone.try_acquire_at(1, 0);
        }
    });
    
    // Let thread 1 run for a while to establish lock contention
    thread::sleep(Duration::from_millis(10));
    
    // Try to acquire from main thread - should encounter contention failures
    // Due to try_lock() usage, we expect some ContentionFailure errors
    let mut contention_count = 0;
    for _ in 0..1000 {
        if let Err(RateLimitError::ContentionFailure) = bucket.try_acquire_at(1, 0) {
            contention_count += 1;
        }
    }
    
    should_stop.store(true, Ordering::Relaxed);
    handle.join().unwrap();
    
    assert!(contention_count > 0, "Should observe some contention failures");
}

#[test]
fn test_leak_amount_equals_capacity() {
    let bucket = LeakyBucketCore::new(100, 10, 100); // leak_amount = capacity
    
    // Fill the bucket completely
    assert_eq!(bucket.try_acquire_at(100, 0), Ok(()));
    
    // After one leak interval, entire capacity should leak out
    // remaining = 100 - 100 = 0, so we can add full capacity again
    assert_eq!(bucket.try_acquire_at(100, 10), Ok(()));
}

#[test]
fn test_leak_amount_exceeds_capacity() {
    let bucket = LeakyBucketCore::new(50, 10, 100); // leak_amount > capacity
    
    // Fill the bucket completely
    assert_eq!(bucket.try_acquire_at(50, 0), Ok(()));
    
    // After leak interval, more than capacity would leak (100 > 50)
    // Due to saturating_sub, remaining becomes 0
    assert_eq!(bucket.try_acquire_at(50, 10), Ok(()));
}

#[test]
fn test_high_frequency_leak() {
    let bucket = LeakyBucketCore::new(100, 1, 1); // Leak 1 token every tick
    
    // Fill the bucket completely
    assert_eq!(bucket.try_acquire_at(100, 0), Ok(()));
    
    // Test gradual leaking - each tick leaks 1 token
    for i in 1..=10 {
        // At tick i: i tokens have leaked, so we can add 1 token
        // remaining = 100 - i, after adding 1: remaining = 100 - i + 1 = 101 - i
        assert_eq!(bucket.try_acquire_at(1, i), Ok(()));
    }

    // At tick 10: remaining = 100 - 10 + 10 = 100, bucket is full
    assert_eq!(bucket.try_acquire_at(90, 10), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_high_frequency_leak2() {
    let bucket = LeakyBucketCore::new(100, 1, 1); // Leak 1 token every tick
    
    // Fill the bucket: remaining = 100
    assert_eq!(bucket.try_acquire_at(100, 0), Ok(()));
    
    // tick 10: elapsed = 10, leaked = 10, remaining = 100 - 10 = 90
    // Try to add 10: remaining = 90 + 10 = 100 ✅
    assert_eq!(bucket.try_acquire_at(10, 10), Ok(()));
    
    // Now bucket is full, cannot add more
    assert_eq!(bucket.try_acquire_at(1, 10), Err(RateLimitError::ExceedsCapacity));
    
    // tick 20: leaked another 10, remaining = 100 - 10 = 90
    assert_eq!(bucket.try_acquire_at(10, 20), Ok(()));
}

#[test]
fn test_minimum_capacity_bucket() {
    let bucket = LeakyBucketCore::new(1, 10, 1);
    
    // Use the single token of capacity
    assert_eq!(bucket.try_acquire_at(1, 0), Ok(()));
    
    // Cannot add more - bucket is at capacity
    assert_eq!(bucket.try_acquire_at(1, 0), Err(RateLimitError::ExceedsCapacity));
    
    // After leak interval, 1 token leaks out, making space available
    assert_eq!(bucket.try_acquire_at(1, 10), Ok(()));
}

#[test]
fn test_leak_boundary_timing() {
    let bucket = LeakyBucketCore::new(100, 10, 5);
    
    // Add tokens at tick 5
    assert_eq!(bucket.try_acquire_at(50, 5), Ok(()));
    
    // tick 9: elapsed = 9 - 0 = 9, no leak yet (9 < 10)
    assert_eq!(bucket.try_acquire_at(50, 9), Ok(()));
    
    // tick 10: elapsed = 10 - 0 = 10, exactly one leak interval
    // remaining = 100 - 5 = 95, add 5: remaining = 100
    assert_eq!(bucket.try_acquire_at(5, 10), Ok(()));
    assert_eq!(bucket.try_acquire_at(1, 10), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_last_leak_tick_alignment() {
    let bucket = LeakyBucketCore::new(100, 10, 5);
    
    // Add tokens at tick 3
    assert_eq!(bucket.try_acquire_at(30, 3), Ok(()));
    
    // tick 17: elapsed = 17 - 0 = 17, leak_times = 1, leaked = 5
    // last_leak_tick updates to 0 + 1*10 = 10
    assert_eq!(bucket.try_acquire_at(20, 17), Ok(())); // remaining = 30 - 5 + 20 = 45
    
    // tick 19: elapsed = 19 - 10 = 9, no leak yet (9 < 10)
    assert_eq!(bucket.try_acquire_at(50, 19), Ok(())); // remaining = 45 + 50 = 95
    
    // tick 20: elapsed = 20 - 10 = 10, another leak occurs
    // remaining = 95 - 5 = 90, add 10: remaining = 100
    assert_eq!(bucket.try_acquire_at(10, 20), Ok(()));
}

#[test]
fn test_same_tick_multiple_calls() {
    let bucket = LeakyBucketCore::new(100, 10, 5);
    
    // Multiple calls at the same tick should accumulate
    assert_eq!(bucket.try_acquire_at(20, 5), Ok(()));
    assert_eq!(bucket.try_acquire_at(30, 5), Ok(()));
    assert_eq!(bucket.try_acquire_at(40, 5), Ok(()));
    assert_eq!(bucket.try_acquire_at(10, 5), Ok(()));
    
    // Total: 20 + 30 + 40 + 10 = 100, bucket should be at capacity
    assert_eq!(bucket.try_acquire_at(1, 5), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_leak_calculation_precision() {
    let bucket = LeakyBucketCore::new(1000, 7, 13); // Irregular interval and amount
    
    // Fill the bucket
    assert_eq!(bucket.try_acquire_at(1000, 0), Ok(()));
    
    // tick 21: elapsed = 21, leak_times = 21/7 = 3, total_leaked = 3*13 = 39
    // remaining = 1000 - 39 = 961, add 39: remaining = 1000
    assert_eq!(bucket.try_acquire_at(39, 21), Ok(()));
    
    // tick 28: elapsed = 28 - 21 = 7, leak_times = 1, total_leaked = 13
    // last_leak_tick updates to 21 + 1*7 = 28
    // remaining = 1000 - 13 = 987, add 13: remaining = 1000
    assert_eq!(bucket.try_acquire_at(13, 28), Ok(()));
}