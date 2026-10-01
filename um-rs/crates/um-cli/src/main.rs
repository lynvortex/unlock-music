use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use um_core::audio_type::detect_audio_extension;
use um_core::decipher::{decrypt_any, DecryptOptions};

#[derive(Parser)]
#[command(name = "um-cli", about = "Unlock Music 命令行工具", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 解密文件
    Decrypt {
        /// 待解密的文件，可传多个
        #[arg(required = true, value_name = "FILE")]
        inputs: Vec<PathBuf>,

        /// 输出目录，默认为输入文件所在目录
        #[arg(short, long, value_name = "DIR")]
        output: Option<PathBuf>,

        /// QMCv2 用户密钥（ekey）
        #[arg(long, value_name = "KEY")]
        qmc2_key: Option<String>,

        /// KWMv2 密钥
        #[arg(long, value_name = "KEY")]
        kwm2_key: Option<String>,

        /// 酷狗密钥数据库路径
        #[arg(long, value_name = "PATH")]
        kugou_key: Option<String>,

        /// 蜻蜓 FM Android 密钥
        #[arg(long, value_name = "KEY")]
        qingting_key: Option<String>,

        /// 输出文件已存在时覆盖
        #[arg(short, long)]
        force: bool,
    },

    /// 只探测文件的音频类型，不做解密
    Detect {
        /// 待探测的文件，可传多个
        #[arg(required = true, value_name = "FILE")]
        inputs: Vec<PathBuf>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let mut failed = 0usize;

    match cli.command {
        Command::Decrypt {
            inputs,
            output,
            qmc2_key,
            kwm2_key,
            kugou_key,
            qingting_key,
            force,
        } => {
            let base = DecryptOptions {
                file_name: String::new(),
                qmc2_key,
                kwm2_key,
                kugou_key,
                qingting_android_key: qingting_key,
            };

            // 本批全部输入文件都不允许被输出覆盖
            let protected = inputs.clone();

            for input in &inputs {
                match decrypt_one(input, &base, output.as_deref(), force, &protected) {
                    Ok(message) => println!("{message}"),
                    Err(error) => {
                        eprintln!("[失败] {}：{error}", input.display());
                        failed += 1;
                    }
                }
            }
        }

        Command::Detect { inputs } => {
            for input in &inputs {
                match std::fs::read(input) {
                    Ok(buffer) => {
                        let extension = detect_audio_extension(&buffer);
                        println!(
                            "{}：音频类型 {}（{} 字节）",
                            input.display(),
                            extension,
                            buffer.len()
                        );
                    }
                    Err(error) => {
                        eprintln!("[失败] {}：{error}", input.display());
                        failed += 1;
                    }
                }
            }
        }
    }

    if failed > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// 解密单个文件并写出结果。
///
/// 输出文件已存在时不覆盖，除非显式传入 `--force`；
/// 但 `protected` 里的输入文件**任何情况下都不会被覆盖**。
fn decrypt_one(
    input: &Path,
    base: &DecryptOptions,
    output_dir: Option<&Path>,
    force: bool,
    protected: &[PathBuf],
) -> Result<String, String> {
    let buffer = std::fs::read(input).map_err(|error| format!("读取失败：{error}"))?;

    let options = DecryptOptions {
        file_name: input
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        ..base.clone()
    };

    let result = decrypt_any(&buffer, &options).map_err(|error| error.message().to_owned())?;

    let stem = input
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output".to_owned());

    let dir: &Path = match output_dir {
        Some(dir) => dir,
        None => input.parent().unwrap_or(Path::new(".")),
    };
    std::fs::create_dir_all(dir).map_err(|error| format!("创建输出目录失败：{error}"))?;

    let mut out_path = dir.join(format!("{stem}.{}", result.extension));

    if protected.iter().any(|source| same_path(source, &out_path)) {
        // 目标正好是某个输入文件：与 --force 无关，一律改名，绝不覆盖源文件
        let mut index = 1;
        while out_path.exists() {
            out_path = dir.join(format!("{stem} ({index}).{}", result.extension));
            index += 1;
        }
    } else if out_path.exists() && !force {
        return Err(format!(
            "输出文件已存在，加 --force 覆盖：{}",
            out_path.display()
        ));
    }

    std::fs::write(&out_path, &result.data).map_err(|error| format!("写入失败：{error}"))?;

    Ok(format!(
        "[成功] {} → {}（解密器 {}，{} 字节）",
        input.display(),
        out_path.display(),
        result.cipher_name,
        result.data.len()
    ))
}

/// 归一化路径用于比较：只解析父目录（一定存在），再拼回文件名。
fn normalize_path(path: &Path) -> PathBuf {
    let (dir, name) = match (path.parent(), path.file_name()) {
        (Some(dir), Some(name)) if !dir.as_os_str().is_empty() => (dir, Some(name)),
        _ => (path, None),
    };

    let base = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    match name {
        Some(name) => base.join(name),
        None => base,
    }
}

/// 判断两个路径是否指向同一个文件（Windows 下大小写不敏感）。
fn same_path(a: &Path, b: &Path) -> bool {
    let a = normalize_path(a);
    let b = normalize_path(b);
    if cfg!(windows) {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}
