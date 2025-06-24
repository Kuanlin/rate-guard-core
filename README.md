# `rate_limiter_core`
A comprehensive rate limiting library for Rust applications with multiple thread-safe algorithms.
## Features
- **5 Rate Limiting Algorithms**: Token Bucket, Leaky Bucket, Fixed Window, Sliding Window, and Approximate Sliding Window  
- **Thread-Safe**: All algorithms use non-blocking locks for high concurrency  
- **Zero Dependencies**: Lightweight with no external dependencies  
- **High Performance**: Optimized for speed and memory efficiency  
- **Flexible Time**: Works with any time unit via abstract “ticks”  
- **Configurable Tick Precision**: Compile-time feature flags allow choosing `u64` (default) or `u128` for tick units  
- **Rust 1.60+**: Compatible with older Rust versions  
---

## Quick Start
Add to your `Cargo.toml`:
```toml
[dependencies]
rate-limiter-core = { git = "https://github.com/Kuanlin/rate_limiter_core", tag = "v0.1.1" }
```

## Tick Precision (u64 / u128)
By default, the crate uses `u64` as the tick unit, allowing up to ~584 years of nanosecond-resolution time.
If your application needs ultra-long durations or ultra-high precision, you can enable `u128` support via feature flags:

```toml
[dependencies]
rate-limiter-core = { git = "https://github.com/Kuanlin/rate_limiter_core", default-features = false, features = ["tick_u128"] }
```

---

## Usage Examples
### Token Bucket
Perfect for APIs that allow occasional bursts while maintaining average rate:
```rust
use rate_limiter_core::rate_limiters::TokenBucketCore;
// Allow 100 requests, refill 10 every 5 seconds
let limiter = TokenBucketCore::new(100, 5, 10); 
let current_tick = std::time::SystemTime::now() 
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()
    .as_secs();
match limiter.try_acquire_at(1, current_tick) {
    Ok(()) => println!("Request allowed!"),
    Err(e) => println!("Rate limited: {}", e),
}
```

### Leaky Bucket
Great for maintaining steady traffic flow:
```rust
use rate_limiter_core::rate_limiters::LeakyBucketCore;
// capacity = 50 tokens, refill = 5 tokens per 10 ticks
let limiter = LeakyBucketCore::new(50, 10, 5);
assert_eq!(limiter.try_acquire_at(10, 0), Ok(()));
```
### Fixed Window Counter

```rust
use rate_limiter_core::rate_limiters::FixedWindowCounterCore;
// capacity = 100 per fixed window, window size = 60 ticks
let limiter = FixedWindowCounterCore::new(100, 60);
assert_eq!(limiter.try_acquire_at(1, 30), Ok(()));
```

### Sliding Window Counter
```rust
use rate_limiter_core::rate_limiters::SlidingWindowCounterCore;
// Window of 100 tokens across 6 buckets of 10 ticks each (50 ticks window)
let limiter = SlidingWindowCounterCore::new(100, 10, 6);
assert_eq!(limiter.try_acquire_at(5, 25), Ok(()));
```

### Approximate Sliding Window
#### (A Memory-Optimized Version: Sliding Window Counter)
formula: UsedCapacities = (1-X%) * lastWindowRequests + currentWindowRequests.   (X is the propotion of request time within the current window.)
```rust
use rate_limiter_core::rate_limiters::ApproximateSlidingWindowCore;
// Create ApproximateSlidingWindow limiter with capacity 100, window size 60 ticks
let limiter = ApproximateSlidingWindowCore::new(100, 60);
assert_eq!(limiter.try_acquire_at(10, 30), Ok(()));
```

## Error Handling
All rate limiters return an `AcquireResult`:
```rust
use rate_limiter_core::{RateLimitError, AcquireResult};
match limiter.try_acquire_at(1, tick) {
    Ok(()) => {
        // Request allowed
    },
    Err(RateLimitError::ExceedsCapacity) => {
        // Rate limit exceeded
    },
    Err(RateLimitError::ContentionFailure) => {
        // Lock contention, you can do sleep and retry here.
    },
    Err(RateLimitError::ExpiredTick) => {
        // Time went backwards
    },
}
```

---
## Time Management
The library uses abstract “ticks” for time. You can map any time source:
```rust
// Seconds
let tick = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
// Milliseconds
let tick = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
// Custom time
let tick = my_monotonic_timer.elapsed_ticks();
```

---
## Thread Safety
```rust
use std::sync::Arc;
use std::thread;
use rate_limiter_core::rate_limiters::TokenBucketCore;
let limiter = Arc::new(TokenBucketCore::new(100, 1, 10));
for _ in 0..10 {
    let limiter = limiter.clone();
    thread::spawn(move || {
        match limiter.try_acquire_at(1, get_current_tick()) {
            Ok(()) => println!("Request processed"),
            Err(e) => println!("Rate limited: {}", e),
        }
    });
}
```
---

## License
Licensed under either of Apache License, Version 2.0 or MIT license at your option.

---
---
## Contributing
Contributions are welcome! Please feel free to submit a Pull Request.

