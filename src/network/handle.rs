
use async_trait::async_trait;
use futures::channel::mpsc;

use crate::{herrors::HError, network::Message, nodes::NodeInfo};

#[async_trait]
pub trait HandlerTrait {
    async fn handle_introduce(&self, msg: Message, pipe: Pipe<Message>) -> Result<NodeInfo, HError>{

    }
}

pub struct Request<T> {
    pub sender: oneshot::Sender<T>,
    pub data: T,
}

impl Request<T> {
    pub fn new(data: T) -> Self {
        let (tx, rx) = oneshot::channel();
        Self { sender: tx, data }
    }    
}

pub struct RequestSender<T> {
    pub sender: mpsc::Sender<Request<T>>,
}

impl RequestSender<T> {
    pub fn channel(capacity: usize) -> (RequestReciver<T>, Self) {
        let (sender, request_receiver) = 
            mpsc::channel::<Request<T>>(capacity);
        (request_receiver, Self { sender })
    }
}

type RequestReciver<T> = mpsc::Receiver<Request<T>>;

#[async_trait]
pub trait Reciver{
    async fn recv() -> Result<Request<T>, HError>;
}
impl <T> Reciver for RequestReciver<T> {
    async fn recv() -> Result<Request<T>, HError> {
        self.rev().await
    }   
}


