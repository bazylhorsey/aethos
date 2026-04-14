mod error;
mod password;
mod token;

pub use error::AuthError;
pub use password::PasswordHasher;
pub use token::{generate_token, hash_token};

#[cfg(feature = "argon2")]
pub use password::Argon2Hasher;

#[cfg(feature = "bcrypt")]
pub use password::BcryptHasher;
