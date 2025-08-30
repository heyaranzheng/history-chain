mod producer_cosumer;
mod ringbuf;


use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncRead, AsyncWrite};
use crate::herrors::HError;
use crate::fpsc::producer_cosumer::{ ConsumerBuf, ProducerBuf};
use crate::fpsc::ringbuf::Ringbuf;


pub fn new <C>(capacity: usize) -> (ProducerBuf, ConsumerBuf<C>) 
    where C: FnMut(&mut [u8])-> Result<(), HError> + Unpin,
{
    let (writer, reader) = Ringbuf::new(capacity);
    let consumer = ConsumerBuf::new(reader);
    let producer = ProducerBuf::new(writer);
    (producer, consumer)
}

///reset the buffer, so we can reuse the consumer and producer.
///We can't change the capacity of the buffer, if you want a new capacity
///just create a new one with fpsc::new().
#[inline]
pub fn reuse_buf_from_consumer<C> (consumer: &mut ConsumerBuf<C>)
    where C: FnMut(&mut [u8])-> Result<(), HError> + Unpin,
{
    consumer.ringbuf.reuse();
}
#[inline]
pub fn produce_to_producer(producer: &mut ProducerBuf) {
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
        let data = vec![1u8; 20];
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
    //this test will waste a lot of time, just mask it.
    //#[tokio::test]
    async fn test_throughput() {
        //create a fake data source , 600 MB;
        let data_size =   1024 * 1024 * 600 ;
        let data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();
        
        //start the timer.
        let start = std::time::Instant::now();

        //create a producer and a consumer, with a capacity of 1MB bytes.
        let (mut producer, mut consumer) = 
            fpsc::new( 1024 * 1024 );
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
        let data_size =   1024 * 1024 * 600 ;
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
