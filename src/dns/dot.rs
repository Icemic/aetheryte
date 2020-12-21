use crate::dns::DNSServer;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use domain::base::Message;
use rustls_native_certs::load_native_certs;
use std::io::{stdout, Read, Write};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::prelude::*;
use tokio::{net::TcpStream, time::sleep, time::timeout, time::Duration};
use tokio_rustls::{rustls::ClientConfig, webpki::DNSNameRef, TlsConnector};

impl DNSServer {
    pub async fn lookup_dot(
        &self,
        message: Message<Vec<u8>>,
        remote_addr: SocketAddr,
    ) -> Result<Message<Vec<u8>>, String> {
        let mut config = ClientConfig::new();
        config.root_store = load_native_certs().unwrap();
        let connector = TlsConnector::from(Arc::new(config));

        let socket = TcpStream::connect("google.com:443").await.unwrap();

        println!("conntcted2");
        // socket.set_nodelay(true).unwrap();
        let mut socket = connector
            .connect(
                DNSNameRef::try_from_ascii_str("google.com").unwrap(),
                socket,
            )
            .await
            .unwrap();
        // socket.set_recv_buffer_size(512).unwrap();

        println!("conntcted");

        let x = concat!(
            "GET / HTTP/1.1\r\n",
            "Host: google.com\r\n",
            "Connection: close\r\n",
            "Accept-Encoding: identity\r\n",
            "\r\n"
        );

        // let (mut r, mut w) = tokio::io::split(socket);
        // socket.write_all(&message.into_octets()).await.unwrap();
        socket.write_all(x.as_bytes()).await.unwrap();

        println!("conntcted2");
        // w.write(&[0]).await.unwrap();
        // tokio::spawn(async move {
        //     sleep(Duration::from_millis(1000)).await;
        //     // w.flush().await.unwrap();
        // });

        let duration = tokio::time::Duration::from_millis(10000);
        let mut ret_message;
        let mut is_started = false;
        let mut buf = Vec::with_capacity(4096);
        let mut expected_size = 0;
        loop {
            let size = match timeout(duration, socket.read_to_end(&mut buf)).await {
                Ok(r) => r.unwrap(),
                Err(_) => {
                    return Err("Query timeout.".to_string());
                }
            };

            // println!("{}", size);

            // if !is_started {
            //     if buf.len() > 1 {
            //         let plen = buf.get_u16();
            //         expected_size = plen as usize;
            //         if plen < 12 {
            //             panic!("below DNS minimum packet length");
            //         }
            //     } else {
            //         buf.clear();
            //     }
            // }

            
            stdout().write_all(&buf).unwrap();
            
            if expected_size + buf.len() + size != 0 {
                println!("{}, {}, {}", expected_size, buf.len(), size);
                panic!("!!!");
            }

            if expected_size > 0 && buf.len() >= expected_size {
                ret_message = Message::from_octets(buf.to_vec()).unwrap();
                if self.is_valid_response(&ret_message) {
                    break;
                } else {
                    is_started = false;
                    buf.clear();
                }
            }
            break;
        }

        // Ok(ret_message)
        Err("".to_string())
    }
}
