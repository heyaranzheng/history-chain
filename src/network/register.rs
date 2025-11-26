
use std::collections::HashMap;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::herrors;
use crate::herrors::HError;
use crate::network::Payload;
use crate::req_resp::{self, Request, RequestWorker, Response, Worker, WrokReceiver};

///this enum is used to describe the payload type, 
///The enum Payload we use in the network, may carry different types of data.
///In order to avoid dealing with data, we use this enum to describe the payload type.
#[derive(Eq, Hash, PartialEq, Debug)]
enum PayloadTypes {
    Introduce,
    Empty,
    Others,
}

impl PayloadTypes {
    fn from_payload(payload: &Payload) -> Self {
        match payload {
            Payload::Introduce => PayloadTypes::Introduce,
            Payload::Empty => PayloadTypes::Empty,

            //----------------------------------------------------
            //for now we just ignor the other type of Payload in network.
            //because the two kinds of types above is easy to handle for testing.
            _ => PayloadTypes::Others,
        }
    }

}


///this is a register for handlers to deal with different payloads.
pub struct Register{
    handlers: HashMap<PayloadTypes, fn(Payload) -> Result<Payload,HError>>,
} 

impl Register {
    /// register a handler for a specific payload type.
    fn reg(
        &mut self,
        payload_type: PayloadTypes,
        handler: fn(Payload) -> Result<Payload,HError>,
    ) -> Result<(),HError>
    {
        // just call the handler
        self.handlers.insert(payload_type, handler);
        Ok(())
    }


    /// create a new register
    fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }
}


///a handler of payload
pub trait Handler {
    ///get payload type then call the realted handler to handle the payload
    fn handle(&self, payload: Payload) -> Result<Payload,HError>;
}    

///the hadnler to deal with payload in network
pub struct PayloadHandler {
    register: Register,
}

impl PayloadHandler {
    fn new() -> Self {
        Self {
            register: Register::new(),
        }
    }

    #[inline]
    pub fn reg(
        &mut self, 
        payload_type: PayloadTypes, 
        handler: fn(Payload) -> Result<Payload,HError>
    ) -> Result<(),HError> 
    {
        self.register.reg(payload_type, handler)
    }

    async fn spawn_handler(self,capacity: usize, canc_tokern: CancellationToken) 
        -> Result<RequestWorker<Payload>, HError> 
    {
        let (work_request,mut  work_receiver) 
            = req_resp::create_channel::<Payload>(capacity);
        tokio::spawn(
            async move {
                loop {
                    tokio::select! {
                        request = work_receiver.recv_work() => {
                            if let Some(req) = request {
                                let payload = req.data.clone();
                                let result = self.handle(payload);

                                match result {
                                    Ok(payload) => {
                                        req.send_back(payload);
                                    }
                                    Err(err) => {
                                        println!("error: {:?}", err);
                                    }
                                    
                                }
                                    
                            }else {
                                // ignore the message, continue the loop
                                continue;
                            }
                        }
                        _ = canc_tokern.cancelled() => {
                            //now, we  get a cancellation token, break the loop, end the task
                            println!("payload handler cancelled");
                            break;
                        }
                    }

                }
            }
        );
        Ok(work_request)   
    }

}

impl Handler for PayloadHandler {
    fn handle(&self, payload: Payload) -> Result<Payload,HError> {   
        //get the payload type
        let payload_type = PayloadTypes::from_payload(&payload);

        //the result of this function
        let result;

        //get the handler, then call it.
        if let Some(handler) = 
            self.register.handlers.get(&payload_type)
        {
            result = handler(payload)
        } else {
            return Err(HError::NetWork {
                message:  format!("No handler for payload type: {:?}", 
                    payload_type).to_string(),
            });
        }
        return result;
    }
}

mod tests {
    use std::thread::sleep;

    use super::*;

    use crate::herrors::HError;
    use crate::network::{Payload, Message};
    use crate::req_resp::{Request, Response, RequestWorker};


    #[tokio::test(flavor = "multi_thread")]
    async fn test_payload_handler() {
        // create a new payload handler
        let mut handler = PayloadHandler::new();

        //create a handler function
        let handle = 
            |payload| -> Result<Payload, HError> {
                println!("we got  payload");
                Ok(payload)
            };
        
        // register the handler, ignore the result
        let _ = handler.reg(PayloadTypes::Empty, handle);
        
        //create a canncellation token for the handler task.
        let canc_token = CancellationToken::new();

        // spawn the handler task, get the request_worker for communication
        let woker_result = 
            handler.spawn_handler(1, canc_token.clone()).await;
        assert_eq!(woker_result.is_ok(), true );
        let request_worker = woker_result.unwrap();

        // sleep for a while, make sure the handler task is running
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

        // make a request to the handler task
        let response_result = 
            Request::send(Payload::Empty, request_worker.clone()).await;
        assert_eq!(response_result.is_ok(), true);

        //wait for the response
        let response = response_result.unwrap();
        let payload_result = response.response().await;
        assert_eq!(payload_result.is_ok(), true);
        let payload = payload_result.unwrap();
        assert_eq!(payload, Payload::Empty);
    }

        
}

