// tests/rate_limiter_core_fixed_window.rs

use std::sync::Arc;

use rate_guard_core::{SimpleRateLimitError, SimpleAcquireResult};
use rate_guard_core::rate_limiter_core::RateLimiterCore;
use rate_guard_core::rate_limiters::FixedWindowCounterCore;

/// Helper function to create a FixedWindowCounterCore as RateLimiterCore
fn create_fixed_window_limiter(capacity: u64, window_ticks: u64) -> Box<dyn RateLimiterCore> {
    Box::new(FixedWindowCounterCore::new(capacity, window_ticks))
}

#[test]
fn test_rate_limiter_core_window_basics() {
    let limiter: Box<dyn RateLimiterCore> = create_fixed_window_limiter(100, 10);
    
    // Initial window has full capacity
    assert_eq!(limiter.capacity_remaining(0), 100);
    
    // Use tokens within window [0-9]
    assert_eq!(limiter.try_acquire_at(5, 30), Ok(()));
    assert_eq!(limiter.capacity_remaining(5), 70);
    
    assert_eq!(limiter.try_acquire_at(9, 70), Ok(()));
    assert_eq!(limiter.capacity_remaining(9), 0);
    
    // Window is full
    assert_eq!(limiter.try_acquire_at(9, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}

#[test]
fn test_rate_limiter_core_window_reset() {
    let limiter: Box<dyn RateLimiterCore> = create_fixed_window_limiter(50, 10);
    
    // Fill first window [0-9]
    assert_eq!(limiter.try_acquire_at(5, 50), Ok(()));
    assert_eq!(limiter.capacity_remaining(8), 0);
    
    // New window [10-19] resets counter
    assert_eq!(limiter.capacity_remaining(10), 50);
    assert_eq!(limiter.try_acquire_at(12, 30), Ok(()));
    assert_eq!(limiter.capacity_remaining(15), 20);
    
    // Another window [20-29]
    assert_eq!(limiter.capacity_remaining(20), 50);
}

#[test]
fn test_rate_limiter_core_window_boundaries() {
    let limiter: Box<dyn RateLimiterCore> = create_fixed_window_limiter(100, 5);
    
    // Test exact boundary transitions
    // Window 0: [0-4]
    assert_eq!(limiter.try_acquire_at(4, 60), Ok(())); // Last tick of window
    assert_eq!(limiter.capacity_remaining(4), 40);
    
    // Window 1: [5-9] - exactly at boundary
    assert_eq!(limiter.capacity_remaining(5), 100); // Reset at new window
    assert_eq!(limiter.try_acquire_at(5, 100), Ok(()));
    
    // Window 2: [10-14]
    assert_eq!(limiter.try_acquire_at(10, 50), Ok(()));
    assert_eq!(limiter.capacity_remaining(14), 50); // Last tick
    
    // Window 3: [15-19]
    assert_eq!(limiter.capacity_remaining(15), 100); // Reset again
}

#[test]
fn test_rate_limiter_core_skip_windows() {
    let limiter: Box<dyn RateLimiterCore> = create_fixed_window_limiter(80, 10);
    
    // Use tokens in window 0 [0-9]
    assert_eq!(limiter.try_acquire_at(3, 40), Ok(()));
    
    // Jump multiple windows to tick 35 (window 3: [30-39])
    assert_eq!(limiter.capacity_remaining(35), 80); // Fresh window
    assert_eq!(limiter.try_acquire_at(37, 80), Ok(()));
    assert_eq!(limiter.capacity_remaining(38), 0);
}

#[test]
fn test_rate_limiter_core_time_consistency() {
    let limiter: Box<dyn RateLimiterCore> = create_fixed_window_limiter(100, 10);
    
    // Establish time in window 1 [10-19]
    assert_eq!(limiter.try_acquire_at(15, 20), Ok(()));
    
    // Going back before window start should fail
    let result: SimpleAcquireResult = limiter.try_acquire_at(9, 10);
    assert_eq!(result, Err(SimpleRateLimitError::ExpiredTick));
    
    // Within current window is ok
    assert_eq!(limiter.try_acquire_at(18, 30), Ok(()));
    assert_eq!(limiter.capacity_remaining(19), 50);
}

#[test]
fn test_rate_limiter_core_zero_operations() {
    let limiter: Box<dyn RateLimiterCore> = create_fixed_window_limiter(100, 20);
    
    // Zero token requests always succeed
    assert_eq!(limiter.try_acquire_at(0, 0), Ok(()));
    assert_eq!(limiter.try_acquire_at(50, 0), Ok(()));
    assert_eq!(limiter.try_acquire_at(100, 0), Ok(()));
    
    // Capacity unchanged
    assert_eq!(limiter.capacity_remaining(100), 100);
}

#[test]
fn test_rate_limiter_core_burst_at_boundaries() {
    let limiter: Box<dyn RateLimiterCore> = create_fixed_window_limiter(100, 10);
    
    // End of window 0 [0-9]
    assert_eq!(limiter.try_acquire_at(9, 100), Ok(()));
    assert_eq!(limiter.capacity_remaining(9), 0);
    
    // Immediate burst at window 1 [10-19]
    assert_eq!(limiter.try_acquire_at(10, 100), Ok(()));
    assert_eq!(limiter.capacity_remaining(10), 0);
    
    // Can burst again at next window
    assert_eq!(limiter.try_acquire_at(20, 100), Ok(()));
}

#[test]
fn test_rate_limiter_core_concurrent_window_access() {
    use std::sync::Arc;
    use std::thread;
    
    let counter = Arc::new(FixedWindowCounterCore::new(100, 10));
    let limiter: Arc<dyn RateLimiterCore> = counter;
    
    let mut handles = vec![];
    
    // Multiple threads accessing same window
    for i in 0..10 {
        let limiter_clone = limiter.clone();
        let handle = thread::spawn(move || {
            // All threads try to acquire in window 0
            limiter_clone.try_acquire_at(i % 10, 15)
        });
        handles.push(handle);
    }
    
    let results: Vec<_> = handles.into_iter()
        .map(|h| h.join().unwrap())
        .collect();
    
    // Count successes - should total <= 100
    let success_count = results.iter().filter(|r| r.is_ok()).count();
    assert!(success_count <= 6); // 100 / 15 = 6.67, so max 6 can succeed
}

#[test]
fn test_rate_limiter_core_window_size_variations() {
    // Test different window sizes
    let configs = vec![
        (100, 1),   // 1-tick windows
        (100, 10),  // 10-tick windows
        (100, 100), // 100-tick windows
    ];
    
    for (capacity, window_ticks) in configs {
        let limiter: Box<dyn RateLimiterCore> = create_fixed_window_limiter(capacity, window_ticks);
        
        // Each should start with full capacity
        assert_eq!(limiter.capacity_remaining(0), capacity);
        
        // Use half capacity
        assert_eq!(limiter.try_acquire_at(0, capacity / 2), Ok(()));
        assert_eq!(limiter.capacity_remaining(0), capacity / 2);
        
        // Next window resets
        assert_eq!(limiter.capacity_remaining(window_ticks), capacity);
    }
}

#[test]
fn test_rate_limiter_core_interface_consistency() {
    let limiter: Box<dyn RateLimiterCore> = create_fixed_window_limiter(75, 15);
    
    // Window 0 [0-14]
    assert_eq!(limiter.capacity_remaining(5), 75);
    assert_eq!(limiter.try_acquire_at(7, 25), Ok(()));
    assert_eq!(limiter.capacity_remaining(10), 50);
    
    // Use exact remaining
    assert_eq!(limiter.try_acquire_at(12, 50), Ok(()));
    assert_eq!(limiter.capacity_remaining(14), 0);
    
    // Verify exhausted
    assert_eq!(limiter.try_acquire_at(14, 1), Err(SimpleRateLimitError::InsufficientCapacity));
    
    // Window 1 [15-29] resets
    assert_eq!(limiter.capacity_remaining(15), 75);
}

#[test]
fn test_rate_limiter_core_as_trait_object() {
    let limiter: Box<dyn RateLimiterCore> = Box::new(FixedWindowCounterCore::new(50, 5));
    
    // Complex usage pattern
    // Window 0 [0-4]
    assert_eq!(limiter.try_acquire_at(2, 20), Ok(()));
    assert_eq!(limiter.try_acquire_at(4, 20), Ok(()));
    assert_eq!(limiter.capacity_remaining(4), 10);
    
    // Window 1 [5-9]
    assert_eq!(limiter.try_acquire_at(6, 50), Ok(()));
    assert_eq!(limiter.capacity_remaining(8), 0);
    
    // Window 2 [10-14]
    assert_eq!(limiter.try_acquire_at(11, 25), Ok(()));
    assert_eq!(limiter.try_acquire_at(13, 25), Ok(()));
    assert_eq!(limiter.capacity_remaining(14), 0);
    
    // Verify window independence
    assert_eq!(limiter.try_acquire_at(14, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}

#[test]
fn test_rate_limiter_core_polymorphic_usage() {
    let limiters: Vec<(Box<dyn RateLimiterCore>, &str)> = vec![
        (create_fixed_window_limiter(100, 1), "high frequency"),
        (create_fixed_window_limiter(100, 60), "minute window"),
        (create_fixed_window_limiter(100, 3600), "hour window"),
    ];
    
    for (limiter, window_type) in limiters.iter() {
        // All start with full capacity
        assert_eq!(
            limiter.capacity_remaining(0), 
            100,
            "Window type '{}' should start at full capacity",
            window_type
        );
        
        // All can use full capacity
        assert_eq!(
            limiter.try_acquire_at(0, 100),
            Ok(()),
            "Window type '{}' should allow full capacity usage",
            window_type
        );
        
        // All are exhausted in current window
        assert_eq!(
            limiter.capacity_remaining(0),
            0,
            "Window type '{}' should be exhausted",
            window_type
        );
    }
}

#[test]
fn test_rate_limiter_core_trait_bounds() {
    // Verify Send + Sync bounds
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Box<dyn RateLimiterCore>>();
    assert_send_sync::<FixedWindowCounterCore>();
    
    // Can share across threads
    let _shared: Arc<dyn RateLimiterCore> = Arc::new(FixedWindowCounterCore::new(100, 10));
}

#[test]
fn test_rate_limiter_core_window_alignment() {
    let limiter: Box<dyn RateLimiterCore> = create_fixed_window_limiter(100, 7);
    
    // Test non-aligned window boundaries
    // Window calculation: tick 13 -> window 1 (13/7=1), start_tick = 7
    assert_eq!(limiter.try_acquire_at(13, 50), Ok(()));
    assert_eq!(limiter.capacity_remaining(13), 50);
    
    // tick 20 -> window 2 (20/7=2), start_tick = 14
    assert_eq!(limiter.capacity_remaining(20), 100); // New window
    
    // tick 28 -> window 4 (28/7=4), start_tick = 28
    assert_eq!(limiter.try_acquire_at(28, 30), Ok(()));
    assert_eq!(limiter.capacity_remaining(34), 70); // Same window
}