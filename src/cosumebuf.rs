use tokio::io::{AsyncWrite, AsyncRead};
use std::task::{Poll, Context, Waker};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::herrors::HError;

///this is a ring buffer for single producer and single consumer


struct Inner {
    buf: Vec<u8>,
    state: BufState,
    //this counter is to record the number of references to the this inner,
    //The drop will execute only when the counter is 1,
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
                
                //This means all the data has been written, we need to notify the reader.
                if nwrite == 0 {
                    unsafe {
                        (*this.buf.inner).state = BufState::WriteFinished;
                    }
                    return Poll::Ready(Ok(0));
                }
                vec_buf.clear();
                vec_buf.extend_from_slice(&buf[..nwrite]);

                //wake the reader if it is waiting for data.
                unsafe {
                    (*this.buf.inner).state = BufState::Readable;
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


mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    #[tokio::test]
    async fn test_ringbuf() {
        let (mut reader, mut writer) = Ringbuf::new(4);
        let data = vec![1u8; 21];

        let write_task = async move {
            let mut n = 0;
            while n < data.len() {
                n += writer.write(&data[n..]).await.unwrap();
            }
        };
        let mut read_buf = vec![0u8; 31];
        let read_task = async move {
            while reader.read(&mut read_buf[..]).await.unwrap() > 0 {
                println!("read data: {:?}", read_buf);
            }
        };

        tokio::join!(write_task, read_task);


  
      
    }    
}