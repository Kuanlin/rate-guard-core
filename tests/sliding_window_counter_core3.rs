// tests/rate_limiter_core_sliding_window.rs (修正版)

use std::sync::Arc;

use rate_guard_core::{RateLimitError, AcquireResult};
use rate_guard_core::rate_limiter_core::RateLimiterCore;
use rate_guard_core::rate_limiters::SlidingWindowCounterCore;

/// Helper function to create a SlidingWindowCounterCore as RateLimiterCore
fn create_sliding_window_limiter(capacity: u64, bucket_ticks: u64, bucket_count: u64) -> Box<dyn RateLimiterCore> {
    Box::new(SlidingWindowCounterCore::new(capacity, bucket_ticks, bucket_count))
}
#[test]
fn test_rate_limiter_core_sliding_basics() {
    // 4 buckets of 5 ticks each = 20 tick sliding window
    let limiter: Box<dyn RateLimiterCore> = create_sliding_window_limiter(100, 5, 4);
    
    // Initial capacity
    assert_eq!(limiter.capacity_remaining(0), 100);
    
    // Use tokens across different buckets
    assert_eq!(limiter.try_acquire_at(25, 0), Ok(()));   // bucket 0 [0-4]
    assert_eq!(limiter.try_acquire_at(25, 5), Ok(()));   // bucket 1 [5-9]
    assert_eq!(limiter.try_acquire_at(25, 10), Ok(()));  // bucket 2 [10-14]
    assert_eq!(limiter.try_acquire_at(25, 15), Ok(()));  // bucket 3 [15-19]
    
    // Window is full
    assert_eq!(limiter.capacity_remaining(15), 0);
    assert_eq!(limiter.try_acquire_at(1, 15), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_rate_limiter_core_sliding_expiry() {
    // 3 buckets of 10 ticks = 30 tick window
    let limiter: Box<dyn RateLimiterCore> = create_sliding_window_limiter(100, 10, 3);
    
    // Fill bucket 0 [0-9]
    assert_eq!(limiter.try_acquire_at(40, 5), Ok(()));
    assert_eq!(limiter.capacity_remaining(5), 60);
    
    // Fill bucket 1 [10-19]
    assert_eq!(limiter.try_acquire_at(30, 15), Ok(()));
    assert_eq!(limiter.capacity_remaining(15), 30);
    
    // At tick 35, sliding window is [5, 35] (35.saturating_sub(30) = 5)
    // Bucket 0 [0-9] is still in window (start_tick=0 >= 5? NO)
    // Actually, the window_start_tick calculation seems to be tick.saturating_sub(window_ticks)
    // So at tick 35: window_start = 35 - 30 = 5
    // Bucket 0: start_tick=0 < 5, so NOT in window
    assert_eq!(limiter.capacity_remaining(35), 70); // Only bucket 1 counts
    
    // Can use freed capacity
    assert_eq!(limiter.try_acquire_at(70, 35), Ok(()));
    assert_eq!(limiter.capacity_remaining(35), 0);
}

#[test]
fn test_rate_limiter_core_bucket_rotation() {
    // 2 buckets of 5 ticks = 10 tick sliding window
    let limiter: Box<dyn RateLimiterCore> = create_sliding_window_limiter(80, 5, 2);
    
    // Bucket 0 [0-4]
    assert_eq!(limiter.try_acquire_at(30, 2), Ok(()));
    
    // Bucket 1 [5-9]  
    assert_eq!(limiter.try_acquire_at(40, 7), Ok(()));
    assert_eq!(limiter.capacity_remaining(9), 10); // 80 - 30 - 40 = 10
    
    // At tick 10: bucket 0 resets, gets 10 tokens
    assert_eq!(limiter.try_acquire_at(10, 10), Ok(()));
    assert_eq!(limiter.capacity_remaining(10), 30); // 80 - 40 - 10 = 30, not 0!
    
    // At tick 15: bucket 1 resets (loses 40 tokens)
    // Only bucket 0 (10 tokens) remains in window
    assert_eq!(limiter.capacity_remaining(15), 70); // 80 - 10 = 70
}

#[test]
fn test_rate_limiter_core_precise_window() {
    // 4 buckets of 3 ticks = 12 tick window
    let limiter: Box<dyn RateLimiterCore> = create_sliding_window_limiter(60, 3, 4);
    
    // Distribute tokens across buckets
    assert_eq!(limiter.try_acquire_at(15, 1), Ok(()));   // bucket 0 [0-2]
    assert_eq!(limiter.try_acquire_at(15, 4), Ok(()));   // bucket 1 [3-5]
    assert_eq!(limiter.try_acquire_at(15, 7), Ok(()));   // bucket 2 [6-8]
    assert_eq!(limiter.try_acquire_at(15, 10), Ok(()));  // bucket 3 [9-11]
    

    // At tick 12:
    // window_start = 12 - 12 = 0
    // Current bucket_index = (12/3) % 4 = 0
    // Current bucket_start_tick = (12/3) * 3 = 12
    // Since bucket 0's start_tick was 0, but now should be 12, bucket 0 gets reset!
    // After the reset, buckets in window [0, 12]:
    // Bucket 0: start_tick=12, 0 tokens (just reset) → in window
    // Bucket 1: start_tick=3, 15 tokens → in window
    // Bucket 2: start_tick=6, 15 tokens → in window
    // Bucket 3: start_tick=9, 15 tokens → in window
    // Total used = 0 + 15 + 15 + 15 = 45, remaining = 60 - 45 = 15
    assert_eq!(limiter.capacity_remaining(12), 15);
    
    // At tick 15:
    // window_start = 15 - 12 = 3
    // Current bucket_index = (15/3) % 4 = 1
    // Current bucket_start_tick = (15/3) * 3 = 15
    // Since bucket 1's start_tick was 3, but now should be 15, bucket 1 gets reset!
    // After the reset, buckets in window [3, 15]:
    // Bucket 0: start_tick=12 ≥ 3 → in window (0 tokens, was reset at tick 12)
    // Bucket 1: start_tick=15 ≥ 3 → in window (0 tokens, just reset at tick 15)
    // Bucket 2: start_tick=6 ≥ 3 → in window (15 tokens)
    // Bucket 3: start_tick=9 ≥ 3 → in window (15 tokens)
    // Total used = 0 + 0 + 15 + 15 = 30, remaining = 60 - 30 = 30
    assert_eq!(limiter.capacity_remaining(15), 30);
}
#[test]
fn test_rate_limiter_core_time_consistency() {
    let limiter: Box<dyn RateLimiterCore> = create_sliding_window_limiter(100, 5, 4);
    
    // Establish time at tick 20
    assert_eq!(limiter.try_acquire_at(20, 20), Ok(()));
    
    // Going backwards should fail
    let result: AcquireResult = limiter.try_acquire_at(10, 15);
    assert_eq!(result, Err(RateLimitError::ExpiredTick));
    
    // Current time ok
    assert_eq!(limiter.try_acquire_at(10, 20), Ok(()));
}

#[test]
fn test_rate_limiter_core_zero_operations() {
    let limiter: Box<dyn RateLimiterCore> = create_sliding_window_limiter(100, 10, 5);
    
    // Zero token requests always succeed
    assert_eq!(limiter.try_acquire_at(0, 0), Ok(()));
    assert_eq!(limiter.try_acquire_at(0, 25), Ok(()));
    assert_eq!(limiter.try_acquire_at(0, 100), Ok(()));
    
    // Capacity unchanged
    assert_eq!(limiter.capacity_remaining(100), 100);
}

#[test]
fn test_rate_limiter_core_single_bucket_config() {
    // Single bucket behaves like fixed window
    let limiter: Box<dyn RateLimiterCore> = create_sliding_window_limiter(50, 10, 1);
    
    // Fill the bucket
    assert_eq!(limiter.try_acquire_at(50, 5), Ok(()));
    assert_eq!(limiter.capacity_remaining(8), 0);
    
    // New cycle at tick 10, old bucket expires
    assert_eq!(limiter.capacity_remaining(15), 50);
    assert_eq!(limiter.try_acquire_at(30, 18), Ok(()));
    
    // Another cycle at tick 20
    assert_eq!(limiter.capacity_remaining(25), 50);
}

#[test]
fn test_rate_limiter_core_concurrent_buckets() {
    use std::sync::Arc;
    use std::thread;
    
    let counter = Arc::new(SlidingWindowCounterCore::new(100, 5, 4));
    let limiter: Arc<dyn RateLimiterCore> = counter;
    
    let mut handles = vec![];
    
    // Threads targeting different buckets
    for i in 0..4 {
        let limiter_clone = limiter.clone();
        let handle = thread::spawn(move || {
            let tick = i * 5 + 2; // Spread across buckets
            limiter_clone.try_acquire_at(30, tick)
        });
        handles.push(handle);
    }
    
    let results: Vec<_> = handles.into_iter()
        .map(|h| h.join().unwrap())
        .collect();
    
    // Only 3 should succeed (100 capacity / 30 tokens each)
    let success_count = results.iter().filter(|r| r.is_ok()).count();
    assert!(success_count <= 3);
}

#[test]
fn test_rate_limiter_core_bucket_granularity() {
    // Test different bucket configurations
    let configs = vec![
        (100, 1, 10),  // 10 buckets of 1 tick each
        (100, 5, 4),   // 4 buckets of 5 ticks each
        (100, 10, 2),  // 2 buckets of 10 ticks each
    ];
    
    for (capacity, bucket_ticks, bucket_count) in configs {
        let limiter: Box<dyn RateLimiterCore> = 
            create_sliding_window_limiter(capacity, bucket_ticks, bucket_count);
        
        let window_size = bucket_ticks * bucket_count;
        
        // All start with full capacity
        assert_eq!(limiter.capacity_remaining(0), capacity);
        
        // Use half capacity
        assert_eq!(limiter.try_acquire_at(capacity / 2, 0), Ok(()));
        
        // Jump past window - should have full capacity again
        assert_eq!(limiter.capacity_remaining(window_size + 1), capacity);
    }
}

#[test]
fn test_rate_limiter_core_interface_consistency() {
    let limiter: Box<dyn RateLimiterCore> = create_sliding_window_limiter(90, 6, 3);
    
    // Use tokens across window
    assert_eq!(limiter.try_acquire_at(20, 2), Ok(()));   // bucket 0
    assert_eq!(limiter.try_acquire_at(30, 8), Ok(()));   // bucket 1
    assert_eq!(limiter.try_acquire_at(25, 14), Ok(()));  // bucket 2
    
    assert_eq!(limiter.capacity_remaining(14), 15); // 90 - 75 = 15
    
    // Use exact remaining
    assert_eq!(limiter.try_acquire_at(15, 14), Ok(()));
    assert_eq!(limiter.capacity_remaining(14), 0);
    
    // Verify exhausted
    assert_eq!(limiter.try_acquire_at(1, 14), Err(RateLimitError::ExceedsCapacity));
}

#[test]
fn test_rate_limiter_core_as_trait_object() {
    let limiter: Box<dyn RateLimiterCore> = Box::new(SlidingWindowCounterCore::new(100, 4, 5));
    
    // Window size = 20 ticks
    // Fill buckets gradually
    for i in 0..5 {
        let tick = i * 4 + 1;
        assert_eq!(limiter.try_acquire_at(20, tick), Ok(()));
    }
    
    // At tick 19: window_start = 19 - 20 = -1 (saturates to 0)
    // All buckets in window
    assert_eq!(limiter.capacity_remaining(19), 0);
    
    // At tick 21: window_start = 21 - 20 = 1
    // All buckets still in window (all start_ticks >= 1)
    // Wait, bucket 0 starts at tick 0 (for tick 1), so start_tick=0 < 1
    // So bucket 0 is NOT in window
    assert_eq!(limiter.capacity_remaining(21), 20);
    
    // At tick 24: window_start = 24 - 20 = 4
    // - Bucket 0 [0-3]: 20@1
    // - Bucket 1 [4-7]: 20@5
    // - Bucket 2 [8-11]: 20@9
    // - Bucket 3 [12-15]: 20@13
    // - Bucket 4 [16-19]: 20@17
    // - Bucket 0 [20-23]: 0
    // - Bucket 1 [24-27]: 0
    // Total = 20 + 20 + 20 = 60, remaining = 100 - 60 = 40
    // The rest are in window
    assert_eq!(limiter.capacity_remaining(24), 40);
}

#[test]
fn test_rate_limiter_core_polymorphic_comparison() {
    let limiters: Vec<(Box<dyn RateLimiterCore>, &str)> = vec![
        (create_sliding_window_limiter(100, 1, 60), "fine-grained"),
        (create_sliding_window_limiter(100, 10, 6), "balanced"),
        (create_sliding_window_limiter(100, 60, 1), "coarse"),
    ];
    
    for (limiter, config) in limiters.iter() {
        assert_eq!(
            limiter.capacity_remaining(0), 
            100,
            "Config '{}' should start at full capacity",
            config
        );
        
        // Use capacity
        assert_eq!(
            limiter.try_acquire_at(50, 0),
            Ok(()),
            "Config '{}' should allow partial usage",
            config
        );
        
        assert_eq!(
            limiter.capacity_remaining(0),
            50,
            "Config '{}' should track usage correctly",
            config
        );
    }
}

#[test]
fn test_rate_limiter_core_trait_bounds() {
    // Verify Send + Sync
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Box<dyn RateLimiterCore>>();
    assert_send_sync::<SlidingWindowCounterCore>();
    
    // Shareable across threads
    let _shared: Arc<dyn RateLimiterCore> = 
        Arc::new(SlidingWindowCounterCore::new(100, 5, 4));
}

#[test]
fn test_rate_limiter_core_edge_cases() {
    let limiter: Box<dyn RateLimiterCore> = create_sliding_window_limiter(100, 7, 3);
    
    // Window = 21 ticks
    // Spread across buckets
    assert_eq!(limiter.try_acquire_at(30, 3), Ok(()));   // bucket 0 [0-6]
    assert_eq!(limiter.try_acquire_at(30, 10), Ok(()));  // bucket 1 [7-13]
    assert_eq!(limiter.try_acquire_at(30, 17), Ok(()));  // bucket 2 [14-20]
    
    // At tick 22: window_start = 22 - 21 = 1
    // - Bucket 0 [0-6]: start_tick=0 < 1, NOT in window
    // - Bucket 1 [7-13]: start_tick=7 >= 1 ✓
    // - Bucket 2 [14-20]: start_tick=14 >= 1 ✓
    // Total = 60, remaining = 100 - 60 = 40
    assert_eq!(limiter.capacity_remaining(22), 40);
    
    // At tick 28: window_start = 28 - 21 = 7
    // - Bucket 0 [0-6]: 30@3
    // - Bucket 1 [7-13]: 30@10
    // - Bucket 2 [14-20]: 30@17
    // - Bucket 0 [21-27]: 0
    // - Bucket 1 [28-34]: 0
    // Total = 30, remaining = 70
    assert_eq!(limiter.capacity_remaining(28), 70);
}