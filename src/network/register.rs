
use std::sync::Arc;
use async_trait::async_trait;
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;
use std::pin::Pin;

use crate::herrors;
use crate::herrors::HError;
use crate::network::{Message, Payload};
use crate::req_resp::{self, Request, RequestWorker, Response, Worker, WorkReceiver};

///this enum is used to describe the payload type, 
///The enum Payload we use in the network, may carry different types of data.
///In order to avoid dealing with data, we use this enum to describe the payload type.
///We can get the payloadtype from the Payload by using the 
/// PayloadTypes::from_payload() method.
/// # Example:
///     let payload_type = PayloadTypes::from_payload(&payload);
/// 
/// # Note:
///     * The PayloadTypes is used to classify the handlers for different payloads.
///     * So the Payloadtypes may NOT have a one-to-one correspondence with the enum Payload.
#[derive(Eq, Hash, PartialEq, Debug, Clone)]
pub enum PayloadTypes {
    Introduce,
    Empty,
    Others,
}

impl PayloadTypes {
    ///return a PayloadTypes from a Payload
    pub fn from_payload(payload: &Payload) -> Self {
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

/// This trait is used to define the interface of a async handler,
/// which save the handlers to deal with different kinds of payloads in the Message
/// type.
#[async_trait]
pub trait AsyncHandler{
    async fn handle(
        &self,
        payload: Payload,
    ) -> Result<Payload, HError>;

    /// spawn a task to handle a network message once, if it is ok, then send it
    /// to a signing task to sign the message.
    /// # Arguments
    /// * `msg_addr` -it's a (Message, SocketAddr) tuple, the address is the sender's
    /// address in the network.
    /// * `sign_worker` -it's the RequestWorker of the signning task. we can construct a 
    /// request to the signing task with it.
    async fn spawn_handle(
        &self,
        msg_addr: (Message, SocketAddr),
        sign_worker: RequestWorker<(Message, SocketAddr)>,
    ) -> Result<(), HError>;

    /// register a async handler for a specific payload type.
    fn reg_async <F, Fut>(&mut self, payload_type: PayloadTypes, async_handler: F) 
        -> Result<(), HError>
    where F: Fn(Payload) -> Fut + Send + Sync + 'static,
          Fut: Future<Output = Result<Payload, HError>> + Send + Sync + 'static;
    
    /// spawn a task to deal with the payload by async_handlers.
    /// The task will listen the requests created with the request_worker.
    /// 
    /// This function will consume the AsyncHandler, so we can't add more async_handlers
    /// after we spawning the task.
    /// # Arguments
    /// * `capacity` - the capacity of the channel used to communicate with the task.
    /// * `canc_token` - the cancellation token used to stop the task.
    /// # Returns
    /// * `RequestWorker<Payload>` - the request_worker used to create and send requests 
    /// to the task. 
    /// 
    /// # Example
    ///     //create a new task to handle the payload. 
    ///     let request_worker = self.spawn_run(100, cancellation_token).await?;  
    ///     // use the request_worker and your payload data to create a request and send 
    ///     //the it to the task we spawned.
    ///     let resp = Request::send(payload, request_worker)?; 
    ///     // get the response from the dealing task.
    ///     let payload = resp.recv().await?; 
    /// 
    /// the client will get the payload from the dealing task's response.
    async fn spawn_run(
        self,
        capacity: usize,
        canc_token: CancellationToken,
    ) -> Result<RequestWorker<Payload>, HError>;
}




/// a register for async handlers.
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

// 实现 AsyncPayloadHandler 结构体
impl AsyncPayloadHandler {
    pub fn new() -> Self {
        Self {
            register: AsyncRegister::new(),
        }
    }
}

#[async_trait]
impl AsyncHandler for AsyncPayloadHandler {

    ///This function will find out the relevent handler to handle the payload
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
    
    /// handle a message then send it to signing task
    async fn spawn_handle(
        &self,
        msg_addr: (Message, SocketAddr),
        sign_worker: RequestWorker<(Message, SocketAddr)>,
    ) -> Result<(), HError> 
    {
        let msg = msg_addr.0;
        let payload = msg.payload.clone();
        let addr = msg_addr.1;
        
        //Note:
        //The new sender should be setted with the old receiver, the 
        //new receiver should be setted with the old sender.
        let new_payload = self.handle(payload).await?;
        let new_msg = Message::new(msg.receiver, msg.sender, new_payload);
        Request::send((new_msg, addr), sign_worker).await?;
        Ok(())

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

    /// spawn a new task to handle the payload in some network messages.
    /// the task will not end until we have got a CancellationToken.
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
                        //listen the requests
                        request = request_reciver.recv_work() => {
                            //if get a request, get the payload, call the handler,
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
                        //listen the cancellation token. 
                        //if get a cancellation token, break the loop.
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

#[async_trait]
pub trait SyncHandler {
    fn handle(&self, payload: Payload) -> Result<Payload, HError>;
    fn reg(
        &mut self, 
        payload_type: PayloadTypes, 
        handler:fn(Payload) -> Result<Payload,HError>
    ) -> Result<(),HError>;

    async fn spawn_run(
        self,
        capacity: usize,
        canc_token: CancellationToken,
    )-> Result<RequestWorker<Payload>, HError>;
    
}


/// the hadnler to deal with payload in network
/// This handler only use sync handler to deal with the payloads.
/// If you wwant to use an async handler, you should use the AsyncPayloadHandler.
pub struct PayloadHandler {
    register: Register,
}

impl PayloadHandler {
    fn new() -> Self {
        Self {
            register: Register::new(),
        }
    }

}

#[async_trait]
impl SyncHandler for PayloadHandler {

    #[inline]
    fn reg(
        &mut self, 
        payload_type: PayloadTypes, 
        handler:fn(Payload) -> Result<Payload,HError>
    ) -> Result<(),HError> 
    {
        self.register.reg(payload_type, handler)
    }

    /// spawn a task to deal with the payload with sync handler.
    /// 
    async fn spawn_run(self,capacity: usize, canc_token: CancellationToken) 
        -> Result<RequestWorker<Payload>, HError> 
    {
        //create a new request worker for client to create new requests.
        //create a new reciver for new task (the handler task) to listen the requests.
        let (work_request,mut  work_receiver) 
            = req_resp::create_channel::<Payload>(capacity);

        //spawn a task to handle the payload
        tokio::spawn(
            async move {
                //listen the requests until get a cancellation token.
                loop {
                    tokio::select! {
                        //listen the requests
                        request = work_receiver.recv_work() => {
                            //if get a request, get the payload, call the handler,
                            //then send the response back.
                            if let Some(req) = request {
                                let payload = req.data.clone();
                                let result = self.handle(payload);

                                //send the response back to the client who send this request.
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
                        _ = canc_token.cancelled() => {
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
            handler.spawn_run(1, canc_token.clone()).await;
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

