mod producer_cosumer;
mod ringbuf;


use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncRead, AsyncWrite};
use crate::herrors::HError;
use crate::fpsc::producer_cosumer::{ ConsumerBuf, ProducerBuf};
use crate::fpsc::ringbuf::Ringbuf;


pub fn new <'a,C>(capacity: usize) -> (ProducerBuf<'a>, ConsumerBuf<C>)
    where C: FnMut(&mut [u8])-> Result<(), HError> + Unpin,
{
    let (writer, reader) = Ringbuf::new(capacity);
    let consumer = ConsumerBuf::new(reader);
    let producer = ProducerBuf::new(writer);
    (producer, consumer)
}

///reset the buffer, so we can reuse the consumer and producer.
///We can't change the capacity of the buffer, if you want a new capacity of it,
///just create a new pair with fpsc::new().
#[inline]
pub fn reuse_buf_from_consumer<C> (consumer: &mut ConsumerBuf<C>)
    where C: FnMut(&mut [u8])-> Result<(), HError> + Unpin,
{
    consumer.ringbuf.reuse();
}
#[inline]
pub fn produce_to_producer(producer: &mut ProducerBuf) 
{
    producer.ringbuf.reuse();
}



mod tests {
    use super::*;
    use crate::fpsc::producer_cosumer::ConsumerBuf;
    use crate::fpsc;


    ///test all the basic functions of the consumer and producer. This is a example.
    #[tokio::test]
    async fn test_producer_consumer_all() {
        //This is a real  example of using the producer and consumer. It will produce 10 pieces of data, 
        //and then consume them.
        //create a producer and a consumer, and set the task of the consumer to print the data.
        let (mut producer, mut consumer) = 
            fpsc::new(10);
        let data = vec![1u8; 21];
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
 
}
