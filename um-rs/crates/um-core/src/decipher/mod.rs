//! 解密器抽象与调度。
//!
//! 对应 TS 侧的 `src/decrypt-worker/Deciphers.ts`（抽象与注册表）和
//! `src/decrypt-worker/worker/decrypt.ts`（调度与结果校验）。

mod kgm;
mod kuwo;
mod mg3d;
mod ncm;
mod qmc;
mod qmc_v1;
mod qtfm;
mod transparent;
mod xiami;
mod ximalaya;

pub use kgm::KugouMusicDecipher;
pub use kuwo::KuwoMusicDecipher;
pub use mg3d::Migu3dDecipher;
pub use ncm::NetEaseCloudMusicDecipher;
pub use qmc::QQMusicV2Decipher;
pub use qmc_v1::QQMusicV1Decipher;
pub use qtfm::QingTingFmDecipher;
pub use transparent::TransparentDecipher;
pub use xiami::XiamiDecipher;
pub use ximalaya::{XimalayaAndroidDecipher, XimalayaAndroidKind, XimalayaPCDecipher};

use crate::audio_type::detect_audio_extension;
use crate::error::UnsupportedSourceFile;

/// 解密所需的可选参数，对应 TS 侧 `DecryptCommandOptions`。
#[derive(Debug, Clone, Default)]
pub struct DecryptOptions {
    pub file_name: String,
    pub qmc2_key: Option<String>,
    pub kwm2_key: Option<String>,
    pub kugou_key: Option<String>,
    pub qingting_android_key: Option<String>,
}

/// 解密成功的产物。
#[derive(Debug)]
pub struct Deciphered {
    pub data: Vec<u8>,
    pub override_extension: Option<String>,
    pub cipher_name: &'static str,
}

/// 单个解密器的尝试结果。
#[derive(Debug)]
pub enum DecipherOutcome {
    Ok(Box<Deciphered>),
    /// 这个文件不属于本解密器，请换下一个
    NotThisCipher,
    /// 本解密器认领了，但解密过程出错
    Failed(String),
}

/// 一个格式的解密器。
///
/// 每个 `Decipher` 都是无状态的：不认识就返回 [`DecipherOutcome::NotThisCipher`]，
/// 由调度器继续尝试下一个。
pub trait Decipher {
    fn cipher_name(&self) -> &'static str;
    fn decrypt(&self, buffer: &[u8], options: &DecryptOptions) -> DecipherOutcome;
}

/// 解密器注册表。
///
/// 顺序与 TS 侧 `allCryptoFactories` 严格一致：有固定文件头的格式排前面，
/// 有固定文件尾的排中间，没有明显特征的排最后（性能考虑）。
pub fn all_ciphers() -> Vec<Box<dyn Decipher>> {
    vec![
        // —— 有固定文件头 ——
        // NCM (*.ncm)
        Box::new(NetEaseCloudMusicDecipher),
        // KGM (*.kgm, *.vpr)
        Box::new(KugouMusicDecipher),
        // KWMv1 (*.kwm)
        Box::new(KuwoMusicDecipher),
        // 喜马拉雅 PC (*.xm)
        Box::new(XimalayaPCDecipher),
        // 虾米 (*.xm)
        Box::new(XiamiDecipher),
        // 蜻蜓 FM Android (*.qta)
        Box::new(QingTingFmDecipher),
        // —— 有固定文件尾 ——
        // QMCv2 (*.mflac)：先试用户密钥，再试文件内嵌的 ekey
        Box::new(QQMusicV2Decipher::with_user_key()),
        Box::new(QQMusicV2Decipher::with_embedded_ekey()),
        // —— 既无头部也无尾部特征，只能靠解密结果判断 ——
        // Migu3D/Keyless (*.wav; *.m4a)
        Box::new(Migu3dDecipher),
        // QMCv1 (*.qmcflac)
        Box::new(QQMusicV1Decipher),
        // 喜马拉雅 Android (*.x2m, *.x3m)
        Box::new(XimalayaAndroidDecipher::new(XimalayaAndroidKind::X2M)),
        Box::new(XimalayaAndroidDecipher::new(XimalayaAndroidKind::X3M)),
        // 未加密文件兜底
        Box::new(TransparentDecipher),
    ]
}

/// 最终解密结果。
#[derive(Debug)]
pub struct DecryptResult {
    pub data: Vec<u8>,
    /// 探测出的音频扩展名，已把 mp4 归一化为 m4a
    pub extension: String,
    pub cipher_name: &'static str,
}

/// 依次尝试所有解密器，返回第一个成功的结果。
///
/// 移植自 TS 侧 `DecryptCommandHandler.decrypt`：单个解密器失败**不会**中断流程，
/// 而是记录错误后继续尝试下一个；解出来的东西不像音频时同样继续尝试；
/// 全部失败时汇总所有错误一起报出。
pub fn decrypt_any(
    buffer: &[u8],
    options: &DecryptOptions,
) -> Result<DecryptResult, UnsupportedSourceFile> {
    let mut errors: Vec<String> = Vec::new();

    for decipher in all_ciphers() {
        let name = decipher.cipher_name();

        match decipher.decrypt(buffer, options) {
            DecipherOutcome::Ok(done) => {
                let mut extension = match &done.override_extension {
                    Some(ext) => ext.clone(),
                    None => detect_audio_extension(&done.data),
                };

                // 没有显式指定扩展名，又探测不出音频类型，说明解出来的不是音频。
                // 这一步很关键：像 QMCv1 这种靠「解密后是否像音频」判断的格式
                // 判定非常宽松，靠这里兜底。
                if done.override_extension.is_none() && extension == "bin" {
                    errors.push(format!("{name}: unable to produce valid audio file"));
                    continue;
                }

                if extension.eq_ignore_ascii_case("mp4") {
                    extension = "m4a".to_owned();
                }

                return Ok(DecryptResult {
                    data: done.data,
                    extension,
                    cipher_name: done.cipher_name,
                });
            }
            DecipherOutcome::NotThisCipher => errors.push(format!("{name}: no response")),
            DecipherOutcome::Failed(message) => errors.push(format!("{name}: {message}")),
        }
    }

    Err(UnsupportedSourceFile::new(errors.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 未加密的 mp3 样本，用于验证「兜底」路径能原样透传。
    const MP3_FIXTURE: &[u8] = include_bytes!(
        "../../../../vendor/lib_um_crypto_rust/um_audio/src/__fixtures__/mp3_with_id3v2.bin"
    );

    #[test]
    fn registry_covers_every_format_from_ts() {
        let names: Vec<&str> = all_ciphers().iter().map(|c| c.cipher_name()).collect();
        assert_eq!(
            names,
            vec![
                "NCM/PC",
                "Kugou",
                "Kuwo",
                "Ximalaya (PC)",
                "Xiami (XM)",
                "QingTingFM (Android, qta)",
                "QQMusic/QMC2(user_key=1)",
                "QQMusic/QMC2(user_key=0)",
                "Migu3D (Keyless)",
                "QQMusic/QMC1",
                "Ximalaya (Android, X2M)",
                "Ximalaya (Android, X3M)",
                "none",
            ]
        );
    }

    #[test]
    fn transparent_passes_plain_audio_through() {
        let result = decrypt_any(MP3_FIXTURE, &DecryptOptions::default()).expect("应能解出结果");
        assert_eq!(result.extension, "mp3");
        assert_eq!(result.cipher_name, "None");
        assert_eq!(result.data, MP3_FIXTURE);
    }

    #[test]
    fn non_audio_data_is_rejected() {
        let err = decrypt_any(&[0u8; 64], &DecryptOptions::default()).unwrap_err();
        assert!(err.message().contains("unable to produce valid audio file"));
    }

    #[test]
    fn empty_file_is_rejected() {
        assert!(decrypt_any(&[], &DecryptOptions::default()).is_err());
    }

    #[test]
    fn reports_every_failed_cipher_when_nothing_matches() {
        let err = decrypt_any(&[0u8; 64], &DecryptOptions::default()).unwrap_err();
        // 每个解密器都应留下记录，方便定位
        assert!(err.message().contains("NCM/PC"));
        assert!(err.message().contains("Kugou"));
        assert!(err.message().contains("QQMusic/QMC2"));
        assert!(err.message().contains("none"));
    }
}
