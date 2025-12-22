mod common;

#[cfg(feature = "auth")]
mod auth;
#[cfg(feature = "signing")]
mod signing;

#[cfg(feature = "auth")]
pub use auth::{authenticate, AuthValidationError};
#[cfg(feature = "signing")]
pub use signing::HeaderProvider;
