use tokio::io::{AsyncWrite, AsyncRead, AsyncWriteExt, AsyncReadExt};
use std::task::{Poll, Context, Waker};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};


///this is a ring buffer for single producer and single consumer
pub struct Inner {
    pub buf: Vec<u8>,
    state: BufState,
    //this counter is to record the number of references to the this inner,
    //The drop will exactly execute only when the counter is 1,
    //The default value is 1, and it will be incresed when the Inner is cloned.
    counter: AtomicUsize,
    w_waker: Option<Waker>,
    r_waker: Option<Waker>,
}
#[derive(Clone, PartialEq, Eq)]
pub enum BufState {
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

pub struct BufPtr {
    pub inner: *mut Inner,
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
pub struct Ringbuf{
    pub buf: BufPtr,
}

impl  Ringbuf
{
    //crate a new Ringbuf with the given capacity, return a tuple of (writer, reader)
    pub fn new(capacity: usize) -> (Self, Self) {
        let inner = Inner::new(capacity);

        let writer =Self {
            // leak the inner to the heap, so we can use in mutiple tasks.
            buf: BufPtr { inner: Box::leak(Box::new(inner)) as *mut Inner},
        };
        let reader = writer.clone();
        (writer, reader)
    }
    //return  the inner buffer as a mutable reference of vector<u8>
    #[inline]
    pub fn get_mut_vec_buf(&mut self) -> &mut Vec<u8> {
        unsafe {
            &mut (*self.buf.inner).buf
        }
    }
    //clone the inner buffer's state, return a BufState
    #[inline]
    pub fn clone_buf_state(&self) -> BufState {
        unsafe {
            (*self.buf.inner).state.clone()
        }
    }
    //set the inner buffer's state
    #[inline]
    pub fn set_buf_state(&mut self, state: BufState) {
        unsafe {
            (*self.buf.inner).state = state;
        }
    }
    //wake the writer
    #[inline]
    pub fn wake_writer(&mut self) {
        unsafe {
            if let Some(waker) = (*self.buf.inner).w_waker.take() {
                waker.wake();
            }
        }
    }
    //wake the reader
    pub fn wake_reader(&mut self) {
        unsafe {
            if let Some(waker) = (*self.buf.inner).r_waker.take() {
                waker.wake();
            }
        }
    }
    //save writer's waker
    #[inline]
    pub fn writer_waker_save(&mut self, writer_waker: Waker) {
        unsafe {
            (*self.buf.inner).w_waker = Some(writer_waker);
        }
    }
    //save reader's waker
    #[inline]
    pub fn reader_waker_save(&mut self, reader_waker: Waker) {
        unsafe {
            (*self.buf.inner).r_waker = Some(reader_waker);
        }
    }
    #[inline]
    pub fn capacity(&self) -> usize {
        unsafe {
            (*self.buf.inner).buf.capacity()
        }
    }
    #[inline]
    pub fn len(&self) -> usize {
        unsafe {
            (*self.buf.inner).buf.len()
        }
    }
    ///clear the inner's buffer, and change the state to Writable.
    #[inline]
    pub fn reuse(&mut self) {
        let vec_buf = self.get_mut_vec_buf();
        vec_buf.clear();
        self.set_buf_state(BufState::Writable);
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
        let save_state = this.clone_buf_state();
        let  vec_buf = this.get_mut_vec_buf();
        match save_state {
            BufState::Writable => {
                //caculate the number of bytes we can write.
                //because we can only read after a write, and can only write after last writen data is read,
                //so every time we write from the beginning of the buffer, not the last write position.
                let nwrite = vec_buf.capacity().min(buf.len());
                
                vec_buf.clear();
                vec_buf.extend_from_slice(&buf[..nwrite]);

            
                //if this is the last write, change the state to WriteFinished
                //NOTE: 
                //   We can't leave a 0 byte for the next write, because when we use 
                //   while n < data.len() {
                //       n += writer.write(&data[n..]).await?;
                //   }
                // we can't return 0, the reader will wait forever.
                // The state WriteFinished only can be setted in the write function.
                // For example, data.len() == 10, writer's buffer size is 10 too,
                // The timelines in  this loop are:
                //      1. writer write 10 bytes to the buffer, state changed to readable,
                //      2. wake the reader, reader read 10 bytes, state changed to writable,
                //      and reader wait for another write, or it will be blocked forever.!!!!
                //      3. n = 10, that means it will out of the while loop. 
                // That means we can't change the state to WriteFinished and wake the 
                // sleeping reader anymore. unless we add another 0 write at the end of the 
                // loop to change the state like this:
                //       while n < data.len() {
                //           n += writer.write(&data[n..]).await?;
                //       }
                //       writer.write(&[]).await?;
                // But this will make the code more complex.
                // So we just do a more check here: nwrite == buf.len(), to check if
                // We can write down all the data at this time. If so, we can change the state 
                //to WriteFinished.
                if  nwrite == buf.len() {
                    this.set_buf_state(BufState::WriteFinished);
                }else {
                    this.set_buf_state(BufState::Readable);
                }
                this.wake_reader();
                return Poll::Ready(Ok(nwrite));
            }
            BufState::Readable => {
                this.writer_waker_save(cx.waker().clone());
                Poll::Pending
            }
            BufState::WriteFinished => {
                //here, we must wake the reader, because the reader may be waiting for data,
                this.wake_reader();
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
        let save_state = this.clone_buf_state();
        let  vec_buf = this.get_mut_vec_buf();
        
        match save_state {
            BufState::Readable | BufState::WriteFinished => {
                //check if the reader can read all the data.
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
                //copy the data to the reader's buffer.
                buf.put_slice(&vec_buf[..nread]);

                //if we have read all the data, we don't need to notify the writer or store the r_waker.
                if save_state == BufState::WriteFinished {

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

                //change  the state to Writable, and wake the writer.
                this.set_buf_state(BufState::Writable);
                this.wake_writer();

                return Poll::Ready(Ok(()));
            }
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
    async fn test_ringbuf() {
        let (mut reader, mut writer) = Ringbuf::new(10);
        let data = vec![1u8; 21];

        let write_task = async move {
            let mut n = 0;
            while n < data.len() {
                n += writer.write(&data[n..]).await.unwrap();
            }

        };
        let mut read_buf = vec![0u8; 30];
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
        let data = vec![1u8; 20];

        let write_task = async move {
            writer.write_all(&data).await.unwrap();
        };
        let mut read_buf = vec![0u8; 10];
        let read_task = async move {
            while reader.read(&mut read_buf[..]).await.unwrap() > 0 {
            println!("data :{:?}", &read_buf);
            }
        };

        tokio::join!(write_task,read_task);
    }
}