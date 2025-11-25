
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
        -> Result<(), HError> 
    {
        let (work_request,mut  work_receiver) 
            = req_resp::create_channel::<Payload>(capacity);
        tokio::spawn(
            async move {
                let request = work_receiver.recv_work().await;
                if let Some(req) = request {
                    let payload = req.data.clone();
                    let result = self.handle(payload);

                    req.send_back(result);
                    Ok(())
                }
            }
        );
        Ok(())   
    }

}

impl Handler for PayloadHandler {
    fn handle(&self, payload: Payload) -> Result<Payload,HError> {   
        //get the payload type
        let payload_type = PayloadTypes::from_payload(payload);

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


