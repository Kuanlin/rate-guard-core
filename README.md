# `rate-guard-core`
A comprehensive rate limiting library for Rust applications with multiple thread-safe algorithms.

## Features
- **5 Rate Limiting Algorithms**: Token Bucket, Leaky Bucket, Fixed Window Counter, Sliding Window Counter, and Approximate Sliding Window  
- **Thread-Safe**: All algorithms use non-blocking locks  
- **Zero Dependencies**: Lightweight with no external dependencies  
- **Flexible Time**: Works with any time unit via abstract “ticks”  
- **Configurable Tick Precision**: Compile-time feature flags allow choosing `u64` (default) or `u128` for tick units  
- **Rust 1.60+**: Compatible with older Rust versions  

---

## Quick Start

### from crate.io
Add to your `Cargo.toml`:
```toml
[dependencies]
rate-guard-core = { version = "0.5.2" }
```

### from Github
Add to your `Cargo.toml`:
```toml
[dependencies]
rate-guard-core = { git = "https://github.com/Kuanlin/rate-guard-core", tag = "v0.5.2" }
```

---

## Tick Precision (u64 / u128)
By default, the crate uses `u64` as the tick unit, allowing up to ~584 years of nanosecond-resolution time.
If your application needs ultra-long durations or ultra-high precision, you can enable `u128` support via feature flags:

### from crate.io
```toml
[dependencies]
rate-guard-core = { version = "0.5.2", default-features = false, features = ["tick_u128"] }
```

### from Github
```toml
[dependencies]
rate-guard-core = { git = "https://github.com/Kuanlin/rate-guard-core", tag = "v0.5.2", default-features = false, features = ["tick_u128"] }
```

---

## Usage Examples

### Token Bucket  
Perfect for APIs that allow occasional bursts while maintaining average rate:

```rust
use rate_guard_core::cores::{TokenBucketCore, TokenBucketCoreConfig};

let config = TokenBucketCoreConfig {
    capacity: 100,
    refill_interval: 5,
    refill_amount: 10,
};

// Option 1: Using `into()` – idiomatic Rust way to convert config into limiter
let limiter: TokenBucketCore = config.into();

// Option 2: Using `from()` – explicitly convert config into limiter
let limiter_alt = TokenBucketCore::from(config);
```

---

### Leaky Bucket  
Great for maintaining steady traffic flow:

```rust
use rate_guard_core::cores::{LeakyBucketCore, LeakyBucketCoreConfig};

let config = LeakyBucketCoreConfig {
    capacity: 50,
    leak_interval: 10,
    leak_amount: 5,
};

// Option 1: Using `into()` – idiomatic Rust way to convert config into limiter
let limiter: LeakyBucketCore = config.into();

// Option 2: Using `from()` – explicitly convert config into limiter
let limiter_alt = LeakyBucketCore::from(config);
```

---

### Fixed Window Counter

```rust
use rate_guard_core::cores::{FixedWindowCounterCore, FixedWindowCounterCoreConfig};

let config = FixedWindowCounterCoreConfig {
    capacity: 100,
    window_size: 60,
};

// Option 1: Using `into()` – idiomatic Rust way to convert config into limiter
let limiter: FixedWindowCounterCore = config.into();

// Option 2: Using `from()` – explicitly convert config into limiter
let limiter_alt = FixedWindowCounterCore::from(config);
```

---

### Sliding Window Counter

```rust
use rate_guard_core::cores::{SlidingWindowCounterCore, SlidingWindowCounterCoreConfig};

let config = SlidingWindowCounterCoreConfig {
    capacity: 100,
    bucket_ticks: 10,
    bucket_count: 6,
};

// Option 1: Using `into()` – idiomatic Rust way to convert config into limiter
let limiter: SlidingWindowCounterCore = config.into();

// Option 2: Using `from()` – explicitly convert config into limiter
let limiter_alt = SlidingWindowCounterCore::from(config);

```

---

### Approximate Sliding Window  
A memory-optimized version of sliding window counter.  
Formula:  
`Used = (1 - X%) * lastWindow + currentWindow` where X is the proportion of request time within the current window.

```rust
use rate_guard_core::cores::{ApproximateSlidingWindowCore, ApproximateSlidingWindowCoreConfig};

let config = ApproximateSlidingWindowCoreConfig {
    capacity: 100,
    window_ticks: 60,
};

// Option 1: Using `into()` – idiomatic Rust way to convert config into limiter
let limiter: ApproximateSlidingWindowCore = config.into();

// Option 2: Using `from()` – explicitly convert config into limiter
let limiter_alt = ApproximateSlidingWindowCore::from(config);
```

> Both `into()` and `from()` are functionally equivalent in Rust.  
> `into()` is shorter and idiomatic; `from()` is more explicit and beginner-friendly.  
> These examples are duplicated to help both Rust newcomers and non-Rust readers understand the conversion logic.


---

### Approximate Sliding Window  
A memory-optimized version of sliding window counter.  
Formula:  
`Used = (1 - X%) * lastWindow + currentWindow` where X is the proportion of request time within the current window.

```rust
use rate_guard_core::cores::{ApproximateSlidingWindowCore, ApproximateSlidingWindowCoreConfig};

let config = ApproximateSlidingWindowCoreConfig {
    capacity: 100,
    window_ticks: 60,
};
let limiter: ApproximateSlidingWindowCore = ApproximateSlidingWindowCore::from(config);
```

---

## Error Handling
All limiters' try_acquire_at returns `SimpleRateLimitResult`:
```Rust
use rate_guard_core::{SimpleRateLimitError, SimpleRateLimitResult};
match limiter.try_acquire_at(tick, 1) {
    Ok(()) => {
        // Request allowed
    },
    Err(SimpleRateLimitError::InsufficientCapacity) => {
        // Rate limit exceeded
    },
    Err(SimpleRateLimitError::BeyondCapacity) => {
        // Acquiring too much
    },
    Err(SimpleRateLimitError::ExpiredTick) => {
        // Time went backwards
    },
    Err(SimpleRateLimitError::ContentionFailure) => {
        // Lock contention, you can do sleep and retry here.
    },
}
```

---

## Verbose Error Reporting

Each limiter also supports `try_acquire_verbose_at(tick, tokens)`, which returns a `VerboseRateLimitError` with richer diagnostics:

- `ContentionFailure`: Lock was unavailable
- `ExpiredTick { min_acceptable_tick }`: Time went backwards
- `BeyondCapacity { acquiring, capacity }`: Requested tokens exceed max
- `InsufficientCapacity { acquiring, available, retry_after_ticks }`: Not enough tokens now, but suggests how long to wait before retrying

```Rust
use rate_guard_core::{VerboseRateLimitError, VerboseRateLimitResult};

match limiter.try_acquire_verbose_at(tick, 5) {
    Ok(()) => {
        // Request allowed
    }
    Err(VerboseRateLimitError::InsufficientCapacity { retry_after_ticks, .. }) => {
        println!("Retry after {} ticks", retry_after_ticks);
    }
    Err(e) => {
        println!("Rate limit error: {:?}", e);
    }
}
```

> `try_acquire_verbose_at` is useful for retry logic, logging, or adaptive throttling.

---

## Time Management
The library uses abstract “ticks” for time. You can map any time source:
```Rust
// Seconds
let tick = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
// Milliseconds
let tick = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
// Custom time
let tick = my_monotonic_timer.elapsed_ticks();
```

---

## Thread Safety
```Rust
use std::sync::Arc;
use std::thread;
use rate_guard_core::cores::TokenBucketCore;
let limiter = Arc::new(TokenBucketCore::new(100, 1, 10));
for _ in 0..10 {
    let limiter = limiter.clone();
    thread::spawn(move || {
        match limiter.try_acquire_at(get_current_tick(), 1) {
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

## Contributing
Contributions are welcome! Please feel free to submit a Pull Request.
