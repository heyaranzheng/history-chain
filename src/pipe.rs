use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::herrors::HError;

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
            return Err(HError::Pipe(PipeError::Closed))
        }
        self.sender.send(value).await.map_err(
            |e|  HError::Message { message: format!("Error sending value: {}", e) }
        )
    }

    pub async fn recv(&mut self) -> Result<T, HError> {
        self.receiver.recv().await.map_err(
            |e| HError::Message { message: format!("Error receiving value: {}", e) }
        )   
    }

    pub fn close(mut self) {
        self.receiver.close();
    }

    pub async fn is_closed(&mut self) -> bool {
        
    }
}