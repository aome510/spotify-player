use std::{
    io::{Read, Write},
    time::Duration,
};

use anyhow::{Context, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const IO_TIMEOUT: Duration = Duration::from_secs(30);
pub const MAX_REQUEST_SIZE: usize = 4 * 1024;
pub const MAX_RESPONSE_SIZE: usize = 32 * 1024 * 1024;

fn frame_length(size: usize, max_size: usize, description: &str) -> Result<u32> {
    anyhow::ensure!(
        size <= max_size,
        "{description} is {size} bytes, but the maximum is {max_size} bytes"
    );
    u32::try_from(size).context("frame length exceeds the protocol limit")
}

pub fn read_frame(reader: &mut impl Read, max_size: usize, description: &str) -> Result<Vec<u8>> {
    let mut header = [0; size_of::<u32>()];
    reader
        .read_exact(&mut header)
        .with_context(|| format!("read {description} length"))?;
    let size = u32::from_be_bytes(header) as usize;
    frame_length(size, max_size, description)?;

    let mut data = vec![0; size];
    reader
        .read_exact(&mut data)
        .with_context(|| format!("read complete {description} body ({size} bytes)"))?;
    Ok(data)
}

pub fn write_frame(
    writer: &mut impl Write,
    data: &[u8],
    max_size: usize,
    description: &str,
) -> Result<()> {
    let size = frame_length(data.len(), max_size, description)?;
    writer
        .write_all(&size.to_be_bytes())
        .with_context(|| format!("write {description} length"))?;
    writer
        .write_all(data)
        .with_context(|| format!("write complete {description} body"))?;
    writer.flush().context("flush IPC stream")?;
    Ok(())
}

pub async fn read_frame_async(
    reader: &mut (impl AsyncRead + Unpin),
    max_size: usize,
    description: &str,
) -> Result<Vec<u8>> {
    let mut header = [0; size_of::<u32>()];
    reader
        .read_exact(&mut header)
        .await
        .with_context(|| format!("read {description} length"))?;
    let size = u32::from_be_bytes(header) as usize;
    frame_length(size, max_size, description)?;

    let mut data = vec![0; size];
    reader
        .read_exact(&mut data)
        .await
        .with_context(|| format!("read complete {description} body ({size} bytes)"))?;
    Ok(data)
}

pub async fn write_frame_async(
    writer: &mut (impl AsyncWrite + Unpin),
    data: &[u8],
    max_size: usize,
    description: &str,
) -> Result<()> {
    let size = frame_length(data.len(), max_size, description)?;
    writer
        .write_all(&size.to_be_bytes())
        .await
        .with_context(|| format!("write {description} length"))?;
    writer
        .write_all(data)
        .await
        .with_context(|| format!("write complete {description} body"))?;
    writer.flush().await.context("flush IPC stream")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Cursor, Read},
        net::{TcpListener, TcpStream},
        thread,
    };

    use super::{
        read_frame, read_frame_async, write_frame, write_frame_async, MAX_REQUEST_SIZE,
        MAX_RESPONSE_SIZE,
    };

    struct ChunkedReader<R> {
        inner: R,
        max_chunk_size: usize,
    }

    impl<R: Read> Read for ChunkedReader<R> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let limit = buf.len().min(self.max_chunk_size);
            self.inner.read(&mut buf[..limit])
        }
    }

    #[test]
    fn oversized_frame_returns_an_error() {
        let mut output = Vec::new();
        let data = vec![0; MAX_REQUEST_SIZE + 1];

        let error = write_frame(&mut output, &data, MAX_REQUEST_SIZE, "CLI request").unwrap_err();

        assert!(error.to_string().contains("maximum"));
        assert!(output.is_empty());
    }

    #[test]
    fn incomplete_frame_returns_an_error() {
        let mut data = (5_u32).to_be_bytes().to_vec();
        data.extend_from_slice(b"ab");

        let error = read_frame(&mut Cursor::new(data), 10, "CLI response").unwrap_err();

        assert!(error.to_string().contains("complete CLI response body"));
    }

    #[test]
    fn response_can_arrive_in_many_transport_chunks() {
        let expected = vec![7; 3 * 4096 + 17];
        let mut frame = Vec::new();
        write_frame(&mut frame, &expected, MAX_RESPONSE_SIZE, "CLI response").unwrap();
        let mut reader = ChunkedReader {
            inner: Cursor::new(frame),
            max_chunk_size: 37,
        };

        let actual = read_frame(&mut reader, MAX_RESPONSE_SIZE, "CLI response").unwrap();

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn async_frames_support_payloads_larger_than_the_stream_buffer() {
        let expected = vec![3; 8193];
        let sent = expected.clone();
        let (mut sender, mut receiver) = tokio::io::duplex(31);
        let write_task = tokio::spawn(async move {
            write_frame_async(&mut sender, &sent, MAX_RESPONSE_SIZE, "CLI response").await
        });

        let actual = read_frame_async(&mut receiver, MAX_RESPONSE_SIZE, "CLI response")
            .await
            .unwrap();

        write_task.await.unwrap().unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn concurrent_tcp_clients_keep_responses_isolated() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let handlers = (0..2)
                .map(|_| {
                    let (mut stream, _) = listener.accept().unwrap();
                    thread::spawn(move || {
                        let request = read_frame(&mut stream, 128, "test request").unwrap();
                        write_frame(&mut stream, &request, 128, "test response").unwrap();
                    })
                })
                .collect::<Vec<_>>();
            for handler in handlers {
                handler.join().unwrap();
            }
        });

        let clients = [b"first client".as_slice(), b"second client".as_slice()].map(|request| {
            thread::spawn(move || {
                let mut stream = TcpStream::connect(addr).unwrap();
                write_frame(&mut stream, request, 128, "test request").unwrap();
                read_frame(&mut stream, 128, "test response").unwrap()
            })
        });

        let [first, second] = clients.map(|client| client.join().unwrap());
        assert_eq!(first, b"first client");
        assert_eq!(second, b"second client");
        server.join().unwrap();
    }
}
