use crate::AuthError;

/// Unified password hashing interface — implement this to plug in any algorithm.
pub trait PasswordHasher: Send + Sync + 'static {
    fn hash(&self, password: &str) -> Result<String, AuthError>;
    fn verify(&self, password: &str, hash: &str) -> Result<bool, AuthError>;
}

// ── Argon2 ────────────────────────────────────────────────────────────────────

#[cfg(feature = "argon2")]
pub use argon2_impl::Argon2Hasher;

#[cfg(feature = "argon2")]
mod argon2_impl {
    use argon2::{
        password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher as _, PasswordVerifier, SaltString},
        Argon2,
    };
    use crate::{AuthError, PasswordHasher};

    /// Argon2id hasher — recommended default. More memory-hard than bcrypt.
    #[derive(Default, Clone)]
    pub struct Argon2Hasher;

    impl PasswordHasher for Argon2Hasher {
        fn hash(&self, password: &str) -> Result<String, AuthError> {
            let salt = SaltString::generate(&mut OsRng);
            Argon2::default()
                .hash_password(password.as_bytes(), &salt)
                .map(|h| h.to_string())
                .map_err(|e| AuthError::HashError(e.to_string()))
        }

        fn verify(&self, password: &str, hash: &str) -> Result<bool, AuthError> {
            let parsed = PasswordHash::new(hash)
                .map_err(|e| AuthError::HashError(e.to_string()))?;
            Ok(Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
        }
    }
}

// ── Bcrypt ────────────────────────────────────────────────────────────────────

#[cfg(feature = "bcrypt")]
pub use bcrypt_impl::BcryptHasher;

#[cfg(feature = "bcrypt")]
mod bcrypt_impl {
    use crate::{AuthError, PasswordHasher};

    /// Bcrypt hasher. `cost` is the work factor (4–31, default 12).
    #[derive(Clone)]
    pub struct BcryptHasher {
        pub cost: u32,
    }

    impl Default for BcryptHasher {
        fn default() -> Self {
            Self { cost: 12 }
        }
    }

    impl PasswordHasher for BcryptHasher {
        fn hash(&self, password: &str) -> Result<String, AuthError> {
            bcrypt::hash(password, self.cost)
                .map_err(|e| AuthError::HashError(e.to_string()))
        }

        fn verify(&self, password: &str, hash: &str) -> Result<bool, AuthError> {
            bcrypt::verify(password, hash)
                .map_err(|e| AuthError::HashError(e.to_string()))
        }
    }
}
