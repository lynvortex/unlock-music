use std::io::Write;
use std::path::PathBuf;

use um_core::decipher::DecryptOptions;

/// 应用设置。
///
/// 明文保存在用户配置目录下，格式是每行一个 `键=值`。
/// 值里可以包含 `=`（ekey 就是 base64），所以按第一个 `=` 切分。
#[derive(Debug, Clone, Default)]
pub struct Settings {
    /// 输出目录；留空表示写到源文件所在目录
    pub output_dir: String,
    /// 输出文件已存在时覆盖
    pub overwrite: bool,
    /// QMCv2 用户密钥（ekey）
    pub qmc2_key: String,
    /// KWMv2 密钥
    pub kwm2_key: String,
    /// 酷狗密钥数据库路径
    pub kugou_key: String,
    /// 蜻蜓 FM 设备密钥（十六进制）
    pub qingting_key: String,
}

impl Settings {
    /// 转换成解密核心使用的参数。`file_name` 由调用方按文件逐个填充。
    pub fn to_options(&self, file_name: String) -> DecryptOptions {
        DecryptOptions {
            file_name,
            qmc2_key: none_if_empty(&self.qmc2_key),
            kwm2_key: none_if_empty(&self.kwm2_key),
            kugou_key: none_if_empty(&self.kugou_key),
            qingting_android_key: none_if_empty(&self.qingting_key),
        }
    }

    pub fn load() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };

        let mut settings = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "output_dir" => settings.output_dir = value.to_owned(),
                "overwrite" => settings.overwrite = value == "true",
                "qmc2_key" => settings.qmc2_key = value.to_owned(),
                "kwm2_key" => settings.kwm2_key = value.to_owned(),
                "kugou_key" => settings.kugou_key = value.to_owned(),
                "qingting_key" => settings.qingting_key = value.to_owned(),
                _ => {}
            }
        }
        settings
    }

    pub fn save(&self) -> Result<(), String> {
        let path = config_path().ok_or_else(|| "无法确定配置目录".to_owned())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败：{e}"))?;
        }

        let mut text = String::new();
        text.push_str("output_dir=");
        text.push_str(&self.output_dir);
        text.push('\n');
        text.push_str(&format!("overwrite={}\n", self.overwrite));
        text.push_str(&format!("qmc2_key={}\n", self.qmc2_key));
        text.push_str(&format!("kwm2_key={}\n", self.kwm2_key));
        text.push_str(&format!("kugou_key={}\n", self.kugou_key));
        text.push_str(&format!("qingting_key={}\n", self.qingting_key));

        // 先写临时文件再重命名：中途失败也不会把已有配置截断成半截
        let temp = path.with_extension("conf.tmp");
        let written = (|| -> std::io::Result<()> {
            let mut file = std::fs::File::create(&temp)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()
        })();

        if let Err(error) = written {
            let _ = std::fs::remove_file(&temp);
            return Err(format!("写入配置失败：{error}"));
        }

        std::fs::rename(&temp, &path).map_err(|error| {
            let _ = std::fs::remove_file(&temp);
            format!("保存配置失败：{error}")
        })
    }
}

fn none_if_empty(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

/// 配置文件路径。
///
/// Windows 用 `%APPDATA%`，其它平台用 `$XDG_CONFIG_HOME` 或 `$HOME/.config`。
/// 两者都拿不到时返回 `None`：**不回落到当前工作目录**，避免把密钥写进项目目录。
fn config_path() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    }?;

    Some(base.join("um-gui").join("settings.conf"))
}
