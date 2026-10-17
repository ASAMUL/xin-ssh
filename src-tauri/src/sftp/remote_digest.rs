use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use russh::{client, ChannelMsg};
use tokio::sync::{Mutex as AsyncMutex, Semaphore};
use tokio::time::{interval, timeout, Duration, MissedTickBehavior};

use crate::error::{codes, AppError};
use crate::ssh::client::ConnectionTestClient;
use crate::util::shell_quote;

// 小文件直接读回来只要几个往返，开 exec 通道再起进程反而更慢。
const REMOTE_DIGEST_MIN_BYTES: u64 = 1024 * 1024;
// sshd 默认 MaxSessions 是 10，终端、SFTP、监控也要占通道，摘要命令不能把名额吃光。
const REMOTE_DIGEST_CONCURRENCY: usize = 2;
// 时限按慢板子估：读盘加哈希约 16 MiB/s，再留出起进程和排队的余量。
const REMOTE_DIGEST_BASE_TIMEOUT_SECS: u64 = 30;
const REMOTE_DIGEST_BYTES_PER_SEC: u64 = 16 * 1024 * 1024;
const CANCEL_POLL_INTERVAL_MS: u64 = 250;
// 正常输出只有一行摘要；限长防止奇怪的登录脚本刷屏占内存。
const MAX_DIGEST_OUTPUT_BYTES: usize = 4096;
// 文件本身读不了时脚本以 2 退出：只影响这一个文件，不代表服务器不支持。
const EXIT_UNREADABLE: u32 = 2;

// 写成一行且不含单引号，整体单引号包住就能交给任意登录 shell（csh 也行）；路径走位置参数，不拼进脚本。
// 第二个参数是字节数或 all：all 读到文件末尾，文件被追加过也能对出来。
const DIGEST_SCRIPT: &str = r#"[ -f "$1" ] && [ -r "$1" ] || exit 2; if command -v sha256sum >/dev/null 2>&1; then h=sha256sum; elif command -v shasum >/dev/null 2>&1; then h="shasum -a 256"; else exit 127; fi; if [ "$2" = all ]; then $h < "$1"; else head -c "$2" < "$1" | $h; fi"#;

#[derive(Clone, Copy)]
pub(super) enum DigestRange {
    // 文件开头的若干字节：续传前核对已经传过去的那部分
    Prefix(u64),
    // 整个文件；参数是预期大小，用来判断值不值得在服务器上算、估时限
    Whole(u64),
}

impl DigestRange {
    fn bytes(self) -> u64 {
        match self {
            Self::Prefix(bytes) | Self::Whole(bytes) => bytes,
        }
    }

    fn argument(self) -> String {
        match self {
            Self::Prefix(bytes) => bytes.to_string(),
            Self::Whole(_) => "all".to_string(),
        }
    }
}

// 让服务器在本机算 SHA-256，只回传几十字节，核对时不用把文件内容再走一遍网络。
// 这只是加速：对上了才算数；对不上或算不了，都交回调用方按原来的方式逐块读回核对。
pub(super) struct RemoteDigest {
    session: Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>,
    // 服务器没有 sh / sha256sum、或者只开了 SFTP 时，同一批传输里不再反复尝试
    unsupported: AtomicBool,
    permits: Semaphore,
}

impl RemoteDigest {
    pub(super) fn new(session: Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>) -> Self {
        Self {
            session,
            unsupported: AtomicBool::new(false),
            permits: Semaphore::new(REMOTE_DIGEST_CONCURRENCY),
        }
    }

    pub(super) fn applies_to(&self, bytes: u64) -> bool {
        bytes >= REMOTE_DIGEST_MIN_BYTES && !self.unsupported.load(Ordering::Relaxed)
    }

    pub(super) async fn matches(
        &self,
        path: &str,
        range: DigestRange,
        expected: &[u8],
        cancellation: &Arc<AtomicBool>,
    ) -> Result<bool, AppError> {
        if !self.applies_to(range.bytes()) {
            return Ok(false);
        }
        let Ok(_permit) = self.permits.acquire().await else {
            return Ok(false);
        };
        // 排队期间，别的文件可能已经确认这台服务器算不了
        if self.unsupported.load(Ordering::Relaxed) {
            return Ok(false);
        }

        let command = format!(
            "sh -c {} xin-digest {} {}",
            shell_quote(DIGEST_SCRIPT),
            shell_quote(path),
            range.argument(),
        );
        let Some((exit_status, stdout)) = self.execute(command, range.bytes(), cancellation).await?
        else {
            self.unsupported.store(true, Ordering::Relaxed);
            return Ok(false);
        };

        match (exit_status.unwrap_or(0), parse_sha256_output(&stdout)) {
            (0, Some(actual)) => Ok(actual.as_slice() == expected),
            (EXIT_UNREADABLE, _) => Ok(false),
            _ => {
                self.unsupported.store(true, Ordering::Relaxed);
                Ok(false)
            }
        }
    }

    // None 表示这条路走不通（开不了通道、命令被拒、超时）；只有用户取消才作为错误返回。
    async fn execute(
        &self,
        command: String,
        bytes: u64,
        cancellation: &Arc<AtomicBool>,
    ) -> Result<Option<(Option<u32>, Vec<u8>)>, AppError> {
        let mut channel = {
            let session = self.session.lock().await;
            match session.channel_open_session().await {
                Ok(channel) => channel,
                Err(_) => return Ok(None),
            }
        };
        if channel.exec(true, command).await.is_err() {
            return Ok(None);
        }
        // 只开了 SFTP 的账号会把任何命令换成 internal-sftp，不告诉它输入结束它就一直等
        let _ = channel.eof().await;

        let limit = Duration::from_secs(
            REMOTE_DIGEST_BASE_TIMEOUT_SECS + bytes / REMOTE_DIGEST_BYTES_PER_SEC,
        );
        let mut ticker = interval(Duration::from_millis(CANCEL_POLL_INTERVAL_MS));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut stdout = Vec::new();
        let mut exit_status = None;

        let outcome = timeout(limit, async {
            loop {
                tokio::select! {
                    message = channel.wait() => match message {
                        Some(ChannelMsg::Data { data }) => {
                            if stdout.len() < MAX_DIGEST_OUTPUT_BYTES {
                                stdout.extend_from_slice(&data);
                            }
                        }
                        Some(ChannelMsg::ExitStatus { exit_status: status }) => exit_status = Some(status),
                        // Eof 之后还可能有 ExitStatus，等通道真正关闭再结束；
                        // 服务器拒绝执行时不一定关通道，不能干等到超时
                        Some(ChannelMsg::Close | ChannelMsg::Failure) | None => return Ok(()),
                        Some(_) => {}
                    },
                    _ = ticker.tick() => {
                        if cancellation.load(Ordering::SeqCst) {
                            return Err(AppError::new(codes::TRANSFER_CANCELED, "Transfer canceled"));
                        }
                    }
                }
            }
        })
        .await;

        match outcome {
            Ok(Ok(())) => Ok(Some((exit_status, stdout))),
            Ok(Err(error)) => {
                let _ = channel.close().await;
                Err(error)
            }
            Err(_) => {
                let _ = channel.close().await;
                Ok(None)
            }
        }
    }
}

fn parse_sha256_output(stdout: &[u8]) -> Option<[u8; 32]> {
    // bash 被 sshd 拉起时会读 .bashrc，有人在里面打印欢迎语；摘要总在最后一行
    let line = std::str::from_utf8(stdout)
        .ok()?
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())?;
    let token = line.split_whitespace().next()?;
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }

    let mut digest = [0_u8; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&token[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn parses_sha256sum_and_shasum_stdin_output() {
        let digest = parse_sha256_output(format!("{EMPTY_SHA256}  -\n").as_bytes()).unwrap();
        assert_eq!(digest[0], 0xe3);
        assert_eq!(digest[31], 0x55);
    }

    #[test]
    fn skips_greeting_lines_before_the_digest() {
        let output = format!("Welcome to the box\n{EMPTY_SHA256}  -\n\n");
        assert!(parse_sha256_output(output.as_bytes()).is_some());
    }

    #[test]
    fn rejects_output_that_is_not_a_digest() {
        assert!(parse_sha256_output(b"").is_none());
        assert!(parse_sha256_output(b"sh: sha256sum: not found\n").is_none());
        assert!(parse_sha256_output(format!("+{}  -\n", &EMPTY_SHA256[1..]).as_bytes()).is_none());
    }

    #[test]
    fn prefix_passes_a_byte_count_and_whole_reads_to_the_end() {
        assert_eq!(DigestRange::Prefix(1048576).argument(), "1048576");
        assert_eq!(DigestRange::Whole(1048576).argument(), "all");
    }
}
