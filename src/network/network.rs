use async_trait::async_trait;

use crate::herrors::HError;
use crate::message;
use crate::hash:: {HashValue, Hasher};



#[async_trait]
pub trait NetWork{
    ///get a friend node's address by its name
    fn get_friend_address(&self, name: HashValue) -> Result<Option<String>, HError>;
    ///get my address
    fn my_address(&self) -> Option<String>;

}




mod tests {
    use super::*;
    use crate::{fpsc::new, message::MessageType};

    #[tokio::test(flavor = "multi_thread")]
    async fn test_network() {
        let msg = Message::new_with_zero();
        let save_msg = msg.clone();
        struct Test;

        impl NetWork for Test {
            fn get_friend_address(&self,name:HashValue) -> Result<Option<String> ,HError> {
                Ok(None)
            }
            fn my_address(&self) -> Option<String> {
                Some( "127.0.0.1:8080".to_string())
            }
        }
        let test = Test;
        let dst_addr = "127.0.0.1:8081".to_string();
        let send_task = async move {
            test.udp_send_to(dst_addr, &msg).await.unwrap();
        };
        let test = Test;
        
        use tokio::sync::mpsc;
        let (sender, mut receiver) = mpsc::channel(1);
        let recv_task = async move {
            let (msg, src_addr) = test.udp_recv_from().await.unwrap();
            sender.send( (msg, src_addr)).await.unwrap();
        };
        tokio::spawn(send_task);
        tokio::spawn(recv_task);
        let (recv_msg, src_addr) = receiver.recv().await.unwrap();
        assert_eq!(save_msg, recv_msg);
        assert_eq!(src_addr, "127.0.0.1:8080"); 

        //tcp testing
     
       
    }

}
