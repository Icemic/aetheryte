use futures::future::try_join;
use std::net::IpAddr;
use tokio::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub async fn proxy(
    mut inbound: TcpStream,
    dst_ip: IpAddr,
    dst_port: u16,
    info_message: String,
) -> Result<String, String> {
    let mut outbound = match TcpStream::connect("127.0.0.1:1086").await {
        Err(e) => {
            return Err(format!("{}: {}", info_message, e));
        }
        Ok(r) => r,
    };

    match outbound.write(&[5, 1, 0]).await {
        Err(e) => {
            return Err(format!("{}: {}", info_message, e));
        }
        Ok(r) => r,
    };

    let mut buf = [0 as u8; 20];
    let size = outbound.read(&mut buf).await.unwrap();
    if size != 2 || buf[0] != 5 || buf[1] != 0 {
        return Err(format!("{}: connect to socks5 proxy failed.", info_message));
    }

    let mut data: [u8; 10] = [5, 1, 0, 1, 0, 0, 0, 0, 0, 0];
    let mut i = 4;
    for item in dst_ip.to_string().split(".") {
        data[i] = item.parse::<u8>().unwrap();
        i = i + 1;
    }

    let port_big_endian = dst_port.to_be_bytes();

    data[8] = port_big_endian[0];
    data[9] = port_big_endian[1];

    match outbound.write(&data).await {
        Err(e) => {
            return Err(format!("{}: {}", info_message, e));
        }
        Ok(r) => r,
    };

    let size = outbound.read(&mut buf).await.unwrap();
    if size == 10 && buf[0] == 5 && buf[1] == 0 {
        // println!("连接建立成功");
    } else {
        return Err(format!(
            "{}: connect to socks5 proxy failed with error code {}",
            info_message, buf[1]
        ));
    }

    let (mut ri, mut wi) = inbound.split();
    let (mut ro, mut wo) = outbound.split();

    let client_to_server = async {
        io::copy(&mut ri, &mut wo).await?;
        wo.shutdown().await
    };

    let server_to_client = async {
        io::copy(&mut ro, &mut wi).await?;
        wi.shutdown().await
    };

    match try_join(client_to_server, server_to_client).await {
        Ok(_) => Ok(info_message),
        Err(e) => Err(format!("{}: {}", info_message, e)),
    }
}
