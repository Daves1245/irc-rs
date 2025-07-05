use tokio::net::TcpStream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::error::Error;

struct IrcMessage {
    prefix: Option<String>,
    command: String,
    params: Vec<String>
}

impl IrcMessage {
    fn parse(line: &str) -> Option<Self> {
        let mut contents = line.split_whitespace();
        let mut prefix = None;

        let first = contents.next()?;
        let (command, params) = if first.starts_with(':') {
            prefix = Some(first[1..].to_string());
            let cmd = contents.next()?.to_string();
            (cmd, contents.collect::<Vec<_>>())
        } else {
            (first.to_string(), contents.collect::<Vec<_>>())
        };

        // Handle trailing parameter (" :")
        if let Some(colon_pos) = line.find(" :") {
            let (before_colon, after_colon) = line.split_at(colon_pos + 2);
            let mut new_params: Vec<String> = before_colon.split_whitespace()
                .skip(if prefix.is_some() { 2 } else { 1 })
                .map(|s| s.to_string())
                .collect();
            new_params.push(after_colon.to_string());
            return Some(IrcMessage { prefix, command, params: new_params });
        }

        Some(IrcMessage {
            prefix,
            command,
            params: params.into_iter().map(|s| s.to_string()).collect(),
        })
    }
}

struct IrcConfig {
    server: String,
    port: u16,
    nick: String,
    username: String,
    realname: String,
    channels: Vec<String>
}

impl Default for IrcConfig {
    fn default() -> Self {
        Self {
            server: "localhost".to_string(),
            port: 6667,
            nick: "user".to_string(),
            username: "user".to_string(),
            realname: "user".to_string(),
            channels: vec!["#general".to_string()],
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = IrcConfig::default();
    let server_addr = format!("{}:{}", config.server, config.port);

    println!("Connecting to {}...", server_addr);

    // ? - return early (from main) if this errors.
    let stream = TcpStream::connect(&server_addr).await?;
    let (reader, mut writer) = stream.into_split();

    // OwnedReadHalf (returned by stream connect) doesn't implement AsyncBufRead, so we wrap in a
    // BufReader to be able to call read_line later
    let mut reader = BufReader::new(reader);

    // Connect to the irc server
    let connection_request1 = format!("NICK {}", config.nick);
    let connection_request2 = format!("USER {} 0 * :{}", config.username, config.realname);

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

        let raw_message = input.trim();

        if let Some(parsed_message) = IrcMessage::parse(raw_message) {
            if let Some(response) = handle_message(&parsed_message, &config).await {
                send_message(&mut writer, &response).await?;
            }
        } else {
            println!("Failed to parse message: {}", raw_message);
        }
    }

    Ok(())
}

async fn send_message(writer: &mut tokio::net::tcp::OwnedWriteHalf, message: &str) -> Result<(), Box<dyn Error>> {
    writer.write_all(format!("{}\r\n", message).as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

fn handle_numeric_reply(code: &str, message: &IrcMessage) {
    match code {
        "001" => println!("Connected to server"),
        "353" => {
            if message.params.len() >= 4 {
                let channel = &message.params[2];
                let users = &message.params[3];
                println!("Users in {}: {}", channel, users);
            }
        },
        "366" => {
            if message.params.len() >= 2 {
                println!("End of names list for {}", message.params[1]);
            }
        },
        "372" => {
            if let Some(msg) = message.params.last() {
                println!("{}", msg);
            }
        },
        "375" => println!("--- Message of the Day ---"),
        "376" => println!("--- End of MOTD ---"),
        _ => {
            // The above have important information. For the rest, a minimal display suffices
            if !message.params.is_empty() {
                if let Some(msg) = message.params.last() {
                    if msg.len() > 1 {
                        println!("{}", msg);
                    }
                }
            }
        }
    }
}

async fn handle_message(message: &IrcMessage, config: &IrcConfig) -> Option<String> {
    match message.command.as_str() {
        "PING" => {
            if let Some(server) = message.params.first() {
                println!("< PING {}", server);
                println!("> PONG {}", server);
                Some(format!("PONG {}", server))
            } else {
                None
            }
        }
        "001" => {
            handle_numeric_reply("001", message);
            if let Some(channel) = config.channels.first() {
                Some(format!("JOIN {}", channel))
            } else {
                None
            }
        }
        "PRIVMSG" => {
            if message.params.len() >= 2 {
                let channel = &message.params[0];
                let msg = &message.params[1];
                if let Some(ref prefix) = message.prefix {
                    let nick = prefix.split('!').next().unwrap_or(prefix);
                    println!("[{}] <{}> {}", channel, nick, msg);
                }
            }
            None
        }
        "JOIN" => {
            if let Some(channel) = message.params.first() {
                if let Some(ref prefix) = message.prefix {
                    let nick = prefix.split('!').next().unwrap_or(prefix);
                    println!("* {} joined {}", nick, channel);
                }
            }
            None
        }
        "PART" => {
            if let Some(channel) = message.params.first() {
                if let Some(ref prefix) = message.prefix {
                    let nick = prefix.split('!').next().unwrap_or(prefix);
                    println!("* {} left {}", nick, channel);
                }
            }
            None
        }
        _ => {
            if message.command.chars().all(|c| c.is_ascii_digit()) {
                handle_numeric_reply(&message.command, message);
            } else {
                println!("< {}", message.command);
            }
            None
        }
    }
}
