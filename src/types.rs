//! Unsigned integer type alias for rate limiter capacities and ticks.
//!
//! This module defines `Uint` as the integer type used for all rate limiter token counts and time ticks.
//! The exact type can be switched at compile time using feature flags.

/// Alias for the basic unsigned integer type used for capacities and ticks.
///
/// This type is selected at compile time using feature flags:
/// - `tick_u64` (default): uses [`u64`]
/// - `tick_u128`: uses [`u128`]
#[cfg(all(feature = "tick_u64", feature = "tick_u128"))]
compile_error!("You cannot enable both `tick_u64` and `tick_u128` features at the same time");

#[cfg(feature = "tick_u64")]
pub type Uint = u64;

#[cfg(feature = "tick_u128")]
pub type Uint = u128;
