use std::collections::HashMap;
use async_trait::async_trait;
use chrono::Utc;

use crate::block::{Block, BlockArgs, Carrier, Digester};
use crate::hash::{HashValue, Hasher};
use crate::herrors::HError;
use crate::nodes::node::{Node, NodeState, Reputation};
use crate::executor::ChainExecutor;
use crate::nodes::normal::NormalNode;

type NodeName = HashValue;

pub trait Central {

}

struct CentralNode <B, D> 
    where B: Block + Carrier,
          D: Block + Digester,   
{
    ///name of the node
    pub name: NodeName,
    ///address of the node
    pub address: String,
    ///node's birthday
    pub timestamp: u64,
    ///friend nodes, HashMap<name, NomalNode>
    pub friends: HashMap< NodeName, NormalNode<B, D>>,
    ///chain's executor
    pub executor: Option<ChainExecutor<B, D>>,
    ///node's status
    pub reputation: Reputation,
    ///node's state
    pub state: NodeState,
}

impl <B, D> CentralNode <B, D>
    where B: Block + Carrier,
          D: Block + Digester,
{
    ///create a new center with all zero but timestamp.
    ///timestamp is the current time in seconds.
    fn new(name: NodeName, address: String) -> Self{

        let timestamp = Utc::now().timestamp() as u64;

        Self {
            name,
            address,
            timestamp,
            friends: HashMap::new(),
            executor: None,
            //have a defaault reputation score with new method
            reputation: Reputation::new(),
            state: NodeState::Active,
        }
    }
}



pub struct Nodebuilder<B, D> 
    where B: Block + Carrier,
          D: Block + Digester,
{
    name: Option<NodeName>,
    address: Option<String>,
    center_address: Option<String>,
    executor: Option<ChainExecutor<B, D>>,
    friends: Option<HashMap< NodeName, NormalNode<B, D>>>,
    reputation: Option<Reputation>,
}

impl <B, D> Nodebuilder<B, D> 
    where B: Block + Carrier,
          D: Block + Digester,
{
    pub fn new() -> Self {
        Self {
            name: None,
            address: None,
            center_address: None,
            executor: None,
            reputation: None,
            friends: None,
        }
    }

    pub fn name(&mut self, name: NodeName) -> &mut Self {
        self.name = Some(name);
        self
    }
    pub fn address(&mut self, address: String) -> &mut Self {
        self.address = Some(address);
        self
    }

    pub fn center_address(&mut self, center_address: String) -> &mut Self {
        self.center_address = Some(center_address);
        self
    }

    pub fn executor(&mut self, executor: ChainExecutor<B, D>) -> &mut Self {
        self.executor = Some(executor);
        self
    }

    pub fn reputation(&mut self, reputation: Reputation) -> &mut Self {
        self.reputation = Some(reputation);
        self
    }
    pub fn friends (&mut self, friends: Option<HashMap< NodeName, NormalNode<B, D>>>) -> &mut Self {
        self.friends = friends;
        self
        
    }

    ///this is a helper to check if an option field with name "field_name" have a value.
    ///If the field is None, return an error with the message "field_name is required".
    fn check_fields<T>(&self, option: &Option<T>, fild_name: &str)-> Result<(), HError> {
        if option.is_none() {
            return Err(
                HError::Nodes {
                    message: format!("{} is required", fild_name),
                }
            );
        }
        Ok(())
        
    } 


    pub fn build(self) -> Result<CentralNode<B, D>, HError> 
    
    {
        //check if have a name
        self.check_fields(&self.name, "name")?;
        //check if have an address
        self.check_fields(&self.address, "address")?;

        //get the name and address
        let name = self.name.unwrap();
        let address = self.address.unwrap();

        //create a new center
        let mut center = 
            CentralNode::<B, D>::new(name, address);
        center.executor = self.executor;
        
        //-----------------default value------
        center.state = NodeState::Active;
        //reputation is still a default value.

        //if friends is Node, create a new HashMap<NodeName, NomalNode>
        if let Some(friends) = self.friends {
            center.friends = friends;
        }else {
            center.friends = HashMap::new();
        }
        
        Ok(center)        
    }
}

