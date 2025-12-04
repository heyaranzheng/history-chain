
use async_trait::async_trait;
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;
use std::pin::Pin;

use crate::herrors;
use crate::herrors::HError;
use crate::network::Payload;
use crate::req_resp::{self, Request, RequestWorker, Response, Worker, WrokReceiver};

///this enum is used to describe the payload type, 
///The enum Payload we use in the network, may carry different types of data.
///In order to avoid dealing with data, we use this enum to describe the payload type.
#[derive(Eq, Hash, PartialEq, Debug)]
pub enum PayloadTypes {
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

#[async_trait]
pub trait AsyncHandler{
    async fn handle(
        &self,
        payload: Payload,
    ) -> Result<Payload, HError>;

    fn reg_async <F, Fut>(&mut self, payload_type: PayloadTypes, async_handler: F) 
        -> Result<(), HError>
    where F: Fn(Payload) -> Fut + Send + Sync + 'static,
          Fut: Future<Output = Result<Payload, HError>> + Send + Sync + 'static;
    async fn spawn_run(
        self,
        capacity: usize,
        canc_token: CancellationToken,
    ) -> Result<RequestWorker<Payload>, HError>;
}

///a register for async handlers
pub struct AsyncRegister
{
    handlers: HashMap<
        PayloadTypes,
        Box<
            dyn Fn(Payload) -> Pin<Box<dyn Future<Output = Result<Payload, HError> > + Send >>
            + Send 
            + Sync
        >
    >,
}


impl AsyncRegister {
    fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }
}


pub struct AsyncPayloadHandler {
    register: AsyncRegister,  
}

impl AsyncPayloadHandler {
    pub fn new() -> Self {
        Self {
            register: AsyncRegister::new(),
        }
    }
}

#[async_trait]
impl AsyncHandler for AsyncPayloadHandler {
    async fn handle(&self, payload: Payload)
        -> Result<Payload, HError> 
    {
        let payload_type = PayloadTypes::from_payload(&payload);
        if let Some(handler) = 
            self.register.handlers.get(&payload_type)
        {
            //return the result
            handler(payload).await
        }else {
            // return error: NotFound
            Err(
                HError::Message{
                    message: format!("no handler for payload type")
                }
            )
        } 
    }

    fn reg_async <F, Fut>(&mut self, payload_type: PayloadTypes, async_handler: F)
        -> Result<(), HError>
    where F: Fn(Payload) -> Fut + Send + Sync + 'static,
          Fut: Future<Output = Result<Payload, HError>> + Send + Sync + 'static,
    {
        let box_pinned_handler = Box::new(
            move |payload: Payload| {
                Box::pin(async_handler(payload)) 
                as Pin<Box<dyn Future<Output = Result<Payload, HError>> + Send>>
        
            })
          as Box<
            dyn Fn(Payload) -> Pin<Box<dyn Future<Output = Result<Payload, HError> > + Send >>
            + Send 
            + Sync
        >; 
       
        self.register.handlers.insert(payload_type, box_pinned_handler);
        Ok(())
    }

    async fn spawn_run(
        self,
        capacity: usize,
        canc_token: CancellationToken,
    ) -> Result<RequestWorker<Payload>, HError> 
    {
        let (request_worker, mut request_reciver) 
            = req_resp::create_channel::<Payload>(capacity);
        
        //spawn a task to handle the payload
        tokio::spawn(
            async move {
                loop {
                    tokio::select! {
                        request = request_reciver.recv_work() => {
                            if let Some(req) = request {
                                let payload = req.data.clone();
                                let payload_result = self.handle(payload).await;
                                if let Ok(payload) = payload_result {
                                    let _ =req.send_back(payload);
                                }else {
                                    println!("error in handle payload");
                                }
                            }else {
                                println!("bad request");
                                continue;
                            }
                        }
                        _ = canc_token.cancelled() => {
                            println!("cancelled");
                            break;
                        }
                    }
                }
            }

        );
        Ok(request_worker)
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

pub struct HandlerContext {
    pub request_wokers: HashMap<PayloadTypes, RequestWorker<Payload>>,
}

impl HandlerContext {
    pub fn new() -> Self {
        Self {
            request_wokers: HashMap::new(),
        }
    }

    pub fn get_request_worker(
        &self,
        payload_type: PayloadTypes,
    ) -> Option<RequestWorker<Payload>> {
        self.request_wokers.get(&payload_type).cloned()
    }

    async fn send(
        &self, 
        payload: Payload,
    ) -> Result<Response<Payload>, HError>  
    {
        let payload_type = PayloadTypes::from_payload(&payload);
        if let Some(req_worker) = self.get_request_worker(payload_type)
        {
            Request::send(payload, req_worker).await
        }else {
            Err(
                HError::Message{
                    message: format!("no request worker for payload type")
                }
            )
        }

    }
    

}


///a handler of payload
pub trait Handler {
    ///get payload type then call the realted handler to handle the payload
    fn handle(&self, payload: Payload) -> Result<Payload, HError>;
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
        handler:fn(Payload) -> Result<Payload,HError>
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
                                        let _= req.send_back(payload);
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

    use crate::fpsc::new;
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

    

    #[tokio::test(flavor = "multi_thread")]
    async fn test_async_payload_handler() {
        //create a new payload handler
        let mut handler = AsyncPayloadHandler::new();
        let async_handle = 
            |payload| 
                async {
                tokio::time::sleep(std::time::Duration::from_micros(1000)).await;
                println!("we got  payload");
                Ok::<Payload, HError>(payload)
            }
            
        ;
        
                // register the handler, ignore the result
        let _ = handler.reg_async(PayloadTypes::Empty, async_handle);

        //create a canncellation token for the handler task.
        let canc_token = CancellationToken::new();

        // spawn the handler task, get the request_worker for communication
        let request_worker_result = 
            handler.spawn_run(1, canc_token).await;
        assert_eq!(request_worker_result.is_ok(), true);
        let request_worker = request_worker_result.unwrap();

        // sleep for a while, make sure the handler task is running
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

        // make a request to the handler task
        let result = 
            Request::send(Payload::Empty, request_worker.clone()).await;
        assert_eq!(result.is_ok(), true);
        let response = result.unwrap();
        
        //wait for the response
        let payload_result = response.response().await;
        assert_eq!(payload_result.is_ok(), true);
        let payload = payload_result.unwrap();
        assert_eq!(payload, Payload::Empty);


    }


        

        
}

