/// Serialize and Deserialize Rust data structures to and from bytes.


use tokio::io::{AsyncWrite, AsyncRead, AsyncWriteExt, AsyncReadExt};
use bincode::{Decode, Encode};
use async_trait::async_trait;
use std::marker::Unpin;

use crate::herrors::HError;


use helpers::*;
/// 128 kB buffer size, in function save_into_stream,
const  BUFFER_SIZE: usize = 1024 * 128;

#[async_trait]
pub trait Serializer: Encode + Decode<()>{
    ///encode into bytes with bigendian, just wrap the bincode::encode_into_slice 
    fn encode_into_slice(&self, dst: &mut [u8]) -> Result<usize, HError>{
        let config  = bincode::config::standard()
            .with_big_endian();
        
        let size = bincode::encode_into_slice(self, dst, config)?;
        Ok(size)
    }

    ///decode from bytes with bigendian, just wrap the bincode::decode_from_slice 
    fn decode_from_slice(&self, src: &[u8]) -> Result<Self, HError>{
        let config  = bincode::config::standard()
            .with_big_endian();
        let (val, _) = bincode::decode_from_slice::<Self, _>(src, config)
            .map_err( |e| 
                HError::Message { message:  
                    format!("deserialize error: {}", e)
                }
            )?;
        Ok(val)
    }

    ///save it into a async stream, return the size of the serialized data
    async fn save_into_stream<W: AsyncWrite + Unpin>(&self, stream: W ) -> Result<usize, HError> {
        let config = bincode::config::standard()
            .with_big_endian();

        //create a buffer with the estimated size
        let buffer_size = estimate_serialized_size(self);
        let mut buffer = vec![0u8; buffer_size];

        //encode the data into the buffer, and get the size of bytes
        let header_size: u32;
        let total_size: usize;
        let bytes_encoded = bincode::encode_into_slice(self, &mut buffer[4..], config)?;
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
    
    pub(super) fn estimate_serialized_size<T: Serializer>(val: &T) -> usize {
        //the size of the struct itself
        let two_times_struct_size = std::mem::size_of::<T>() * 2;

        //the size of the default buffer
        let default_buffer_size = BUFFER_SIZE;

        //check if the size of the struct is much smaller or larger than the 
        //default buffer size.
        let magnitude = two_times_struct_size as f64 / default_buffer_size as f64;
        
        //two_times_struct_size is greater or much more smaller than default_buffer_size,
        //we set the size with the magnitude of two_times_struct_size
        if magnitude < 0.2 || magnitude > 1  {
            //align the size to 128 bytes
            return size_to_align(two_times_struct_size, 128);
        }else {
            //  magitude is between 0.2 and 1,
            // we just use the default buffer size
            return default_buffer_size;
        }        
    }

}