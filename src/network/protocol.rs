use std::mem::MaybeUninit;
use std::net::{SocketAddr};
use bincode::{Decode, Encode, config};
use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncRead, AsyncWrite,};
use tokio::net::TcpStream;
use tokio_util::sync::CancellationToken;
use async_trait::async_trait;

use crate::constants::{MAX_MSG_SIZE, MAX_UDP_MSG_SIZE, ZERO_HASH};
use crate::chain::{Chain, BlockChain};
use crate::herrors::HError;
use crate::hash:: {HashValue, Hasher};
use crate::pipe::Pipe;
use crate::block::{Block};

///the signature of the message.
type SignatureBytes = [u8; 64];



///This is the header of stream, when we create a connection by tcp.
#[derive(Debug, Decode, Encode, PartialEq)]
struct Header {
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

    //encode the header to a vec
    fn enocde_to_vec(&self, data: &[u8]) -> Result<Vec<u8>, HError> {
        //create a config for bincode, with big-endian
        let config = bincode::config::standard().with_big_endian();
        //encode the header to a vec
        let vec = bincode::encode_to_vec(data, config)
           .map_err(|_| HError::Message { message: "encode error in header".to_string() })?;
        Ok(vec)
    }

    //encode the header into a slice
    fn encode_into_slice(&self, data: &[u8], buffer: &mut [u8]) -> Result<usize, HError> {
        //create a config for bincode, with big-endian
        let config = bincode::config::standard().with_big_endian();
        //encode the header to a vec
        let size = bincode::encode_into_slice(data, buffer, config)
            .map_err(|_| HError::Message { message: "encode error in header".to_string() })?;
        Ok(size)
    }
    
    //decode the header from a slice
    fn decode_from_slice <T> (&self, data: &[u8]) -> Result<T, HError> 
        where T: Decode<()>,
    {
        //create a config for bincode, with big-endian
        let config = bincode::config::standard().with_big_endian();
        //decode the header from a vec 
        let result = 
            bincode::decode_from_slice::<T, _>(data, config)
            .map_err(|e| HError::Message { message: format!("decode error in header: {:?}", e) })?;
        Ok(result.0)
    }

    //extract the header from the stream
    async fn get_header_from_stream <T> (&self, stream:&mut T) -> Result<Header, HError> 
        where T: AsyncReadExt + Unpin,
    {
        //read the first 4 + 64 + 32 bytes of the stream, which is the length and signature of the data.
        let mut header_enc = [0u8; 4 + 32 + 64];
        let _ = stream.read_exact(&mut header_enc[..]).await?;

        let header = self.decode_from_slice( &header_enc[..])?;
        Ok(header)
    }
    //add the header to the stream
    async fn add_header_into_stream<T>(&self, stream: &mut T) -> Result<(), HError> 
        where T: AsyncWrite + Unpin,
    {
        
        //create a vec to store the header
        let mut  header_enc = Vec::<u8>::with_capacity(4 + 32 + 64);

        //encode the header to a vec
        self.encode_into_slice(self, buffer)
        let _ = self.encode_into_slice(&length_u8[..], &mut header_enc[..4])?;
        let _ = self.encode_into_slice(&signature_u8[..], &mut header_enc[4..])?;
        
        //check the size of the header_encoded
        if header_enc.len() != 32 + 4 {
            return Err(HError::Message { message: "header size error".to_string() });
        };

        //write the header to the stream
        stream.write_all(&header_enc[..]).await?;
     
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
    

}



#[derive(Debug, Clone, Decode, Encode, PartialEq)]
pub enum MessageType
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
}

#[derive(Debug, Clone, Decode, Encode, PartialEq)]
pub struct RequestInfo {
    src_addr: String,
    timestamp: u64,
}


unsafe impl Send for MessageType {}

/// There is no poller's name and timestamp because the first voteblock in the chain is the poller, 
/// we can get the information from the first block of the chain (not the genesis block, the poller's
/// block is the second block strictly speaking in the chain).
/// The result will be a number between 0 and 1, the consensus of the network, ervery block's result
/// is the result according to the votes of the previous blocks.
#[derive(Debug, Clone, Decode, Encode, PartialEq)]
pub struct VoteBlock {
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
}
impl Block for VoteBlock {}



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
    ///the hash name of the sender node，the public key of the sender node.
    pub sender: HashValue,
    ///the signature of the message, the signature is the hash of the message.
    pub signature: SignatureBytes,
    ///message's timestamp
    pub timestamp: u64,
    ///message's type 
    pub message_type: MessageType,
    ///the hash name of the receiver node
    pub receiver: HashValue,
}

impl Message {
    pub fn new_with_zero() -> Self{
        Self {
            sender: ZERO_HASH,
            timestamp: 0,
            message_type: 
                MessageType::ChainRequest(RequestInfo{src_addr: "".to_string(), timestamp: 0}),
            receiver: ZERO_HASH,  
            signature: [0u8; 64],
        }
    }
    pub fn decode_from_slice(slice: &[u8]) -> Result<Self, HError> {
        //create a config for bincode, with big-endian
        let config = bincode::config::standard().with_big_endian();
        //decode the slice to a message
        let (msg, _): ( Self, _) = bincode::decode_from_slice(slice, config)
            .map_err(|_| HError::Message { message: "decode error in message".to_string() })?;
        Ok(msg)
    }

    pub fn encode_to_vec(&self) -> Result<Vec<u8>, HError> {
        //create a config for bincode, with big-endian
        let config = bincode::config::standard().with_big_endian();
        //encode the message to a vec
        let vec = bincode::encode_to_vec(self, config)
           .map_err(|_| HError::Message { message: "encode error in message".to_string() })?;

        //check the size of the message
        if vec.len() > MAX_MSG_SIZE {
            return Err(HError::Message { message: "message too large, It's bigger than UDP_MSG_SIZE".to_string() });
        }
        Ok(vec)
    }
    pub async fn get_from_stream <S> (stream: &mut S) -> Result<Message, HError> 
        where S: tokio::io::AsyncRead + Unpin,
    {
        let mut buf: [u8; MAX_UDP_MSG_SIZE] = unsafe { MaybeUninit::uninit().assume_init()};
        let n = stream.read(&mut buf[..]).await?;
        let msg = Message::decode_from_slice(&buf[..n])?;
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
        match msg.message_type {
            MessageType::BlockRecitify(_) => self.handle_block_recitify(msg),
            MessageType::ChainRequest(_) => self.handle_chain_request(msg),
            MessageType::VoteBlock(_) => self.handle_vote_block(msg),
            MessageType::SearchFriend(_) => self.handle_search_friend(msg),
        }
    }

}


mod tests {
    use super::*;

    #[test]
    fn test_decode_and_encode() {
        let msg = Message::new_with_zero();

        let encoded = msg.encode_to_vec().unwrap();
        let decoded = Message::decode_from_slice(&encoded).unwrap();

        assert_eq!(msg, decoded);
    }
}