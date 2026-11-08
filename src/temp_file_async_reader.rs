use std::{
    io,
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

/// The reader of a `TempFile` that is stored on disk.
/// A `TempFile` buffered in memory never reaches this reader because its content is sent as plain bytes instead.
pub(crate) struct TempFileAsyncReader<'v> {
    // Never read, but holding it keeps the temporary file on disk until the whole response has been sent.
    #[expect(dead_code)]
    temp_file:  Box<TempFile<'v>>,
    async_file: AsyncFile,
}

impl<'v> TempFileAsyncReader<'v> {
    #[inline]
    pub(crate) fn new(temp_file: Box<TempFile<'v>>, async_file: AsyncFile) -> Self {
        TempFileAsyncReader {
            temp_file,
            async_file,
        }
    }
}

impl AsyncRead for TempFileAsyncReader<'_> {
    #[inline]
    fn poll_read(
        mut self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.async_file).poll_read(ctx, buf)
    }
}
