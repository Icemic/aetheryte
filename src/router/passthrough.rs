use futures::future::try_join;
use std::net::{IpAddr, SocketAddr};
use tokio::io;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

pub async fn passthrough(
    mut inbound: TcpStream,
    dst_ip: IpAddr,
    dst_port: u16,
    info_message: String,
) -> Result<String, String> {
    let dst_addr: SocketAddr = (dst_ip, dst_port).into();
    let mut outbound = match TcpStream::connect(dst_addr).await {
        Err(e) => {
            return Err(format!("{}: {}", info_message, e));
        }
        Ok(r) => r,
    };

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
        Err(e) => Err(format!("{}: {}", info_message, e)),
        Ok(_) => Ok(info_message),
    }
}
