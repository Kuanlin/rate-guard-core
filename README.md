# `rate_limiter_core`
A comprehensive rate limiting library for Rust applications with multiple thread-safe algorithms.
## Features
- **5 Rate Limiting Algorithms**: Token Bucket, Leaky Bucket, Fixed Window, Sliding Window, and Approximate Sliding Window  
- **Thread-Safe**: All algorithms use non-blocking locks for high concurrency  
- **Zero Dependencies**: Lightweight with no external dependencies  
- **High Performance**: Optimized for speed and memory efficiency  
- **Flexible Time**: Works with any time unit via abstract “ticks”  
- **Rust 1.60+**: Compatible with older Rust versions  
---

## Quick Start
Add to your `Cargo.toml`:
```toml
[dependencies]
rate_limiter_core = "0.1"

```
---

## Usage Examples
### Token Bucket (Allow Bursts)
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

### Leaky Bucket (Smooth Traffic)
Great for maintaining steady traffic flow:
```rust
use rate_limiter_core::rate_limiters::LeakyBucketCore;
let limiter = LeakyBucketCore::new(50, 10, 5);
assert_eq!(limiter.try_acquire_at(10, 0), Ok(()));
```
### Fixed Window Counter (Simple Quotas)

```rust
use rate_limiter_core::rate_limiters::FixedWindowCounterCore;
let limiter = FixedWindowCounterCore::new(100, 60);
assert_eq!(limiter.try_acquire_at(1, 30), Ok(()));
```

### Sliding Window Counter (Accurate Limiting)
```rust
use rate_limiter_core::rate_limiters::SlidingWindowCounterCore;
let limiter = SlidingWindowCounterCore::new(100, 10, 6);
assert_eq!(limiter.try_acquire_at(5, 25), Ok(()));
```

### Approximate Sliding Window (Memory Efficient)
```rust
use rate_limiter_core::rate_limiters::ApproximateSlidingWindowCore;
let limiter = ApproximateSlidingWindowCore::new(100, 60);
assert_eq!(limiter.try_acquire_at(10, 30), Ok(()));
```
---

## Algorithm Comparison
| Algorithm             | Memory Usage | Accuracy | Burst Handling | Use Case                |
|-----------------------|--------------|----------|----------------|--------------------------|
| Token Bucket          | Low          | High     | Allow bursts   | API rate limiting       |
| Leaky Bucket          | Low          | High     | Smooth only    | Constant rate traffic   |
| Fixed Window          | Low          | Medium   | Boundary bursts| Simple quotas           |
| Sliding Window        | Medium       | High     | Smooth bursts  | Accurate limiting       |
| Approximate Sliding W.| Low          | Good     | Good           | Efficient approximation |
---

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

