use std::{
    error::Error,
    time::Duration,
    io::{Read, Write},
    net::TcpStream,
};
use byteorder::{BigEndian, WriteBytesExt};
use reqwest::Client;


#[derive(Debug)]
pub enum ResponseResult {
    Success(i32),
    StatusError(String),
}

pub async fn get_request_response_time(url: &str) -> Result<ResponseResult, Box<dyn Error>> {
    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| Box::new(e) as Box<dyn Error>)?;

    let start = std::time::Instant::now();

    let response = client.get(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.3")
        .header("Accept", "*/*")
        .header("Connection", "keep-alive")
        .send()
        .await?;

    let status = response.status();
    
    if status.is_success() {
        Ok(ResponseResult::Success(start.elapsed().as_millis() as i32))
    } else {
        Ok(ResponseResult::StatusError(status.as_str().to_string()))
    }
}

fn write_varint(val: i32, buf: &mut Vec<u8>) {
    let mut value = val as u32;
    loop {
        let mut temp = (value & 0b0111_1111) as u8;
        value >>= 7;
        if value != 0 {
            temp |= 0b1000_0000;
        }
        buf.push(temp);
        if value == 0 {
            break;
        }
    }
}

fn read_varint<R: Read>(stream: &mut R) -> std::io::Result<i32> {
    let mut result = 0;
    let mut shift = 0;
    
    loop {
        let mut byte = [0u8; 1];
        stream.read_exact(&mut byte)?;
        
        let value = (byte[0] & 0b0111_1111) as i32;
        result |= value << shift;
        
        if byte[0] & 0b1000_0000 == 0 {
            break;
        }
        
        shift += 7;
        if shift >= 32 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "VarInt is too big",
            ));
        }
    }
    
    Ok(result)
}

fn create_handshake_packet(host: &str, port: u16) -> Vec<u8> {
    let mut data = Vec::new();
    write_varint(0, &mut data);
    write_varint(764, &mut data);
    write_varint(host.len() as i32, &mut data);
    data.extend_from_slice(host.as_bytes());
    data.write_u16::<BigEndian>(port).unwrap();
    write_varint(1, &mut data);

    let mut packet = Vec::new();
    write_varint(data.len() as i32, &mut packet);
    packet.extend_from_slice(&data);
    packet
}

fn send_packet(stream: &mut TcpStream, data: &[u8]) -> std::io::Result<()> {
    let mut packet = Vec::new();
    write_varint(data.len() as i32, &mut packet);
    packet.extend_from_slice(data);
    stream.write_all(&packet)
}


pub fn get_minecraft_response_time(host: &str, port: u16) -> Result<i32, Box<dyn Error>> {
    let start = std::time::Instant::now();
    
    let response_time = match TcpStream::connect((host, port)) {
        Ok(mut stream) => {
            stream.set_read_timeout(Some(Duration::from_secs(2)))?;
            stream.set_write_timeout(Some(Duration::from_secs(2)))?;

            if let Err(_) = stream.write_all(&create_handshake_packet(host, port)) {
                return Ok(0);
            }

            if let Err(_) = send_packet(&mut stream, &[0x00]) {
                return Ok(0);
            }

            match read_varint(&mut stream) {
                Ok(_) => start.elapsed().as_millis() as i32,
                Err(_) => 0,
            }
        }
        Err(_) => 0,
    };

    Ok(response_time)
}