use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::herrors::HError;



/// A request containing data to be processed and a sender to return the response
pub struct Request<T> {
    /// One-shot sender to send the response back to the requester.
    sender: oneshot::Sender<T>,
    /// The data to be processed by the worker
    pub data: T,
}

impl <T> Request<T> {
    /// Create a new request with the given data
    /// Returns a tuple of (Request, Response) where Request is sent to worker and Response is used by requester
    fn new(data: T) -> (Self, Response<T>) {
        let (tx, rx) = oneshot::channel::<T>();
        (
            Self { sender: tx, data },
            Response {receiver: rx},
        )
    }

    /// Send a request to a worker and get a response object
    /// This is the main entry point for clients to send work to workers
    pub async fn send(data: T, worker: RequestWorker<T>) -> Result<Response<T>, HError>{
        let (request, response) = Request::new(data);
        worker.send_to_worker(request).await?;
        Ok(response)
    }

    /// -------------- this function may have a bug ------------------
    /// This is a way to send a response back to the requestor or the client, 
    /// if the server get the request from a client and wants to send a response data
    /// back to the client.
    pub fn send_back(self, data: T) -> Result<(), HError> {
        self.sender.send(data).map_err(|_| 
            HError::Message { message: format!("Request send back error" ).to_string()
            }
        )
    }

}

/// Response object that can be used to await the result from a worker
pub struct Response<T> {
    /// One-shot receiver to receive the response from the worker
    receiver: oneshot::Receiver<T>,
}

impl <T> Response<T> {
    /// Wait for and return the response from the worker
    pub async fn response(self) -> Result<T, HError> {
        self.receiver.await.map_err(|e| 
            HError::Message { message: format!("Response error:{:?}", e).to_string()
            }
        )
    }
}

/// Wrapper around MPSC sender for sending requests to workers
/// 
#[derive(Clone)]
pub struct RequestWorker<T> {
    /// The underlying MPSC sender
    pub sender: mpsc::Sender<Request<T>>,
}

impl <T> RequestWorker<T> {
    /// Internal method to send a request to the worker
    async fn send_to_worker(&self, req: Request<T>) -> Result<(), HError> {       
        self.sender.send(req).await.map_err(|e| 
            HError::Message { message: 
                format!("Request send to worker error:{:?}", e).to_string()
            }
        )
    }
}


/// Wrapper around MPSC receiver for receiving requests in workers
pub struct WorkReceiver<T> {
    /// The underlying MPSC receiver
    pub receiver: mpsc::Receiver<Request<T>>,
}

impl <T> WorkReceiver<T> {
    /// Create a new receiver wrapper
    fn new(receiver: mpsc::Receiver<Request<T>>) -> Self {
        Self { receiver }
    }

    /// Receive a work request from the channel
    pub async fn recv_work(&mut self) -> Option<Request<T>> {
        self.receiver.recv().await
    }

    /// receieve a work request from the channel, then extract the data from it
    pub async fn recv_data(&mut self) -> Result<T, HError> {
        let option = self.recv_work().await;
        if let Some(req) = option {
            Ok(req.data)
        }
        else {
            Err(
                HError::Message { 
                    message: format!("request handler recviece a invalid request")
                }
            )
        }
    }
}

///create a channle to communicate with other tasks
/// # Arguments
/// * `capacity` the queue's capacity of the channel
///# Example
/// ```
/// struct DataType;
/// let request_worker = spawn( async {
///     let (request_worker, work_reciever) = create_channel::<DataType>(1);
///     let data_result = work_reciever.recv_data().await;
///     match data_result {
///         Ok(value) => {
///             //do something
///         }
///         Err(e) => {
///             logger_error_with_error(&e);
///         }
///     }
/// });
/// //use request_worker to create a Requst
/// let data = DataType
/// let resp = Request::send(data, request_worker).await?;
/// .....
/// 
/// ```
/// 
pub fn create_channel<T>(capacity: usize) -> 
    (RequestWorker<T>, WorkReceiver<T>)
{
    let (sender, receiver) 
        = mpsc::channel::<Request<T>>(capacity);   
    (RequestWorker { sender }, WorkReceiver::new(receiver))
}

/// A worker that processes requests using a provided function
pub struct Worker<T> {
    /// Sender for sending new requests to this worker
    sender: RequestWorker<T>,
    /// Receiver for receiving requests to process
    receiver: WorkReceiver<T>,
    /// The function used to process incoming requests
    task: fn(T) -> Result<T, HError>,
}

impl <T> Worker<T> 
    where T: Send + 'static,
{
    /// Create a new worker with the specified capacity and processing function
    /// 
    /// # Arguments
    /// * `capacity` - The buffer size of the internal channel
    /// * `task` - Function that will process the requests
    pub fn new(
        capacity: usize,
        task: fn(T)-> Result<T, HError>,
    ) -> Self  {
        let (sender, receiver) = mpsc::channel::<Request<T>>(capacity);
        let sender = RequestWorker { sender };
        let receiver = WorkReceiver::new(receiver);
        
        Self {
            sender,
            receiver,
            task: task,
        }
    }

    /// Run the worker loop, processing requests until cancelled
    /// 
    /// This spawns a background task that continuously receives requests,
    /// processes them with the provided task function, and sends back responses.
    /// 
    /// # Arguments
    /// * `canc_token` - Token used to signal cancellation of the worker
    pub async fn run(mut self, canc_token: CancellationToken ) -> Result<(),HError> {
        // Spawn the actual worker loop as a separate task
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Receive a request to process
                    request = self.receiver.recv_work() => {
                        match request {
                            Some(req) => {
                                // Process the request with our task function
                                let res = (self.task)(req.data);
                                
                                // If processing failed, skip sending response
                                if !res.is_ok() {
                                    continue;
                                }
                                
                                // Send the result back to the requester
                                let response = res.unwrap();
                                let _ = req.sender.send(response)
                                    .map_err(|_| {
                                        // Log error if we couldn't send response
                                        HError::Message { 
                                            message: format!("Response send error").to_string()
                                        }
                                    });
                            }
                            // No request available, continue loop
                            None => {
                                continue;
                            }
                        }
                    }

                    // Check for cancellation signal
                    _ = canc_token.cancelled() => {
                        // Exit the loop when cancelled
                        break;
                    }
                }
            }
        });
        
        // Return immediately since the actual work is done in the spawned task
        Ok(())
    }
}

#[cfg(test)]
mod test{
    use super::*;
    use tokio::sync::{oneshot, mpsc};

    /// Test basic functionality of the worker system
    /// Creates a worker, sends a request, and verifies the response
    #[tokio::test(flavor = "multi_thread")]
    async fn test_request_worker() {
        // Simple task function that adds 1 to input
        let task = |x: i32| {
            Ok(x + 1)
        };
        
        // Create worker with the task
        let worker = Worker::new(10, task);
        let request_worker = worker.sender.clone();
        let canc_token = CancellationToken::new();     

        // Start the worker running
        let _ = worker.run(canc_token.clone()).await;

        // Send a request and wait for response
        let response = Request::send(1, request_worker.clone()).await.unwrap();
        let result = response.response().await.unwrap();

        
        // Verify we got the expected result (1 + 1 = 2)
        assert_eq!(result, 2);

        // Send another request and wait for response (3 + 1 = 4)
        let response2 = 
            Request::send(3, request_worker.clone()).await.unwrap();
        let result2 = response2.response().await.unwrap();
        assert_eq!(result2, 4);

        // Cancel the worker's task, then send another request, we will
        // get an error
        canc_token.cancel();
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        let response3 = 
            Request::send(5, request_worker.clone()).await;
        assert_eq!(response3.is_ok(), false);
        
    }
}
