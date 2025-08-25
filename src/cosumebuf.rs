///this is a ring buffer for single producer and single consumer

struct ConsumeBuf<'a,T> 
    where T: Default + Clone
{
    buf: &'a mut [T],
    head: usize,
    tail: usize,
    state: BufState,
}

enum BufState {
    Writable,
    Readable,
}

impl <'a, T> ConsumeBuf<'a, T>
    where T: Default + Clone
{
    pub fn new(capacity: usize) -> Self {
        //we create a buffer with a 'static lifetime, so we need to leak it firstly.
        let buf = vec![T::default(); capacity];
        let ptr = Box::leak(Box::new(buf)) as *mut Vec<T>;
        Self {
            buf: unsafe { std::slice::from_raw_parts_mut((*ptr).as_mut_ptr(), capacity)},
            head: 0,
            tail: 0,
            state: BufState::Writable,
        }  
    }
}

///because we have a 'static lifetime buf in ConsumeBuf, we need to implement the Drop
/// trait to free the memory of the buffer.
impl <'a, T> Drop for ConsumeBuf<'a, T> 
    where T: Default + Clone
{   
    fn drop(&mut self) {
        unsafe {
            //rebuild the Vec<T> from the raw pointer.
            //because the buf's legnth is fixed, so the Vec<T>'s capcity is equal to the buf's length.
            let length = self.buf.len();
            let vec = Vec::from_raw_parts(self.buf.as_mut_ptr(), length, length );
            drop(vec);
        } 
    }
}

mod tests {
    use super::*;
    #[test]
    fn test_drop() {
        let consume_buf = ConsumeBuf::<i32>::new(1);
        
        //add a value to the buffer
        consume_buf.buf[0] = 1;

        //this pointer is pointing to the same memory as consume_buf's buf.
        let double_ptr ;
        unsafe {
            //get the raw pointer of consume_buf's buf.
            //This is unsafe because we have two pointers to the same memory.
            double_ptr = consume_buf.buf.as_ptr() as *mut i32;

            //check the value of the buffer by the raw pointer. it should be 1.
            assert_eq!(*(double_ptr as *mut i32) , 1);

            //drop the consume_buf, this will free the memory of the buffer, if the drop
            //function is implemented correctly.
            drop(consume_buf);

            //we can't get the value of the consume_buf's buf after it's dropped.
            //So it must be a  random value, can't be 1.
            let value = *double_ptr;
            assert_eq!(value != 1, true);
        }
    }
}
