
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
    fn buffer(&mut self) -> &mut Vec<u8>;
    //have its data, this will take the ownership of the data, and return it
    fn take_data(&mut self) -> Option<Self::DataType>;
    //set the data to the serializer
    fn set_data(&mut self, data: Self::DataType);
    //resize the buffer
    fn resize_buffer(&mut self, size: usize) -> Result<(), HError>;

    /// just wrap the bincode::encode_into_slice, the src data is the data we stored 
    /// in the serializer
    fn encode_data_into_slice(&mut self, dst: &mut [u8]) -> Result<usize, HError> {
        let config  = bincode::config::standard()
            .with_big_endian();

        let data_opt = self.take_data();

        match data_opt {
            Some(data) => {
                bincode::encode_into_slice(&data, dst, config)
                    .map_err(|e| HError::Message { message: format!("error in serializer encode_into_slice {}", e) })
                    ?;
                Ok(dst.len())
            },
            None => Err(HError::Message { message: "no data in serializer".to_string() })
        }
    }

    ///encode the data into the buffer, and add a u32 to store the size of the bytes at
    /// the beginning of the vector.
    /// if the vector buffer is not enough, resize it and try again.   
    fn encode_into_vec_with_header(&mut self, dst: &mut Vec<u8>) -> Result<usize, HError> {
        let config  = bincode::config::standard()
            .with_big_endian();
        
        let val_opt = self.take_data();
        
        match val_opt {
            Some(val) => {

                //try to encode the data into the buffer, if the buffer is not enough, resize it.
                let  mut size = dst.len();
                loop {
                    match bincode::encode_into_slice(&val, &mut dst[4..], config) {
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
                            dst.resize(size, 0);
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
                            let total_size = 4 + size;
                            
                            let size_header = size as u32;
                            //add the header size to the buffer
                            let header_bytes = size_header.to_be_bytes() as [u8;4];
                            dst[0..4].copy_from_slice(&header_bytes);

                            //return the data to the serializer
                            self.set_data(val);

                            return Ok(total_size);
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

    ///If we have a vector of data, use this function to encode them into the 
    /// serializer's buffer, and return the size of the serialized data.
    /// #NOTE: each data will encoded then stored behind its size header.
    /// So we can take it out easily.
    fn encode_vec_into(&mut self,src: &Vec<Self::DataType>) -> Result<usize, HError>
    {
        let config = bincode::config::standard()
            .with_big_endian();
        let dst = self.buffer();
        let time_resize = 1;

        let mut offset = 0;
        for data in src {
            loop {
                //leaev a palce for the size header
                let result = 
                    bincode::encode_into_slice(data, &mut dst[offset + 4..], config);
                match result {
                    // buffer is not enough, extend it, then retry
                    Err(EncodeError::UnexpectedEnd) => {
                        dst.resize(BUFFER_SIZE * (time_resize + 1) *time_resize, 0);
                        continue;
                    }
                    Err(e) => {
                        return Err(
                            HError::Message { 
                                message: 
                                    format!("error in serializer encode_into_vec {}" ,e)
                            }
                        )
                    }
                    Ok(size) => {
                        save_size_header_into_slice(size as u32, &mut dst[offset..])?;
                        offset = offset + 4 + size;
                        
                        //we got a right size, break the loop
                        break;
                    }
                }
                
            }
        }
        Ok(offset)
    } 


    fn decode_from_slice_with_header(buffer: &[u8]) -> Result<Vec<Self::DataType>, HError>
    {
        //the vector to hold the decoded data
        let mut  vec_ret = Vec::<Self::DataType>::new();

        //get the length of the buffer
        let end = buffer.len();

        //decode the bytes
        let mut offset = 0;
        while offset < end {
            //get one of the encoded data's size
            let this_size = Self::get_header(& buffer[offset..])?;

            //add the offset of the header size
            offset += 4;
            
            //decode it 
            let val = 
                Self::decode_from_slice(& buffer[offset..offset+this_size])?;

            //push it into the vector
            vec_ret.push(val);

            //update the offset
            offset += this_size;
        }

        //check if the data is valid
        if offset != end {
            return Err(
                HError::Message { 
                    message: 
                        format!("error in decode_from_slice_with_header, 
                    we may have a invalid data") 
                }
            )
        }
        
        Ok(vec_ret)

    }

    ///save it into a async stream, return the size of the serialized data
    async fn save_into_asyncwrite<W>(&mut self, stream: &mut W ) 
        -> Result<usize, HError> 
        where W: AsyncWrite + Unpin + Send
    {
        let config = bincode::config::standard()
            .with_big_endian();

        //get the data first
        let data = self.take_data();
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
                    bincode::encode_into_slice(&data, &mut buffer[4..], config)
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

                //return the data to the serializer
                self.set_data(data);

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

    /// read all the data from the stream, and decode it into a vector of data.
    async fn decode_from_asyncread<R>(&mut self, stream: &mut R) -> Result<Vec<Self::DataType>, HError>
        where R: AsyncRead + Unpin + Send
    {
        
        let buffer = self.buffer();
        //get all the bytes from the stream
        let nread = read_all_from_stream(stream, buffer).await?;

        Self::decode_from_slice_with_header(&buffer[..nread])
    }


}

/// This is a serializer, we use this serializer to serialize data.
#[derive(Clone)]
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
        Serializer::<T> { 
            buffer: vec![0u8; BUFFER_SIZE], 
            data: data
        }
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
    fn buffer(&mut self) ->  &mut Vec<u8> {
        &mut self.buffer
    }

    ///resize the buffer of the serializer
    fn resize_buffer(&mut self,size:usize) -> Result<(),HError> {
        self.buffer.resize(size, 0);
        Ok(())
    }

    ///get the data of the serializer
    fn take_data(&mut self) -> Option<T> {
        self.data.take()
    }


    ///set the data for the serializer, cosunme the serializer
    /// then return a new serializer with the data setted.
    fn set_data(&mut self, data: T)  {
        self.data = Some(data);
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

    ///read all the data from the stream into the vector buffer, if the buffer is not
    /// enough, extend it.
    pub(super) async fn read_all_from_stream<R>(
        stream: &mut R,  
        buffer: &mut Vec<u8>
    ) -> Result<usize, HError>
        where R: AsyncRead + Unpin + Send
    {
        let mut total_size = 0;
        let mut tmper_buffer = vec![0u8; BUFFER_SIZE];
        let mut resize_time = 0;

        while let Ok(nread) = stream.read(&mut tmper_buffer[..]).await  {
            if nread == 0 {
                break;
            }

            //check if our own buffer is enough to hold the data
            let buffer_len = buffer.len();
            if buffer_len > total_size + nread {
                //the buffer is enough, just copy the data into the buffer
                buffer[total_size..total_size + nread].copy_from_slice(&tmper_buffer[..nread]);

            }else if buffer_len < total_size + nread {
                //the buffer is not enough, we need to extend it.
                buffer.extend_from_slice(&tmper_buffer[..]);

                //copy the data into the buffer
                buffer[total_size..total_size + nread].copy_from_slice(&tmper_buffer[..nread]);

                //resize the temper buffer, so when the next extend happens, the buffer
                //will be bigger and bigger.
                tmper_buffer.resize(BUFFER_SIZE * (resize_time + 1) * resize_time, 0);
                resize_time += 1;

            } 

            buffer.extend_from_slice(&tmper_buffer[..nread]);
            total_size += nread;
            
        }  
        Ok(total_size)
    }

    pub(super) fn save_size_header_into_slice(size: u32, dst: &mut [u8]) 
    -> Result<(), HError> 
    {
        let size_bytes = size.to_be_bytes() as [u8;4];
        //copy the size bytes into the dst
        dst[..4].copy_from_slice(&size_bytes);
        Ok(())
    }

    
} 


#[cfg(test)]
mod tests{
    use tokio::io::AsyncSeekExt;

    use super::*;

    #[tokio::test]
    async fn test_serializer() {
        //create a new serializer with no data
        let mut test_ser= Serializer::<String>::new(None);
        let data = "i am a test string".to_string();

        //--------------test set_data()-------------------------------------
        test_ser.set_data(data.clone());
        assert_eq!(test_ser.get_data().unwrap(), "i am a test string");
        
        //--------------test decode_into_slice() decode_from_slice()----------
        let mut dst = vec![0u8; 4];
        let result = test_ser.encode_into_vec_with_header(&mut dst);
        assert_eq!(result.is_ok(), true);
        let size = result.unwrap();
        let result = 
            Serializer::<String>::decode_from_slice_with_header(&dst[..size]);
        assert_eq!(result.is_ok(), true);
        assert_eq!(result.unwrap()[0], "i am a test string".to_string());

       
        //create a file for test
        let result = tokio::fs::File::options()
            .write(true)
            .create(true)
            .read(true)
            .append(true)
            .open("test.file")
            .await;
        assert_eq!(result.is_ok(), true);
        let mut stream = result.unwrap();

        //reset the data in the serializer
        test_ser.set_data(data.clone());

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
        
        //clear the file
        let result = stream.set_len(0).await;
        assert_eq!(result.is_ok(), true);

        //save the data twice, so we should get two data
        test_ser.set_data(data.clone());
        let _ = test_ser.save_into_asyncwrite(&mut stream).await;

        test_ser.set_data(data.clone());
        let _ = test_ser.save_into_asyncwrite(&mut stream).await;

        //set the seek position to the beginning of the data
        let result = stream.seek(tokio::io::SeekFrom::Start(0)).await;
        assert_eq!(result.is_ok(), true);
        
        let result = test_ser.decode_from_asyncread(&mut stream).await;
        
        
        assert_eq!(result.is_ok(), true);
        let mut vec_ret = result.unwrap();
        assert_eq!(vec_ret.pop().unwrap(), "i am a test string".to_string());
        assert_eq!(vec_ret.pop().unwrap(), "i am a test string".to_string());


        
        //remove the file
        let result = tokio::fs::remove_file("test.file").await;
        assert_eq!(result.is_ok(), true);



    }


        


}