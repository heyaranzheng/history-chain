use tokio::io::{AsyncWrite, AsyncRead, AsyncWriteExt, AsyncReadExt};
use std::task::{Poll, Context, Waker};
use std::pin::Pin;
use std::future::Future;


use crate::herrors::HError;
use crate::fpsc::ringbuf::Ringbuf;
use crate::fpsc::ringbuf::BufState;

///This is a wrapper of Ringbuf, it is used to provide a producer-consumer pattern.
pub struct ProducerBuf 
{
    pub ringbuf: Ringbuf,
}

impl   ProducerBuf 
{
    pub fn new(ringbuf: Ringbuf) -> Self {
        Self { ringbuf, }
    }
    
    //warpper the method of write_all() as another public method named produce_all
    pub async fn produce_all(&mut self, data: &[u8]) -> Result<(), HError> {
        self.ringbuf.write_all(data).await?;
        Ok(())
    }
    //warpper the method of write() as another public method named produce(), return the
    //number of bytes written.
    pub async fn produce(&mut self, data: &[u8]) -> Result<usize, HError> {
        let n = self.ringbuf.write(data).await?;
        Ok(n)
    }
    ///return the capacity of the inner buffer
    pub fn capacity(&self) -> usize {
        unsafe {self.ringbuf.capacity() }
    }

    ///copy the data from some steam to the inner buffer.
    pub async fn produce_from_stream<S> (&mut self, stream: &mut S) 
        -> Result<usize, HError>
        where S: AsyncRead + Unpin
    {
        let vec_buf = self.ringbuf.get_mut_vec_buf();
        let nwrite = stream.read(&mut vec_buf[..]).await?;
        Ok(nwrite)
    }   
    
}
/* 
impl<T> Future for ProducerBuf<T> 
    where T: AsyncRead + Unpin
{
    type Output = Result<usize, HError>;
}
*/


pub struct ConsumerBuf<T>
    where T: FnMut(&mut [u8])-> Result<(), HError>,
        Self: Unpin
{
    pub ringbuf: Ringbuf,
    task: Option<T>,
}


impl <C> ConsumerBuf<C> 
    where C: FnMut(&mut [u8])-> Result<(), HError>,
        Self: Unpin
{
    pub fn new(reader: Ringbuf) -> Self {
        let consumer = Self {
            ringbuf: reader,
            task: None,
        };
        consumer
    }
    ///change the task or add a new task to the consumer.
    #[inline]
    pub fn task(&mut self, task: C) -> &mut Self {
        self.task = Some(task);
        self
    }

    pub fn with_closure(capacity: usize, closure: C) -> (ProducerBuf, Self) {
        let (writer, reader) = Ringbuf::new(capacity);
        let consumer = Self {
            ringbuf: reader,
            task: Some(closure),
        };
        let producer = ProducerBuf::new(writer);
        (producer, consumer)
    }
    
    ///return the capacity of the inner buffer
    pub fn capacity(&self) -> usize {
       self.ringbuf.capacity() 
    }

    //return the mutable reference of the inner buffer
    #[inline]
    fn get_mut_vec_buf(&mut self) -> &mut Vec<u8> {
        self.ringbuf.get_mut_vec_buf()
    }

    //return the state of the inner buffer
    #[inline]
    fn clone_buf_state(&self) -> BufState {
        self.ringbuf.clone_buf_state()
    }

    //set the state of the inner buffer
    #[inline]
    fn set_buf_state(&mut self, state: BufState) {
        self.ringbuf.set_buf_state(state)
    }

    //wake the writer
    #[inline]
    fn wake_writer(&mut self) {
        self.ringbuf.wake_writer()
    }

    //save reader's waker
    #[inline]
    fn reader_waker_save(&mut self, reader_waker: Waker) {
        self.ringbuf.reader_waker_save(reader_waker)
    }

  

    ///just use the data once, and return the number of bytes consumed.
    ///Note: 
    ///     There is a risk to casue a deadlock if the  producer write more than one time.
    ///     If'd better touse consume() and produce() at the same time, or consume_all() and
    ///     produce_all() instead. 
    pub async fn consume(&mut self) -> Result<usize, HError> {
        let pinned_consumer = Pin::new(self);
        let n = pinned_consumer.await?;
        Ok(n)
    }

    ///consume all the data in from the producer and return the number of bytes consumed.
    ///Note: 
    ///     There is a risk to casue a deadlock if the  producer write more than one time.
    ///     If'd better touse consume() and produce() at the same time, or consume_all() and
    ///     produce_all() instead.
    pub async fn consume_all(&mut self) -> Result<usize, HError> {
        let mut total = 0;
        let capacity = self.capacity();
        loop {
            let pinned_consumer = Pin::new(&mut (*self));
            let n = pinned_consumer.await?;
            total += n;

            //This is safe to use state to judge whether all the data is consumed, because we 
            //have only two tasks at the same time, and this one is waiting for the data.
            //It will block at  await point, unless the data is ready to consume. So, we don't
            //need to worry about the state changed to WriteFinished when we don't read all 
            //the data.
            let state = self.clone_buf_state();
            if state == BufState::WriteFinished { 
                break;
            }
        }
        Ok(total)
    }
}

///The Future wrapper a number of bytes consumed by the closure. 
///Note: 
///     We CAN NOT use the number of bytes "0" ,which is wrapped in this Future, to 
///     judge whether all the data is consumed derectly. Unless that the first write is 0 byte, there
///     is no way to get a 0 byte from this wrapper Future.
impl <T> Future for ConsumerBuf<T> 
    where T: FnMut(&mut [u8])-> Result<(), HError>,
        Self: Unpin
{
    type Output = Result<usize, HError>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let save_state = this.clone_buf_state();
        
        //use this unsafe code to avoid the borrow checker of "this" pointer.
        //Or we have to implement a Clone trait for task in the ConsumerBuf struct.
        let  vec_buf = unsafe {
            &mut (*this.ringbuf.buf.inner).buf
        };
        match save_state {
            BufState::Readable => {
                //if we have some tast to do, we will do it here.
                if let Some(task) = &mut this.task {
                    let task_result = task(& mut vec_buf[..]);
                    match task_result {
                        Ok(_) => {
                            //caculate the number of bytes we can use.
                            let nread = vec_buf.len();
                            this.set_buf_state(BufState::Writable);
                            this.wake_writer();
                           
                            return Poll::Ready(Ok(nread));
                        }
                        Err(_) => {
                            return Poll::Ready(
                                Err(
                                    HError::Message { message: "error in consumer closure".to_string() }
                                )
                            );
                        }
                    }
                }
                //the closure is None, so we just return Ready(Ok(()))
                this.set_buf_state(BufState::Writable);
                this.wake_writer();
               
                return Poll::Ready(Ok(0));
            }

            //the action of this state is almost like the state of Readable, but we don't need
            //to change the state and notify the writer.
            BufState::WriteFinished => {
                //if we have some tast to do, we will do it here.
                if let Some(closure) = & mut this.task {
                    let result =closure(& mut vec_buf[..]);
                    match result {
                        Ok(_) => {
                            return Poll::Ready(Ok(vec_buf.len()));
                        }
                        Err(_) => {
                            return Poll::Ready(
                                Err(
                                    HError::Message { message: "error in consumer closure".to_string() }
                                )
                            );
                        }
                    }
                }
                //the closure is None, so we just return Ready(Ok(()))
                return Poll::Ready(Ok(0));
            }
            //during this state, we can't do anything, so we just return Pending.
            BufState::Writable => {
                this.reader_waker_save(cx.waker().clone());
               
                return Poll::Pending;
            }
        }
    }
}



mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

  
 
    #[tokio::test]
    async fn test_producer_consumer() {
        let (mut producer, mut consumer) = 
            ConsumerBuf::with_closure(10, |data: &mut [u8]| {
                println!("data in consumer: {:?}", data);
                Ok(())
        });
        let data = vec![1u8; 11];

        let produce_task = async move {
            producer.produce_all(&data).await.unwrap();
        };

        let capacity = consumer.capacity();
        let consume_task = async move {
            loop {
                let pinned_consumer = Pin::new(&mut consumer);
                let _ = pinned_consumer.await.unwrap();
                //This is safe, because we have only two tasks, and this one is waiting for the data.
                //It will block here unless the data is ready to consume. So, we don't need to worry 
                //about the state changed to WriteFinished when we don't read all the data.
                let state = consumer.clone_buf_state();
                if state == BufState::WriteFinished {
                    break;
                }
            }

        };

        tokio::join!(produce_task, consume_task);
    }

   

}