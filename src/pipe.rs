use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::herrors::HError;
use crate::herrors::PipeError;

pub struct Pipe<T> {
    pub sender: Sender<T>,
    pub receiver: Receiver<T>,
}

impl <T> Pipe<T> {
    //create a pipe with a given capacity
    pub fn new(capacity: usize) ->(Self, Self) {
        let (sender, receiver) = channel::<T>(capacity);
        let (sender2, receiver2) = channel(capacity);
        ( 
            Pipe { sender: sender, receiver: receiver2 },
            Pipe { sender: sender2, receiver: receiver }    
        )
    }
    pub async fn send(&mut self, value: T) -> Result<(), HError> {
        if self.sender.is_closed() {
            return Err(HError::Pipe(PipeError::Close))
        }
        self.sender.send(value).await.map_err(
            |e|  HError::Message { message: format!("Error sending value: {}", e) }
        )
    }

    pub async fn recv(&mut self) -> Result<T, HError> {
        if let Some(value) = self.receiver.recv().await {
            Ok(value)
        }else {
            Err(HError::Message { message: format!("Error in receiving value") })
        }
    }

    pub fn close(&mut self) {
        self.receiver.close();
    }

    pub async fn is_closed(&mut self) -> bool {
        self.receiver.is_closed()
    }

    pub  fn try_recv(&mut self) -> Result<T, HError> {
        self.receiver.try_recv().map_err(
            |e| 
            {
                match e {
                    tokio::sync::mpsc::error::TryRecvError::Empty => HError::Pipe(PipeError::Empty),
                    tokio::sync::mpsc::error::TryRecvError::Disconnected => HError::Pipe(PipeError::Close),
                }
            }
        )
    }
    pub fn try_send(&mut self, value: T) -> Result<(), HError> {
        if self.sender.is_closed() {
            return Err(HError::Pipe(PipeError::Close))
        }
        self.sender.try_send(value).map_err(
            |e|
            {
                match e {
                    tokio::sync::mpsc::error::TrySendError::Full(_) => HError::Pipe(PipeError::Full),
                    tokio::sync::mpsc::error::TrySendError::Closed(_) => HError::Pipe(PipeError::Close),
                }
            }
        )
    }
}