
use std::collections::HashMap;

use async_trait::async_trait;

use crate::nodes::{Identity, NodeInfo};
use crate::pipe::Pipe;
use crate::network::{Message, Payload, PayloadTypes};
use crate::herrors::HError;
use crate::req_resp::{RequestWorker, Response, Request};
use crate::network::register::{AsyncHandler, AsyncRegister};

/// AysncPayloadHandler is a struct that contains AsyncRegister.
pub struct AysncPayloadHandler<'context>{
    rigister: AsyncRegister,    
}


#[async_trait]
impl AsyncHandler for AysncPayloadHandler{
    async fn handle(
        &self,
        payload: Payload,
    ) -> Result<Payload, HError> {
        let payload_ret = self.rigister.handle(payload).await?;
        Ok(payload_ret)        
    }

    async fn reg_async<F, Fut>(
        &mut self,
        payload_type: PayloadTypes,
        async_handler: F,
    ) -> Result<(), HError>
        where F: Fn(Payload) -> Fut + Send + Sync + 'static,
              Fut: Future<Output = Result<Payload, HError>> + Send + 'static 
    {

    }

}



#[cfg(test)]
mod tests{
    use super::*;
    use tokio_util::sync::CancellationToken;
    use crate::network::register::*;
    use crate::{nodes::*};
    use crate::herrors::HError;
    

    #[tokio::test]
    async fn test_handler_context() {
        let nodeinfo = NodeInfo ::new();
        let cancel_token = CancellationToken::new();

        


    }
    
}