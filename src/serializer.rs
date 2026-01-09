
use tokio::io::{AsyncWrite, AsyncRead, AsyncWriteExt, AsyncReadExt};
use bincode::{Decode, Encode, error::EncodeError};
use async_trait::async_trait;
use std::marker::Unpin;

use crate::herrors::HError;


use helpers::*;
/// 128 kB buffer size, in function save_into_stream,
const  BUFFER_SIZE: usize = 1024 * 128;
const  BIGGEST_BUFFER_SIZE: usize = 1024 * BUFFER_SIZE;



#[async_trait]
pub trait Serialize
{
    type DataType: Encode + Decode<()> + Sized + Send + Unpin;
    /// have a buffer
    fn buffer(&mut self) -> &mut [u8];
    //have its data
    fn data(&mut self) -> Option<Self::DataType>;
    //resize the buffer
    fn resize_buffer(&mut self, size: usize) -> Result<(), HError>;


    ///encode into bytes with bigendian, just wrap the bincode::encode_into_slice 
    fn encode_into_slice(&mut self, dst: &mut [u8]) -> Result<usize, HError>{
        let config  = bincode::config::standard()
            .with_big_endian();

        let val_opt = self.data();

        match val_opt {
            Some(val) => {
                
                //try to encode the data into the buffer, if the buffer is not enough, resize it.
                let  mut size = self.buffer().len();
                loop {
                    match bincode::encode_into_slice(&val, dst, config) {
                        //the error means the buffer is not enough, resize it and try again.
                        Err(EncodeError::UnexpectedEnd) => {
                            size *= 2;

                            //check if the buffer is too big to use a lot of memory.
                            //if the size of buffer is over BIGGEST_BUFFER_SIZE, return an error
                            if size > BIGGEST_BUFFER_SIZE {
                                return Err(
                                    HError::Message { 
                                        message: format!("the buffer is 
                                        too big to use a lot of memory, 
                                        size: {}", size)
                                    }
                                )
                            }

                            //resize the buffer and try again.
                            self.resize_buffer(size)?;
                            continue;
                        }

                        //if we get an other error, break and return the error
                        Err(e) => {
                            return Err(
                                HError::Message { 
                                    message: 
                                        format!("error in serializer encode_into_slice {}" ,e)
                                }
                            )
                        }

                        //the buffer is enough, return the size of bytes
                        Ok(size) => {
                            return Ok(size);
                        }
                    }
                    
                }
            }

            // we have no data in the serializer, return an error
            None => {
                return Err(
                    HError::Message {
                        message: format!("error in serializer encode_into_slice
                            error: {}", "no data")
                    }
                );
            }
        }

    }

    ///decode from bytes with bigendian, just wrap the bincode::decode_from_slice 
    fn decode_from_slice(src: &[u8]) -> Result<Self::DataType, HError>
    {
        let config  = bincode::config::standard()
            .with_big_endian();
        let (val, _) = 
            bincode::decode_from_slice::<Self::DataType, _>(src, config)
            .map_err( |e| 
                HError::Message { message:  
                    format!("deserialize error: {}", e)
                }
            )?;
        Ok(val)
    }

    ///save it into a async stream, return the size of the serialized data
    async fn save_into_asyncwrite<W>(&mut self, stream: &mut W ) 
        -> Result<usize, HError> 
        where W: AsyncWrite + Unpin + Send
    {
        let config = bincode::config::standard()
            .with_big_endian();

        //get the data first
        let data = self.data();
        match data {  
            //if we can't get the data, return an error   
            None => {
                return Err(
                    HError::Message { message: 
                        format!("error in save_into_asyncwrite
                            error: {}", "no data")
                    }
                );
            }

            //if we have the data, enncode it into the buffer, and write 
            //it into the stream.
            Some(data) => {
                //encode the data into the buffer, and get the size of bytes
                let header_size: u32;
                let buffer = self.buffer();
                
                let total_size: usize;
                let bytes_encoded = 
                    bincode::encode_into_slice(data, &mut buffer[4..], config)
                    .map_err(|e|
                        HError::Message { message: 
                            format!("errror in encode_into_slice 
                                error: {}", e)
                        }
                    )?;
                header_size = bytes_encoded as u32;
                total_size = 4 + bytes_encoded;
                
                //add the header size to the buffer
                let header_bytes = header_size.to_be_bytes() as [u8;4];
                buffer[0..4].copy_from_slice(&header_bytes);

                //write the effective buffer into the stream
                stream.write_all(&buffer[..total_size]).await?;

                return Ok(total_size);
            }            
        }
    }

    ///get the header from the buffer.
    fn get_header(buffer: &[u8]) -> Result<usize, HError> {
        //check if the buffer is valid
        if buffer.len() < 4 {
            return Err(
                HError::Message { 
                    message: format!("the buffer is not valid")   
                }
            );
        }

        //try to get the header size        
        let mut header_bytes = [0u8;4];
        header_bytes.copy_from_slice(&buffer[0..4]);

        let size = u32::from_be_bytes(header_bytes) as usize;
        Ok(size)
    }

    ///fill the serializer's buffer with the bytes from the stream, if it is not
    /// enough, create a new temporary buffer to hold the data. 
    async fn decode_from_asyncread<R>(&mut self, stream: &mut R) -> Result<Vec<Self::DataType>, HError>
        where R: AsyncRead + Unpin + Send
    {
        let vec_ret = Vec::new::<Self::DataType>();
        //get the buffer of serializer
        let buffer = self.buffer();
        let buffer_size = buffer.len();
    
        //fill the buffer from the stream
        let nread = stream.read_exact(&mut buffer).await?;
        
        //check the nread
        match nread {
            //0 means we get nothing from the stream, return an empty vector
            0 => {
                return Ok(vec_ret);
            }

            //the buffer is filled, but the data may be imcomplete, so we need to 
            //create a new temporary buffer to hold the data.
            buffer_size => {
                let mut vec_buffer: Vec<Vce<u8>>;
                let new_buffer_size = buffer.len();
                loop {
                    //check if the buffer is too big to use a lot of memory.
                    if new_buffer_size > BIGGEST_BUFFER_SIZE {
                        new_buffer_size = BIGGEST_BUFFER_SIZE;
                    }else {
                        new_buffer_size *= 2;
                    }
                    
                    let new_buffer = vec![0u8; new_buffer_size];
                    let nread = stream.read_exact(&mut new_buffer).await?;

                    //check the result of read_exact
                    if nread == 0 {
                        //we have nothing to read from the stream, break and return the vec_buffer
                        break;
                    }else if nread ==  new_buffer_size {
                        //the buffer is filled, add it to the vec_buffer
                        vec_buffer.push(new_buffer);
                        //may the data is not complete, so we need to read more.
                        continue;
                    }else { 
                        //the data in the stream is complete, add it to the vec_buffer
                        vec_buffer.push(new_buffer[..nread].to_vec());
                        //break and return the vec_buffer   
                        break;
                    }
                }
                // all the data in the stream is complete, decode it one by one.
                
            }
             
        }
        
        
        Ok(vec_ret)
    }


    ///get the bytes and decode it one by one.
    /// all the value will be collected into a vector.
    async fn decode_all<R>(&mut self, stream: &mut R) -> Result<Vec<Self::DataType>, HError>
        where R: AsyncRead + Unpin + Send
    {
        //return vector
        let mut vec = Vec::new();

        //check the  length of reused buffer, if it is not enough, resize it,
        //then get the buffer.
        let buffer_size = estimate_serialized_size::<Self::DataType>();
        let own_buffer_size = self.buffer().len();
        if own_buffer_size < buffer_size {
            self.resize_buffer(buffer_size)?;
        }
        let buffer = self.buffer();

        //get the bytes until it is empty
        loop {
            //check if we get something
            let  result  = 
                Self::read_bytes_from_asyncread(stream, &mut buffer[..]).await;
            match result {
                Ok(nread) => {
                    //if we read something, decode it.
                    if nread != 0 {
                        let val = Self::decode_from_slice(&mut buffer[..nread])?;
                        vec.push(val);
                    }else {
                        //we get nothing from the stream, break
                        break;
                    }
                }
                Err(e) => {
                    //exit and return  the error
                    return Err(e);
                }
            }
        }
        
        Ok(vec)
    }

}

/// This is a serializer, we use this serializer to serialize data.
pub struct Serializer <T> 
    where T: Sized + Encode + Decode<()> + Send
{
    buffer: Vec<u8>,
    data: Option<T>
}

impl  <T> Serializer <T> 
    where T: Sized + Encode + Decode<()> + Send + Unpin 
{
    ///create a new serializer with or without data
    /// # Arguments
    /// * `data` - the data we want to serialize, if is optional,
    pub fn new(data: Option<T>) -> Self {
        let buffer_size  estimate_serialized_size::<T>();
        Serializer::<T> { 
            buffer: vec![0u8; buffer_size], 
            data: data
        }
    }


    ///set the data for the serializer, cosunme the serializer
    /// then return a new serializer with the data setted.
    pub fn set_data(mut self, data: T) -> Self {
        self.data = Some(data);
        self
    }

    ///return a reference of the data of the serializer if we have it.

    pub fn get_data(&self) -> Option<&T> {
        self.data.as_ref()
    }    

}

///impl Serialize for Serializer<T>  
/// # Note
/// * we impl this trait for Serializer<T>, so we can use the Serializer<T>
/// as a serializer.
impl <T> Serialize for Serializer<T>  
    where T: Sized + Encode + Decode<()> + Send + Unpin
{
    type DataType = T;
    
    ///get the buffer of the serializer
    fn buffer(&mut self) ->  &mut [u8] {
        self.buffer.as_mut_slice()
    }

    ///resize the buffer of the serializer
    fn resize_buffer(&mut self,size:usize) -> Result<(),HError> {
        self.buffer.resize(size, 0);
        Ok(())
    }

    ///get the data of the serializer
    fn data(&mut self) -> Option<T> {
        self.data.take()
    }
}


mod helpers{
    use super::*;


    ///give a size number, return a aligned size number.
    /// # Arguments
    /// * `size` - the size number we want to figure out the aligned size
    /// * `align` - the align number
    /// # Note
    /// * if we have a size 10, and we want to align it by 4, the aligned size
    /// should be 12.
    pub(super) fn size_to_align(size: usize, align: usize) -> usize {
        let quotient = size / align;
        let remainder = size % align;
        if remainder == 0 {
            return quotient * align;
        } else {
            return quotient * align + align;
        }
    }

    
}


#[cfg(test)]
mod tests{
    use tokio::io::AsyncSeekExt;

    use super::*;

    #[tokio::test]
    async fn test_serializer() {
        //create a new serializer with no data
        let test_ser= Serializer::<String>::new(None);
        let data = "i am a test string".to_string();

        //--------------test set_data()-------------------------------------
        let mut test_ser = test_ser.set_data(data.clone());
        assert_eq!(test_ser.get_data().unwrap(), "i am a test string");
        
        //--------------test decode_into_slice() decode_from_slice()----------
        let mut dst = vec![0u8; 100];
        let result = test_ser.encode_into_slice(&mut dst);
        assert_eq!(result.is_ok(), true);
        let size = result.unwrap();
        let result = 
            Serializer::<String>::decode_from_slice(&dst[..size]);
        assert_eq!(result.is_ok(), true);
        assert_eq!(result.unwrap(), "i am a test string".to_string());

       
        //create a file for test
        let result = tokio::fs::File::options()
            .write(true)
            .create(true)
            .read(true)
            .open("test.file")
            .await;
        assert_eq!(result.is_ok(), true);
        let mut stream = result.unwrap();

        //reset the data in the serializer
        test_ser = test_ser.set_data(data);

        //--------------test save_into_asyncwrite() 
        //--------------test read_from_asyncread()---------------------
        let result = 
            test_ser.save_into_asyncwrite(&mut stream).await;
        stream.flush().await.unwrap();
        assert_eq!(result.is_ok(), true);
        let size = result.unwrap();

        //set the seek position to the beginning of the file
        let result = stream.seek(tokio::io::SeekFrom::Start(0)).await;
        assert_eq!(result.is_ok(), true);

        let mut buffer = [0u8; 4];
        let result = stream.read_exact(&mut buffer).await;
        if result.is_err() {
            println!("error in read_exact: {}", result.unwrap_err());
        }
        let header_size = u32::from_be_bytes(buffer);
        assert_eq!(header_size, (size - 4) as u32);
        
        let result = 
            Serializer::<String>::decode_from_asyncread(&mut stream).await;
        assert_eq!(result.is_ok(), true);

        
        //remove the file
        let result = tokio::fs::remove_file("test.file").await;
        assert_eq!(result.is_ok(), true);



    }


}