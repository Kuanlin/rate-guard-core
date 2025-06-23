use rate_limiter_core::{RateLimitError};
use rate_limiter_core::rate_limiters::SlidingWindowCounterCore;

#[test]
fn test_contention_failure() {
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;
    use std::sync::atomic::{AtomicBool, Ordering};
    
    let counter = Arc::new(SlidingWindowCounterCore::new(100, 5, 4));
    let counter_clone = counter.clone();
    let should_stop = Arc::new(AtomicBool::new(false));
    let should_stop_clone = should_stop.clone();
    
    // Thread 1: Continuously call try_acquire_at to monopolize the lock
    let handle = thread::spawn(move || {
        while !should_stop_clone.load(Ordering::Relaxed) {
            let _ = counter_clone.try_acquire_at(1, 0);
        }
    });
    
    // Let thread 1 run for a while to establish lock contention
    thread::sleep(Duration::from_millis(10));
    
    // Try to acquire from main thread - should encounter contention failures
    // Due to try_lock() usage, we expect some ContentionFailure errors
    let mut contention_count = 0;
    for _ in 0..1000 {
        if let Err(RateLimitError::ContentionFailure) = counter.try_acquire_at(1, 0) {
            contention_count += 1;
        }
    }
    
    should_stop.store(true, Ordering::Relaxed);
    handle.join().unwrap();
    
    assert!(contention_count > 0, "Should observe some contention failures");
}

#[test]
fn test_bucket_index_calculation() {
    let counter = SlidingWindowCounterCore::new(100, 7, 3); // window_ticks = 21
    
    // Test different ticks map to correct bucket indices and window behavior
    // tick 3: bucket 0 [0-6], window [0, 3]
    assert_eq!(counter.try_acquire_at(10, 3), Ok(()));
    
    // tick 10: bucket 1 [7-13], window [0, 10]  
    assert_eq!(counter.try_acquire_at(10, 10), Ok(()));
    
    // tick 17: bucket 2 [14-20], window [0, 17]
    assert_eq!(counter.try_acquire_at(10, 17), Ok(()));
    
    // tick 25: bucket 0 new cycle [21-27], window [4, 25]
    // bucket 0 original [0-6] start_tick=0 < 4, expires
    // Valid: bucket 1 [7-13] (10) + bucket 2 [14-20] (10) = 20
    assert_eq!(counter.try_acquire_at(20, 25), Ok(())); // bucket 0 resets and adds 20
    
    // Current total: 0 + 10 + 10 + 20 = 40
    assert_eq!(counter.try_acquire_at(60, 25), Ok(())); // total = 100
    assert_eq!(counter.try_acquire_at(1, 25), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_sliding_window_range_calculation() {
    let counter = SlidingWindowCounterCore::new(60, 5, 3); // window_ticks = 15
    
    // tick 0: bucket 0 [0-4], window [0, 0] (due to 0.saturating_sub(15)=0)
    assert_eq!(counter.try_acquire_at(20, 0), Ok(()));
    
    // tick 10: bucket 2 [10-14], window [0, 10]
    // bucket 0 [0-4] start_tick=0 within window
    assert_eq!(counter.try_acquire_at(20, 10), Ok(())); // total = 20 + 20 = 40
    
    // tick 20: bucket 1 new cycle [20-24], window [5, 20]  
    // bucket 0 [0-4] start_tick=0 < 5, expires
    // bucket 2 [10-14] start_tick=10 within window
    assert_eq!(counter.try_acquire_at(20, 20), Ok(())); // bucket 1 resets + 20
    // total = 0 + 20 + 20 = 40
    
    assert_eq!(counter.try_acquire_at(20, 20), Ok(())); // total = 60
    assert_eq!(counter.try_acquire_at(1, 20), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_same_tick_multiple_calls() {
    let counter = SlidingWindowCounterCore::new(100, 10, 2); // window_ticks = 20
    
    // Multiple calls at same tick accumulate in same bucket
    assert_eq!(counter.try_acquire_at(15, 5), Ok(()));
    assert_eq!(counter.try_acquire_at(20, 5), Ok(()));
    assert_eq!(counter.try_acquire_at(25, 5), Ok(()));
    assert_eq!(counter.try_acquire_at(30, 5), Ok(()));
    
    // Now bucket 0 has 90 tokens
    assert_eq!(counter.try_acquire_at(10, 5), Ok(())); // total = 100
    assert_eq!(counter.try_acquire_at(1, 5), Err(RateLimitError::ExceedsCapacity));
    
    // Switch to new bucket - but still within same window
    // tick 15: window [0, 15], bucket 0 (100) + bucket 1 both within window
    // So can only use remaining capacity = 0
    assert_eq!(counter.try_acquire_at(1, 15), Err(RateLimitError::ExceedsCapacity));
    
    // tick 25: window [5, 25], bucket 0 [0-9] start_tick=0 < 5, expires
    // Only bucket 1 [10-19] within window, but bucket 1 had no tokens at tick 15
    // tick 25: bucket 1 new cycle [20-29], bucket 1 gets reset
    assert_eq!(counter.try_acquire_at(50, 25), Ok(())); // bucket 1 resets and adds 50
    
    // Same tick again
    assert_eq!(counter.try_acquire_at(50, 25), Ok(())); // total = 100
    assert_eq!(counter.try_acquire_at(1, 25), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_complex_bucket_expiry_scenarios() {
    let counter = SlidingWindowCounterCore::new(80, 5, 4); // window_ticks = 20
    
    // Set up multiple buckets
    assert_eq!(counter.try_acquire_at(15, 2), Ok(()));   // bucket 0 [0-4]
    assert_eq!(counter.try_acquire_at(20, 7), Ok(()));   // bucket 1 [5-9]  
    assert_eq!(counter.try_acquire_at(25, 12), Ok(()));  // bucket 2 [10-14]
    assert_eq!(counter.try_acquire_at(20, 17), Ok(()));  // bucket 3 [15-19]
    
    // tick 25: bucket 1 new cycle [25-29], window [5, 25]
    // bucket 0 [0-4] start_tick=0 < 5, expires
    // Valid: bucket 1 [5-9] (20) + bucket 2 [10-14] (25) + bucket 3 [15-19] (20) = 65
    // bucket 1 gets reset to 0, so actual: 0 + 25 + 20 = 45
    assert_eq!(counter.try_acquire_at(15, 25), Ok(())); // bucket 1 resets and adds 15
    // total = 15 + 25 + 20 = 60
    assert_eq!(counter.try_acquire_at(20, 25), Ok(())); // total = 80
    assert_eq!(counter.try_acquire_at(1, 25), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_bucket_expiry_boundary_conditions() {
    let counter = SlidingWindowCounterCore::new(100, 3, 3); // window_ticks = 9
    
    // bucket 0 [0-2], bucket 1 [3-5], bucket 2 [6-8]
    assert_eq!(counter.try_acquire_at(30, 1), Ok(()));   // bucket 0
    assert_eq!(counter.try_acquire_at(30, 4), Ok(()));   // bucket 1
    assert_eq!(counter.try_acquire_at(30, 7), Ok(()));   // bucket 2
    
    // tick 9: bucket 0 new cycle [9-11], window [0, 9]
    // All buckets within window, but bucket 0 gets reset
    // total = 0 + 30 + 30 = 60
    assert_eq!(counter.try_acquire_at(40, 9), Ok(())); // bucket 0 resets and adds 40
    // total = 40 + 30 + 30 = 100
    assert_eq!(counter.try_acquire_at(1, 9), Err(RateLimitError::ExceedsCapacity));
    
    // tick 12: bucket 1 new cycle [12-14], window [3, 12]
    // bucket 0 [9-11] start_tick=9 within window, tokens=40
    // bucket 1 [12-14] start_tick=12 within window, tokens=0 (reset)
    // bucket 2 [6-8] start_tick=6 within window, tokens=30
    // total = 40 + 0 + 30 = 70
    assert_eq!(counter.try_acquire_at(30, 12), Ok(())); // bucket 1 resets and adds 30
    // total = 40 + 30 + 30 = 100
    assert_eq!(counter.try_acquire_at(1, 12), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_single_bucket_configuration() {
    let counter = SlidingWindowCounterCore::new(50, 10, 1); // window_ticks = 10
    
    // Only one bucket, all operations use bucket 0
    assert_eq!(counter.try_acquire_at(25, 5), Ok(()));
    assert_eq!(counter.try_acquire_at(25, 8), Ok(())); // total = 50
    assert_eq!(counter.try_acquire_at(1, 9), Err(RateLimitError::ExceedsCapacity));
    
    // tick 15: bucket 0 new cycle [10-19], window [5, 15]
    // Old bucket 0 [0-9] start_tick=0 < 5, expires
    assert_eq!(counter.try_acquire_at(50, 15), Ok(())); // bucket 0 resets and adds 50
    
    // tick 25: bucket 0 new cycle [20-29], window [15, 25]  
    // Old bucket 0 [10-19] start_tick=10 < 15, expires
    assert_eq!(counter.try_acquire_at(50, 25), Ok(())); // bucket 0 resets and adds 50
}

#[test]
fn test_high_frequency_buckets() {
    let counter = SlidingWindowCounterCore::new(60, 1, 5); // window_ticks = 5
    
    // Each tick gets its own bucket
    assert_eq!(counter.try_acquire_at(10, 0), Ok(()));  // bucket 0 [0]
    assert_eq!(counter.try_acquire_at(10, 1), Ok(()));  // bucket 1 [1]
    assert_eq!(counter.try_acquire_at(10, 2), Ok(()));  // bucket 2 [2]
    assert_eq!(counter.try_acquire_at(10, 3), Ok(()));  // bucket 3 [3]
    assert_eq!(counter.try_acquire_at(10, 4), Ok(()));  // bucket 4 [4]
    
    // tick 5: bucket 0 new cycle [5], window [0, 5]
    // All buckets within window, but bucket 0 gets reset
    // total = 0 + 10 + 10 + 10 + 10 = 40
    assert_eq!(counter.try_acquire_at(10, 5), Ok(())); // bucket 0 resets and adds 10
    // total = 10 + 10 + 10 + 10 + 10 = 50
    assert_eq!(counter.try_acquire_at(10, 5), Ok(())); // total = 60
    assert_eq!(counter.try_acquire_at(1, 5), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_bucket_start_tick_precision() {
    let counter = SlidingWindowCounterCore::new(100, 6, 3); // bucket_ticks=6
    
    // bucket 0 [0-5], bucket 1 [6-11], bucket 2 [12-17]
    assert_eq!(counter.try_acquire_at(20, 2), Ok(()));   // bucket 0, start_tick=0
    assert_eq!(counter.try_acquire_at(25, 8), Ok(()));   // bucket 1, start_tick=6
    assert_eq!(counter.try_acquire_at(30, 14), Ok(()));  // bucket 2, start_tick=12
    
    // tick 18: window [1, 18] (because 18-18+1=1, saturating_sub(17)=1)
    // Actually should be window [1, 18]
    // bucket 0 start_tick=0 < 1, should expire
    assert_eq!(counter.try_acquire_at(25, 18), Ok(())); // bucket 0 (new cycle), start_tick=18
    
    // Verify only valid buckets are counted
    assert_eq!(counter.try_acquire_at(20, 18), Ok(())); // Check total capacity
}