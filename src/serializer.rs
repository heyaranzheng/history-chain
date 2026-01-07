
use tokio::io::{AsyncWrite, AsyncRead, AsyncWriteExt, AsyncReadExt};
use bincode::{Decode, Encode};
use async_trait::async_trait;
use std::marker::Unpin;

use crate::herrors::HError;


use helpers::*;
/// 128 kB buffer size, in function save_into_stream,
const  BUFFER_SIZE: usize = 1024 * 128;



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
                let size = bincode::encode_into_slice(val, dst, config)
                    .map_err(|e|
                        HError::Message {
                            message: format!("error in serializer encode_into_slice
                                error: {}", e
                            )
                        }
                    )
                ?;
                Ok(size)
            }
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
    fn decode_from_slice(src: &[u8]) -> Result<Self::DataType, HError>{
        let config  = bincode::config::standard()
            .with_big_endian();
        let (val, _) = bincode::decode_from_slice::<Self::DataType, _>(src, config)
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

        //create a buffer with the estimated size
        let buffer_size = estimate_serialized_size::<Self::DataType>();

        //get the data first
        let data = self.data();

        //get the our own buffer's length, if it is not enough, resize it
        let my_buffer_size = self.buffer().len();
        if my_buffer_size < buffer_size {
            self.resize_buffer(buffer_size)?;
        }
        let buffer = self.buffer();
        
        //encode the data into the buffer, and get the size of bytes
        let header_size: u32;
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

    ///this funcion resolve a header, get out the fixed number of bytes according 
    /// the header from the stream WITHOUT DECODING
    async fn read_bytes_from_asyncread<R>(stream: &mut R, buffer: &mut [u8]) 
        -> Result<usize, HError> 
        where R: AsyncRead + Unpin + Send
    {
        //read the header size from the stream
        let mut header_bytes = [0u8; 4];

        //try to read some bytes from the stream.
        let nread = stream.read_exact(&mut header_bytes).await?;

        //if we can't get anything, we just return 0
        if nread == 0 {
            return Ok(0);
        }

        let header_size = u32::from_be_bytes(header_bytes);

        //check if the buffer is too small
        if buffer.len() < header_size as usize{
            return Err(
                HError::Message { 
                    message:  format!("the given buffer is too small 
                        for store the encode bytes in functiin read_from_
                        asyncread
                    ")
                }
            );
        }

        //get the bytes from the stream
        stream.read_exact(&mut buffer[..header_size as usize]).await?;

        return Ok(header_size as usize);
    }

    async fn decode_from_asyncread<R>(stream: &mut R) -> Result<Self::DataType, HError>
        where R: AsyncRead + Unpin + Send
    {
        //get the bytes from the steam
        let buffer_size = estimate_serialized_size::<Self::DataType>();
        let mut buffer = vec![0u8; buffer_size];
        let size = Self::read_bytes_from_asyncread(stream, &mut buffer[..]).await?;

        //check if we read some thing.
        if size == 0 {
            return Err(
                HError::Message { 
                    message: format!("read nothing in the stream")
                }
            )
        }
        //decode it 
        let val = Self::decode_from_slice(&mut buffer[..size])?;

        Ok(val)
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
        let buffer_size = estimate_serialized_size::<T>();
        Serializer { 
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

    ///this function is used etismate thhe size of the serialized data.
    
    pub(super) fn estimate_serialized_size<T: Sized>() -> usize {
        //the size of the struct itself
        let two_times_struct_size = std::mem::size_of::<T>() * 2;

        //the size of the default buffer
        let default_buffer_size = BUFFER_SIZE;

        //check if the size of the struct is much smaller or larger than the 
        //default buffer size.
        let magnitude = two_times_struct_size as f64 / default_buffer_size as f64;
        
        //two_times_struct_size is greater or much more smaller than default_buffer_size,
        //we set the size with the magnitude of two_times_struct_size
        if magnitude < 0.2 || magnitude > 1.0 {
            //align the size to 128 bytes
            return size_to_align(two_times_struct_size, 128);
        }else {
            //  magitude is between 0.2 and 1,
            // we just use the default buffer size
            return default_buffer_size;
        }        
    }

}


#[cfg(test)]
mod tests{
    use super::*;

    #[test]
    fn test_encode_into_slice() {
        
    }

}