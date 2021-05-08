use std::fs::File;
use std::io::{self};
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::rocket::data::TempFile;

use crate::rocket::tokio::fs::File as AsyncFile;
use crate::rocket::tokio::io::{AsyncRead, ReadBuf};

enum TempFileAsyncReaderInner<'v> {
    File {
        async_file: AsyncFile,
    },
    Buffered {
        content: &'v [u8],
        pos: usize,
    },
}

pub(crate) struct TempFileAsyncReader<'v> {
    #[allow(dead_code)]
    temp_file: Box<TempFile<'v>>,
    inner: TempFileAsyncReaderInner<'v>,
}

impl<'v> TempFileAsyncReader<'v> {
    pub(crate) fn from(temp_file: Box<TempFile<'v>>) -> Result<Self, io::Error> {
        let content = if let TempFile::Buffered {
            content,
        } = temp_file.as_ref()
        {
            Some(content.as_bytes())
        } else {
            None
        };

        if let Some(content) = content {
            return Ok(TempFileAsyncReader {
                temp_file,
                inner: TempFileAsyncReaderInner::Buffered {
                    content,
                    pos: 0,
                },
            });
        }

        let async_file = if let TempFile::File {
            path,
            ..
        } = temp_file.as_ref()
        {
            println!("{:?}", path);
            AsyncFile::from_std(File::open(path)?)
        } else {
            unreachable!()
        };

        Ok(TempFileAsyncReader {
            temp_file,
            inner: TempFileAsyncReaderInner::File {
                async_file,
            },
        })
    }
}

impl<'v> AsyncRead for TempFileAsyncReader<'v> {
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
            }
        }
    }
}
