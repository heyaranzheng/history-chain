use tokio::io::{AsyncWrite, AsyncRead, AsyncWriteExt, AsyncReadExt};
use std::task::{Poll, Context, Waker};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::future::Future;

use crate::herrors::HError;

///this is a ring buffer for single producer and single consumer


struct Inner {
    buf: Vec<u8>,
    state: BufState,
    //this counter is to record the number of references to the this inner,
    //The drop will exactly execute only when the counter is 1,
    //The default value is 1, and it will be incresed when the Inner is cloned.
    counter: AtomicUsize,
    w_waker: Option<Waker>,
    r_waker: Option<Waker>,
}
#[derive(Clone, PartialEq, Eq)]
enum BufState {
    Writable,
    Readable,
    WriteFinished,
}

impl  Inner
{
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
            state: BufState::Writable,
            counter: AtomicUsize::new(1),
            w_waker: None,
            r_waker: None,
        
        }  
    }
}

struct BufPtr {
    inner: *mut Inner,
}

unsafe impl Send for BufPtr {}
unsafe impl Sync for BufPtr {}

//This stuct is to avoid the orphan rule.
impl  Clone for BufPtr{
    fn clone(&self) -> Self {
        unsafe {
            (*(*self).inner).counter.fetch_add(1, Ordering::Acquire);
            Self {
                //it is leagle for raw pointer has implemented Copy trait
                inner: (*self).inner,
            }
        }
    }
}


#[derive(Clone)]
struct Ringbuf{
    buf: BufPtr,
}

impl  Ringbuf
{
    pub fn new(capacity: usize) -> (Self, Self) {
        let inner = Inner::new(capacity);

        let writer =Self {
            // leak the inner to the heap, so we can use in mutiple tasks.
            buf: BufPtr { inner: Box::leak(Box::new(inner)) as *mut Inner},
        };
        let reader = writer.clone();
        (writer, reader)
    }
}

//we will clone the Ringbuf if we use it in multiple tasks, so we will have serveral pointers pointed to 
//the same inner, so we need to make sure the inner is not dropped until all the pointers are dropped.
impl  Drop for Ringbuf
{
    fn drop(&mut self) {
        let inner_ptr  = self.buf.inner;
        unsafe {
            //this is the last pointer to the inner, we need to drop it.
            //fetch_sub will return the previous value of the counter, then decrese it by 1.
            if  (*inner_ptr).counter.fetch_sub(1,Ordering::Release) == 1 {
                std::sync::atomic::fence(Ordering::Acquire);
                drop(Box::from_raw(inner_ptr) as Box<Inner>);
            }
        }
    }
}   

impl  AsyncWrite for Ringbuf 
{
    fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize, std::io::Error>> {
        let this = self.get_mut();
        let save_state = unsafe { (*this.buf.inner).state.clone() } ;
        let  vec_buf = unsafe {
            &mut (*this.buf.inner).buf
        };
        match save_state {
            BufState::Writable => {
                //caculate the number of bytes we can write.
                //because we can only read after a write, and can only write after all data is read,
                //so every time we write from the beginning of the buffer, not the last write position.
                let nwrite = vec_buf.capacity().min(buf.len());
                
                vec_buf.clear();
                vec_buf.extend_from_slice(&buf[..nwrite]);

                unsafe {
                    //if this is the last write, change the state to WriteFinished
                    if nwrite < vec_buf.capacity() {
                        (*this.buf.inner).state = BufState::WriteFinished;
                    }else {
                        (*this.buf.inner).state = BufState::Readable;
                    }
                    if let Some(waker) = (*this.buf.inner).r_waker.take() {
                        waker.wake();
                    }
                }
                return Poll::Ready(Ok(nwrite));
            }
            BufState::Readable => {
                unsafe {
                    (*this.buf.inner).w_waker = Some(cx.waker().clone());
                }
                Poll::Pending
            }
            BufState::WriteFinished => {
                //here, we must wake the reader, because the reader may be waiting for data,
                unsafe {
                    if let Some(waker) = (*this.buf.inner).r_waker.take() {
                        waker.wake();
                    }
                }
                return Poll::Ready(Ok(0));
            }
        }

    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        return Poll::Ready(Ok(()));
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        return Poll::Ready(Ok(()));
    }
}

impl AsyncRead for Ringbuf {
    fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let save_state = unsafe { (*this.buf.inner).state.clone() } ;
        let  vec_buf = unsafe {
            &mut (*this.buf.inner).buf
        };
        match save_state {
            BufState::Readable | BufState::WriteFinished => {
                let total = vec_buf.len();
     
                let nread = total.min(buf.capacity());
                if nread < total {
                    return Poll::Ready(
                        Err(
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                format!("your buffer to store the data is too small, 
                                need {} bytes, but only {} bytes are available", total, nread)
                            )
                        )
                    );
                }
                buf.put_slice(&vec_buf[..nread]);

                //if we have read all the data, we don't need to notify the writer or store the r_waker.
                if save_state == BufState::WriteFinished {
                    //rectify the writer if it is waiting for a wake up.
                    unsafe{ 
                        if let Some(waker) = (*this.buf.inner).w_waker.take() {
                            waker.wake();
                        }
                    }

                    //we may have data in the inner's buffer, so if we use a while loop use like this:
                    // while reader.read(&mut read_buf[..]).await.unwrap() > 0 {
                    //}
                    //when the last write is not 0, there are still have data in the inner's buffer, 
                    //so the tokio::io::ReadBuf will not be empty, the read function will not return 0,
                    //even the last read has done. This means it will loop forever.
                    //So, we must clear the inner's buffer after the last read.
                    vec_buf.clear();
                    return Poll::Ready(Ok(()));
                }
                unsafe { 
                    (*this.buf.inner).state = BufState::Writable;
                    if let Some(waker) = (*this.buf.inner).w_waker.take() {
                        waker.wake();
                    }
                }
                return Poll::Ready(Ok(()));
            }
            BufState::Writable => {
                unsafe {
                    (*this.buf.inner).r_waker = Some(cx.waker().clone());
                }
                return Poll::Pending;
            }
        }
    }
}

///This is a wrapper of Ringbuf, it is used to provide a producer-consumer pattern.
pub struct ProducerBuf{
    ringbuf: Ringbuf,
}

impl ProducerBuf {
    pub fn new(ringbuf: Ringbuf) -> Self {
        Self { ringbuf }
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
        unsafe {(*self.ringbuf.buf.inner).buf.capacity() }
    }
}


pub struct ConsumerBuf<T>
    where T: FnMut(&mut [u8])-> Result<(), HError>,
        Self: Unpin
{
    ringbuf: Ringbuf,
    task: Option<T>,
}



impl <C> ConsumerBuf<C> 
    where C: FnMut(&mut [u8])-> Result<(), HError>,
        Self: Unpin
{
    pub fn new(capacity: usize) -> (ProducerBuf, Self) {
        let (writer, reader) = Ringbuf::new(capacity);
        let consumer = Self {
            ringbuf: reader,
            task: None,
        };
        let producer = ProducerBuf::new(writer);
        (producer, consumer)
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
        unsafe {(*self.ringbuf.buf.inner).buf.capacity() }
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
            if n == 0 || n < capacity {
                break;
            }
        }
        Ok(total)
    }
}

///The Future wrapper a number of bytes consumed by the closure. 
///Note: 
///     We CAN NOT use the number of bytes "0" ,which is wrapped in this Future, to 
///     judge whether all the data is consumed derectly. because the last write may not be "0".
///     The condition to judge whether all the data is consumed may such like this:
///     loop {
///         let pinned_consumer = Pin::new(&mut consumer);
///         let n = pinned_consumer.await.unwrap();
///         if n == 0 || n < consumer.capacity() {
///             break;
///         }
///      }
impl <T> Future for ConsumerBuf<T> 
    where T: FnMut(&mut [u8])-> Result<(), HError>,
        Self: Unpin
{
    type Output = Result<usize, HError>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let save_state = unsafe {
            (*this.ringbuf.buf.inner).state.clone()
        };
        let  vec_buf = unsafe {
            &mut (*this.ringbuf.buf.inner).buf
        };
        match save_state {
            BufState::Readable => {
                //if we have some tast to do, we will do it here.
                if let Some(task) = &mut this.task {
                    let result = task(& mut vec_buf[..]);
                    match result {
                        Ok(_) => {
                            //caculate the number of bytes we can use.
                            let nread = vec_buf.len();
                            unsafe {
                                (*this.ringbuf.buf.inner).state = BufState::Writable;
                                if let Some(waker) = (*this.ringbuf.buf.inner).w_waker.take() {
                                    waker.wake();
                                }
                            }
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
                unsafe {
                    (*this.ringbuf.buf.inner).state = BufState::Writable;
                    if let Some(waker) = (*this.ringbuf.buf.inner).w_waker.take() {
                        waker.wake();
                    }
                }
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
                unsafe {
                    (*this.ringbuf.buf.inner).r_waker = Some(cx.waker().clone());
                }
                return Poll::Pending;
            }
        }
    }
}


mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    #[tokio::test]
    async fn test_ringbuf() {
        let (mut reader, mut writer) = Ringbuf::new(10);
        let data = vec![1u8; 11];

        let write_task = async move {
            let mut n = 0;
            while n < data.len() {
                n += writer.write(&data[n..]).await.unwrap();
            }

        };
        let mut read_buf = vec![0u8; 31];
        let mut total = 0;
        let mut nread= 0;
   
        let read_task = async move {
            loop {
                nread = reader.read(&mut read_buf[total..]).await.unwrap();
                total += nread;
                if nread == 0 {
                    println!("data :{:?}", &read_buf[..total]);
                    break;
                }
            };
        };

        tokio::join!(write_task, read_task);
    }    
     
    #[tokio::test]
    async fn test_ringbuf_write_all() {
        let (mut reader, mut writer) = Ringbuf::new(10);
        let data = vec![1u8; 11];

        let write_task = async move {
            writer.write_all(&data).await.unwrap();
        };
        let mut read_buf = vec![0u8; 31];
        let read_task = async move {
            while reader.read(&mut read_buf[..]).await.unwrap() > 0 {
            println!("data :{:?}", &read_buf);
            }
        };

        tokio::join!(write_task,read_task);
    }

    #[tokio::test]
    async fn test_producer_consumer() {
        let (mut producer, mut consumer) = ConsumerBuf::with_closure(10, |data: &mut [u8]| {
            println!("data in consumer: {:?}", data);
            Ok(())
        });
        let data = vec![1u8; 11];

        let produce_task = async move {
            producer.produce_all(&data).await.unwrap();
        };

        let capacity = unsafe {(*consumer.ringbuf.buf.inner).buf.capacity()};
        let consume_task = async move {
            loop {
                let pinned_consumer = Pin::new(&mut consumer);
                let n = pinned_consumer.await.unwrap();
                if n == 0 || n < capacity {
                    break;
                }
            }

        };

        tokio::join!(produce_task, consume_task);
    }

    ///test all the basic functions of the consumer and producer. This is a example.
    #[tokio::test]
    async fn test_producer_consumer_all() {
        //This is a real  example of using the producer and consumer. It will produce 10 pieces of data, 
        //and then consume them.
        //create a producer and a consumer, and set the task of the consumer to print the data.
        let (mut producer, mut consumer) = ConsumerBuf::new(10);
        let data = vec![1u8; 11];
        let task = |data: &mut [u8]| {
            println!("data in consumer: {:?}", data);
            Ok(())   
        };
        //add the task to the executor of tokio.
        let produce_task = async move {
            producer.produce_all(&data).await.unwrap();
        };
        let cosumer_task = async move {
            consumer.task(task).consume_all().await.unwrap();
        };
        //run the tasks.
        tokio::join!(produce_task, cosumer_task);
    }

    #[tokio::test]
    async fn test_throughput() {
        //create a fake data source , 1 GB;
        let data_size =   1024 * 1024 * 600 + 1;
        let data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();
        
        //start the timer.
        let start = std::time::Instant::now();

        //create a producer and a consumer, with a capacity of 512KB bytes.
        let (mut producer, mut consumer) = 
            ConsumerBuf::new( 1024 * 1024 );
        let  a = 0;
        let task = |data: &mut [u8]| {
            //verify the data if it is correct.
            for (i, &byte) in data.iter().enumerate() {
                //do a mutiple arithmetic operation to simulate the data processing.
                let _ = a + (byte as i32 + i as i32 ) % 256; 
            }
            Ok(()) 
        };
        //add the task to the executor of tokio.
        let produce_task = async move {
            producer.produce_all(&data).await.unwrap();
        };
        let cosumer_task = async move {
            consumer.task(task).consume_all().await.unwrap();
        };

        tokio::join!(produce_task, cosumer_task);
        let duration = start.elapsed();
        println!("Test completed in {:?}", duration);
        println!("Throughput: {:.2} MB/s",
            data_size as f64 / (1024.0 * 1024.0) / duration.as_secs_f64());
         //create a fake data source , 600MB;
        let data_size =   1024 * 1024 * 600 + 1;
        let data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();
        
        let start_cmp = std::time::Instant::now();
        let a = 0;
        for i in 0..data_size {
            let _ = a + (data[i] as i32 + i as i32 ) % 256; 
        }
        let duration_cmp = start_cmp.elapsed();
        println!("Compare completed in {:?}", duration_cmp);
        println!("Compare Throughput: {:.2} MB/s",
            data_size as f64 / (1024.0 * 1024.0) / duration_cmp.as_secs_f64());


    }

}