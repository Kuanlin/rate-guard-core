## [0.6.1] - 2025-07-11

### Fixed

Removed Chinese comment from lib.rs for better code consistency
Fixed Uint type imports in test files (fixed_window_counter_core3.rs, sliding_window_counter_core3.rs, token_bucket_core3.rs)
Updated test helper functions to use Uint type instead of hardcoded u64 for better type flexibility
Improved type compatibility when using different tick precision features

### Changed

Enhanced code consistency across test files
Better support for both u64 and u128 tick precision in tests

> This is a patch release that fixes minor issues without breaking changes.


## [0.6.0] - 2025-07-11

### Removed

 **BREAKING CHANGE**: Removed `LeakyBucketCore` and `LeakyBucketCoreConfig`
 Removed all leaky bucket related tests and documentation
 Updated module exports to remove leaky bucket references

### Changed

 Updated documentation to reflect the 4 available algorithms (previously 5)
 Simplified algorithm comparison table in documentation
 Updated README examples to focus on the remaining 4 algorithms

### Rationale

 The previous `LeakyBucketCore` implementation was actually a "leaky bucket as meter" variant, which is functionally equivalent to the token bucket algorithm
 True leaky bucket behavior requires request queuing and background processing, which is beyond the scope of this synchronous rate limiting library
 This change eliminates confusion and focuses the library on distinct, well-defined rate limiting algorithms
 Users requiring leaky bucket behavior should implement request queuing at the application layer using the remaining algorithms

> This version introduces breaking changes and increases the major version to `0.6.0` from `0.5.2` per semantic versioning policy.


## [0.5.2] - 2025-07-08

### Fixed

- Updated crate version to `0.5.2` to reflect prior documentation changes published under `v0.5.1`

> No code or API changes included in this release.


## [0.5.1] - 2025-07-08

### Fixed

- Corrected `README.md` examples to use `cores::` instead of the outdated `rate_limiters::` path


## [0.5.0] - 2025-07-08

### Changed

- Renamed module directory from `rate_limiters/` to `cores/`
- Renamed `rate_limiter_core.rs` to `rate_limits.rs`
- Renamed result type aliases:
  - `SimpleAcquireResult` → `SimpleRateLimitResult`
  - `VerboseAcquireResult` → `VerboseRateLimitResult`
- `RateLimitCore::capacity_remaining()` now returns `Result<Uint, SimpleRateLimitError>`

### Added

- `capacity_remaining_or_0(tick)` fallback method for all limiters
- `current_capacity_or_0()` to retrieve approximate capacity without error handling

> This version introduces breaking changes and increases the minor version to `0.5.0` from `0.4.0` per semantic versioning policy.


## [0.4.0] - 2025-07-07

### Changed
- Fixed `LeakyBucketCore::capacity_remaining()` to return the available capacity instead of the number of tokens in the bucket.

### Added
- Added `tokens_in_bucket()` to both `LeakyBucketCore` and `TokenBucketCore`.


## [0.3.1] - 2025-07-06

### Changed
- Corrected terminology in comments for sliding window counter and fixed window counter.


## [0.3.0] - 2025-07-06

### Added
- Introduced config structs for each rate limiter.
- Implemented `From<Config>` for each corresponding limiter type.
- Implemented `try_acquire_verbose_at` for all limiter cores.

### Changed
- Major API cleanup: renamed methods and updated argument order.
- Bumped version to `0.3.0`.


## [0.2.2] - 2025-07-05

### Fixed
- Updated `types.rs` and `Cargo.toml` to prevent build conflicts on docs.rs.

### Changed
- Added `.orig` to `.gitignore` and excluded it from `Cargo.toml`.


## [0.2.1] - Unreleased (inferred from commits)

### Added
- Completed `RateLimitCore` trait implementation for all algorithms.
- Added and implemented trait methods across cores.


## [0.1.2] - 2025-07-01 (estimated)

### Changed
- Renamed crate from `rate-limiter-core` to `rate-guard-core`.
- Bumped version to `v0.1.2`.

### Fixed
- Updated README and Cargo metadata.


## [0.1.1] - 2025-06-30 (estimated)

### Added
- Added feature flag support for `Uint` to be either `u64` or `u128`.


## [0.1.0] - 2025-06-29 (estimated)

### Added
- Initial release of `rate-guard-core`.
- Basic README and metadata setup.
