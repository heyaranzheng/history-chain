use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::constants::{ZERO_HASH, MAX_MSG_SIZE};
use crate::chain::NomalChain;
use crate::herrors::HError;
use crate::hash:: {HashValue, Hasher};

///A message that can be sent between nodes in the network.
#[derive(Debug, Clone, Serialize, Deserialize, Decode, Encode, PartialEq)]
pub struct Message {
    //the hash name of the sender node
    pub sender: HashValue,
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
            message_type: MessageType::ChainRequest(0),
            receiver: ZERO_HASH,  
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
}

unsafe impl Send for Message {}


#[derive(Debug, Clone, Serialize, Deserialize, Decode, Encode, PartialEq)]
pub enum MessageType {
    ///request for history chain, with a timestamp bigger than the given one
    ChainRequest(u64),
    ///vote for a block if the block's validity is suspected.
    VoteBlock(NomalChain<VoteBlock>),
    ///if the vote report show that the block is invalid, the data of the block keeped should be
    ///recitified by the network.
    BlockRecitify(BlockReci),
    ///serch friends, HashValue is the name of the node that want to search for friends.
    SearchFriend(HashValue),
}

unsafe impl Send for MessageType {}

/// There is no poller's name and timestamp because the first voteblock in the chain is the poller, 
/// we can get the information from the first block of the chain (not the genesis block, the poller's
/// block is the second block strictly speaking in the chain).
/// The result will be a number between 0 and 1, the consensus of the network, ervery block's result
/// is the result according to the votes of the previous blocks.
#[derive(Debug, Clone, Serialize, Deserialize, Decode, Encode, PartialEq)]
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

unsafe impl Send for VoteBlock {}

///block recitification message, the data of the block keeped should be 
///recitified by the network.
#[derive(Debug, Clone, Serialize, Deserialize, Decode, Encode, PartialEq)]
pub struct BlockReci {
    old_block: HashValue,
    new_block: HashValue,
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