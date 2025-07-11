//! Core rate limiting algorithm implementations.
//!
//! This module contains the core algorithms for various rate limiting strategies.
//! Each core provides a thread-safe, low-level implementation that can be used
//! to build higher-level rate limiter abstractions.
//!
//! # Available Algorithms
//!
//! - **[`TokenBucketCore`]** - Allows bursts up to capacity while maintaining average rate
//! - **[`FixedWindowCounterCore`]** - Simple window-based counting with reset at boundaries
//! - **[`SlidingWindowCounterCore`]** - Accurate sliding window using multiple buckets
//! - **[`ApproximateSlidingWindowCore`]** - Memory-efficient approximate sliding window
//!
//! # Algorithm Comparison
//!
//! | Algorithm | Memory Usage | Accuracy | Burst Handling | Use Case |
//! |-----------|-------------|----------|----------------|----------|
//! | Token Bucket | Low | High | Allow bursts | Bursty traffic |
//! | Fixed Window | Low | Medium | Boundary bursts | Simple counting |
//! | Sliding Window | Medium | High | Smooth bursts | Accurate limiting |
//! | Approximate SW | Low | Good | Good | Efficient approximation |
//!
//! # Thread Safety
//!
//! All cores use internal mutexes and provide thread-safe operations through
//! the `try_acquire_at` method, which may return `ContentionFailure` if the
//! lock cannot be acquired immediately.

pub mod token_bucket_core;
pub use token_bucket_core::TokenBucketCore;
pub use token_bucket_core::TokenBucketCoreConfig;

pub mod fixed_window_counter_core;
pub use fixed_window_counter_core::FixedWindowCounterCore;
pub use fixed_window_counter_core::FixedWindowCounterCoreConfig;

pub mod sliding_window_counter_core;
pub use sliding_window_counter_core::SlidingWindowCounterCore;
pub use sliding_window_counter_core::SlidingWindowCounterCoreConfig;

pub mod approximate_sliding_window_core;
pub use approximate_sliding_window_core::ApproximateSlidingWindowCore;
pub use approximate_sliding_window_core::ApproximateSlidingWindowCoreConfig;