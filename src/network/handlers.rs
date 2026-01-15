
use async_trait::async_trait;

use crate::nodes::Identity;
use crate::pipe::Pipe;
use crate::network::Message;
use crate::herrors::HError;


///a handler for network messages.
#[async_trait]
trait Handler
{
    fn handle_block_recitify(&self, identity: &mut Identity, msg: Message) -> Result<(), HError>;
    fn handle_chain_request(&self, identity: &mut Identity, msg: Message) -> Result<(), HError>;
    fn handle_vote_block(&self, identity: &mut Identity, msg: Message) -> Result<(), HError>;
    fn handle_search_friend(&self, identity: &mut Identity, msg: Message) -> Result<(), HError>;
    
    
    //--------------
    //async fn handle_introduce(&self, identity: &mut Identity, msg: Message) {}
}


pub struct MessageHandler  {
    //this is a pipe to this node's chain keeper.
    pipe: Pipe<Message>,
}

#[async_trait]
impl Handler for MessageHandler {
   
    fn handle_block_recitify(&self, identity: &mut Identity, msg: Message) 
        -> Result<(), HError> 
    {
        //TO DO
        Ok(())
    }
    fn handle_chain_request(&self, identity: &mut Identity, msg: Message) 
        -> Result<(), HError> 
    {
        //TO DO
        Ok(())
    }
 
    
    fn handle_vote_block(&self, identity: &mut Identity, msg: Message) 
        -> Result<(), HError> 
    {
        //TO DO
        Ok(())
    }
    fn handle_search_friend(&self, identity: &mut Identity, msg: Message) 
        -> Result<(), HError> 
    {
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
    async fn handle_message(&self, identity: &mut Identity, msg: Message) 
        -> Result<(), HError> { 
            //TO DO
            Ok(())
    }

}