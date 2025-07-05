use rate_guard_core::{SimpleRateLimitError};
use rate_guard_core::rate_limiters::ApproximateSlidingWindowCore;

#[test]
fn test_contention_failure() {
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;
    use std::sync::atomic::{AtomicBool, Ordering};
    
    let counter = Arc::new(ApproximateSlidingWindowCore::new(100, 10));
    let counter_clone = counter.clone();
    let should_stop = Arc::new(AtomicBool::new(false));
    let should_stop_clone = should_stop.clone();
    
    // Thread 1: Continuously call try_acquire_at to monopolize the lock
    let handle = thread::spawn(move || {
        while !should_stop_clone.load(Ordering::Relaxed) {
            let _ = counter_clone.try_acquire_at(0, 1);
        }
    });
    
    // Let thread 1 run for a while to establish lock contention
    thread::sleep(Duration::from_millis(10));
    
    // Try to acquire from main thread - should encounter contention failures
    // Due to try_lock() usage, we expect some ContentionFailure errors
    let mut contention_count = 0;
    for _ in 0..1000 {
        if let Err(SimpleRateLimitError::ContentionFailure) = counter.try_acquire_at(0, 1) {
            contention_count += 1;
        }
    }
    
    should_stop.store(true, Ordering::Relaxed);
    handle.join().unwrap();
    
    assert!(contention_count > 0, "Should observe some contention failures");
}

#[test]
fn test_weighted_contribution_calculation() {
    let counter = ApproximateSlidingWindowCore::new(100, 10);
    
    // Establish state of two windows
    assert_eq!(counter.try_acquire_at(5, 30), Ok(()));   // Window 0 [0-9]
    assert_eq!(counter.try_acquire_at(15, 40), Ok(()));  // Window 1 [10-19]
    
    // tick 18: sliding window [9, 18]
    // Window 0 [0-9] overlaps with sliding window [9, 18] at [9, 9] = 1 tick
    // Window 1 [10-19] overlaps with sliding window [9, 18] at [10, 18] = 9 ticks
    // Weighted calculation: 30 * 1 + 40 * 9 = 30 + 360 = 390
    // Capacity contribution: 100 * 10 = 1000
    // New request contribution: 20 * 10 = 200
    // 390 + 200 = 590 <= 1000, should succeed
    assert_eq!(counter.try_acquire_at(18, 20), Ok(()));
    
    // Try another request that would exceed
    // Current total contribution: 390 + 200 = 590, remaining: 1000 - 590 = 410
    // New request 50 * 10 = 500 > 410, should fail
    assert_eq!(counter.try_acquire_at(18, 50), Err(SimpleRateLimitError::InsufficientCapacity));
}

#[test]
fn test_no_overlap_scenario() {
    let counter = ApproximateSlidingWindowCore::new(50, 10);
    
    // Window 0: add tokens
    assert_eq!(counter.try_acquire_at(5, 30), Ok(()));   // Window 0 [0-9]
    
    // tick 25: sliding window [16, 25], Window 1 [20-29]
    // Window 0 [0-9] has no overlap with sliding window [16, 25]
    // Should be able to use full capacity
    assert_eq!(counter.try_acquire_at(25, 50), Ok(()));
}

#[test]
fn test_full_overlap_scenario() {
    let counter = ApproximateSlidingWindowCore::new(60, 15);
    
    // Window 0: fill
    assert_eq!(counter.try_acquire_at(10, 40), Ok(()));  // Window 0 [0-14]
    
    // tick 12: sliding window [0, 12] (because 12-(15-1)=-2, saturating_sub gives 0)
    // Window 0 [0-14] overlaps with sliding window [0, 12] at [0, 12] = 13 ticks
    // Weighted contribution: 40 * 13 = 520
    // Capacity contribution: 60 * 15 = 900
    // New request: 20 * 15 = 300
    // 520 + 300 = 820 <= 900, should succeed
    assert_eq!(counter.try_acquire_at(12, 20), Ok(()));
}

#[test]
fn test_window_switching_consistency() {
    let counter = ApproximateSlidingWindowCore::new(80, 12);
    
    // Add to Window 0
    assert_eq!(counter.try_acquire_at(8, 25), Ok(()));   // Window 0 [0-11]
    
    // Switch to Window 1
    assert_eq!(counter.try_acquire_at(15, 30), Ok(()));  // Window 1 [12-23]
    
    // Switch back to Window 0 (new cycle)
    assert_eq!(counter.try_acquire_at(28, 20), Ok(()));  // Window 0 [24-35]
    
    // Verify old window data is handled correctly
    assert_eq!(counter.try_acquire_at(30, 5), Ok(()));
}

#[test]
fn test_same_tick_multiple_calls() {
    let counter = ApproximateSlidingWindowCore::new(100, 20);
    
    // Multiple calls at same tick should accumulate in current window
    assert_eq!(counter.try_acquire_at(10, 20), Ok(()));
    assert_eq!(counter.try_acquire_at(10, 25), Ok(()));
    assert_eq!(counter.try_acquire_at(10, 30), Ok(()));
    assert_eq!(counter.try_acquire_at(10, 25), Ok(()));
    
    // Now current window has 100 tokens, should reach capacity
    assert_eq!(counter.try_acquire_at(10, 1), Err(SimpleRateLimitError::InsufficientCapacity));
    
    // Switch to new window
    assert_eq!(counter.try_acquire_at(25, 30), Ok(()));
}

#[test]
fn test_window_starts_alignment() {
    let counter = ApproximateSlidingWindowCore::new(120, 8);
    
    // Window 0 [0-7]
    assert_eq!(counter.try_acquire_at(3, 30), Ok(()));
    
    // Window 1 [8-15]
    assert_eq!(counter.try_acquire_at(12, 40), Ok(()));
    
    // Window 0 new cycle [16-23]
    assert_eq!(counter.try_acquire_at(20, 35), Ok(()));
    
    // Verify window_starts are updated correctly
    // tick 24: sliding window [17, 24] (because 24-(8-1)=17)
    // Window 1 [8-15] end=15 < 17, completely expired
    // Window 0 [16-23] overlaps with [17, 24] at [17, 23] = 7 ticks
    // Weighted contribution: 35 * 7 = 245
    // Capacity contribution: 120 * 8 = 960
    // New request: 50 * 8 = 400
    // 245 + 400 = 645 <= 960, should succeed
    assert_eq!(counter.try_acquire_at(24, 50), Ok(()));
    
    // Verify remaining capacity: 960 - 645 = 315
    // New request: 40 * 8 = 320 > 315, should fail
    assert_eq!(counter.try_acquire_at(24, 40), Err(SimpleRateLimitError::InsufficientCapacity));
    
    // But 39 * 8 = 312 <= 315, should succeed
    assert_eq!(counter.try_acquire_at(24, 39), Ok(()));
}

#[test]
fn test_single_tick_window() {
    let counter = ApproximateSlidingWindowCore::new(20, 1);
    
    // Window 0 [0-0]
    assert_eq!(counter.try_acquire_at(0, 10), Ok(()));
    
    // Window 1 [1-1], sliding window [1, 1] (because 1-(1-1)=1)
    // Window 0 [0-0] has no overlap with sliding window [1, 1]
    // Only new request: 10 * 1 = 10 <= 20
    assert_eq!(counter.try_acquire_at(1, 10), Ok(()));
    
    // Add more at same tick, total 20
    assert_eq!(counter.try_acquire_at(1, 10), Ok(()));
    
    // Now exceeds capacity
    assert_eq!(counter.try_acquire_at(1, 1), Err(SimpleRateLimitError::InsufficientCapacity));
    
    // Window 0 new cycle [2-2], sliding window [2, 2]
    // Window 1 [1-1] has no overlap with sliding window [2, 2]
    // New request can use full capacity
    assert_eq!(counter.try_acquire_at(2, 20), Ok(()));
}

#[test]
fn test_large_window_size() {
    let counter = ApproximateSlidingWindowCore::new(500, 1000);
    
    // Operations in large window
    assert_eq!(counter.try_acquire_at(500, 200), Ok(()));   // Window 0 [0-999]
    assert_eq!(counter.try_acquire_at(800, 150), Ok(()));   // Still in Window 0
    assert_eq!(counter.try_acquire_at(999, 150), Ok(()));   // Still in Window 0
    
    // Total now 500, at capacity
    assert_eq!(counter.try_acquire_at(999, 1), Err(SimpleRateLimitError::InsufficientCapacity));
    
    // Switch to Window 1 [1000-1999]
    assert_eq!(counter.try_acquire_at(1200, 101), Err(SimpleRateLimitError::InsufficientCapacity));
    assert_eq!(counter.try_acquire_at(1200, 100), Ok(()));
}

#[test]
fn test_partial_overlap_precision() {
    let counter = ApproximateSlidingWindowCore::new(100, 20);
    
    // Window 0 [0-19]: add 50
    assert_eq!(counter.try_acquire_at(10, 50), Ok(()));
    
    // tick 25: sliding window [6, 25], Window 1 [20-39]
    // Window 0 [0-19] overlaps with [6, 25] at [6, 19] = 14 ticks
    // Window 1 [20-39] overlaps with [6, 25] at [20, 25] = 6 ticks
    // Weighted contribution: 50 * 14 + 0 * 6 = 700
    // Capacity contribution: 100 * 20 = 2000
    // New request: 30 * 20 = 600
    // 700 + 600 = 1300 <= 2000, should succeed
    assert_eq!(counter.try_acquire_at(25, 30), Ok(()));
    
    // Verify precise calculation
    // Current contribution: 700 + 600 = 1300, remaining: 2000 - 1300 = 700
    // New request: 40 * 20 = 800 > 700, should fail
    assert_eq!(counter.try_acquire_at(25, 40), Err(SimpleRateLimitError::InsufficientCapacity));
}

#[test]
fn test_edge_case_overlap_calculation() {
    let counter = ApproximateSlidingWindowCore::new(60, 10);
    
    // Window 0 [0-9]
    assert_eq!(counter.try_acquire_at(5, 30), Ok(()));
    
    // Window 1 [10-19]
    assert_eq!(counter.try_acquire_at(12, 20), Ok(()));
    
    // tick 19: sliding window [10, 19]
    // Window 0 [0-9] end=9 < 10, no overlap
    // Window 1 [10-19] completely overlaps with [10, 19] = 10 ticks
    // Only count Window 1: 20 * 10 = 200
    // Capacity: 60 * 10 = 600
    // New request: 40 * 10 = 400
    // 200 + 400 = 600, exactly at capacity
    assert_eq!(counter.try_acquire_at(19, 40), Ok(()));
    assert_eq!(counter.try_acquire_at(19, 1), Err(SimpleRateLimitError::InsufficientCapacity));
}