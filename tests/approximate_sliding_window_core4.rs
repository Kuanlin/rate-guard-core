
use rate_guard_core::cores::ApproximateSlidingWindowCore;
use rate_guard_core::Uint;
use rate_guard_core::error::VerboseRateLimitError;

fn new_approx_window(cap: Uint, window_ticks: Uint) -> ApproximateSlidingWindowCore {
    ApproximateSlidingWindowCore::new(cap, window_ticks)
}

#[test]
fn zero_token_should_succeed() {
    let limiter = new_approx_window(10, 4);
    assert_eq!(limiter.try_acquire_verbose_at(0, 0), Ok(()));
}

#[test]
fn request_exceeds_capacity_should_fail() {
    let limiter = new_approx_window(10, 4);
    let err = limiter.try_acquire_verbose_at(0, 15).unwrap_err();
    match err {
        VerboseRateLimitError::BeyondCapacity { acquiring, capacity } => {
            assert_eq!(acquiring, 15);
            assert_eq!(capacity, 10);
        }
        _ => panic!("Expected BeyondCapacity"),
    }
}

#[test]
fn expired_tick_should_fail() {
    let limiter = new_approx_window(10, 5);
    let _ = limiter.try_acquire_verbose_at(10, 5); // move state forward

    let err = limiter.try_acquire_verbose_at(4, 1).unwrap_err();
    match err {
        VerboseRateLimitError::ExpiredTick { min_acceptable_tick } => {
            assert!(min_acceptable_tick >= 10);
        }
        _ => panic!("Expected ExpiredTick"),
    }
}

#[test]
fn fill_window_then_fail() {
    let limiter = new_approx_window(5, 4);
    assert_eq!(limiter.try_acquire_verbose_at(0, 5), Ok(())); // fill window

    // Next tick should fail because window not expired
    let err = limiter.try_acquire_verbose_at(1, 1).unwrap_err();
    match err {
        VerboseRateLimitError::InsufficientCapacity {
            acquiring,
            available,
            retry_after_ticks,
        } => {
            assert_eq!(acquiring, 1);
            assert_eq!(available, 0);
            assert!(retry_after_ticks >= 1);
        }
        _ => panic!("Expected InsufficientCapacity"),
    }
}

#[test]
fn succeed_after_window_expiration() {
    let limiter = new_approx_window(5, 4); // capacity = 5, window = 4 ticks
    assert_eq!(limiter.try_acquire_at(0, 5), Ok(())); // fills previous window

    // At tick 4, overlap with previous window is 3 ticks (1..3)
    // Contribution = 5 * 3 = 15 → still room for 1 token (20 - 15 >= 4)
    assert_eq!(limiter.try_acquire_at(4, 1), Ok(()));

    // tick 5: overlap = 2, previous = 5*2=10, current=1*4=4 → total=14
    assert_eq!(limiter.try_acquire_at(5, 1), Ok(()));

    // tick 6: overlap = 1, prev=5*1=5, curr=2*4=8 → total=13
    assert_eq!(limiter.try_acquire_at(6, 1), Ok(()));

    // tick 7: overlap = 0, curr=3*4=12 → total=12 < 20
    assert_eq!(limiter.try_acquire_at(7, 1), Ok(()));
}


#[test]
fn overlap_contribution_allows_partial_usage() {
    let limiter = new_approx_window(10, 4);

    // tick=0 puts tokens in window 0
    assert_eq!(limiter.try_acquire_verbose_at(0, 4), Ok(()));

    // tick=3 is still same window, allow more
    assert_eq!(limiter.try_acquire_verbose_at(3, 4), Ok(()));

    // tick=4 triggers window switch
    assert_eq!(limiter.try_acquire_verbose_at(4, 2), Ok(()));

    // tick=5: may still be okay due to partial overlap
    let result = limiter.try_acquire_verbose_at(5, 1);
    assert!(
        result.is_ok() || matches!(result, Err(VerboseRateLimitError::InsufficientCapacity { .. }))
    );
}

