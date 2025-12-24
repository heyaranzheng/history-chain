use bincode::{Decode, Encode};
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncWriteExt, AsyncWrite};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{self, Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::env;

use crate::hash::HashValue;
use crate::herrors::HError;
use crate:: uuidbytes::UuidBytes;
use crate::chain::{self, ChainLimit, BlockChain, ChainRef, ChainInfoBuilder};
use crate::block::{Block, DataBlock, DataBlockArgs, DigestBlock, DigestBlockArgs};
use crate::constants::MAX_FILE_NAME_LEN;

///A saver is responsible for saving the data to some storage.
#[async_trait]
pub trait DataBase {
    ///save the data to some Database.
    async fn save(&self, src_bytes: &[u8], meta: Meta) -> Result<(), HError>;
    ///get out the data from some database by the uuid.
    async fn find(&self, uuid: UuidBytes) -> Result<Option<String>, HError>;
}

#[derive(Clone, Decode, Encode)]
pub struct Meta {
    name: String,
    uuid: UuidBytes,
}


impl Meta {
    //encode it into a given buffer, return the size of encoded data in the buffer
    pub fn encode_into(self, buffer: &mut [u8]) -> Result<usize, HError> {
        //encode it with big endian, and fixed size
        let config = bincode::config::standard()
            .with_big_endian()
            .with_fixed_int_encoding();
        let size = bincode::encode_into_slice(self, buffer, config)
            .map_err(|e|
                HError::Message { message: 
                    format!("error in encode_into of Meta: {}", e)
                }
            )?;
        Ok(size)
    }

    //decode data from the given data
    pub fn decode_from(buffer: &[u8]) -> Result<Meta, HError> {
        let config = bincode::config::standard()
            .with_big_endian()
            .with_fixed_int_encoding();
        let (meta, _) = 
            bincode::decode_from_slice::<Meta, _>(buffer, config)
            .map_err( |e| 
                HError::Message { 
                    message: format!("error in decode_from of Meta: {}", e)
                }    
            )?;
        Ok(meta)
    }

    ///the String of name should be less than 128 Bytes
    pub fn set_name(mut self, name: String) -> Result<Self, HError> {
        //check the size of the name
        if name.len() > MAX_FILE_NAME_LEN {

            return Err(
                HError::Message { message: 
                    format!("the file's name is too long")
                }
            );
        }
        else {
            self.name = name;
        }

        Ok(self)
    }

    pub fn set_uuid(mut self, uuid: UuidBytes) -> Self {
        self.uuid = uuid;
        self
    }

    pub fn uuid(&self) -> UuidBytes {
        self.uuid
    }

    //return the name of the file
    pub fn name(&self) -> &String {
        &self.name
    }

}


///the data structure can save itself to some storage.
pub struct FileDataBase{
    uuid_list: Arc<Mutex<Vec<UuidBytes>>>,
    data_dir: PathBuf,
    bundle_file: PathBuf,
}

impl FileDataBase {
    pub fn new(data_absolute_dir_opt: Option<String>) -> Result<Self, HError> {
        let data_dir;
        if let Some(dir) = data_absolute_dir_opt {
            data_dir = PathBuf::from(dir);
        }else {
            //have no given absolute directory, just use the current directory.
            data_dir = std::env::current_dir()?;
        }
        let mut bundle_file = data_dir.clone();
        bundle_file.push("bundle");
        let file_database = Self {
            uuid_list: Arc::new(Mutex::new(Vec::new())),
            data_dir,
            bundle_file 
        };
        
        Ok(file_database)
    }
}


#[async_trait]
impl DataBase for FileDataBase {
    ///For now, we just create a file named "data", then save the data into it.
    async fn save(&self, src_bytes: &[u8], meta: Meta) -> Result<(), HError> {
        let name = meta.name.clone();
        let mut path = self.data_dir.clone();
        path.push(name);
        
        let mut file_stream = OpenOptions::new()
            .create(true)
            .append(true)
            .write(true)
            .open(path) 
            .await?;
        file_stream.write(src_bytes).await?;

        //save the meta data to the "meta_data" file.
        let mut meta_path = self.data_dir.clone();
        meta_path.push("meta_data");
        let mut meta_stream = OpenOptions::new()
            .create(true)
            .append(true)
            .write(true)
            .open(meta_path)
            .await?;

        //meta only have two fields, the file name and a uuid. the size of 
        //each field is less than MAX_FILE_NAME_LEN
        let mut buffer = vec![0u8;8 * MAX_FILE_NAME_LEN];
        let meta_bytes_size = meta.encode_into(&mut buffer)?;
        buffer.truncate(meta_bytes_size);
        meta_stream.write(&buffer).await?;
        
        Ok(())
    }

    async fn find(&self, uuid: UuidBytes) -> Result<Option<String>, HError> {

    }


}



///the data structure to store the block chains
///# Note:
///  * chains: the chains in this bundle.
///  * counter: the counter of the bundle.
///  * origin: the origin timestamp of the bundle.
///  * time_gap: the biggest timestamp gap we can accept in this bundle.
///  * max_len: the max length of the block chains in this bundle.
pub struct Bundle <B>
    where B: Block
{
    chains: Vec<BlockChain<B>>,
    counter: u32,
    origin: u64,
    time_gap: u64,
    max_len: u32,
}
