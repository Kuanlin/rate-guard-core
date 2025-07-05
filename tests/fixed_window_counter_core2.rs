use rate_guard_core::{SimpleRateLimitError};
use rate_guard_core::rate_limiters::FixedWindowCounterCore;

#[test]
fn test_contention_failure() {
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;
    use std::sync::atomic::{AtomicBool, Ordering};
    
    let counter = Arc::new(FixedWindowCounterCore::new(100, 10));
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
fn test_window_boundary_precise() {
    let counter = FixedWindowCounterCore::new(100, 10); // Windows: [0-9], [10-19], [20-29]...
    
    // Last tick of window 0
    assert_eq!(counter.try_acquire_at(9, 50), Ok(())); // count = 50 in window 0
    
    // First tick of window 1 - counter should reset
    assert_eq!(counter.try_acquire_at(10, 100), Ok(())); // count = 100 in window 1
    
    // Last tick of window 1
    assert_eq!(counter.try_acquire_at(19, 1), Err(SimpleRateLimitError::InsufficientCapacity));
    
    // First tick of window 2 - counter resets again
    assert_eq!(counter.try_acquire_at(20, 100), Ok(())); // count = 100 in window 2
}

#[test]
fn test_exact_window_transitions() {
    let counter = FixedWindowCounterCore::new(50, 5); // Windows: [0-4], [5-9], [10-14]...
    
    // Window 0: tick 4 (last tick of window)
    assert_eq!(counter.try_acquire_at(4, 30), Ok(()));
    
    // Window 1: tick 5 (boundary transition)
    assert_eq!(counter.try_acquire_at(5, 50), Ok(()));
    
    // Still in window 1: tick 9 (last tick)
    assert_eq!(counter.try_acquire_at(9, 1), Err(SimpleRateLimitError::InsufficientCapacity));
    
    // Window 2: tick 10 (boundary transition)
    assert_eq!(counter.try_acquire_at(10, 50), Ok(()));
    
    // Window 3: tick 15 (boundary transition)
    assert_eq!(counter.try_acquire_at(15, 50), Ok(()));
}

#[test]
fn test_same_tick_multiple_calls() {
    let counter = FixedWindowCounterCore::new(100, 10);
    
    // Multiple calls at the same tick should accumulate
    assert_eq!(counter.try_acquire_at(5, 20), Ok(())); // count = 20
    assert_eq!(counter.try_acquire_at(5, 30), Ok(())); // count = 50
    assert_eq!(counter.try_acquire_at(5, 25), Ok(())); // count = 75
    assert_eq!(counter.try_acquire_at(5, 25), Ok(())); // count = 100
    
    // Now at capacity, should fail
    assert_eq!(counter.try_acquire_at(5, 1), Err(SimpleRateLimitError::InsufficientCapacity));
    
    // Transition to new window, counter resets
    assert_eq!(counter.try_acquire_at(10, 50), Ok(())); // count = 50 in new window
}

#[test]
fn test_window_reset_verification() {
    let counter = FixedWindowCounterCore::new(60, 8); // Windows: [0-7], [8-15], [16-23]...
    
    // Window 0: partial usage
    assert_eq!(counter.try_acquire_at(3, 40), Ok(())); // count = 40
    
    // Window 1: complete usage
    assert_eq!(counter.try_acquire_at(12, 60), Ok(())); // count = 60 in new window
    assert_eq!(counter.try_acquire_at(15, 1), Err(SimpleRateLimitError::InsufficientCapacity));
    
    // Window 2: verify reset was successful
    assert_eq!(counter.try_acquire_at(16, 60), Ok(())); // count = 60 in new window
    
    // Window 3: verify reset again
    assert_eq!(counter.try_acquire_at(24, 60), Ok(())); // count = 60 in new window
    
    // Skip multiple windows and verify reset
    assert_eq!(counter.try_acquire_at(100, 60), Ok(())); // Window [96-103], count = 60
}

#[test]
fn test_start_tick_alignment() {
    let counter = FixedWindowCounterCore::new(30, 10);
    
    // Start using at tick 7
    assert_eq!(counter.try_acquire_at(7, 15), Ok(())); // count = 15 in window 0 [0-9]
    
    // tick 12: new window [10-19], start_tick should be 10
    assert_eq!(counter.try_acquire_at(12, 20), Ok(())); // count = 20 in window 1
    
    // Verify cannot go back before window start
    assert_eq!(counter.try_acquire_at(9, 5), Err(SimpleRateLimitError::ExpiredTick));
    
    // But can operate within current window
    assert_eq!(counter.try_acquire_at(15, 10), Ok(())); // count = 30 in window 1
    
    // New window [20-29], start_tick should be 20
    assert_eq!(counter.try_acquire_at(25, 30), Ok(())); // count = 30 in window 2
    
    // Verify cannot go back to previous window
    assert_eq!(counter.try_acquire_at(19, 5), Err(SimpleRateLimitError::ExpiredTick));
}

#[test]
fn test_single_tick_window() {
    let counter = FixedWindowCounterCore::new(10, 1); // Each tick is a separate window
    
    // Window 0: tick 0
    assert_eq!(counter.try_acquire_at(0, 10), Ok(()));
    assert_eq!(counter.try_acquire_at(0, 1), Err(SimpleRateLimitError::InsufficientCapacity));
    
    // Window 1: tick 1 (counter resets)
    assert_eq!(counter.try_acquire_at(1, 10), Ok(()));
    
    // Window 2: tick 2 (counter resets)
    assert_eq!(counter.try_acquire_at(2, 10), Ok(()));
    
    // Window 100: tick 100 (counter resets)
    assert_eq!(counter.try_acquire_at(100, 10), Ok(()));
}

#[test]
fn test_very_large_window() {
    let counter = FixedWindowCounterCore::new(1000, 1000000); // Very large window
    
    // Multiple operations within the large window
    assert_eq!(counter.try_acquire_at(1000, 200), Ok(()));   // count = 200
    assert_eq!(counter.try_acquire_at(500000, 300), Ok(())); // count = 500
    assert_eq!(counter.try_acquire_at(999999, 400), Ok(())); // count = 900 (last tick of window)
    assert_eq!(counter.try_acquire_at(999999, 100), Ok(())); // count = 1000
    
    // Now at capacity
    assert_eq!(counter.try_acquire_at(999999, 1), Err(SimpleRateLimitError::InsufficientCapacity));
    
    // Next window starts
    assert_eq!(counter.try_acquire_at(1000000, 1000), Ok(())); // count = 1000 in new window
}

#[test]
fn test_window_calculation_edge_cases() {
    let counter = FixedWindowCounterCore::new(50, 7); // Windows: [0-6], [7-13], [14-20]...
    
    // Verify window calculation boundaries
    assert_eq!(counter.try_acquire_at(6, 25), Ok(()));    // Window 0 end
    assert_eq!(counter.try_acquire_at(7, 50), Ok(()));    // Window 1 start
    assert_eq!(counter.try_acquire_at(13, 1), Err(SimpleRateLimitError::InsufficientCapacity)); // Window 1 end
    assert_eq!(counter.try_acquire_at(14, 50), Ok(()));   // Window 2 start
    
    // Jump to distant window
    assert_eq!(counter.try_acquire_at(70, 50), Ok(()));   // Window 10 [70-76]
    assert_eq!(counter.try_acquire_at(77, 50), Ok(()));   // Window 11 [77-83]
}

#[test]
fn test_zero_usage_window_transitions() {
    let counter = FixedWindowCounterCore::new(100, 5);
    
    // Window 0: no usage
    
    // Window 1: partial usage
    assert_eq!(counter.try_acquire_at(7, 30), Ok(())); // count = 30 in window 1 [5-9]
    
    // Window 2: no usage
    
    // Window 3: complete usage
    assert_eq!(counter.try_acquire_at(15, 100), Ok(())); // count = 100 in window 3 [15-19]
    
    // Window 4: verify reset works even after unused windows
    assert_eq!(counter.try_acquire_at(20, 100), Ok(())); // count = 100 in window 4 [20-24]
}