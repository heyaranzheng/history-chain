
use ed25519_dalek::ed25519::signature::SignerMut;
use ed25519_dalek::{VerifyingKey, Signature, SigningKey, Verifier};
use rand::rngs::OsRng;

use crate::herrors::HError;
use crate::hash::HashValue;

///An identity is a public key and a secret key
pub struct Identity{
    pub public_key: VerifyingKey,
    secret_key: Option<SigningKey>,
}

impl Identity {
    fn create_keypair() -> (VerifyingKey, SigningKey) {
        let mut csprng = OsRng;
        let secret_key = SigningKey::generate(&mut csprng);

        let public_key = secret_key.verifying_key();
        (public_key, secret_key)
    }

    ///create a new identity
    pub fn new() -> Self {
        let (public_key, secret_key) = Self::create_keypair();
        Identity {
            public_key,
            secret_key: Some(secret_key),
        }
    }

    #[inline]
    ///sign a message using the secret key
    pub fn sign_msg(&mut self, message: &[u8]) -> Result<[u8; 64], HError> {
        if let Some(secret_key) = &mut self.secret_key {
            Ok( secret_key.sign(message).to_bytes() )
        }
        else {
            Err(HError::Identity {message: "No secret key".to_string()})
        }
    }
    
    ///transform public key to bytes
    #[inline]
    pub fn public_key_to_bytes(&self) -> [u8; 32] {
        self.public_key.to_bytes()
    }

    ///verify the signature 
    #[inline]
    pub fn verify_signature_bytes(data: &[u8], public_key: &[u8;32], signature: &[u8; 64]) -> Result<(), HError> {
        let signature = Signature::from_bytes(signature);
        let public_key = VerifyingKey::from_bytes(public_key).unwrap();
        public_key.verify(data, &signature)
            .map_err(|e| HError::Identity { message: {format!("verify signature error: {}", e)} })
    }
    

}

unsafe impl Send for Identity {}
unsafe impl Sync for Identity {}

///don't allow to clone the secret key
impl Clone for Identity {
    fn clone(&self) -> Self {
        Self {
            public_key: self.public_key.clone(),
            secret_key: None,
        }
    }
}


mod tests {
    use super::*;

    #[test]
    fn test_identity() -> Result<(), HError>{
        let mut identity = Identity::new();
        let message = b"hello world";
        let signature = identity.sign_msg(message).unwrap();
        Identity::verify_signature_bytes(message, &identity.public_key_to_bytes(), &signature)?;
        Ok(())
    }
}