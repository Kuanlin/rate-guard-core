use rate_guard_core::rate_limiters::SlidingWindowCounterCore;
use rate_guard_core::Uint;
use rate_guard_core::error::VerboseRateLimitError;

fn new_sliding_window(cap: Uint, bucket_ticks: Uint, bucket_count: Uint) -> SlidingWindowCounterCore {
    SlidingWindowCounterCore::new(cap, bucket_ticks, bucket_count)
}

#[test]
fn zero_token_should_succeed() {
    let limiter = new_sliding_window(10, 2, 5);
    assert_eq!(limiter.try_acquire_verbose_at(0, 0), Ok(()));
}

#[test]
fn request_exceeds_capacity_should_fail() {
    let limiter = new_sliding_window(10, 2, 5);
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
    let limiter = new_sliding_window(10, 2, 4);
    assert_eq!(limiter.try_acquire_verbose_at(8, 3), Ok(())); // last bucket tick = 8
    let err = limiter.try_acquire_verbose_at(2, 1).unwrap_err(); // tick too old
    match err {
        VerboseRateLimitError::ExpiredTick { min_acceptable_tick } => {
            assert!(min_acceptable_tick >= 8);
        }
        _ => panic!("Expected ExpiredTick"),
    }
}

#[test]
fn fill_window_then_fail() {
    let limiter = new_sliding_window(5, 1, 5); // window_ticks = 5
    for i in 0..5 {
        assert_eq!(limiter.try_acquire_verbose_at(i, 1), Ok(())); 
    }

    let err = limiter.try_acquire_verbose_at(5, 2).unwrap_err(); 
    match err {
        VerboseRateLimitError::InsufficientCapacity {
            acquiring,
            available,
            retry_after_ticks,
        } => {
            assert_eq!(acquiring, 2);
            assert_eq!(available, 1);
            assert!(retry_after_ticks > 0);
        }
        _ => panic!("Expected InsufficientCapacity"),
    }
}

#[test]
fn succeed_after_tokens_expire_from_window() {
    let limiter = new_sliding_window(5, 1, 5);
    for i in 0..5 {
        assert_eq!(limiter.try_acquire_verbose_at(i, 1), Ok(()));
    }

    assert!(limiter.try_acquire_verbose_at(5, 2).is_err());

    assert_eq!(limiter.try_acquire_verbose_at(6, 1), Ok(()));
}

#[test]
fn partial_bucket_recovery_allows_progressive_acquire() {
    let limiter = new_sliding_window(6, 2, 3); // window = 6 ticks

    assert_eq!(limiter.try_acquire_verbose_at(0, 2), Ok(()));
    assert_eq!(limiter.try_acquire_verbose_at(2, 2), Ok(()));
    assert_eq!(limiter.try_acquire_verbose_at(4, 2), Ok(())); // full now

    // tick 5, still full
    assert!(limiter.try_acquire_verbose_at(5, 1).is_err());

    // tick 6: bucket at tick=0 is now expired
    assert_eq!(limiter.try_acquire_verbose_at(6, 1), Ok(()));
}
