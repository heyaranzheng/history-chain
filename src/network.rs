use tokio::net::{TcpStream, UdpSocket, TcpListener};
use std::net::SocketAddr;
use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use std::sync::Arc;

use crate::constants::{
    MAX_CONNECTIONS , UDP_SENDER_PORT, UDP_RECV_PORT, TCP_SENDER_PORT, 
    TCP_RECV_PORT, MAX_MSG_SIZE, MAX_UDP_MSG_SIZE, MTU_SIZE};
use crate::herrors;
use crate::herrors::HError;
use crate::herrors::logger_error_with_error;
use crate::herrors::logger_info;
use crate::message;
use crate::message::Message;
use crate::hash:: {HashValue, Hasher};
use crate::nodes::UserNode;
use crate::pipe::Pipe;

///signals used to  communicate with other threads
pub enum Signal {
    Close,
    ListenResult(TcpStream, String),
}
impl Signal {
    ///create a new listen result signal
    fn new_listen_result( listen_result: (TcpStream, SocketAddr)) -> Self {
        let sockaddr_str = format!("{}:{}", listen_result.1.ip(), listen_result.1.port());
        Self::ListenResult(listen_result.0, sockaddr_str)
    }
}

#[async_trait]
pub trait NetWork{
    ///get a friend node's address by its name
    fn get_friend_address(&self, name: HashValue) -> Result<Option<String>, HError>;
    ///get my address
    fn my_address(&self) -> Option<String>;

    ///udp connection, receive message from all nodes, return message and source address
    async fn udp_recv_from(&self) -> Result< (Message, String), HError> {
        let mut buf = vec![0; MAX_UDP_MSG_SIZE];
 
        //bind to udp port for any address
        let udp_socket = UdpSocket::bind(format!("0.0.0.0:{}", UDP_RECV_PORT)).await?;
        //receive data from udp socket
        let (size, src_addr) = udp_socket.recv_from(&mut buf).await?;
        let src_addr_str = format!("{}:{}", src_addr.ip(), src_addr.port());
        
        //deserialize data to message
        let msg = Message::decode_from_slice(&buf[..size])?;

        Ok((msg, src_addr_str))
    }
    ///udp connection, send message to another node
    async fn udp_send_to(&self, dst_addr: String, msg: &Message) -> Result<usize, HError> 
    {
        let msg_encoded = msg.encode_to_vec()?;
        //check the buffer size
        if msg_encoded.len() > MAX_UDP_MSG_SIZE {
            return Err(
                HError::NetWork { 
                    message: format!("buffer size is too large, need at most {}", MAX_UDP_MSG_SIZE)
                } 
            );
        }
        //bind self address and udp port
        if let Some(my_addr) = self.my_address() { 
            let udp_socket =UdpSocket::bind(format!("{}", my_addr)).await?;
            //send data to dst_addr
            let _ = udp_socket.send_to(&msg_encoded[..], dst_addr).await?;
            Ok(msg_encoded.len())
        }else {
            Err(
                HError::NetWork {
                    message: format!("my address is not set")
                }
            )
        }
    }
}

///create a new thread to listen tcp port for incoming connections
async fn tcp_listen_with_thread( mut pipe: Pipe<Signal>) -> Result<(), HError> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", TCP_RECV_PORT)).await?;
    let liesten_task = async move {
        loop {
            //if we get a close signal, break the loop, end the thread
            if let Ok(Signal::Close) = pipe.try_recv() {
                break;
            }

            //accept incoming connections
            if let Ok(listen_result) = listener.accept().await {
                //save the address
                let save_addr = listen_result.1.clone();
                //create a signal to send to conn_task
                let signal = Signal::new_listen_result(listen_result);
                //make sure the other thread receive this signal
                if let Err(e) = pipe.send(signal).await{
                    //failed to send signal, log the error
                    let msg = format!("listen task have a send error in Pipe, error: {}", e);
                    herrors::logger_error(&msg);
                }else {
                    //the signal is sent successfully, log the info
                    let msg = 
                        format!("listen task have a new connection, and send it to conn_task, src_addr: {}:{}", 
                            save_addr.ip(), save_addr.port());
                    logger_info(&msg);
                }
            }else{ //error in accept
                let msg = format!("listen task have a accept error");
                herrors::logger_error(&msg);
            }
        }
    };
    tokio::spawn(liesten_task);
    Ok(())
}

async fn tcp_conn_with_thread(mut pipe: Pipe<Signal>, buf: &mut [u8]) -> Result<(), HError> {
    tokio::spawn(async move {
        recv_signal_and_deal(&mut pipe).await;
    });
    Ok(())
}

async fn handle_signal_listen(tcp_stream:  TcpStream, src_addr: String) -> Result<(), HError>{
    //wait for some time until some connections are closed
    let conn_counter = Arc::new(Mutex::new(0));
    let save_counter = 0;
    let mut wait_times = 0;
    loop {
        //get the connection counter
        let counter = conn_counter.lock().await;
        let save_counter_old = save_counter;
        let save_counter = *counter;
        drop(counter);

        //can deal with new connections?
        if save_counter < MAX_CONNECTIONS {
            //yes, break the loop
            break;
        }else if save_counter > save_counter_old {
            //No, the counter still increase, wait for more time
            tokio::time::sleep(std::time::Duration::from_secs(wait_times) * 2).await;
        }else {
            //No, the counter is down, wait for a while
            tokio::time::sleep(std::time::Duration::from_secs(wait_times)).await;
        }
        wait_times += 1;

        //check if it is waiting too long
        if wait_times > 10 {
            return Err::<(), HError>(
                HError::NetWork { message: format!("tcp connection is too many! waiting too long ") }
            );
        }
    }
    //deal with this connection
    tokio::spawn(async move {
        //resolute the request from client, and handle it.
        let result = 
            message::resolute_message(tcp_stream).await;
        if let Some(msg) = herrors::logger_result(result) {
            let result = message::handle_message(&msg);
            herrors::logger_result(result);
        }
    });
    Ok(())
}

async fn recv_signal_and_deal(pipe: &mut Pipe<Signal>) {
    match pipe.recv().await {
        Ok(Signal::ListenResult(tcp_stream, src_addr)) => {
            handle_signal_listen(tcp_stream, src_addr).await;
        }
        Ok(Signal::Close) => {
            return ;
        }
        Err(e) => {
            herrors::logger_error(&format!("pipe recv error: {}", e));
            return
        }
    }
}

mod tests {
    use super::*;
    use crate::{fpsc::new, message::MessageType};

    #[tokio::test(flavor = "multi_thread")]
    async fn test_udp() {
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
       
    }

}
