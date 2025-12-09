use std::path::Path;

use ed25519_dalek::ed25519::signature::SignerMut;
use ed25519_dalek::{VerifyingKey, Signature, SigningKey, Verifier};
use rand::rngs::OsRng;
use tokio::sync::mpsc::{self, Sender, Receiver};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::constants::MAX_UDP_MSG_SIZE;
use crate::herrors::{self, HError};
use crate::hash::HashValue;
use crate::network::Message;


///request to sign a message
pub struct SignRequest {
    pub bytes_vec: Vec<u8>,
    pub response: oneshot::Sender<Result<[u8; 64], HError>>,
}
impl SignRequest {
    ///create a new sign request
    ///# Arguments
    /// * `bytes_vec` - the bytes to sign 
    /// * `response` - the oneshot channel to send the result

    fn new(bytes_vec: Vec<u8>, response: oneshot::Sender<Result<[u8; 64], HError>>) -> Self {
        SignRequest {
            bytes_vec,
            response,
        }
    }

}



unsafe impl Send for SignRequest {}
unsafe impl Sync for SignRequest {}



#[derive(Clone)]
pub struct SignHandle {
    public_key_bytes: [u8; 32],
    sender: Sender<SignRequest>,
    }

impl SignHandle {
    ///create a new task to handle the signing requests with a given identity
    /// - This function will cusume the id, and start a new task to deal 
    /// with the sign requests.
    /// # Arguments
    /// * `identity` - the identity to create the handle with
    /// * `capacity` - the capacity of the signer task's channel, 
    /// * `cancel_token` - the cancellation token to cancel the signer task
    /// return a Result<SignHandle, HError>
    /// 
    /// # Example
    /// ```
    /// //create a new identity if you don't have one
    /// let id = Identity::new();
    /// //create a new task to handle the Signing requests, this will cusume the id,
    /// //with a 64 capacity channel and a cancellation token.
    /// let handle = Signhandle::spawn_new(id, 32, cancel_token).await?;
    /// //then you can use the handle to sign messages
    /// let signature = handle.sign(b"hello world").await?;
    /// ````
    async fn spawn_new(mut identity: Identity, capacity: usize, cancel_token: CancellationToken) -> 
        Result<SignHandle, HError> {
        let (tx, mut rx) = 
            mpsc::channel::<SignRequest>(capacity);
        let tx_clone = tx.clone();

        let public_key_bytes = identity.public_key_to_bytes();

        //spawn a task and listen to the requests and cancellation token
        tokio::task::spawn(
            async move {
                loop {
                    tokio::select! {
                        //there is a cancellation token, break the loop 
                        _ = cancel_token.cancelled() => {
                            break;
                        }
                        //get some sign_request from the channel
                        Some(request) = rx.recv() => {
                            let sig_result = 
                                identity.sign_msg(&request.bytes_vec.as_slice());
                            //check if the request is valid
                            match sig_result {
                                Err(e) => {
                                    herrors::logger_error_with_error(&e);
                                    continue;
                                }
                                Ok(sig) => {
                                    //send the signature to the requester
                                    let _ = request.response.send(Ok(sig));
                                }
                            }
                        }
                    }
                }
            }
        );


        //don't wait for the task to start, return the handle.
        Ok(SignHandle {
            public_key_bytes,
            sender: tx_clone 
        })
        

    }



    ///create a new thread to handle the signing affairs
    /// # Note:
    ///  Use Identity::spawn_new() instead of this function.
    async fn new(id: Identity) -> Result<Self, HError> {
        //this function init_signer() just create a new thread to handle the signing
        //so the await will NOT block the main thread
        SignHandle::init_signer(id).await
    }

    ///initialize a singer for the identity this
    /// # Note:
    ///  Use Identity::spawn_new() instead of this function.
    async fn init_signer(mut id: Identity) -> Result<SignHandle, HError>{ 
        let (tx, mut rx) = 
            mpsc::channel::<SignRequest>(64);
        let tx_clone = tx.clone();

        let public_key_bytes = id.public_key_to_bytes();
        //spawn a thread to handle the signing requests
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
    ///send a sign_request to the signer
    /// # Arguments
    /// * `sign_request` - the sign_request to send
    /// return a Result<(), HError>
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
    ///This function will create a tokio::sync::oneshot channel to send and 
    /// receive the infomation between the signer and the requester.
    /// # Arguments
    /// * `bytes: &[u8]` - the bytes to sign
    /// return a Result<[u8; 64], HError>, [u8; 64] is the signature type.
    /// # Example
    /// ```
    /// //create a new identity if you don't have one
    /// let id = Identity::new();
    /// //create a new sign handle with your identity 
    /// //this will create a new task to deal with the sign requests
    /// let handle = SignHandle::new(id).await?;
    /// //sign a message
    /// let signature = handle.sign(b"hello world").await?
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


/// This is the identity of a node in the network, it contains a public key and a 
/// secret key. It is unique in the network. 
/// * The public_key is used as a node's name and verifications in the network.
/// * The secret_key is used to sign messages.
pub struct Identity{
    pub public_key: VerifyingKey,
    secret_key: Option<SigningKey>,
}

impl Identity {
    /// a helper function for Identity::new()
    fn create_keypair() -> (VerifyingKey, SigningKey) {
        let mut csprng = OsRng;
        let secret_key = SigningKey::generate(&mut csprng);

        let public_key = secret_key.verifying_key();
        (public_key, secret_key)
    }

    ///create a new identity.
    /// This is the only way to create a new identity
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

    /// verify the signature.
    /// If you want to verify a signature, all you need is 
    /// 1. the signated data, 2. the public key to verify, 3. the signature to verify.
    /// # Arguments
    /// * `data` - the data to verify
    /// * `public_key` - the public key to verify
    /// * `signature` - the signature to verify
    /// return a Result<(), HError>
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

    #[tokio::test(flavor = "multi_thread")]
    async fn test_spawn_new() {
        let id = Identity::new();
        let cancel_token = CancellationToken::new();

        let handle = 
            SignHandle::spawn_new(id, 32, cancel_token).await.unwrap();
        
        let msg = b"hello world";
        let signature = handle.sign(msg).await.unwrap();

        let msg2 = b"hello world, wow!";
        let signature2 = handle.sign(msg2).await.unwrap();

        let public_key = handle.public_key_bytes();
        let result = 
            Identity::verify_signature_bytes(msg, &public_key, &signature);
        assert_eq!(result.is_ok(), true);

        let result2 = 
            Identity::verify_signature_bytes(msg2, &public_key, &signature2);
        assert_eq!(result2.is_ok(), true);
        
    }

}