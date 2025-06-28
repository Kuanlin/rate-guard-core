//! Unsigned integer type alias for rate limiter capacities and ticks.
//!
//! This module defines `Uint` as the integer type used for all rate limiter
//! token counts and time ticks. The actual type is determined at compile time
//! via feature flags.
//!
//! # Features
//! - `tick_u64` (default): uses [`u64`] as `Uint`
//! - `tick_u128`: uses [`u128`] as `Uint`
//!   (Both features cannot be enabled at the same time.)
//! - If neither feature is enabled, `u64` is used as the default type.

/// Alias for the unsigned integer type used for capacities and ticks.
///
/// The type is selected at compile time using feature flags:
/// - **`tick_u64`** (default): uses [`u64`]
/// - **`tick_u128`**: uses [`u128`]
///
/// > **Note:** Enabling both `tick_u64` and `tick_u128` at the same time
///   will result in a compile error. If neither is enabled, [`u64`] is used.
#[cfg(all(feature = "tick_u64", feature = "tick_u128"))]
compile_error!("You cannot enable both `tick_u64` and `tick_u128` features at the same time");

#[cfg(all(feature = "tick_u64", not(feature = "tick_u128")))]
pub type Uint = u64;

#[cfg(all(feature = "tick_u128", not(feature = "tick_u64")))]
pub type Uint = u128;

#[cfg(not(any(feature = "tick_u64", feature = "tick_u128")))]
pub type Uint = u64;
