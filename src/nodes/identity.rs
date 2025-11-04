use base64::Engine;
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
    
    ///save the secret key to the keyring
    fn  save_secret_key_to_system(&self) -> Result<(), HError> {

        //get the public key and secret key as pem format string
        let public_key_bytes = self.public_key_to_bytes();        
        let secret_key_bytes = match &self.secret_key {
            Some(secret_key_ref) => {
                secret_key_ref.to_bytes()
            },
            None => {
                return Err(HError::Identity {message: "No secret key".to_string()})
            }
        };

        //encode the public key and secret key as base64 string
        let engine = base64::prelude::BASE64_URL_SAFE_NO_PAD;
        let public_key_str = engine.encode(public_key_bytes);
        let secret_key_str = engine.encode(secret_key_bytes);

        //create a new entry in the keyring 
        let entry = Entry::new(
            "history-chain-secret-key", &public_key_str
        ).map_err(|e| 
            HError::Identity {message: format!("create entry error in save_secret_key: {}", e)}
        )?;

        //save the secret key in the keyring
        entry.set_password(&secret_key_str)
            .map_err(|e| 
                HError::Identity {message: format!("set password error in save_secret_key: {}", e)}
            )?;
        let string = entry.get_password().unwrap();
        println!("secret key: {}", string);
        Ok(())
    }
    
    ///load the identity from the keyring
    pub fn load_secret_key_from_system(&mut self) -> Result<(), HError> {


        //create a base64 engine to decode and encode
        let engine = base64::prelude::BASE64_URL_SAFE_NO_PAD;

        //public_key_str is usesr name get the secret key from the keyring
        let public_key_bytes = self.public_key_to_bytes();
        let public_key_str = engine.encode(public_key_bytes);
        
        //create a new entry in the keyring
        let entry = Entry::new(
            "history-chain-secret-key", &public_key_str
        ).map_err(|e| 
            HError::Identity {message: format!("create entry error in load_secret_key: {}", e)}
        )?;

        //get the secret key from the keyring
        let secret = entry.get_password()
            .map_err(|e| 
                HError::Identity {
                    message: format!("get password error in load_secret_key: {}", e)
                }
            )?;
     
        //decode the secret key from base64 string
        let secret_key_vec = engine.decode(secret)
            .map_err(|e|
                HError::Identity { message: format!("decode secret key error in load_secret_key: {}", e) }
            )?;
        //check the length of the base64 result
        if secret_key_vec.len() != 32 {
            return Err(
                HError::Identity { 
                    message: format!("secret key length error in load_secret_key: {}", 
                        secret_key_vec.len()) 
                }
            );
        }

        //create a array to copy the data from the base64 result
        let mut secret_key_array = [0u8; 32];
        secret_key_array.copy_from_slice(&secret_key_vec[..]);
        let secret_key = SigningKey::from_bytes(&secret_key_array);

        //keep the secret key in the identity
        self.secret_key = Some(secret_key);
        
        Ok(())
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

    
    #[test]
    fn test_save_load_identity() -> Result<(), HError>{
        let mut identity = Identity::new();
        let save_result = identity.save_secret_key_to_system();
        assert_eq!(save_result.is_ok(), true);
        let load_result = identity.load_secret_key_from_system();
        eprintln!("load_result: {:?}", load_result);
        assert_eq!(load_result.is_ok(), true);
        Ok(())
    }
}