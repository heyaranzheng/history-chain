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

unsafe impl Send for SignRequest {}
unsafe impl Sync for SignRequest {}



#[derive(Clone)]
pub struct SignHandle {
    public_key_bytes: [u8; 32],
    sender: Sender<SignRequest>,
}

impl SignHandle {
    //create a new thread to handle the signing affairs
    pub async fn new(id: Identity) -> Result<Self, HError> {
        //this function init_signer() just create a new thread to handle the signing
        //so the await will NOT block the main thread
        SignHandle::init_signer(id).await
    }

    ///initialize a singer for the identity this
    async fn init_signer(mut id: Identity) -> Result<SignHandle, HError>{ 
        let (tx, mut rx) = 
            mpsc::channel::<SignRequest>(64);
        let tx_clone = tx.clone();

        let public_key_bytes = id.public_key_to_bytes();
        tokio::task::spawn_blocking( move || {
            while let Some(requset) = rx.blocking_recv() {
                let signature = id.sign_msg(&requset.bytes_vec.as_slice());
                let _ = requset.response.send(signature);
            }
        });

        Ok(SignHandle {
            public_key_bytes,
            sender: tx_clone 
        })
        
    }
    pub async fn send(&self, sign_request: SignRequest) -> Result<(), HError>
    {
        self.sender.send(sign_request).await
            .map_err(|e| 
                HError::Identity { message : 
                    format!("error in function send, SignHadnle in identity :
                {}", e)
                }
            )
    }

    ///return the public key of [u8;32]
    pub fn public_key_bytes(&self) -> [u8;32]
    {
        self.public_key_bytes.clone()
    }

    ///sign a message using handler, return the signature as [u8;64]
    pub async fn sign(&self, bytes: &[u8]) -> 
        Result<[u8; 64], HError>
    {
        //create a sign request
        let (tx,  rx) = 
            oneshot::channel::<Result<[u8; 64], HError>>();
        let request = SignRequest {
            bytes_vec: bytes.to_vec(),
            response: tx,
        };

        //send the request to the signer
        self.send(request).await?;

        //wait  for the response
        let signature_result = 
            rx.await.map_err(|e|
                HError::Identity { message : 
                    format!("error in function sign_message, SignHadnle in identity :
                {}", e)
                }
            )?;
        signature_result
    }
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

#[cfg(test)]
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
        let handle = SignHandle::new(id).await.unwrap();
        let public_key = handle.public_key_bytes();

        let handle_clone = handle.clone();
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

    #[tokio::test(flavor = "multi_thread")]
    async fn test_sgin_handle_sign() {
        let id = Identity::new();
        let handle = SignHandle::new(id).await.unwrap();
        let public_key = handle.public_key_bytes();

        let msg = b"hello world";
        let signature = handle.sign(msg).await.unwrap();
        let result =Identity::verify_signature_bytes(
            msg, 
            &public_key, 
            &signature);
        assert_eq!(result.is_ok(), true);
    }

}