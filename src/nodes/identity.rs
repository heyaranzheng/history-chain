use std::path::Path;

use ed25519_dalek::ed25519::signature::SignerMut;
use ed25519_dalek::{VerifyingKey, Signature, SigningKey, Verifier};
use rand::rngs::OsRng;
use tokio::sync::mpsc::{self, Sender, Receiver};
use tokio::sync::oneshot;

use crate::constants::MAX_UDP_MSG_SIZE;
use crate::herrors::HError;
use crate::hash::HashValue;
use crate::network::Message;


///request to sign a message
pub struct SignRequest {
    pub bytes_vec: Vec<u8>,
    pub response: oneshot::Sender<Result<[u8; 64], HError>>,
}






///we can use this handle to send sign requests to the signer
pub struct SignHandle {
    sender: Sender<SignRequest>,
}


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
    pub fn verify_signature_bytes(
        data: &[u8], 
        public_key: &[u8;32],
        signature: &[u8; 64]
        ) 
        -> Result<(), HError> 
    {
        let signature = Signature::from_bytes(signature);
        let public_key = VerifyingKey::from_bytes(public_key)
                .map_err(|e| 
                    HError::Identity { 
                        message: {format!("verify signature error: {}", e)} 
                    }
                )?;
        public_key.verify(data, &signature)
            .map_err(|e| HError::Identity { message: {format!("verify signature error: {}", e)} })
    }

    ///initialize a singer for the identity
    pub async fn init_singer(self) -> Result<SignHandle, HError>{ 
        let (tx, mut rx) = mpsc::channel::<SignRequest>(64);
        let tx_clone = tx.clone();

        tokio::task::spawn_blocking( move || {
            let mut id = self;
            while let Some(requset) = rx.blocking_recv() {
                let signature = id.sign_msg(&requset.bytes_vec.as_slice());
                let _ = requset.response.send(signature);
            }
        });

        Ok(SignHandle { sender: tx_clone })
        
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
    use std::thread::spawn;

    use super::*;

    #[test]
    fn test_identity() -> Result<(), HError>{
        let mut identity = Identity::new();
        let message = b"hello world";
        let signature = identity.sign_msg(message).unwrap();
        Identity::verify_signature_bytes(message, &identity.public_key_to_bytes(), &signature)?;
        Ok(())
    }


    #[tokio::test(flavor = "multi_thread")]
    async fn  test_singer() {
        let id = Identity::new();
        let public_key = id.public_key_to_bytes();

        let handle = id.init_singer().await.unwrap();
        let handle_clone = SignHandle { sender: handle.sender.clone() };
        let msg = b"hello world";
        let signature = sgin_v2_test(msg, handle).await.unwrap();
  
        let result =Identity::verify_signature_bytes(
            msg, 
            &public_key, 
            &signature);
        assert_eq!(result.is_ok(), true);

       
        spawn(async move ||  {
                let msg2 = b"hello world, wow!";
            let signature2 = sgin_v2_test(msg, handle_clone).await.unwrap();
    
            let result =Identity::verify_signature_bytes(
                msg2, 
                &public_key, 
                &signature2);
            assert_eq!(result.is_ok(), true);
            }

        );
    } 
    pub async fn sgin_v2_test(bytes: &[u8], handle: SignHandle) -> Result<[u8; 64], HError> {
        let (tx,  rx) = 
            oneshot::channel::<Result<[u8; 64], HError>>();
        let request = SignRequest {
            bytes_vec: bytes.to_vec(),
            response: tx,
        };
        let _ = handle.sender.send(request).await.unwrap();
        let signature = rx.await.unwrap().unwrap();
        Ok(signature)
        
    }

}