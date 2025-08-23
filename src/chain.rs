use serde::{Serialize, Deserialize};
use bincode::{Decode, Encode};


#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct NomalChain <T> 
{
    pub chain: Vec<T>,
}