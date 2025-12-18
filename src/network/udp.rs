use std::net::{Ipv4Addr, IpAddr, SocketAddr};
use async_trait::async_trait;
use futures::future::join_all;

use tokio::net::{UdpSocket};
use tokio::task::spawn;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use crate::req_resp::{RequestWorker, Request, create_channel};
use crate::constants::{ CHANNEL_CAPACITY,
    MAX_MSG_SIZE, MAX_UDP_MSG_SIZE, UDP_CHECK_PORT, ZERO_HASH,
};
use crate::herrors::{HError, self};
use crate::nodes::Identity;
use crate::network::protocol::{Message, Payload, Header};
use crate::hash::HashValue;
use crate::nodes::SignHandle;

#[async_trait]
pub trait UdpConnection: Send + Sync {
    ///udp connection, receive message from all nodes, return message and source address
    ///if timeout, return error
    async fn udp_recv_from(
        my_name: &HashValue,
        timeout_ms: u64,
        bind_addr: SocketAddr, 
    ) -> Result< (Message, SocketAddr), HError> 
    {
        let mut uninit_buffer = Vec::with_capacity(MAX_UDP_MSG_SIZE);
        unsafe { uninit_buffer.set_len(MAX_UDP_MSG_SIZE) };

        //bind to udp port for any address
        let udp_socket = UdpSocket::bind(bind_addr).await?;

        //create a task to receive data from udp socket.
        let timeout_duration = std::time::Duration::from_millis(timeout_ms);
        let (size, src_addr);
        let result = 
            timeout(timeout_duration, udp_socket.recv_from(&mut uninit_buffer)).await;
        match result {
            Err(_) => {
                return Err(HError::NetWork { message: format!("udp_recv_from: timeout") });
            }
            Ok(result) => {
                match result {
                    Ok(result) => {
                        size = result.0;
                        src_addr = result.1;
                    },
                    Err(_) => {
                        return Err(HError::NetWork { message: format!("udp_recv_from: failed") });
                    }
                }
            }
        }

        //decode message from buffer with header
        let msg =
            Message::decode_from_slice(my_name, &uninit_buffer[..size])?;

        Ok((msg, src_addr))
    }


    /// Default Implementation:
    ///udp connection, send message to another node, return the total size of 
    ///the message and the header 
    /// This function will encode the message with a header, and then send it to a signer,
    /// wait for the signature, and then send the message to the destination address.
    /// # Arguments
    /// * `dst_addr` - the destination address to send the message to.
    /// * `msg` - the message to send.
    /// * `sign_handle` - the signer to sign the message.
    /// # Returns
    /// * `Result<usize, HError>` - the total size of the message and the header.   
    async fn udp_send_to(
        dst_addr: SocketAddr, 
        msg: &Message,
        sign_handle: &SignHandle,
    ) -> Result<usize, HError> 
        where Self: Send + Sync
    
    {
        //encode message into buffer with a header
        let mut buffer =vec![0u8; MAX_UDP_MSG_SIZE];
        let header_size = Header::header_size();
        let size = msg.encode(sign_handle, &mut buffer[..]).await?;
        let total_size = size + header_size;

        //bind self address and udp port
        let my_addr = 
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), UDP_CHECK_PORT);
        let udp_socket =UdpSocket::bind( my_addr).await?;
        //send data to dst_addr 
        let _ = udp_socket.send_to(&buffer[..total_size], dst_addr).await?;
        Ok(total_size)
        
    }

    ///Default Implementation:
    ///Send a udp message to a destination address.
    /// # Arguments
    /// * `bind_addr` - the address to bind to.
    /// * `cancel_token` - the cancellation token to cancel the task.
    /// # Returns
    /// * `Result<RequestWorker<(Vec<u8>, SocketAddr)>, HError>` - a request worker make a request
    /// for this task.
    /// 
    /// # Example
    /// ```
    /// let request_worker = spawn_udp_send_to_task(bind_addr, cancel_token).await?;
    /// let _ = req_resp::Request::send((msg_bytes, dst_addr), &request_worker).await?;
    /// ```
    /// then the task will receive the request .
    async fn spawn_udp_send_to_task(
        bind_addr: SocketAddr,
        cancel_token: CancellationToken,
    ) -> Result<RequestWorker<(Vec<u8>, SocketAddr)>, HError>
    {
        //create a channel to receive requests from other tasks
        let (requester, mut receiver) = 
            create_channel::<(Vec<u8>, SocketAddr)>(CHANNEL_CAPACITY);

        let task = async move {
            loop {
                let bind_addr_clone = bind_addr.clone();
                tokio::select! {
                    //wait for a request from other tasks
                    req_result = receiver.recv_data() => {
                        match req_result {
                            Ok(req_result) => {
                                let (msg_bytes, dst_addr) = req_result;
                                
                                //send the message to dst_addr
                                let result = socket_wrapper::send_to(
                                    bind_addr_clone,
                                    dst_addr,
                                    &msg_bytes[..],
                                ).await;
                                match result {
                                    Ok(size) => {
                                        //send success, log it
                                        let ip = dst_addr.ip();
                                        let port = dst_addr.port();

                                        herrors::logger_info(
                                            &format!(
                                                "udp_send_to_task: send to {}:{} size: {}", ip , port, size
                                            )    
                                        );
                                        continue;
                                    },
                                    Err(e) => {
                                       //send failed, log it
                                       herrors::logger_error_with_error(&e);
                                       continue;
                                    }
                                }
                            },
                            Err(e) => {
                                //bad request, log it
                                herrors::logger_error_with_error(&e);
                                continue;
                            }
                        }
                    },
                    _ = cancel_token.cancelled() => {
                        //got an exitting signal
                        break;
                    }
                }
            }
        };
        tokio::task::spawn(task);
        Ok(requester)
    }

    ///an helper function for check_addresses_available, check if a addr is available
    async fn check_addr_available(
        my_name: &HashValue,
        addr: &SocketAddr, 
        timeout_duration: std::time::Duration,
        msg_byte: &[u8],
    ) -> Result<SocketAddr, HError> {
        let mut buffer = vec![0u8; MAX_UDP_MSG_SIZE];
        //create a socket and send an message to dst_addr
        let bind_addr = format!("0.0.0.0:{}",UDP_CHECK_PORT);
        let udp_socket =UdpSocket::bind(bind_addr).await?;
        
        //------spawn
        udp_socket.send_to(msg_byte, addr).await?;
 

        //waiting for response from dst_addr
        let (size, dst_addr) = 
            timeout(timeout_duration, udp_socket.recv_from(&mut buffer[..]))
            .await
            .map_err(
               |e| HError::NetWork { message: format!("check_addr_available: timeout: {}", e) }
            )??;
        //check the address is the same as dst_addr
        if dst_addr.ip() != addr.ip() {
            return Err(HError::NetWork { message: format!("check_addr_available: failed") });
        }

        //check the message
        let msg = Message::decode_from_slice(my_name, &buffer[..size])?;
        if msg.payload != Payload::Empty { 
            return Err(HError::NetWork { message: format!("check_addr_available: failed") });
        }
        Ok(*addr)
        
    }
   
    async fn check_addresses_available(
        addr_list: &Vec<SocketAddr>, 
        timeout_ms: u64,
        receiver: HashValue,
        sign_handle: SignHandle,
    ) -> Result< Vec<SocketAddr>, HError> 
        where Self: Send + Sync
    {
        //check if the addr_list is empty.
        if addr_list.is_empty() {
            return Err(HError::NetWork { message: format!("upd_connection error: have no addr_list") });
        }

        //the name of the node
        let my_name = &sign_handle.public_key_bytes();
        //a buffer for encoded message
        let mut buffer = vec![0u8; MAX_MSG_SIZE];

        //time out duration, 1 second.
        let timeout_duration = std::time::Duration::from_millis(timeout_ms);

        //create a message and sign it with identity, encode it into a byte array.
        let test_msg = Message::new(*my_name, receiver, Payload::Empty);
        let total_len = test_msg.encode(&sign_handle, &mut buffer[..]).await?;

        //create tasks to check if each address is available 
        let tasks = addr_list.iter().map(|addr| {
            let buffer_clone = buffer.clone();
            async move {
                Self::check_addr_available(
                    my_name, 
                    addr, 
                    timeout_duration, 
                    &buffer_clone[..total_len]).await
            }
        });

        //wait for all tasks to complete and collect the available addresses.
        let results = join_all(tasks).await;
        let addresses_available = results.iter().filter_map(
            |result| {
                match result {
                    Ok(addr) => {
                        Some(*addr)
                    },
                    Err(_) => {
                        None
                    }
                }
            }
        ).collect();

        Ok(addresses_available)
    }
   
}

mod socket_wrapper {

    use std::net::{ SocketAddr};
    use  crate::herrors::HError;
    use crate::constants::MAX_UDP_MSG_SIZE;
    use tokio::net::UdpSocket;
    
    ///this is a wrapper for udp socket send_to 
    pub async fn send_to(
        bind_addr: SocketAddr,
        dst_addr: SocketAddr,
        msg_bytes: &[u8],
    ) -> Result<usize, HError> {
        //check the size of msg_bytes
        if msg_bytes.len() > MAX_UDP_MSG_SIZE {
            return Err(HError::NetWork { message: format!("send_to: msg_bytes is too large") });
        }
        
        let udp_socket = UdpSocket::bind(bind_addr).await?;
        let size = udp_socket.send_to(msg_bytes, dst_addr).await?;
        
        Ok(size)
    }
}

mod tests {
    use sqlx::encode;
    use tokio::time::timeout;
    use tokio_util::sync::CancellationToken;
    use super::*;
    use crate::constants::{ZERO_HASH, MAX_UDP_MSG_SIZE};
    use crate::network::{Message, Payload};
    use crate::nodes::Identity;
    
    struct Test;
    impl UdpConnection for Test {}


    #[tokio::test(flavor = "multi_thread")]
    async fn test_network() {
        let  encode_id = Identity::new();
        let signal_handle = 
            SignHandle::spawn_new(encode_id, 32, CancellationToken::new()).await.unwrap();
        let decode_id = Identity::new();

        let msg = Message::new(
            signal_handle.public_key_bytes(), 
             decode_id.public_key.to_bytes(), 
             Payload::Empty
        );
        let save_msg = msg.clone();

        let dst_addr = SocketAddr::new(
            std::net::Ipv4Addr::new(127, 0, 0, 1).into(), 
            8081
        );
        let send_task = async move {
            Test::udp_send_to( dst_addr, &msg, &signal_handle).await.unwrap();
        };

        
        //crate a mpsc to send and recv message
        use tokio::sync::mpsc;
        let (sender, mut receiver) = mpsc::channel(1);
        let bind_addr = SocketAddr::new(
            std::net::Ipv4Addr::new(127, 0, 0, 1).into(), 
            8080
        );
        let decoder_name = decode_id.public_key.to_bytes();
        let recv_task = async move {
            let (msg, src_addr) = Test::udp_recv_from(
                &decoder_name,
                110,bind_addr
            ).await.unwrap();
            sender.send( (msg, src_addr)).await.unwrap();
        };
        //if we sync the code below, we should use recv_task first, then send_task, 
        tokio::spawn(send_task);
        tokio::spawn(recv_task);
        let (recv_msg, src_addr) = receiver.recv().await.unwrap();
        assert_eq!(save_msg, recv_msg);
        assert_eq!(src_addr, bind_addr); 
         
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn check_addr_available_test() -> Result<(), HError> { 
        let mut server_id = Identity::new();
        let mut client_id = Identity::new();

        let server_name = server_id.public_key.to_bytes();
        let server_name_clone = server_name.clone();
        let client_name = client_id.public_key.to_bytes();
       

        //create a notifier to tell the listener is ready
        let (tx,  rx) = tokio::sync::oneshot::channel();
        let timeout_duration = std::time::Duration::from_millis(3000);
     
        //create a udp listener
        let task  = async move {
            
            let socket = UdpSocket::bind("127.0.0.1:8080").await.unwrap();
            tx.send(true).unwrap();
            let mut buffer = vec![0u8; MAX_UDP_MSG_SIZE];
        
            let looper_task = async {
                loop {
                    let (_, src_addr) = socket.recv_from(&mut buffer[..]).await.unwrap();
                    let msg = Message::new(
                        server_name, 
                        client_name,
                        Payload::Empty
                    );
                    let mut listener_buffer = vec![0u8; MAX_UDP_MSG_SIZE];
                   
                    msg.encode_into_slice(&mut server_id , &mut listener_buffer[..]).unwrap();
                    socket.send_to(&listener_buffer[..], src_addr).await.unwrap();
                }
            };
            match timeout(timeout_duration, looper_task).await {
                Ok(_) => {
                    Ok(())
                },
                Err(_) => {
                    Err(HError::NetWork { message: "check addr available timeout".to_string() })
                }
            }

        };

        //spawn the listener task
        tokio::spawn(task);

        rx.await.unwrap();

        //set addr to 127.0.0.1:8080
        let ip = Ipv4Addr::new(127, 0, 0, 1);
        let valid_port = 8080;
        let valid_addr = SocketAddr::new(ip.into(), valid_port);
        let invalid_ip = Ipv4Addr::new(127, 0, 0, 2);
        let invalid_port = 9999;
        let invalid_addr = SocketAddr::new(invalid_ip.into(), invalid_port);

       

        //create a msg to send
        let msg = Message::new(
            client_id.public_key.to_bytes(), 
            server_name_clone,
            Payload::Empty
        );
        let mut msg_bytes = vec![0u8; MAX_UDP_MSG_SIZE];
        msg.encode_into_slice(&mut client_id,&mut  msg_bytes)?;

    
        let result_valid = 
            Test::check_addr_available(
                &client_id.public_key.to_bytes(),
                &valid_addr, 
                timeout_duration,
                 &msg_bytes[..]
            ).await;
        match result_valid{
            Ok(_) => {
                let address = result_valid.unwrap();
                println!("address: {:?}", address);
            }
            Err(e) => {
                println!("error: {:?}", e);
            }
        }
        //assert!(result_valid.is_ok());

  
        let result_invalid = 
            Test::check_addr_available(
                &client_id.public_key.to_bytes(),
                &invalid_addr, 
                timeout_duration, 
                &msg_bytes[..]
            ).await;
        assert_eq!(result_invalid.is_ok(), false);
        Ok(())

    }

    //test for check_addresses_available function
    #[tokio::test(flavor = "multi_thread")]
    async fn test_check_addresses_available() {
        //preparation 

        //create two identities
        let mut client_id = Identity::new();
        let mut server_id = Identity::new();
        let server_name = server_id.public_key.to_bytes();
        let client_name = client_id.public_key.to_bytes();
        let server_name_clone = server_name.clone();
        let sign_handle =
            SignHandle::spawn_new(client_id, 32, CancellationToken::new()).await.unwrap();


        //create a timeout server to response the check message
        let duration = std::time::Duration::from_millis(3000);  
        let task = async move {
            let udp_listener = UdpSocket::bind("127.0.0.1:8080").await.unwrap();

            let mut buffer = vec![0u8; MAX_UDP_MSG_SIZE];

            let looper_task = async {
                loop {
                    let (_, src_addr) = udp_listener.recv_from(&mut buffer[..]).await.unwrap();
                    let msg = Message::new(
                        server_name, 
                        client_name, 
                        Payload::Empty
                    );
                    let mut listener_buffer = vec![0u8; MAX_UDP_MSG_SIZE];
                    
                    msg.encode_into_slice(&mut server_id , &mut listener_buffer[..]).unwrap();
                    udp_listener.send_to(&listener_buffer[..], src_addr).await.unwrap();
                }
            };

            let _ = timeout(duration, looper_task).await;         
        };

        tokio::spawn(task);
        //wait for the listener to start
        let sleep_duration = std::time::Duration::from_millis(1000);
        tokio::time::sleep(sleep_duration).await;
        //----------preparation end----------


        //addresses list for test
        let addresses = vec![
            SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 8080),
            SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 8081),
            SocketAddr::new(Ipv4Addr::new(127, 0, 0, 2).into(), 8080)
        ];

        let result = 
            Test::check_addresses_available(
                &addresses, 
                3000, 
                server_name_clone,
                sign_handle
            ).await;
        
        assert_eq!(result.is_ok(), true);

        let vec = result.unwrap();
        assert_eq!(vec.len(), 1);
        assert_eq!(vec[0], SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 8080));
    }
}
