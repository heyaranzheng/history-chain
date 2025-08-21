use crate::constants::Hash;
use crate::chain::NomalChain;

///A message that can be sent between nodes in the network.
pub struct Message {
    ///message's hash
    pub hash: Hash,
    //the hash name of the sender node
    pub sender: Hash,
    ///message's timestamp
    pub timestamp: u64,
    ///message's type 
    pub message_type: MessageType,
    ///the hash name of the receiver node
    pub receiver: Hash,
}

pub enum MessageType {
    ///request for history chain, with a timestamp bigger than the given one
    ChainRequest(u64),
    ///vote for a block if the block's validity is suspected.
    VoteBlock(NomalChain<VoteBlock>),
    ///if the vote report show that the block is invalid, the data of the block keeped should be
    ///recitified by the network.
    BlockRecitify(BlockReci),
    ///serch friends, Hash is the name of the node that want to search for friends.
    SearchFriend(Hash),
}

/// There is no poller's name and timestamp because the first voteblock in the chain is the poller, 
/// we can get the information from the first block of the chain (not the genesis block, the poller's
/// block is the second block strictly speaking in the chain).
/// The result will be a number between 0 and 1, the consensus of the network, ervery block's result
/// is the result according to the votes of the previous blocks.
pub struct VoteBlock {
    ///expire time of the vote
    pub expire_time: u64,
    ///the block's hash
    pub block_hash: Hash,
    ///the voter's name.   
    pub voter: Hash,
    ///the voter's vote
    pub vote: bool,
    ///time of this vote
    pub timestamp: u64,
    ///the suspected block's hash
    suspected_block: Hash,
    ///the consensus of the network, the result will be a number between 0 and 1,
    result: f32,
}

///
pub struct BlockReci {
    old_block: Hash,
    new_block: Hash,
}