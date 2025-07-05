// tests/rate_limiter_core_leaky_bucket.rs

use rate_guard_core::Uint;
use rate_guard_core::rate_limiters::LeakyBucketCore;
use rate_guard_core::error::VerboseRateLimitError;


fn new_leaky(capacity: Uint, leak_interval: Uint, leak_amount: Uint) -> LeakyBucketCore {
    LeakyBucketCore::new(capacity, leak_interval, leak_amount)
}

#[test]
fn test_zero_token_ok() {
    let limiter = new_leaky(10, 1, 1);
    assert_eq!(limiter.try_acquire_verbose_at(0, 0), Ok(()));
}

#[test]
fn test_beyond_capacity() {
    let limiter = new_leaky(10, 1, 1);
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
fn test_expired_tick() {
    let limiter = new_leaky(10, 1, 1);
    let _ = limiter.try_acquire_verbose_at(5, 3);
    let err = limiter.try_acquire_verbose_at(3, 1).unwrap_err();
    match err {
        VerboseRateLimitError::ExpiredTick { min_acceptable_tick } => {
            assert!(min_acceptable_tick >= 5);
        }
        _ => panic!("Expected ExpiredTick"),
    }
}

#[test]
fn test_ok_then_insufficient_capacity() {
    let limiter = new_leaky(10, 1, 1);
    assert_eq!(limiter.try_acquire_verbose_at(0, 5), Ok(()));
    let result = limiter.try_acquire_verbose_at(0, 6);
    match result {
        Err(VerboseRateLimitError::InsufficientCapacity { acquiring, available, retry_after_ticks }) => {
            assert_eq!(acquiring, 6);
            assert_eq!(available, 5); // 10 - 5 used = 5 left
            assert!(retry_after_ticks > 0);
        }
        _ => panic!("Expected InsufficientCapacity"),
    }
}

#[test]
fn test_recovery_after_leak() {
    let limiter = new_leaky(10, 2, 2);
    assert_eq!(limiter.try_acquire_verbose_at(0, 10), Ok(()));
    let _ = limiter.try_acquire_verbose_at(0, 1).unwrap_err();

    // Wait enough for 2 tokens to leak out
    assert_eq!(limiter.try_acquire_verbose_at(2, 1), Ok(()));
}

#[test]
fn test_retry_after_tick_behaves_correctly() {
    let limiter = new_leaky(5, 2, 1); // capacity=5, leak 1 token every 2 ticks
    assert_eq!(limiter.try_acquire_verbose_at(0, 5), Ok(())); // fill it up

    // Try acquiring one more token immediately
    let result = limiter.try_acquire_verbose_at(0, 1);
    let retry_tick = match result {
        Err(VerboseRateLimitError::InsufficientCapacity { retry_after_ticks, .. }) => retry_after_ticks,
        _ => panic!("Expected InsufficientCapacity"),
    };

    // Should fail right before retry tick
    let too_early = 0 + retry_tick - 1;
    assert!(
        matches!(limiter.try_acquire_verbose_at(too_early, 1), Err(VerboseRateLimitError::InsufficientCapacity { .. })),
        "Should fail at tick {}",
        too_early
    );

    // Should succeed at retry tick
    let just_right = 0 + retry_tick;
    assert_eq!(limiter.try_acquire_verbose_at(just_right, 1), Ok(()));
}


#[test]
fn test_true_overflow() {
    let limiter = new_leaky(5, 2, 1);

    // fill it up immediately
    assert_eq!(limiter.try_acquire_verbose_at(0, 5), Ok(())); // now full

    // attempt before any leak happens
    let err = limiter.try_acquire_verbose_at(1, 1).unwrap_err();

    assert!(matches!(err, VerboseRateLimitError::InsufficientCapacity { .. }));
}
