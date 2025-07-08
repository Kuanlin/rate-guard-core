
use rate_guard_core::cores::FixedWindowCounterCore;
use rate_guard_core::Uint;
use rate_guard_core::error::VerboseRateLimitError;

fn new_fixed_window(capacity: Uint, window_ticks: Uint) -> FixedWindowCounterCore {
    FixedWindowCounterCore::new(capacity, window_ticks)
}

#[test]
fn zero_token_should_always_succeed() {
    let limiter = new_fixed_window(10, 5);
    assert_eq!(limiter.try_acquire_verbose_at(0, 0), Ok(()));
}

#[test]
fn request_exceeds_capacity_should_fail() {
    let limiter = new_fixed_window(10, 5);
    let err = limiter.try_acquire_verbose_at(0, 20).unwrap_err();
    match err {
        VerboseRateLimitError::BeyondCapacity { acquiring, capacity } => {
            assert_eq!(acquiring, 20);
            assert_eq!(capacity, 10);
        }
        _ => panic!("Expected BeyondCapacity"),
    }
}

#[test]
fn expired_tick_should_fail() {
    let limiter = new_fixed_window(10, 4);
    assert_eq!(limiter.try_acquire_verbose_at(8, 5), Ok(())); // tick = 8
    let err = limiter.try_acquire_verbose_at(3, 1).unwrap_err(); // tick = 3 (expired)
    match err {
        VerboseRateLimitError::ExpiredTick { min_acceptable_tick } => {
            assert!(min_acceptable_tick >= 8);
        }
        _ => panic!("Expected ExpiredTick"),
    }
}

#[test]
fn fill_window_then_fail() {
    let limiter = new_fixed_window(5, 10);
    assert_eq!(limiter.try_acquire_verbose_at(0, 5), Ok(())); // full

    let err = limiter.try_acquire_verbose_at(9, 1).unwrap_err();
    match err {
        VerboseRateLimitError::InsufficientCapacity {
            acquiring,
            available,
            retry_after_ticks,
        } => {
            assert_eq!(acquiring, 1);
            assert_eq!(available, 0);
            assert_eq!(retry_after_ticks, 1); // window ends at tick 9
        }
        _ => panic!("Expected InsufficientCapacity"),
    }
}

#[test]
fn succeed_in_new_window() {
    let limiter = new_fixed_window(5, 10);
    assert_eq!(limiter.try_acquire_verbose_at(0, 5), Ok(())); // full

    // Window [0..9], so tick 10 is a new window
    assert_eq!(limiter.try_acquire_verbose_at(10, 2), Ok(()));
}

#[test]
fn partial_window_usage_still_allows() {
    let limiter = new_fixed_window(10, 10);
    assert_eq!(limiter.try_acquire_verbose_at(1, 4), Ok(()));
    assert_eq!(limiter.try_acquire_verbose_at(5, 6), Ok(()));
}
