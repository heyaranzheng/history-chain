use keyring::Entry;
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

    ///a helper function to transform a public key to a pem string
    pub fn public_key_to_pem(&self) -> Result<pem::Pem, HError> {
        //get the public key and secret key as pem format
        pem::parse(self.public_key_to_bytes())
            .map_err(|e|
                HError::Identity { message: format!("parse public key error: {}", e) }
            )
    }
    
    fn  save_secret_key_to_system(&self) -> Result<(), HError> {

        //get the public key as pem format string
        let public_pem = self.public_key_to_pem().to_string();

        //get the secret key as pem format string
        let secret_pem;
        match &self.secret_key {
            Some(secret_key) => {
                secret_pem = pem::parse(secret_key.to_bytes())
                .map_err(|e|
                    HError::Identity { message: 
                        format!("parse secret key error in save_secret_key: {}", e) 
                    }
                )?
                .to_string();
            }
            None => return Err(HError::Identity {message: "No secret key".to_string()})   
        }

        //create a new entry in the keyring 
        let entry = Entry::new(
            "history-chain-secret-key", &public_pem
        ).map_err(|e| 
            HError::Identity {message: format!("create entry error in save_secret_key: {}", e)}
        )?;

        //save the secret key in the keyring
        entry.set_password(&secret_pem)
            .map_err(|e| 
                HError::Identity {message: format!("set password error in save_secret_key: {}", e)}
            )?;
        
        Ok(())
    }
    
    ///load the identity from the keyring
    pub fn load_secret_key(&mut self,node_name: &[u8; 32]) -> Result<Self, HError> {
        let public_pem = self.public_key_to_pem()?.to_string();

        //create a new entry in the keyring
        let entry = Entry::new(
            "history-chain-secret-key", &public_pem
        ).map_err(|e| 
            HError::Identity {message: format!("create entry error in load_secret_key: {}", e)}
        )?;

        //get the secret key from the keyring
        let secret_pem = entry.get_password()
            .map_err(|e| 
                HError::Identity {
                    message: format!("get password error in load_secret_key: {}", e)
                }
            )?;
        //parse the secret key from pem format
        let secret_key = pem::parse(secret_pem)
            .map_err(|e|
                HError::Identity { message: format!("parse secret key error in load_secret_key: {}", e) }
            )?;
        let secret_key_bytes: &[u8;32] = secret_key.contents().into();
        let signingkey = SigningKey::from_bytes(secret_key_bytes)
            .map_err(|e|
                HError::Identity { message: format!("from bytes error in load_secret_key: {}", e) }
            )?;




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