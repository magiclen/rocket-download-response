use std::{
    fs::File,
    io::{self},
    pin::Pin,
    task::{Context, Poll},
};

use rocket::{
    fs::TempFile,
    tokio::{
        fs::File as AsyncFile,
        io::{AsyncRead, ReadBuf},
    },
};

enum TempFileAsyncReaderInner<'v> {
    File { async_file: AsyncFile },
    Buffered { content: &'v [u8], pos: usize },
}

pub(crate) struct TempFileAsyncReader<'v> {
    // Never read, but holding it keeps the temporary file on disk until the whole response has been sent.
    #[allow(dead_code)]
    temp_file: Box<TempFile<'v>>,
    inner:     TempFileAsyncReaderInner<'v>,
}

impl<'v> TempFileAsyncReader<'v> {
    pub(crate) fn from(temp_file: Box<TempFile<'v>>) -> Result<Self, io::Error> {
        let inner = match temp_file.as_ref() {
            TempFile::Buffered {
                content,
            } => TempFileAsyncReaderInner::Buffered {
                content,
                pos: 0,
            },
            TempFile::File {
                path, ..
            } => TempFileAsyncReaderInner::File {
                // `Responder::respond_to` is not async, so opening the file has to be done with the blocking API.
                async_file: AsyncFile::from_std(File::open(path)?),
            },
        };

        Ok(TempFileAsyncReader {
            temp_file,
            inner,
        })
    }
}

impl AsyncRead for TempFileAsyncReader<'_> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<(), io::Error>> {
        let inner = &mut self.inner;

        match inner {
            TempFileAsyncReaderInner::File {
                async_file,
            } => Pin::new(async_file).poll_read(ctx, buf),
            TempFileAsyncReaderInner::Buffered {
                content,
                pos,
            } => {
                let data = &content[*pos..];

                let read_size = data.len().min(buf.remaining());

                buf.put_slice(&data[..read_size]);

                *pos += read_size;

                Poll::Ready(Ok(()))
            },
        }
    }
}
