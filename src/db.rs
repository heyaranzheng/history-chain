use bincode::{Decode, Encode};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeekExt, AsyncWrite, AsyncWriteExt};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{ Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::env;

use crate::hash::HashValue;
use crate::herrors::{self, HError};
use crate::network::Message;
use crate:: uuidbytes::UuidBytes;
use crate::chain::{self, ChainLimit, BlockChain, ChainRef, ChainInfoBuilder, Chain};
use crate::block::{Block, DataBlock, DataBlockArgs, DigestBlock, DigestBlockArgs};
use crate::constants::MAX_FILE_NAME_LEN;
use crate::serializer::{self, Serialize, Serializer};

///A saver is responsible for saving the data to some storage.
#[async_trait]
pub trait DataBase {
    ///save the data to some Database.
    async fn save_file(&self, file_name: String, meta: MetaData) -> Result<(), HError>;
    ///get out the data from some database by the uuid.
    async fn find_meta(&self, uuid: UuidBytes) -> Option<MetaData>;
    ///save a chain to the database.
    async fn save_chain<B>(&self, chain: &BlockChain<B>) -> Result<(), HError>
        where B: Block;
    ///give a block's uuid, find it in the database and return the chain which
    /// contains the block.
    async fn find_chain<B>(&self, uuid: UuidBytes) -> Option<BlockChain<B>>
        where B: Block;
}

///a meta data is responsible for storing the meta information of a data block.
///It should contain the uuid of the data, the unique id of the data.
pub trait Meta {
    fn uuid(&self) -> UuidBytes;
}
    

#[derive(Clone, Decode, Encode, Debug)]
pub struct MetaData {
    name: String,
    uuid: UuidBytes,
}
impl Meta for MetaData {
    fn uuid(&self) -> UuidBytes {
        self.uuid
    }                         
}

impl MetaData {
    ///create a new meta data with the given name and uuid.
    pub fn new(name: String, uuid: UuidBytes) -> Self {
        Self { name, uuid }
    }

    //encode it into a given buffer, return the size of encoded data in the buffer
    pub fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, HError> {
        //encode it with big endian, and fixed size
        let config = bincode::config::standard()
            .with_big_endian()
            .with_fixed_int_encoding();
        let size = bincode::encode_into_slice(self, buffer, config)
            .map_err(|e|
                HError::Message { message: 
                    format!("error in encode_into of MetaData: {}", e)
                }
            )?;
        Ok(size)
    }

    //decode data from the given data
    pub fn decode_from(buffer: &[u8]) -> Result<MetaData, HError> {
        let config = bincode::config::standard()
            .with_big_endian()
            .with_fixed_int_encoding();
        let (meta, _) = 
            bincode::decode_from_slice::<MetaData, _>(buffer, config)
            .map_err( |e| 
                HError::Message { 
                    message: format!("error in decode_from of MetaData: {}", e)
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
    async fn get_one_meta_from_file(file_stream: &mut File) -> Result<(MetaData, usize), HError> {

        //get the size of the meta data
        let mut size_bytes = [0u8; 4];
        file_stream.read_exact(&mut size_bytes).await?;
        let size = u32::from_be_bytes(size_bytes) as usize;

        let mut meta_bytes = vec![0u8; size];
        file_stream.read_exact(&mut meta_bytes).await?;

        //decode the meta data from the buffer
        let meta = MetaData::decode_from(&meta_bytes)?;
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
pub struct MetaDataIterator {
    file_stream: File,
    current_pos: u64,
    end_pos: u64,
}

impl MetaDataIterator {
    ///create a new MetaDataIterator
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
    async fn next(&mut self) -> Option<Result<MetaData, HError>> {
        //check if we reach the end of the file.
        if self.current_pos == self.end_pos {
            return None;
        }

        match MetaData::get_one_meta_from_file(&mut self.file_stream).await {
            Ok((meta, size)) => {
                //update the current position
                self.current_pos += size as u64 + 4;
                Some(Ok(meta))
            }
            Err(e) => {
                //there is an error, reset the current position
                self.reset();
                Some(Err(e))
            }
        }
    }
}




///the data structure can save itself to some storage.
pub struct FileDataBase{
    uuid_list: Arc<Mutex<HashMap<UuidBytes, MetaData>>>,
    data_dir: PathBuf,
    bundle_file: PathBuf,
}

impl FileDataBase {
    ///create a new FileDataBase
    /// # Arguments:
    /// * - give a absolute directory of the data directory if you have one.
    /// use None if you don't have one.
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
    async fn save_file(&self, file_path: String, meta: MetaData) -> Result<(), HError> {
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
       
        let mut meta_dir = self.data_dir.clone();
        meta_dir.push("meta");
        meta.save_meta_to_file(&meta_dir).await?;

        Ok(())
    }

    ///find a meta data by the uuid.
    ///check the memory first, if we don't have memoery cache, then 
    /// read the meta data from the file and insert it into the memory.
    async fn find_meta(&self, uuid: UuidBytes) -> Option<MetaData> {
        let mut result_meta = None;
        
        //check if we have an list in memory.
        let mut uuid_list = self.uuid_list.lock().await;
        let size = uuid_list.len();
        if size != 0 {
            //we have a list in memory, and find the meta data by the uuid.
            if let Some(meta) = uuid_list.get(&uuid) {
                return Some(meta.clone());
            }else {
                //we have a list in memory, but we don't find the meta data by the uuid.
                //return None;
                return result_meta;
            }
        }else {
            //we don't have a list in memory, so we need to read the meta data from the file.
            //and insert the meta data into the memory.

            //open the "meta" file and read the meta data
            let mut meta_dir = self.data_dir.clone();
            meta_dir.push("meta");
            
            //create a meta data iterator, and check if we have an error.
            let  meta_iterator = MetaDataIterator::new(meta_dir).await;
            if let Err(e) = meta_iterator {
                return result_meta;
            }
            let mut meta_iterator = meta_iterator.unwrap();

            //gether the meta data from the file
            while let Some(meta) = meta_iterator.next().await {
                match meta {
                    Ok(meta) => {
                        //add the all the meta data into the list.
                        uuid_list.insert(meta.uuid(), meta.clone());
                        if meta.uuid() == uuid {
                            result_meta = Some(meta.clone());
                        }
                    }
                    Err(e) => {
                        //logger the error,return None;
                        herrors::logger_error_with_error(&e);
                        return result_meta;
                    }
                }
            }
            result_meta
        }
    }

    //-------------------------------------------------
    ///save a chain to the database.
    async fn save_chain<B>(&self, chain: &BlockChain<B>) -> Result<(), HError>
        where B: Block
    {
        //open a file to save the chain, named with bundle_file.
        let mut bundle_file = self.bundle_file.clone();   
        Ok(())
        
    }

    async fn find_chain<B>(&self, uuid: UuidBytes) -> Option<BlockChain<B>>
        where B: Block
    {
        None
    }
    //-------------------------------------------------
}


const DEFAULT_BUNDLE_FILE: &str = "chains.bundle";

///this is a package manager of some chains.
/// We can use this to store chains into a bundle file.
/// #Fields:
/// `serializer`: the serializer of the bundle, we can use this to serialize a chain,
/// then save it into a bundle file.
/// 'counter': the number of chains in the bundle.
/// 'origin': the origin of the timestamp of all the chains in the bundle.
/// 'time_gap': the allowed biggest time gap between any two blocks, we can use 
/// this to fingure out the limitation of the biggest timestamp of all chains.
/// 'path': the path name of the bundle file.
#[derive(Clone)]
pub struct Bundle <B>
    where B: Block + Encode + Decode<()> + Send + Unpin, 
{
    serializer: Serializer<BlockChain<B>>,
    counter: u32,
    origin: u64,
    time_gap: u64,
    path: String,
}

impl <B> Bundle <B> 
    where B: Block + Encode + Decode<()> + Send + Unpin,
{
    pub fn default_new() -> Self {
        Self{
            serializer: Serializer::<BlockChain<B>>::new(None),
            counter: 0,
            origin: 0,
            time_gap: 0, 
            path: DEFAULT_BUNDLE_FILE.to_string(),
        }
    }

    ///if we want to add a chain to the bundle file, we should first update the 
    /// the descriptions of the bundle file.
    fn update_description_with_chain(&mut self, chain: &BlockChain<B>) 
    -> Result<(), HError> 
    {
        let time_gap = chain.gap();
        
        if let Some(origin) = chain.origin() {
            //we have no chain yet, so we set the origin and time_gap.
            if self.counter == 0 {
                self.origin = origin;
                self.time_gap = chain.gap();
                self.counter = 1;
            }else {
                //caculate the time_gap and origin.
                let time_max = (self.origin + self.time_gap).max(
                        origin + time_gap
                );

                //update the origin, time_gap and the counter
                self.origin = self.origin.min(origin);
                self.time_gap = time_max - self.origin;
                self.counter += 1;
                
            }
        }

        Ok(())
        
    }    

    ///this function will save the block chain into bundle file.
    /// the chain will be take the ownership, then return it back.
    /// # Return Value:
    /// * `Ok(chain)`: if we save the chain into the bundle file successfully.
    /// * `Err(error)`: if we have an error when we save the chain into the bundle file.
    pub async fn save_to_file(&mut self, chain: BlockChain<B> ) 
    -> Result<BlockChain<B>, HError> 
    { 
        let path = self.path.clone();

        let mut stream = tokio::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .append(true)
            .open(path)
            .await?;
        //locate the position to write the data.
        let save_pos = stream.stream_position().await?;

        let chain =self.encode_to_stream(&mut stream, chain).await?;

        //update the description of the bundle file.
        let result = self.update_description_with_chain(&chain);
        match result {
            Ok(()) => {
                return Ok(chain);
            }
            Err(e) => {
                //we should delete the data we just wrote into the file.
                stream.set_len(save_pos).await?;
                return Err(e);
            }
        }
    }

    ///this function will consume the vector of chains, when the 
    async fn save_chains_to_bundle(&mut self, chains:& Vec<BlockChain<B>>)
        -> Result<usize, HError> 
    {
        let path = self.path.clone();
        let serializer = &mut self.serializer;
        
        let size = serializer.encode_vec_into(chains)?;

        let mut stream = tokio::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .await?;

        let nwrite = stream.write(&serializer.buffer()[..size]).await?;

        //check the result of the write operation.
        if nwrite != size {
            return Err(
                HError::Message { message: 
                    "write error in function ssave_chains_to_bundle".to_string() 
                }
            );
        }

        Ok(size)
    }

    
    ///this function will take the ownership of the bundle, and return the 
    /// onwership of it back.
    async fn encode_to_stream<R> (
        &mut self, 
        stream: &mut R, 
        chain: BlockChain<B>
    ) -> Result<BlockChain<B>, HError>
        where  R: AsyncWrite + Unpin + Send

    {
        //get the bundle's serializer
        let serializer = &mut self.serializer;

        //set the data of the serializer with the given chain.
        serializer.set_data(chain);

        //serialize the data into the stream(the serializer will serialize it 
        //and add a size header, then write it into the stream)
        serializer.save_into_asyncwrite(stream).await?;

        //we should return the data we just took.
        //take out of the self from the serializer
        let result = serializer.take_data();
        match result {
            None => {
                return Err(
                    HError::Message { 
                        message: format!("serializer take data failed") 
                    }
                );
            }

            Some(self_back) => {
                //return the ownership of the data.
                return Ok(self_back)
            }
        }
    }


    ///read the bundle file and return a vector of chains.
    pub async fn load_chains_from_bundle(&mut self) 
        -> Result<Vec<BlockChain<B>>, HError> 
    {
        let path = self.path.clone();
        let mut stream = tokio::fs::File::open(path).await?;

        let serilizer =&mut self.serializer;
        serilizer.decode_from_asyncread(&mut stream).await
    }
    
}


#[cfg(test)]
mod tests {
    use std::io::Write;

    use rand::Rng;

    use super::*;

    use crate::uuidbytes::{UuidBytes, Init};


    #[tokio::test(flavor = "multi_thread")]
    async fn test_file_database() {   
        use tokio::time::{Duration, Instant};

        //this is the constant of the test count.
        const TEST_COUNT: u32 = 1000;

        //the message will repeat in the test file for 
        //TestFileMessageRepeatTimes times.
        const TEST_FILE_MSG_REPEAT_TIMES: u64 = 100;

        //create a new directory for the test
        let mut current_dir = std::env::current_dir().unwrap();
        current_dir.push("test_dir");
        let test_dir = current_dir.clone();
        current_dir.push("src");
        let src = current_dir.clone();
        let result =tokio::fs::create_dir_all(src.clone()).await;
        assert_eq!(result.is_ok(), true);

        let mut test_meta_vec = Vec::new();

        //create TestCount files
        for i in  0..TEST_COUNT {
            let file_name = format!("test_file_{}.txt", i);
            let src_clone = src.clone();
            let file_path = src_clone.join(file_name.clone());
            let mut file = std::fs::File::create(file_path).unwrap();
            let content = format!("hello world {}", i);
            for _ in 0..TEST_FILE_MSG_REPEAT_TIMES {
                let _ = file.write(content.as_bytes());
            }
            let uuid = UuidBytes::new();

            let meta = MetaData::new(file_name, uuid);
            test_meta_vec.push(meta);
        }

        //create a new file database, the directory is test_dir
        let test_dir_string = test_dir.to_str().unwrap().to_string();
        let file_db = FileDataBase::new(
            Some(test_dir_string)
        );
        assert_eq!(file_db.is_ok(), true);
        let file_db = file_db.unwrap();

        //test the efficiency of the save function.
        let mut save_times = Vec::with_capacity(TEST_COUNT as usize);
        
        for i in  0..TEST_COUNT {          
            let file_name = format!("test_file_{}.txt", i);
            let file_path = src.clone();
            let file_name = file_path.join(file_name);

            //start the timer
            let start = Instant::now();
            let result = file_db.save_file(
                file_name.to_str().to_owned().unwrap().to_string(), 
                test_meta_vec[i as usize].clone()
            ).await;
            //stop the timer
            let duration = start.elapsed();
            assert_eq!(result.is_ok(), true);
            //save the time
            save_times.push(duration.as_nanos());
        }

        //analyze the statistics of the save function.
        let save_total = save_times.iter().sum::<u128>();       
        let save_avg: f64 = save_total as f64 / TEST_COUNT as f64;
        let save_max = *save_times.iter().max().unwrap();
        let save_min = *save_times.iter().min().unwrap();
        println!("Test Counter: {}", TEST_COUNT);
        println!("save_total: {}", save_total);
        println!("save_avg: {}", save_avg);
        println!("save_max: {}", save_max);
        println!("save_min: {}", save_min);

        //print a split line
        println!("-----------------------------");
        println!("-----------------------------");
        println!("Test File Database");
        //test the meta dataiterator
        let mut meta_iterator = MetaDataIterator::new(
            file_db.data_dir.join("meta")
        ).await.unwrap();

        while let Some(meta_result) = meta_iterator.next().await {
            match meta_result {
                Ok(meta) => {
                    println!("meta: {:?}", meta);
                }
                Err(e) => {
                    println!("error: {:?}", e);
                }
            }
        }

        //print a split line
        println!("----------------------------");
        println!("Test File Database");



        //test the efficiency of the find function.
        let mut find_times = Vec::with_capacity(TEST_COUNT as usize);

        //create a random seed
        let mut rng = rand::thread_rng();

        for _ in  0..TEST_COUNT {
            //use the random seed to get a random index
            let index = rng.gen_range(0..TEST_COUNT);

            //chose a uuid from the test_meta_vec randomly
            let uuid = test_meta_vec[index as usize].uuid();

            //start the timer
            let start = Instant::now();
            let result = file_db.find_meta(uuid).await;
            //stop the timer
            let duration = start.elapsed();
            assert_eq!(result.is_some(), true);
            //save the time
            find_times.push(duration.as_nanos());
        }

        //analyze the statistics of the find function.
        let find_total = find_times.iter().sum::<u128>();       
        let find_avg: f64 = find_total as f64 / TEST_COUNT as f64;
        let find_max = *find_times.iter().max().unwrap();
        let find_min = *find_times.iter().min().unwrap();
        println!("Test Counter: {}", TEST_COUNT);
        println!("find_total: {}", find_total);
        println!("find_avg: {}", find_avg);
        println!("find_max: {}", find_max);

        //pinrt a split line
        println!("-----------------------------------------");
        println!("Test FileDatabase whether has a memory cache");
        let meta_list 
            = file_db.uuid_list.lock().await;
        for _ in 0 .. TEST_COUNT {
            let result = test_meta_vec.pop();
            assert_eq!(result.is_some(), true);
            let meta = result.unwrap();
            let name = meta.name;
            let uuid = meta.uuid;

            //check the list
            let result = meta_list.get(&uuid);
            assert_eq!(result.is_some(), true);
            let name_from_list = result.unwrap().clone().name;

            assert_eq!(name, name_from_list);
        }


        //clear the test directory after the test.
        let result = clear_test_dir().await;
        assert_eq!(result.is_ok(), true);

    }

    async fn clear_test_dir() -> Result<(), HError> {
        //get the test_dir
        let current_dir = std::env::current_dir()?;
        let path = current_dir.join("test_dir");

        //delete the test_dir
        println!("removing test_dir... ");
        let result
           = tokio::fs::remove_dir_all(path).await;
        match result {
            Ok(_) => {
                println!("removed test_dir.");
                Ok(())
            }
            Err(e) => {
                println!("failed to remove test_dir.");
                Err(e.into())
            }
        }
    }

    use crate::utils::faker_data_chain;
    #[tokio::test(flavor = "multi_thread")]
    async fn test_bundle_save_and_load() {
        //create a vector with chains
        let mut vec = Vec::new();
        for i in 0..10 {
            let result = 
                faker_data_chain(10, i, 500);
            assert_eq!(result.is_ok(), true);
            let chain = result.unwrap();
            vec.push(chain);
        }

        let mut bundle = Bundle::<DataBlock>::default_new();

        //the chain ownership will be taken by the bundle, then return it back.
        //we will use this vector to store the returned chains.
        let mut vec_ret = Vec::new();
        for chain in vec {
            let result = 
                bundle.save_to_file(chain).await;
            assert_eq!(result.is_ok(), true);
            let chain_ret = result.unwrap();
            vec_ret.push(chain_ret);
        }

        //set the steam to the head of the bundle file.
        

        //load the chains from the bundle file.
        let result = bundle.load_chains_from_bundle().await;
        assert_eq!(result.is_ok(), true);
        let chains = result.unwrap();
    
        assert_eq!(chains.len(), vec_ret.len());
        assert_eq!(chains, vec_ret);

        //remove the test file
        let _ = tokio::fs::remove_file(bundle.path).await;

    }

}
