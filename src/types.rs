//! Unsigned integer type alias for rate limiter capacities and ticks.
//!
//! This module defines `Uint` as the integer type used for all rate limiter
//! token counts and time ticks. The actual type is determined at compile time
//! via feature flags.
//!
//! # Features
//! - `tick-u64` (default): uses [`u64`] as `Uint`
//! - `tick-u128`: uses [`u128`] as `Uint`
//!   (Both features cannot be enabled at the same time.)
//! - If neither feature is enabled, `u64` is used as the default type.

/// Alias for the unsigned integer type used for capacities and ticks.
///
/// The type is selected at compile time using feature flags:
/// - **`tick-u64`** (default): uses [`u64`]
/// - **`tick-u128`**: uses [`u128`]
///
/// > **Note:** Enabling both `tick-u64` and `tick-u128` at the same time
///   will result in a compile error. If neither is enabled, [`u64`] is used.
#[cfg(all(feature = "tick-u64", feature = "tick-u128"))]
compile_error!("You cannot enable both `tick-u64` and `tick-u128` features at the same time");

#[cfg(all(feature = "tick-u64", not(feature = "tick-u128")))]
pub type Uint = u64;

#[cfg(all(feature = "tick-u128", not(feature = "tick-u64")))]
pub type Uint = u128;

#[cfg(not(any(feature = "tick-u64", feature = "tick-u128")))]
pub type Uint = u64;
