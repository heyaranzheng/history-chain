use serde::{Serialize, Deserialize};
use bincode::{Decode, Encode};


#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode, PartialEq)]
pub struct NomalChain <T> 
{
    pub chain: Vec<T>,
}

pub trait Chain{
    
}