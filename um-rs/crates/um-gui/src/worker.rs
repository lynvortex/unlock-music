use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use um_core::decipher::{decrypt_any, DecryptOptions};

/// 任务编号，由界面分配，用于把结果对回列表里的行。
pub type JobId = u64;

/// 单个文件的处理阶段，用于界面显示进度。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// 正在读取文件
    Reading,
    /// 正在解密
    Decrypting,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Reading => "读取中",
            Phase::Decrypting => "解密中",
        }
    }
}

/// 一个待解密的文件。
pub struct DecryptRequest {
    pub id: JobId,
    pub input: PathBuf,
    /// 已解析好的输出目录（不能为空路径）
    pub output_dir: PathBuf,
    pub options: DecryptOptions,
}

/// 解密成功的结果。
///
/// 这里**不携带解密后的字节**：内容已经写到 `temp_path`，由界面重命名成最终文件名。
/// 这样整个 I/O 过程只有一份文件大小的内存占用，消息队列里也不会堆积整份数据。
pub struct DecryptSuccess {
    pub extension: String,
    pub cipher_name: String,
    /// 已写好的临时文件，等待界面重命名
    pub temp_path: PathBuf,
}

/// 后台线程回传给界面的消息。
pub enum Response {
    /// 阶段推进，用于刷新界面上的进度提示
    Phase { id: JobId, phase: Phase },
    Finished {
        id: JobId,
        result: Result<DecryptSuccess, String>,
    },
}

/// 解密线程池。
///
/// 多个文件并行解密，用最普通的 `std::sync::mpsc` 实现，不引入额外依赖。
pub struct DecryptPool {
    task_tx: Sender<DecryptRequest>,
    result_rx: Receiver<Response>,
}

impl DecryptPool {
    pub fn new(threads: usize) -> Self {
        let (task_tx, task_rx) = channel::<DecryptRequest>();
        let (result_tx, result_rx) = channel::<Response>();
        let task_rx = Arc::new(Mutex::new(task_rx));

        for _ in 0..threads.max(1) {
            let task_rx = Arc::clone(&task_rx);
            let result_tx = result_tx.clone();
            thread::spawn(move || loop {
                // 共享接收端的常规写法：取任务时会短暂持锁，但真正的解密在锁外进行，
                // 所以多个线程仍能同时工作。
                let request = match task_rx.lock() {
                    Ok(guard) => guard.recv(),
                    Err(_) => return,
                };
                // 发送端已丢弃 —— 程序正在退出
                let Ok(request) = request else {
                    return;
                };

                let result = decrypt_file(&request, &result_tx);
                let finished = Response::Finished {
                    id: request.id,
                    result,
                };
                if result_tx.send(finished).is_err() {
                    return;
                }
            });
        }

        Self {
            task_tx,
            result_rx,
        }
    }

    pub fn submit(&self, request: DecryptRequest) -> Result<(), String> {
        self.task_tx
            .send(request)
            .map_err(|_| "解密线程已退出".to_owned())
    }

    /// 非阻塞地取出所有已产生的消息。
    pub fn drain(&self) -> Vec<Response> {
        self.result_rx.try_iter().collect()
    }
}

fn decrypt_file(
    request: &DecryptRequest,
    result_tx: &Sender<Response>,
) -> Result<DecryptSuccess, String> {
    let _ = result_tx.send(Response::Phase {
        id: request.id,
        phase: Phase::Reading,
    });
    let buffer = std::fs::read(&request.input).map_err(|e| format!("读取失败：{e}"))?;

    let _ = result_tx.send(Response::Phase {
        id: request.id,
        phase: Phase::Decrypting,
    });
    let result =
        decrypt_any(&buffer, &request.options).map_err(|error| error.message().to_owned())?;
    // 输入缓冲区到此为止不再需要，尽早释放
    drop(buffer);

    let temp_path = write_temp(&request.output_dir, &request.input, request.id, &result.data)?;

    Ok(DecryptSuccess {
        extension: result.extension,
        cipher_name: result.cipher_name.to_owned(),
        temp_path,
    })
}

/// 把解密结果写到输出目录下的临时文件，返回临时文件路径。
///
/// 文件名带任务编号，避免同一文件重复提交时互相覆盖。
fn write_temp(dir: &Path, input: &Path, id: JobId, data: &[u8]) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("创建输出目录失败：{e}"))?;

    let stem = input
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output".to_owned());

    let temp_path = dir.join(format!(".{stem}.{id}.part"));
    std::fs::write(&temp_path, data).map_err(|e| format!("写入失败：{e}"))?;
    Ok(temp_path)
}
