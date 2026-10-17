use std::{io::SeekFrom, sync::{Arc, atomic::{AtomicBool, Ordering}}};

use ring::digest::{Context, Digest, SHA256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt};

use crate::error::{codes, io_error, AppError};
use crate::sftp::error::remote_io_error;
use crate::sftp::remote_digest::{DigestRange, RemoteDigest};

const VERIFY_BUFFER_BYTES: usize = 256 * 1024;

pub(super) fn resume_mismatch(path: &str) -> AppError {
    AppError::new(codes::TRANSFER_RESUME_MISMATCH, "Resume target does not match the source")
        .with("path", path)
}

pub(super) fn source_changed(path: &str) -> AppError {
    AppError::new(codes::TRANSFER_SOURCE_CHANGED, "Source changed during transfer")
        .with("path", path)
}

fn check_active(cancellation: &Arc<AtomicBool>) -> Result<(), AppError> {
    if cancellation.load(Ordering::SeqCst) {
        Err(AppError::new(codes::TRANSFER_CANCELED, "Transfer canceled"))
    } else {
        Ok(())
    }
}

fn local_prefix_error(error: std::io::Error, path: &str) -> AppError {
    if error.kind() == std::io::ErrorKind::UnexpectedEof { resume_mismatch(path) }
    else { io_error(error).with("path", path) }
}

// 任意旧文件都可能被选作续传目标，核对过已有前缀才允许接着写。先让服务器自己算前缀摘要，
// 只回传几十字节；服务器算不了或对不上，再退回逐块读回来比，绝不在不一致的文件后面追加。
pub(super) async fn verify_resume_prefix<L, R>(
    local: &mut L,
    remote: &mut R,
    remote_digest: &RemoteDigest,
    length: u64,
    path: &str,
    cancellation: &Arc<AtomicBool>,
) -> Result<Context, AppError>
where
    L: AsyncRead + AsyncSeek + Unpin,
    R: AsyncRead + AsyncSeek + Unpin,
{
    if remote_digest.applies_to(length) {
        let digest = digest_prefix(local, length, path, cancellation).await?;
        let expected = digest.clone().finish();
        if remote_digest.matches(path, DigestRange::Prefix(length), expected.as_ref(), cancellation).await? {
            remote.seek(SeekFrom::Start(length)).await.map_err(|error| remote_io_error(error, path))?;
            return Ok(digest);
        }
        local.seek(SeekFrom::Start(0)).await.map_err(|error| io_error(error).with("path", path))?;
    }
    verify_prefix(local, remote, length, path, cancellation).await
}

// 本地读前缀只走磁盘，读完句柄正好停在续传位置。
async fn digest_prefix<L>(
    local: &mut L,
    length: u64,
    path: &str,
    cancellation: &Arc<AtomicBool>,
) -> Result<Context, AppError>
where
    L: AsyncRead + Unpin,
{
    let mut digest = Context::new(&SHA256);
    let mut bytes = vec![0; VERIFY_BUFFER_BYTES];
    let mut remaining = length;
    while remaining > 0 {
        check_active(cancellation)?;
        let count = remaining.min(VERIFY_BUFFER_BYTES as u64) as usize;
        local.read_exact(&mut bytes[..count]).await.map_err(|error| local_prefix_error(error, path))?;
        digest.update(&bytes[..count]);
        remaining -= count as u64;
    }
    Ok(digest)
}

// 逐字节比对两端前缀；内存与文件大小无关。
async fn verify_prefix<L, R>(
    local: &mut L,
    remote: &mut R,
    length: u64,
    path: &str,
    cancellation: &Arc<AtomicBool>,
) -> Result<Context, AppError>
where
    L: AsyncRead + Unpin,
    R: AsyncRead + Unpin,
{
    let mut digest = Context::new(&SHA256);
    let mut local_bytes = vec![0; VERIFY_BUFFER_BYTES];
    let mut remote_bytes = vec![0; VERIFY_BUFFER_BYTES];
    let mut remaining = length;
    while remaining > 0 {
        check_active(cancellation)?;
        let count = remaining.min(VERIFY_BUFFER_BYTES as u64) as usize;
        local.read_exact(&mut local_bytes[..count]).await.map_err(|error| local_prefix_error(error, path))?;
        remote.read_exact(&mut remote_bytes[..count]).await.map_err(|error| {
            if error.kind() == std::io::ErrorKind::UnexpectedEof { resume_mismatch(path) }
            else { remote_io_error(error, path) }
        })?;
        if local_bytes[..count] != remote_bytes[..count] {
            return Err(resume_mismatch(path));
        }
        digest.update(&local_bytes[..count]);
        remaining -= count as u64;
    }
    Ok(digest)
}

// 远端那份先让服务器自己算摘要；算不了或对不上，再整份读回来核对，结论以读回来的为准。
pub(super) async fn verify_remote_contents<R>(
    remote: &mut R,
    remote_digest: &RemoteDigest,
    size: u64,
    expected: &Digest,
    path: &str,
    cancellation: &Arc<AtomicBool>,
) -> Result<(), AppError>
where
    R: AsyncRead + AsyncSeek + Unpin,
{
    if remote_digest.matches(path, DigestRange::Whole(size), expected.as_ref(), cancellation).await? {
        return Ok(());
    }
    verify_contents(remote, size, expected, path, cancellation, |error| remote_io_error(error, path)).await
}

// mtime 可能只有秒级精度，再读一遍内容摘要，避免同大小改写或传输中变化被标为成功。
pub(super) async fn verify_contents<R>(
    reader: &mut R,
    size: u64,
    expected: &Digest,
    path: &str,
    cancellation: &Arc<AtomicBool>,
    map_error: impl Fn(std::io::Error) -> AppError,
) -> Result<(), AppError>
where
    R: AsyncRead + AsyncSeek + Unpin,
{
    reader.seek(SeekFrom::Start(0)).await.map_err(&map_error)?;
    let mut digest = Context::new(&SHA256);
    let mut bytes = vec![0; VERIFY_BUFFER_BYTES];
    let mut remaining = size;
    while remaining > 0 {
        check_active(cancellation)?;
        let count = remaining.min(VERIFY_BUFFER_BYTES as u64) as usize;
        let read = reader.read(&mut bytes[..count]).await.map_err(&map_error)?;
        if read == 0 { return Err(source_changed(path)); }
        digest.update(&bytes[..read]);
        remaining -= read as u64;
    }
    check_active(cancellation)?;
    if reader.read(&mut bytes[..1]).await.map_err(&map_error)? != 0
        || digest.finish().as_ref() != expected.as_ref()
    {
        return Err(source_changed(path));
    }
    Ok(())
}
