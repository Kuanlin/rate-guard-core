use rate_guard_core::Uint;
use rate_guard_core::cores::TokenBucketCore;
use rate_guard_core::error::VerboseRateLimitError;

fn new_token_bucket(cap: Uint, interval: Uint, amount: Uint) -> TokenBucketCore {
    TokenBucketCore::new(cap, interval, amount)
}

#[test]
fn zero_token_should_always_succeed() {
    let limiter = new_token_bucket(10, 2, 1);
    assert_eq!(limiter.try_acquire_verbose_at(0, 0), Ok(()));
}

#[test]
fn request_exceeds_capacity_should_fail() {
    let limiter = new_token_bucket(10, 2, 1);
    let result = limiter.try_acquire_verbose_at(0, 20);
    match result {
        Err(VerboseRateLimitError::BeyondCapacity { acquiring, capacity }) => {
            assert_eq!(acquiring, 20);
            assert_eq!(capacity, 10);
        }
        _ => panic!("Expected BeyondCapacity error"),
    }
}

#[test]
fn expired_tick_should_fail() {
    let limiter = new_token_bucket(10, 2, 1);
    assert_eq!(limiter.try_acquire_verbose_at(5, 3), Ok(())); // move forward
    assert_eq!(limiter.try_acquire_verbose_at(10, 3), Ok(())); // move forward
    let result = limiter.try_acquire_verbose_at(7, 1);
    match result {
        Err(VerboseRateLimitError::ExpiredTick { min_acceptable_tick }) => {
            assert!(min_acceptable_tick >= 5);
        }
        _ => panic!("Expected ExpiredTick error"),
    }
}

#[test]
fn fill_bucket_then_exceed_should_fail() {
    let limiter = new_token_bucket(5, 2, 1);
    assert_eq!(limiter.try_acquire_verbose_at(0, 5), Ok(())); // fill

    // try one more before refill happens
    let result = limiter.try_acquire_verbose_at(1, 1);
    match result {
        Err(VerboseRateLimitError::InsufficientCapacity {
            acquiring,
            available,
            retry_after_ticks,
        }) => {
            assert_eq!(acquiring, 1);
            assert_eq!(available, 0);
            assert!(retry_after_ticks >= 1);
        }
        _ => panic!("Expected InsufficientCapacity"),
    }
}

#[test]
fn retry_after_tick_should_be_correct() {
    let limiter = new_token_bucket(5, 2, 2); // refill 2 tokens every 2 ticks
    assert_eq!(limiter.try_acquire_verbose_at(0, 5), Ok(())); // fill

    let err = limiter.try_acquire_verbose_at(0, 1).unwrap_err();
    let retry = match err {
        VerboseRateLimitError::InsufficientCapacity { retry_after_ticks, .. } => retry_after_ticks,
        _ => panic!("Expected InsufficientCapacity"),
    };

    let tick = 0 + retry;

    // Should still fail before retry tick
    assert!(limiter.try_acquire_verbose_at(tick - 1, 1).is_err());

    // Should succeed at retry tick
    assert_eq!(limiter.try_acquire_verbose_at(tick, 1), Ok(()));
}

#[test]
fn partial_refill_should_not_allow_acquire() {
    let limiter = new_token_bucket(5, 4, 2); // refill 2 tokens every 4 ticks
    assert_eq!(limiter.try_acquire_verbose_at(0, 5), Ok(())); // fill

    // tick 3: not enough refill yet
    assert!(limiter.try_acquire_verbose_at(3, 1).is_err());

    // tick 4: 2 tokens should be available
    assert_eq!(limiter.try_acquire_verbose_at(4, 2), Ok(()));
}

