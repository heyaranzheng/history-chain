use bincode::{Decode, Encode};
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{self, Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::env;

use crate::hash::HashValue;
use crate::herrors::HError;
use crate::network::Message;
use crate:: uuidbytes::UuidBytes;
use crate::chain::{self, ChainLimit, BlockChain, ChainRef, ChainInfoBuilder};
use crate::block::{Block, DataBlock, DataBlockArgs, DigestBlock, DigestBlockArgs};
use crate::constants::MAX_FILE_NAME_LEN;

///A saver is responsible for saving the data to some storage.
#[async_trait]
pub trait DataBase {
    ///save the data to some Database.
    async fn save(&self, file_name: String, meta: Meta) -> Result<(), HError>;
    ///get out the data from some database by the uuid.
    async fn find(&self, uuid: UuidBytes) -> Result<Option<Meta>, HError>;
}

#[derive(Clone, Decode, Encode)]
pub struct Meta {
    name: String,
    uuid: UuidBytes,
}


impl Meta {
    //encode it into a given buffer, return the size of encoded data in the buffer
    pub fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, HError> {
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

    /// set the meta data into a given path
    async fn save_meta_to_file(&self, path: &PathBuf) -> Result<(), HError> {
        
        let mut buffer = vec![0u8; MAX_FILE_NAME_LEN * 2];
        let size = self.encode_into(&mut buffer[4..])? as u32;
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(path)
            .await?;
        //write the size of the meta data into the first 4 bytes of the buffer
        buffer[0..4].copy_from_slice(&size.to_be_bytes());
        //write the meta data into the buffer
        file.write_all(&buffer[..size as usize + 4]).await?;
        
        Ok(())
    }

    /// get the meta data from a given file_stream
    /// Note:
    ///     Make sure the seek position of the file_stream is at the header of the meta data.
    /// Or the we can't get the meta data correctly.
    async fn get_one_meta_from_file(file_stream: &mut File) -> Result<(Meta, usize), HError> {

        //get the size of the meta data
        let mut size_bytes = [0u8; 4];
        file_stream.read_exact(&mut size_bytes).await?;
        let size = u32::from_be_bytes(size_bytes) as usize;

        let mut meta_bytes = vec![0u8; size];
        file_stream.read_exact(&mut meta_bytes).await?;

        //decode the meta data from the buffer
        let meta = Meta::decode_from(&meta_bytes)?;
        Ok((meta, size))
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

///the iterator of the meta data to get all the meta data from the meta file.
pub struct MetaIterator {
    file_stream: File,
    current_pos: u64,
    end_pos: u64,
}

impl MetaIterator {
    ///create a new MetaIterator
    pub async fn new(file_path: PathBuf) -> Result<Self, HError> {
        let file_stream = File::open(file_path).await?;

        //get the size of the file
        let meta_data = file_stream.metadata().await?;
        let end_pos = meta_data.len();
        
        let  meta_iterator = Self {
            file_stream,
            //the current position is the beginning of the file.
            current_pos: 0,
            end_pos,
        };
        Ok(meta_iterator)
    }    

    ///reset the current position to the beginning of the file.
    pub fn reset(&mut self) {
        self.current_pos = 0;
    }

    ///get the next meta data from the file.
    /// # Return Value:
    /// * `Some(Ok(meta))`: if we get a meta data from the file.
    /// * `Some(Err(error))`: if we get an error when we read the meta data from the file.
    /// * `None`: if we reach the end of the file.
    async fn next(&mut self) -> Option<Result<Meta, HError>> {
        //check if we reach the end of the file.
        if self.current_pos == self.end_pos {
            return None;
        }

        match Meta::get_one_meta_from_file(&mut self.file_stream).await {
            Ok((meta, size)) => {
                //update the current position
                self.current_pos += size as u64 + 4;
                Some(Ok(meta))
            }
            Err(e) => {
                Some(Err(e))
            }
        }
    }
}




///the data structure can save itself to some storage.
pub struct FileDataBase{
    uuid_list: Arc<Mutex<HashMap<UuidBytes, Meta>>>,
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
            uuid_list: Arc::new(Mutex::new(HashMap::new())),
            data_dir,
            bundle_file 
        };
        
        Ok(file_database)
    }
}


#[async_trait]
impl DataBase for FileDataBase {
    ///For now, we just create a file named "data", then save the data into it.
    async fn save(&self, file_path: String, meta: Meta) -> Result<(), HError> {
        //just copy the file to the data directory.
        let mut data_dir = self.data_dir.clone();

        //get the file name from the file_path
        let path = Path::new(&file_path)
            .file_name()
            .map(
                |name| name.to_string_lossy().into_owned()
            );
        
        //if we get the file name, then copy the file to the data directory.
        if let Some(name) = path {
            data_dir.push(name);
            tokio::fs::copy(file_path, data_dir).await?;
        } else {
            //can't get the file name, return an error.
            return Err(
                HError::Message { 
                    message: format!("bad file_path, cab't get the the file") 
                }
            )
        }

        //then save the meta data into the "meta" file
        let mut buffer = vec![0u8; MAX_FILE_NAME_LEN * 2];
        let size = meta.encode_into(&mut buffer[4..])?;
        let mut meta_dir = self.data_dir.clone();
        meta_dir.push("meta");
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(meta_dir)
            .await?;
        //write the size of the meta data into the first 4 bytes of the buffer
        buffer[0..4].copy_from_slice(&size.to_be_bytes());
        //write the meta data into the buffer
        file.write_all(&buffer[..size + 4]).await?;

        Ok(())
    }

    async fn find(&self, uuid: UuidBytes) -> Result<Option<Meta>, HError> {
        //check if we have an list
        let uuid_list = self.uuid_list.lock().await;
        if let Some(meta) = uuid_list.get(&uuid) {
            return Ok(Some(meta.clone()));
        }else {
            //drop the lock first, because we may need some time to read the meta.
            drop(uuid_list);
            //open the "meta" file and read the meta data
            let mut meta_dir = self.data_dir.clone();
            meta_dir.push("meta");
            let mut meta = 

        }
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
