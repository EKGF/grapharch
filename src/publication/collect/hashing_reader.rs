use {
    sha2::{Digest, Sha256},
    std::{
        io,
        pin::Pin,
        task::{Context, Poll},
    },
    tokio::io::{AsyncRead, ReadBuf},
};

pub(super) struct HashingReader<R> {
    inner: R,
    hash:  Sha256,
    bytes: u64,
    limit: u64,
}

impl<R> HashingReader<R> {
    pub fn new(inner: R, limit: u64) -> Self {
        Self { inner, hash: Sha256::new(), bytes: 0, limit }
    }

    pub fn revision(self) -> String {
        format!("rdf-v2:{}", hex::encode(self.hash.finalize()))
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for HashingReader<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let previous = buffer.filled().len();
        match Pin::new(&mut this.inner).poll_read(context, buffer) {
            Poll::Ready(Ok(())) => {
                let bytes = &buffer.filled()[previous..];
                this.bytes += bytes.len() as u64;
                if this.bytes > this.limit {
                    return Poll::Ready(Err(io::Error::other(
                        "RDF source exceeds configured byte limit",
                    )));
                }
                this.hash.update(bytes);
                Poll::Ready(Ok(()))
            },
            result => result,
        }
    }
}
