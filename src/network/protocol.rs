use std::mem::MaybeUninit;
use std::net::{SocketAddr};
use bincode::{Decode, Encode, config};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncRead, AsyncWrite,};
use tokio::net::TcpStream;
use tokio_util::sync::CancellationToken;
use async_trait::async_trait;

use crate::constants::{MAX_MSG_SIZE, MAX_UDP_MSG_SIZE, ZERO_HASH};
use crate::chain::{Chain, BlockChain};
use crate::herrors::HError;
use crate::hash:: {HashValue, Hasher};
use crate::network::identity::Identity;
use crate::pipe::Pipe;
use crate::block::{Block, BlockArgs};

///the signature of the message.
type SignatureBytes = [u8; 64];



///This is the header of stream, when we create a connection by tcp.
#[derive(Debug, Decode, Encode, PartialEq)]
pub struct Header {
    //the total size of the data we will get from the stream. 4 bytes.
    length: u32,
    //the signature of the data, 64 bytes.
    signature: SignatureBytes,
    //public key of the sender, 32 bytes.
    public_key: HashValue,
}
impl Header {
    fn new(length: u32, signature: SignatureBytes, public_key: HashValue) -> Self {
        Self { length, signature, public_key}
    }


    ///the size of the header in bytes.
    ///4 + 64 + 32 = 100.
    ///4: length, 64: signature, 32: public_key.
    #[inline]
    pub fn header_size() -> usize {
        100
    }
    
    ///Instead of encoding the header as a whole, we encode its' three fields separately to ensure
    ///the encoded length is a fixed value.
    pub fn encode_into_slice(&self, buffer: &mut [u8]) -> Result<usize, HError> {
        //check buffer size
        if buffer.len() < 100 {
            return Err(HError::Message 
                {
                    message: format!("in header encode_into_slice, buffer size is too small: {}", buffer.len()) 
                })
        }
        let mut total_size = 0;

        //create a config for bincode, with big-endian
        let config = bincode::config::standard()
            .with_big_endian()
            .with_fixed_int_encoding();
        //encode the header 
        
        let size = 
            bincode::encode_into_slice(self.length, &mut buffer[..4], config)
            .map_err(|_| HError::Message { message: "encode error in header".to_string() })?;
        total_size += size;
        println!("size: {:?}", size);
        let size = 
            bincode::encode_into_slice(self.signature,&mut  buffer[4..68], config)
            .map_err(|_| HError::Message { message: "encode error in header".to_string() })?;
        total_size += size;
        println!("size: {:?}", size);
        let size =
            bincode::encode_into_slice(self.public_key, &mut buffer[68..100], config)
            .map_err(|_| HError::Message { message: "encode error in header".to_string() })?;
        total_size += size;
        println!("size: {:?}", size);
        
        Ok(total_size)
    }

    
    //decode the header from a slice
    pub fn decode_from_slice  (data: &[u8]) -> Result<Header, HError> 
    {
        //create a config for bincode, with big-endian, and a fixed int encoding.
        let config = bincode::config::standard()
            .with_big_endian()
            .with_fixed_int_encoding();

        //check the size of the data
        if data.len() < 100 {
            return Err(HError::Message { message: "header too small".to_string() });
        }

        //decode the header from a slice one by one.
        let (length, _) = bincode::decode_from_slice::<u32, _>(&data[..4], config)
            .map_err(|_| HError::Message { message: "decode error in header".to_string() })?;
        let (signature, _) = bincode::decode_from_slice::<SignatureBytes, _>(&data[4..68], config)
            .map_err(|_| HError::Message { message: "decode error in header".to_string() })?;
        let (public_key, _) = bincode::decode_from_slice::<HashValue, _>(&data[68..100], config)
            .map_err(|_| HError::Message { message: "decode error in header".to_string() })?;
        let header = Header::new(length, signature, public_key);
        
        Ok(header)
    }

    //extract the header from the stream
    pub async fn from_stream <T> ( stream: &mut T) -> Result<Header, HError> 
        where T: AsyncReadExt + Unpin,
    {
        //read the first 4 + 64 + 32 bytes of the stream, which is the length and signature of the data.
        let mut header_enc = [0u8; 100];
        let  _ = stream.read_exact(&mut header_enc[..]).await?;

        let header = Header::decode_from_slice(&header_enc[..])?;
        Ok(header)
    }

    //add the header to the stream
    pub async fn into_stream<T>(&self, stream: &mut T) -> Result<(), HError> 
        where T: AsyncWrite + Unpin,
    {
        //create a vec to store the header, maker sure the size is enough.
        let mut  header_enc = vec![0u8; 100];

        //encode the header to a vec
        let size = self.encode_into_slice(&mut header_enc[..])?;
        
        //write the header to the stream
        stream.write_all(&header_enc[..size]).await?;
     
        Ok(())
    }
    //caculate the serialized size of the data.
    fn caculate_encode_size(&self, data: &[u8]) -> Result<usize, HError> {
        //create a config for bincode, with big-endian
        let config = bincode::config::standard().with_big_endian();
        //create a size_writer as a encoder to calculate the serialized size of the data.
        let mut size_writer = bincode::enc::write::SizeWriter::default();
        let _ = bincode::encode_into_writer(data,  &mut size_writer, config);
        let size = size_writer.bytes_written;
        Ok(size)
    }

    //verify the signature of the header
    #[inline]
    fn verify_header(&self, data: &[u8]) -> Result<(), HError> {
        Identity::verify_signature_bytes(data, &self.public_key, &self.signature)
    }
    

}



#[derive(Debug, Clone, Decode, Encode, PartialEq)]
pub enum Payload
{
    ///request for history chain, with a timestamp bigger than the given one
    ChainRequest (RequestInfo),
    ///vote for a block if the block's validity is suspected.
    VoteBlock(BlockChain<VoteBlock>),
    ///if the vote report show that the block is invalid, the data of the block keeped should be
    ///recitified by the network.
    BlockRecitify(BlockRec),
    ///serch friends, HashValue is the name of the node that want to search for friends.
    SearchFriend(HashValue),
    ///empty, 
    Empty,
    ///introduce a friend to the network.
    Introduce,
}

#[derive(Debug, Clone, Decode, Encode, PartialEq)]
pub struct RequestInfo {
    src_addr: String,
    timestamp: u64,
}


unsafe impl Send for Payload {}

pub struct VoteBlockArgs {
    pub prev_hash: HashValue,
    pub expire_time: u64,
    pub block_hash: HashValue,
    pub voter: HashValue,
    pub vote: bool,
    pub suspected_block: HashValue,
    pub result: f32,
    pub index: usize,
    pub digest_id: u32,
}

impl VoteBlockArgs {
    pub fn new(
        prev_hash: HashValue,
        expire_time: u64,
        block_hash: HashValue,
        voter: HashValue,
        vote: bool,
        suspected_block: HashValue,
        result: f32,
        index: usize,
        digest_id: u32,
    ) -> Self {
        Self {
            prev_hash,
            expire_time,
            block_hash,
            voter,
            vote,
            suspected_block,
            result,
            index,
            digest_id,
        }
    }
}
impl BlockArgs for VoteBlockArgs {}

/// There is no poller's name and timestamp because the first voteblock in the chain is the poller, 
/// we can get the information from the first block of the chain (not the genesis block, the poller's
/// block is the second block strictly speaking in the chain).
/// The result will be a number between 0 and 1, the consensus of the network, ervery block's result
/// is the result according to the votes of the previous blocks.
#[derive(Debug, Clone, Decode, Encode, PartialEq)]
pub struct VoteBlock {
    ///hash of this block
    pub hash: HashValue,
    ///hash of the previous block
    pub prev_hash: HashValue,
    ///expire time of the vote
    pub expire_time: u64,
    ///the block's hash
    pub block_hash: HashValue,
    ///the voter's name.   
    pub voter: HashValue,
    ///the voter's vote
    pub vote: bool,
    ///time of this vote
    pub timestamp: u64,
    ///the suspected block's hash
    suspected_block: HashValue,
    ///the consensus of the network, the result will be a number between 0 and 1,
    result: f32,
    ///the index of the block in the chain
    index: usize,
    ///the chain's id, the digest block's id.
    digest_id: u32,
}
impl VoteBlock {
    fn private_new(
        prev_hash: HashValue,
        expire_time: u64,
        block_hash: HashValue,
        voter: HashValue,
        vote: bool,
        suspected_block: HashValue,
        result: f32,
        index: usize,
        digest_id: u32,
    ) -> Self {
        let timestamp = chrono::Utc::now().timestamp() as u64;

        let hash = ZERO_HASH;
        let mut  block = Self {
            hash,
            prev_hash,
            expire_time,
            block_hash,
            voter,
            vote,
            timestamp,
            suspected_block,
            result,
            index,
            digest_id
        };
        let  hash = block.hash_block();
        block.hash = hash;
        block
    
    }

    pub fn new(args: VoteBlockArgs) -> Self {
        Self::private_new(
            args.prev_hash,
            args.expire_time,
            args.block_hash,
            args.voter,
            args.vote,
            args.suspected_block,
            args.result,
            args.index,
            args.digest_id,
        )
    }
}


impl Block for VoteBlock {
    type Args = VoteBlockArgs;
    #[inline]
    fn hash(&self) -> HashValue {
        self.hash
    }
    #[inline]
    fn prev_hash(&self) -> HashValue {
        self.prev_hash
    }
 
    #[inline]
    fn index(&self) -> usize {
        self.index
    }

    #[inline]
    fn digest_id(&self) -> usize {
        self.digest_id as usize
    }

    #[inline]
    fn timestamp(&self) -> u64 {
        self.timestamp
    }

    ///create a new block with the given args.
    fn create(args: Self::Args ) -> Self {
        Self::private_new(
            args.prev_hash,
            args.expire_time,
            args.block_hash,
            args.voter,
            args.vote,
            args.suspected_block,
            args.result,
            args.index,
            args.digest_id,
        )
    }
    ///give a index of the block, return a genesis block.
    fn genesis(digest_id: u32) -> Self {
        let args = VoteBlockArgs {
            prev_hash: ZERO_HASH,
            expire_time: 0,
            block_hash: ZERO_HASH,
            voter: ZERO_HASH,
            vote: false,
            suspected_block: ZERO_HASH,
            result: 0.0,
            index: 0 as usize,
            digest_id ,
        };
        Self::create(args)
    }

    ///compute the hash of the block with its fields ignoring the "hash" field.
    fn hash_block(&self) -> HashValue {
        let mut hasher = Sha256::new();
        hasher.update(self.prev_hash);
        hasher.update(self.timestamp.to_be_bytes());
        hasher.update(self.expire_time.to_be_bytes());
        hasher.update(self.block_hash);
        hasher.update(self.voter);
        hasher.update(self.vote.to_string().as_bytes());
        hasher.update(self.suspected_block);
        hasher.update(self.result.to_be_bytes());
        hasher.update(self.index.to_be_bytes());
        hasher.update(self.digest_id.to_be_bytes());
        let hash:HashValue = hasher.finalize().into();
        hash
    }

    ///verify the block's filed by block's hash.
    fn hash_verify(&self) -> Result<(), HError> {
        let hash = self.hash_block();
        if self.hash!= hash {
            return Err(HError::Block {
                message:
                    format!("block hash is not equal to the hash of its fields,
                        block hash: {:?}, hash of its fields: {:?}", self.hash, hash),
            });
        }
        Ok(())
    }
  
}




unsafe impl Send for VoteBlock {}

///block recitification message, the data of the block keeped should be 
///recitified by the network.
#[derive(Debug, Clone, Decode, Encode, PartialEq)]
pub struct BlockRec {
    old_block: HashValue,
    new_block: HashValue,
}


///A message that can be sent between nodes in the network.
#[derive(Debug, Clone, Decode, Encode, PartialEq)]
pub struct Message {
    ///the header of the message
    ///the hash name of the sender node，the public key of the sender node.
    pub sender: HashValue, 
    ///message's timestamp
    pub timestamp: u64,
    ///message's type 
    pub payload: Payload,
    ///the hash name of the receiver node
    pub receiver: HashValue,
}

impl Message {
    pub fn new(sender: HashValue, receiver: HashValue, payload: Payload ) -> Self {
        let timestamp = chrono::Utc::now().timestamp() as u64;
        Self {
            sender,
            timestamp,
            payload,
            receiver,
        }
    }

    fn new_with_zero() -> Self{
        Self {
            sender: ZERO_HASH,
            timestamp: 0,
            payload: 
                Payload::Empty,
            receiver: ZERO_HASH,  
        }
    }


    ///decode the message from a slice, verify the signature of the message with provided header.
    ///Return the message if the signature is valid, otherwise return an error.
    pub fn decode_from_slice(slice: &[u8], header: &Header) 
    -> Result<Self, HError> {
        //verify the signature of the message
        match header.verify_header(slice) {
            Ok(()) => {
                //create a config for bincode, with big-endian
                let config = bincode::config::standard().with_big_endian();
                //decode the slice to a message
                let (msg, _): ( Self, _) = bincode::decode_from_slice(slice, config)
                    .map_err(|_| HError::Message { message: "decode error in message".to_string() })?;
                Ok(msg)
            }
            Err(e) => {
                Err(HError::Message { message: format!("verify error in message: {}", e) })
            }
        }
    }

    ///encode the message into a slice, signate the message, and add a header at the head of the slice.
    ///slice's size is MAX_MSG_SIZE.
    ///We need a identity to sign the message.
    ///Reture the SIZE of MESSAGE, NOT inluding the header.
    pub fn encode_into_slice(&self, identity:&mut Identity, buffer: &mut [u8]) -> Result<usize, HError> {
        //check buffer size
        if buffer.len() < MAX_UDP_MSG_SIZE {
            return Err(HError::Message { message: "buffer size is too small".to_string() });
        }

        //create a config for bincode, with big-endian
        let config = bincode::config::standard().with_big_endian();
        //encode the message to a vector, leave enough space for the header.
        let  size = bincode::encode_into_slice(self, &mut buffer[100..], config)
            .map_err(|_| HError::Message { message: "encode error in message".to_string() })?;

        //check the size of the message
        let total_size = size + 100;
        if total_size > MAX_UDP_MSG_SIZE {
            //-----------------------------------------
            //NEED TO ADD A CHUNKING FUNCTION HERE( the public_key only send at the first time.)
            //-----------------------
            return Err(HError::Message 
                { message: "message too large, It's bigger than MAX_MSG_SIZE".to_string() 
            });    
        }

        //sign the message
        let signature = identity.sign_msg(&buffer[100..total_size]).unwrap();
        //add the header to the buffer
        let header = Header::new(total_size as u32, signature,
             identity.public_key_to_bytes());
        let _ = header.encode_into_slice(&mut buffer[..100])?;
        Ok(size)
    }

    //send the message to a stream.
    pub async fn into_stream<S> (&self, identity: &mut Identity, stream: &mut S) -> Result<(), HError>
        where S: AsyncWrite + Unpin,
    {
        //encode the message to a vector
        let mut  uninit_buf = Vec::with_capacity(MAX_UDP_MSG_SIZE);
        unsafe {
            uninit_buf.set_len(MAX_UDP_MSG_SIZE);
        };
        let size = self.encode_into_slice(identity, &mut uninit_buf[..])?;
        //write the message to the stream
        stream.write_all(&&uninit_buf[..size]).await?;
        Ok(())
    }
    
    async fn from_stream <S>  (stream: &mut S) -> Result<Message, HError> 
        where S: tokio::io::AsyncRead + Unpin,
    {
        //get the header from the stream
        let header = Header::from_stream(stream).await?;

        //get the encoded message from the stream
        let msg_byte_size = header.length as usize;
        let mut uninit_buf = Vec::with_capacity(msg_byte_size);
        unsafe {
            uninit_buf.set_len(msg_byte_size);
        }
        stream.read_exact(&mut uninit_buf[..]).await?;

        //decode the message from the buffer
        let msg =Message::decode_from_slice(&uninit_buf[..], &header)?;
        Ok(msg)
    }

}


unsafe impl Send for Message {}


///a handler for network messages.
#[async_trait]
trait Handler
{
    fn handle_block_recitify(&self, msg: Message) -> Result<(), HError>;
    fn handle_chain_request(&self, msg: Message) -> Result<(), HError>;
    fn handle_vote_block(&self, msg: Message) -> Result<(), HError>;
    fn handle_search_friend(&self, msg: Message) -> Result<(), HError>;
}


pub struct MessageHandler  {
    //this is a pipe to this node's chain keeper.
    pipe: Pipe<Message>,
}

#[async_trait]
impl Handler for MessageHandler {
    fn handle_block_recitify(&self, msg: Message) -> Result<(), HError> {
        //TO DO
        Ok(())
    }
    fn handle_chain_request(&self, msg: Message) -> Result<(), HError> {
        //TO DO
        Ok(())
    }
 
    
    fn handle_vote_block(&self, msg: Message) -> Result<(), HError> {
        //TO DO
        Ok(())
    }
    fn handle_search_friend(&self, msg: Message) -> Result<(), HError> {
        //TO DO
        Ok(())
    }
}


impl MessageHandler {
    pub fn new(pipe_to_chain_keeper: Pipe<Message>)
        -> Self {
        Self {
            pipe: pipe_to_chain_keeper,
        }
    }

    pub async fn handle(&self, msg: Message) -> Result<(), HError> {
        match msg.payload {
            Payload::BlockRecitify(_) => self.handle_block_recitify(msg),
            Payload::ChainRequest(_) => self.handle_chain_request(msg),
            Payload::VoteBlock(_) => self.handle_vote_block(msg),
            Payload::SearchFriend(_) => self.handle_search_friend(msg),
            Payload::Introduce => Ok(()), //TO DO
            Payload::Empty => Ok(()),
        }
    }

}


mod tests {
    use tokio::io::AsyncSeekExt;

    use super::*;

    #[test]
    fn test_decode_and_encode() {
        let msg = Message::new_with_zero();

        use crate::network::identity::Identity;
        let mut  iden = Identity::new();

        let size = 
            msg.encode_into_slice(&mut iden, &mut [0u8; MAX_MSG_SIZE]).unwrap();
        let header = 
            Header::decode_from_slice(&[0u8; MAX_MSG_SIZE][..100]).unwrap();
        let msg_ret = 
            Message::decode_from_slice(&[0u8; MAX_MSG_SIZE][100..size], &header ).unwrap();

        assert_eq!(msg, msg_ret);
    }
    #[tokio::test]
    async fn test_header() {
        let header = Header::new(10, [2u8; 64], [1u8; 32]);

        let buffer = [0u8; 10000];
        
        use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncRead, AsyncWrite};
        use tokio::fs::{File, OpenOptions};

        let mut stream = OpenOptions::new()
            .read(true)
           .write(true)
           .create(true)
           .open("test.bin")
            .await
            .unwrap();

        header.into_stream(&mut stream).await.unwrap();

        stream.seek(std::io::SeekFrom::Start(0)).await.unwrap();
        let ret_header = Header::from_stream(&mut stream).await.unwrap();
        assert_eq!(header, ret_header);

        tokio::fs::remove_file("test.bin").await.unwrap();

    }
}