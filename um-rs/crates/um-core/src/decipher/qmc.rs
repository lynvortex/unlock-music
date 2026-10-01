use crate::buffer::{chunk_buffer, DEFAULT_BLOCK_LEN};
use crate::decipher::{Decipher, DecipherOutcome, Deciphered, DecryptOptions};
use umc_qmc::footer::{self, FooterParseError, INITIAL_DETECTION_LEN};
use umc_qmc::QMCv2Cipher;

/// 从 footer 中取得的解密所需信息。
struct FooterInfo {
    /// 需要从文件尾部裁掉的长度
    size: usize,
    /// 内嵌的 ekey（尚未解密）
    ekey: Option<String>,
}

/// QMCv2 解密器（QQ 音乐 `.mflac` / `.mgg` / `.mgalaxy` 等）。
///
/// 对应 TS 侧 `src/decrypt-worker/decipher/QQMusic.ts` 的 `QQMusicV2Decipher`。
/// 注册表里会同时放入两个实例：先试用户提供的 ekey，再试文件内嵌的 ekey。
pub struct QQMusicV2Decipher {
    /// 为 true 时强制使用用户提供的 ekey，忽略文件内嵌的
    use_user_key: bool,
}

impl QQMusicV2Decipher {
    /// 使用用户提供的 ekey
    pub fn with_user_key() -> Self {
        Self { use_user_key: true }
    }

    /// 使用文件内嵌的 ekey
    pub fn with_embedded_ekey() -> Self {
        Self { use_user_key: false }
    }

    /// 解析文件尾部的 QMC footer。
    ///
    /// 返回 `Err(outcome)` 时调度器应直接把该结果上报。
    fn parse_footer(&self, buffer: &[u8]) -> Result<FooterInfo, DecipherOutcome> {
        let start = buffer.len().saturating_sub(INITIAL_DETECTION_LEN);

        match footer::from_byte_slice(&buffer[start..]) {
            Ok(Some(metadata)) => Ok(FooterInfo {
                size: metadata.size,
                ekey: metadata.ekey,
            }),
            // WASM 层把 PCv1EKeyTooLarge 也当作「没有 footer」，这里保持一致
            Ok(None) | Err(FooterParseError::PCv1EKeyTooLarge(_)) => {
                if self.use_user_key {
                    // 没有 footer 时不做裁剪，密钥完全依赖调用方提供
                    Ok(FooterInfo {
                        size: 0,
                        ekey: None,
                    })
                } else {
                    Err(DecipherOutcome::NotThisCipher)
                }
            }
            Err(err) => Err(DecipherOutcome::Failed(err.to_string())),
        }
    }
}

impl Decipher for QQMusicV2Decipher {
    fn cipher_name(&self) -> &'static str {
        if self.use_user_key {
            "QQMusic/QMC2(user_key=1)"
        } else {
            "QQMusic/QMC2(user_key=0)"
        }
    }

    fn decrypt(&self, buffer: &[u8], options: &DecryptOptions) -> DecipherOutcome {
        let footer = match self.parse_footer(buffer) {
            Ok(footer) => footer,
            Err(outcome) => return outcome,
        };

        let ekey = if self.use_user_key {
            options.qmc2_key.clone()
        } else {
            footer.ekey
        };

        let Some(ekey) = ekey else {
            return DecipherOutcome::Failed("EKey required".to_owned());
        };

        let cipher = match QMCv2Cipher::new_from_ekey(&ekey) {
            Ok(cipher) => cipher,
            Err(err) => return DecipherOutcome::Failed(err.to_string()),
        };

        let end = buffer.len().saturating_sub(footer.size);
        let mut audio = buffer[..end].to_vec();
        for (block, offset) in chunk_buffer(&mut audio, DEFAULT_BLOCK_LEN) {
            cipher.decrypt(block, offset);
        }

        DecipherOutcome::Ok(Box::new(Deciphered {
            data: audio,
            override_extension: None,
            cipher_name: self.cipher_name(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 官方 umc_qmc 的 footer 样本。
    /// `ekey_android_stag.bin`：size = 0x20，无内嵌 ekey。
    /// `ekey_pc_enc_v1.bin`：size = 0x2C4 (708)，内嵌一个可用（可解密）的 ekey。
    const STAG_NO_EKEY: &[u8] =
        include_bytes!("../../../../vendor/lib_um_crypto_rust/um_crypto/qmc/src/footer/fixtures/ekey_android_stag.bin");
    const PC_V1_WITH_EKEY: &[u8] =
        include_bytes!("../../../../vendor/lib_um_crypto_rust/um_crypto/qmc/src/footer/fixtures/ekey_pc_enc_v1.bin");

    /// 参考输出：由官方 `um_cli qmc2` 对同样的输入解密得到，用于校验本移植的
    /// footer 定位、裁剪与分块偏移量是否与上游一致。
    const REFERENCE_OUTPUT: &[u8] = include_bytes!("__fixture__/qmc2_pc_v1_ekey_expected.bin");

    /// 把官方 footer 样本补齐到 1024 字节（上游 um_cli 要求文件至少这么长）。
    fn padded_fixture() -> Vec<u8> {
        let mut input = vec![0u8; 1024 - PC_V1_WITH_EKEY.len()];
        input.extend_from_slice(PC_V1_WITH_EKEY);
        input
    }

    #[test]
    fn rejects_file_without_footer_in_embedded_mode() {
        // 构造一个「被判定为没有 footer」的输入：
        // PcV1Legacy 会读取末尾 4 字节作为 ekey 长度，超过 0x500 时
        // 解析器报 PCv1EKeyTooLarge，WASM 层将其等同于「没有 footer」。
        let mut buffer = vec![0u8; 2048];
        buffer[2044..].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());

        let outcome = QQMusicV2Decipher::with_embedded_ekey().decrypt(&buffer, &DecryptOptions::default());
        assert!(matches!(outcome, DecipherOutcome::NotThisCipher));
    }

    #[test]
    fn embedded_mode_parses_footer_and_reports_missing_ekey() {
        // 能解析出 footer（说明没有走到 NotThisCipher），但样本里没有 ekey
        let outcome =
            QQMusicV2Decipher::with_embedded_ekey().decrypt(STAG_NO_EKEY, &DecryptOptions::default());
        match outcome {
            DecipherOutcome::Failed(message) => assert_eq!(message, "EKey required"),
            other => panic!("应当因为缺少 ekey 而失败，实际为 {other:?}"),
        }
    }

    #[test]
    fn embedded_mode_decrypts_with_embedded_ekey() {
        // 该样本内嵌了可用的 ekey，size = 0x2C4 (708)，
        // 因此应裁掉尾部 708 字节，并对剩下的字节完成解密。
        let outcome =
            QQMusicV2Decipher::with_embedded_ekey().decrypt(PC_V1_WITH_EKEY, &DecryptOptions::default());
        match outcome {
            DecipherOutcome::Ok(done) => {
                assert_eq!(done.data.len(), PC_V1_WITH_EKEY.len() - 0x2C4);
                assert_eq!(done.cipher_name, "QQMusic/QMC2(user_key=0)");
            }
            other => panic!("应当解密成功，实际为 {other:?}"),
        }
    }

    #[test]
    fn output_matches_upstream_reference_implementation() {
        // 与官方 um_cli 对同一输入的结果逐字节比对，
        // 校验 footer 定位、尾部裁剪与分块偏移量三者都与上游一致。
        let input = padded_fixture();
        assert_eq!(input.len(), 1024);

        let outcome = QQMusicV2Decipher::with_embedded_ekey().decrypt(&input, &DecryptOptions::default());
        match outcome {
            DecipherOutcome::Ok(done) => assert_eq!(done.data, REFERENCE_OUTPUT),
            other => panic!("应当解密成功，实际为 {other:?}"),
        }
    }

    #[test]
    fn user_key_mode_requires_user_key() {
        // 没有 footer 时用户密钥模式仍然继续，只是缺少密钥
        let outcome =
            QQMusicV2Decipher::with_user_key().decrypt(&[0u8; 2048], &DecryptOptions::default());
        match outcome {
            DecipherOutcome::Failed(message) => assert_eq!(message, "EKey required"),
            other => panic!("应当因为缺少 ekey 而失败，实际为 {other:?}"),
        }
    }

    #[test]
    fn cipher_name_encodes_user_key_mode() {
        assert_eq!(QQMusicV2Decipher::with_user_key().cipher_name(), "QQMusic/QMC2(user_key=1)");
        assert_eq!(
            QQMusicV2Decipher::with_embedded_ekey().cipher_name(),
            "QQMusic/QMC2(user_key=0)"
        );
    }
}
