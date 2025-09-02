use tokio::io::{AsyncWrite, AsyncRead, AsyncWriteExt, AsyncReadExt};
use std::task::{Poll, Context, Waker};
use std::pin::Pin;
use std::future::Future;
use tokio::io::ReadBuf;


use crate::herrors::HError;
use crate::fpsc::ringbuf::Ringbuf;
use crate::fpsc::ringbuf::BufState;

///This is a wrapper of Ringbuf, it is used to provide a producer-consumer pattern.
pub struct ProducerBuf <'a>
{
    pub ringbuf: Ringbuf,
    stream: Option<Box<dyn AsyncRead + Unpin + 'a>>,
}

impl <'a> ProducerBuf <'a>
{
    pub fn new(ringbuf: Ringbuf) -> Self {
        Self { ringbuf, stream: None}
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
    
    //add a stream to the producer
    pub fn stream<T>(&mut self, stream: T) -> &mut Self 
        where T: AsyncRead + Unpin + 'a,
    {
        self.stream = Some(Box::new(stream));
        self
    }

    ///copy the data from some stream to the inner buffer.
    pub async fn produce_from_stream<S> (&mut self, stream: S) 
        -> Result<usize, HError>
        where S: AsyncRead + Unpin + 'a 
    {
        //add this stream to the producer   
        self.stream(stream);

        let mut total = 0;
        loop {

            let pinned_producer = Pin::new(&mut (*self));
            let n = pinned_producer.await?;
            total += n;
            if n == 0 {
                break;
            }
        }
        Ok(total)
    }
    
    
}

unsafe impl <'a> Send for ProducerBuf<'a> {}

impl <'a> Future for ProducerBuf<'a>
{
    type Output = Result<usize, HError>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
       
        let save_state = this.ringbuf.clone_buf_state();

        match save_state {
            BufState::Writable => {
                //check if there is some stream to read data from.
                if let Some(stream) = &mut this.stream{
                    //get the mutable reference of the inner buffer
                    let vec_buf = this.ringbuf.get_mut_vec_buf();
                    
                    //create a ReadBuf from the inner buffer, with the capacity of the inner buffer.
                    let uninit_slice = unsafe {
                        let ptr = vec_buf.as_mut_ptr() as *mut std::mem::MaybeUninit<u8>;
                        std::slice::from_raw_parts_mut(ptr, vec_buf.capacity())
                    };
                    let mut read_buf = ReadBuf::uninit(uninit_slice);
                    
                    //there is a stream, try to read
                    let pinned_stream = Pin::new(stream.as_mut());
                    match pinned_stream.poll_read(cx, &mut read_buf) {
                        Poll::Pending => {
                            //the stream is not ready, so we just return Pending.
                            this.ringbuf.writer_waker_save(cx.waker().clone());
                            return Poll::Pending;
                        }
                        Poll::Ready(Ok(())) => {
                            //read some data from the stream, and stored it in the buffer already.

                            //get the number of bytes we get from the stream.
                            let filled_len = read_buf.filled().len();

                            //check if we get all the data from the stream.
                            if filled_len < read_buf.capacity() {
                                //the stream is empty, so we set the state to WriteFinished, and waker the writer.
                                
                                //Note: we have filled data into the inner buffer, but we don't update the
                                //legnth of the buffer. Update now.
                                unsafe {
                                    vec_buf.set_len(filled_len);
                                }   
                                this.ringbuf.set_buf_state(BufState::WriteFinished);
                                this.ringbuf.wake_reader();
                                let nwrite = this.ringbuf.len();
                                return Poll::Ready(Ok(nwrite));
                            }

                            //not get the all data from the stream, so we set the state to Readable, 
                            //and waker the reader.

                            //Note: we have filled data into the inner buffer, but we don't update the
                            //legnth of the buffer. Update now.
                            unsafe {
                                vec_buf.set_len(filled_len);
                            }

                            this.ringbuf.set_buf_state(BufState::Readable);
                            this.ringbuf.wake_reader();
                            let nwrite = this.ringbuf.len();
                            return Poll::Ready( Ok(nwrite));
                        }
                        Poll::Ready(Err(e)) => {
                            return Poll::Ready(Err(
                                HError::Message { message: format!("read stream error: {}\n", e) }
                            ));
                        }
                    }
                }else {
                    //there is no stream, so we just return Pending.
                    return Poll::Ready(Err(
                        HError::Message { message: format!("no stream to read\n") }
                    ))
                }

            }
            BufState::Readable => {
                //the buffer is readable, so we just return Pending.
                this.ringbuf.writer_waker_save(cx.waker().clone());
                return Poll::Pending;
            }
            BufState::WriteFinished | BufState::ReadFinished => {
                //write finished, so we just return Ready. end this task.
                return Poll::Ready(Ok(0));
            }
        }
    }
}


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

    pub fn with_closure<'a>(capacity: usize, closure: C) -> (ProducerBuf<'a>, Self) {
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
            if state == BufState::ReadFinished { 
                break;
            }
        }
        Ok(total)
    }
}

///The Future wrapper a number of bytes consumed by the closure. 
///Note: 
///     We CAN NOT use the number of bytes "0" ,which is wrapped in this Future, to 
///     judge whether all the data is consumed derectly. Unless that the first write is 0 byte
///     or the data produced  from a stream , there is no way to get a 0 byte from this wrapper Future.
impl <T> Future for ConsumerBuf<T> 
    where T: FnMut(&mut [u8])-> Result<(), HError>,
        Self: Unpin
{
    type Output = Result<usize, HError>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let save_state = this.clone_buf_state();
    
        match save_state {
            BufState::Readable => {
                //if we have some tast to do, we will do it here.
                if let Some(task) = &mut this.task {
                    let vec_buf = this.ringbuf.get_mut_vec_buf();
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
            BufState::WriteFinished | BufState::ReadFinished => {
                if save_state == BufState::ReadFinished {
                    //this means everything is consumed, we don't need to deal with the data any more.
                    return Poll::Ready(Ok(0));
                }
                
                //The state is BufState::WriteFinished, at here.

                //if we have some tast to do, we will do it here.
                if let Some(closure) = & mut this.task {
                    let vec_buf = this.ringbuf.get_mut_vec_buf();
                    let result =closure(& mut vec_buf[..]);
                    match result {
                        Ok(_) => {
                            let nread = vec_buf.len();
                            //set the state to ReadFinished, declare the comsume is finished.
                            this.set_buf_state(BufState::ReadFinished);
                            return Poll::Ready(Ok(nread));
                        }
                        Err(_) => {
                            //set the state to ReadFinished, declare the comsume is finished.
                            this.set_buf_state(BufState::ReadFinished);
                            return Poll::Ready(
                                Err(
                                    HError::Message { message: "error in consumer closure".to_string() }
                                )
                            );
                        }
                    }
                }
                //set the state to ReadFinished, declare the comsume is finished.
                this.set_buf_state(BufState::ReadFinished);
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
                if state == BufState::ReadFinished {
                    break;
                }
            }

        };

        tokio::join!(produce_task, consume_task);
    }

    #[tokio::test]
    async fn test_producer_consumer_stream() {
        let (mut producer, mut consumer) = 
            ConsumerBuf::with_closure(10, |data: &mut [u8]| {
                println!("data in consumer: {:?}", data);
                Ok(())
            });

        let data = vec![1u8; 11];
        let reader = std::io::Cursor::new(data.clone());
        let produce_task = async move {
            producer.produce_from_stream(reader).await.unwrap();
        };

        let consume_task = async move {
            consumer.consume_all().await.unwrap();  
        };
        tokio::join!(produce_task, consume_task);
    }

   

}