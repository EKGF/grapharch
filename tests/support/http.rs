use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub async fn read_request_headers(
    stream: &mut (impl tokio::io::AsyncRead + Unpin),
) -> std::io::Result<()> {
    // bounded: this local HTTP fixture accepts at most 2048 header bytes.
    let mut ending = [0; 4];
    for _ in 0..2048 {
        let mut byte = [0];
        stream.read_exact(&mut byte).await?;
        ending.rotate_left(1);
        ending[3] = byte[0];
        if ending == *b"\r\n\r\n" {
            return Ok(());
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "HTTP fixture request headers exceed 2048 bytes",
    ))
}

#[tokio::test]
async fn request_headers_handle_fragmentation_and_reject_truncation() {
    let (mut writer, mut reader) = tokio::io::duplex(1);
    tokio::spawn(async move {
        writer
            .write_all(b"GET / HTTP/1.1\r\nHost: local\r\n\r\n")
            .await
            .unwrap();
    });
    read_request_headers(&mut reader).await.unwrap();
    let mut truncated = &b"GET / HTTP/1.1\r\n"[..];
    assert_eq!(
        read_request_headers(&mut truncated)
            .await
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::UnexpectedEof
    );
}
