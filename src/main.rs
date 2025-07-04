use tokio::net::TcpStream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    let server = "localhost:6667";
    let mut nick = "user";
    let mut channel = "#general";

    println!("Connecting to {}...", server);

    // ? - return early (from main) if this errors.
    let stream = TcpStream::connect(server).await?;
    let (reader, mut writer) = stream.into_split();

    // OwnedReadHalf (returned by stream connect) doesn't implement AsyncBufRead, so we wrap in a
    // BufReader to be able to call read_line later
    let mut reader = BufReader::new(reader);

    // Connect to the irc server
    let connection_request1 = format!("NICK {}", nick);
    let connection_request2 = format!("USER {} 0 * :{}", nick, nick);

    send_message(&mut writer, &connection_request1).await?;
    send_message(&mut writer, &connection_request2).await?;

    let mut input = String::new();
    loop {
        input.clear();
        let bytes_read = reader.read_line(&mut input).await?;

        if bytes_read == 0 {
            println!("Connection closed");
            break;
        }

        let message = input.trim();
        // IRC PING message. server sends
        // PING :irc.server.com
        // client must respond with
        // PONG :irc.server.com
        // or get disconnected after a timeout
        if message.starts_with("PING") {
            let pong = message.replace("PING", "PONG");
            send_message(&mut writer, &pong).await?;
            println!("> {}", pong);
        } else if message.contains("001") { // server messages have 3digit codes, and 001 is
                                            // RPL_WELCOME
            println!("Successfully registered. Joining {}...", channel);
            send_message(&mut writer, &format!("JOIN {}", channel)).await?;
        } else if message.contains("PRIVMSG") {
            println!("Received chat message: {}", message);
        }
    }

    Ok(())
}

async fn send_message(writer: &mut tokio::net::tcp::OwnedWriteHalf, message: &str) -> Result<(), Box<dyn Error>> {
    writer.write_all(format!("{}\r\n", message).as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}
