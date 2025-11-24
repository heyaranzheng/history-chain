
use tokio::sync::{mpsc, oneshot};
use tokio:
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;


use crate::{herrors::HError, network::Message, nodes::NodeInfo};

#[async_trait]
pub trait HandlerTrait {
//    async fn handle_introduce(&self, msg: Message) -> Result<NodeInfo, HError>{
}

#[async_trait]
pub trait Client {
    async fn cli_send<T>(&self, data: T ) -> Result<(), HError>;
}

pub struct Request<T> {
    sender: oneshot::Sender<T>,
    data: T,
}

impl <T> Request<T> {
    fn new(data: T) -> (Self, Response<T>) {
        let (tx, rx) 
            = oneshot::channel::<T>();
        (
            Self { sender: tx, data },
            Response {receiver: rx},
        )
    }

    pub async fn send(data: T, worker: RequestWorker<T>) -> Result<Response<T>, HError>{
        let (request, response) = Request::new(data);
        worker.send_to_worker(request).await?;
        Ok(response)
    }
}


pub struct Response<T> {
    receiver: oneshot::Receiver<T>,
}

impl <T> Response<T> {
    pub async fn response(self) -> Result<T, HError> {
        self.receiver.await.map_err(|e| 
            HError::Message { message: format!("Response error:{:?}", e).to_string()
            }
        )
    }
}



#[derive(Clone)]
pub struct RequestWorker<T> {
    pub sender: mpsc::Sender<Request<T>>,
}

impl <T> RequestWorker<T> {
    async fn send_to_worker(&self, req: Request<T>) -> Result<(), HError> {       
        self.sender.send(req).await.map_err(|e| 
            HError::Message { message: 
                format!("Request send to worker error:{:?}", e).to_string()
            }
        )
    }


}



pub struct WrokReceiver<T> {
    pub receiver: mpsc::Receiver<Request<T>>,
}

impl <T> WrokReceiver<T> {
    fn new(receiver: mpsc::Receiver<Request<T>>) -> Self {
        Self { receiver }
    }

    pub async fn recv_work(&mut self) -> Option<Request<T>> {
        self.receiver.recv().await
    }
}

pub struct Worker<'worker, T> {
    sender: RequestWorker<T>,
    receiver: WrokReceiver<T>,
    task: Box<dyn Fn(T) -> Result<T, HError> + Send + 'worker>,
}

impl <'worker,T> Worker<'worker, T> {
    fn new< F: Fn(T) -> Result<T, HError> + Send + 'worker>(
        capacity: usize,
        task: F,
    ) -> Self  {
        let (sender, receiver) = 
            mpsc::channel::<Request<T>>(capacity);
        let sender = RequestWorker { sender };
        let receiver = WrokReceiver::new(receiver);
        Self {
            sender,
            receiver,
            task: Box::new(task),
        }
        
    }

    pub async fn run(&mut self, canc_token: CancellationToken ) -> Result<(),HError> {
        loop {
            tokio::select! {
                request = self.receiver.recv_work() => {
                    match request {
                        Some(req) => {
                            let response = self.task(req.data)?;
                            req.sender.send(response)?;
                        }
                        None => {
                            continue;
                        }
                    }
                }

                _ =canc_token.cancelled() => {
                    break;
                }
            }
        }
        Ok(())
    }
}




