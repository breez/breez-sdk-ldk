mod common;

#[cfg(feature = "auth")]
mod auth;
#[cfg(feature = "signing")]
mod signing;

#[cfg(feature = "auth")]
pub use auth::{authenticate, AuthenticationFailed};
#[cfg(feature = "signing")]
pub use signing::HeaderProvider;
