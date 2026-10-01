use crate::audio_type::is_data_looks_like_audio;
use crate::decipher::{Decipher, DecipherOutcome, Deciphered, DecryptOptions};
use umc_xmly::pc::Header;
use umc_xmly::XmlyError;

/// Android 端（x2m / x3m）固定读取 0x400 字节头部。
const ANDROID_HEADER_LEN: usize = 0x400;
/// PC 端首次尝试读取的头部长度。
const PC_INITIAL_HEADER_LEN: usize = 1024;

/// 喜马拉雅 Android 端的加密变体。
#[derive(Debug, Clone, Copy)]
pub enum XimalayaAndroidKind {
    /// `*.x2m`
    X2M,
    /// `*.x3m`
    X3M,
}

impl XimalayaAndroidKind {
    fn file_type(self) -> umc_xmly::android::FileType {
        match self {
            Self::X2M => umc_xmly::android::FileType::X2M,
            Self::X3M => umc_xmly::android::FileType::X3M,
        }
    }
}

/// 喜马拉雅 Android 端解密器（`.x2m` / `.x3m`）。
///
/// 对应 TS 侧 `src/decrypt-worker/decipher/Ximalaya.ts` 的 `XimalayaAndroidDecipher`。
/// 该格式没有明显魔数，只能先解密头部再判断是否像音频。
pub struct XimalayaAndroidDecipher {
    kind: XimalayaAndroidKind,
}

impl XimalayaAndroidDecipher {
    pub fn new(kind: XimalayaAndroidKind) -> Self {
        Self { kind }
    }
}

impl Decipher for XimalayaAndroidDecipher {
    fn cipher_name(&self) -> &'static str {
        match self.kind {
            XimalayaAndroidKind::X2M => "Ximalaya (Android, X2M)",
            XimalayaAndroidKind::X3M => "Ximalaya (Android, X3M)",
        }
    }

    fn decrypt(&self, buffer: &[u8], _options: &DecryptOptions) -> DecipherOutcome {
        if buffer.len() < ANDROID_HEADER_LEN {
            return DecipherOutcome::NotThisCipher;
        }

        // 只解密头部用于试探，命中后才把整份数据解出来
        let mut header = [0u8; ANDROID_HEADER_LEN];
        header.copy_from_slice(&buffer[..ANDROID_HEADER_LEN]);
        umc_xmly::android::decrypt_android(self.kind.file_type(), &mut header);

        if !is_data_looks_like_audio(&header) {
            return DecipherOutcome::NotThisCipher;
        }

        let mut result = buffer.to_vec();
        result[..ANDROID_HEADER_LEN].copy_from_slice(&header);

        DecipherOutcome::Ok(Box::new(Deciphered {
            data: result,
            override_extension: None,
            cipher_name: self.cipher_name(),
        }))
    }
}

/// 喜马拉雅 PC 端解密器（`.xm`）。
///
/// 对应 TS 侧 `src/decrypt-worker/decipher/Ximalaya.ts` 的 `XimalayaPCDecipher`。
/// 结构：`[ID3 标签头 + 元数据][被加密的真实音频头][未加密的音频数据]`。
pub struct XimalayaPCDecipher;

impl Decipher for XimalayaPCDecipher {
    fn cipher_name(&self) -> &'static str {
        "Ximalaya (PC)"
    }

    fn decrypt(&self, buffer: &[u8], _options: &DecryptOptions) -> DecipherOutcome {
        let initial_len = PC_INITIAL_HEADER_LEN.min(buffer.len());
        let header = match Header::from_buffer(&buffer[..initial_len]) {
            Ok(header) => header,
            // 元数据比 1024 字节还长时，按解析器给出的长度再读一次
            Err(XmlyError::MetadataTooSmall(required)) => {
                if buffer.len() < required {
                    return DecipherOutcome::NotThisCipher;
                }
                match Header::from_buffer(&buffer[..required]) {
                    Ok(header) => header,
                    Err(_) => return DecipherOutcome::NotThisCipher,
                }
            }
            Err(_) => return DecipherOutcome::NotThisCipher,
        };

        let audio_header = header.copy_m4a_header();
        let Some(encrypted_end) = header
            .data_start_offset
            .checked_add(header.encrypted_header_size)
        else {
            return DecipherOutcome::NotThisCipher;
        };
        if encrypted_end > buffer.len() {
            return DecipherOutcome::NotThisCipher;
        }

        let mut encrypted = buffer[header.data_start_offset..encrypted_end].to_vec();
        let decrypted = match header.decrypt(&mut encrypted) {
            Ok(decrypted) => decrypted.to_vec(),
            Err(err) => return DecipherOutcome::Failed(err.to_string()),
        };

        let mut result =
            Vec::with_capacity(audio_header.len() + decrypted.len() + buffer.len() - encrypted_end);
        result.extend_from_slice(&audio_header);
        result.extend_from_slice(&decrypted);
        result.extend_from_slice(&buffer[encrypted_end..]);

        DecipherOutcome::Ok(Box::new(Deciphered {
            data: result,
            override_extension: None,
            cipher_name: self.cipher_name(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 官方样本：加密头部与对应的明文头部。
    const X2M_HDR: &[u8] =
        include_bytes!("../../../../vendor/lib_um_crypto_rust/um_crypto/xmly/src/__fixture__/x2m_hdr.bin");
    const X2M_HDR_PLAIN: &[u8] =
        include_bytes!("../../../../vendor/lib_um_crypto_rust/um_crypto/xmly/src/__fixture__/x2m_hdr_plain.bin");
    const X3M_HDR: &[u8] =
        include_bytes!("../../../../vendor/lib_um_crypto_rust/um_crypto/xmly/src/__fixture__/x3m_hdr.bin");
    const X3M_HDR_PLAIN: &[u8] =
        include_bytes!("../../../../vendor/lib_um_crypto_rust/um_crypto/xmly/src/__fixture__/x3m_hdr_plain.bin");

    #[test]
    fn android_kind_labels_match_ts() {
        assert_eq!(
            XimalayaAndroidDecipher::new(XimalayaAndroidKind::X2M).cipher_name(),
            "Ximalaya (Android, X2M)"
        );
        assert_eq!(
            XimalayaAndroidDecipher::new(XimalayaAndroidKind::X3M).cipher_name(),
            "Ximalaya (Android, X3M)"
        );
    }

    #[test]
    fn decrypts_x2m_header_fixture() {
        let outcome = XimalayaAndroidDecipher::new(XimalayaAndroidKind::X2M)
            .decrypt(X2M_HDR, &DecryptOptions::default());
        match outcome {
            DecipherOutcome::Ok(done) => assert_eq!(done.data, X2M_HDR_PLAIN),
            other => panic!("应当解密成功，实际为 {other:?}"),
        }
    }

    #[test]
    fn decrypts_x3m_header_fixture() {
        let outcome = XimalayaAndroidDecipher::new(XimalayaAndroidKind::X3M)
            .decrypt(X3M_HDR, &DecryptOptions::default());
        match outcome {
            DecipherOutcome::Ok(done) => assert_eq!(done.data, X3M_HDR_PLAIN),
            other => panic!("应当解密成功，实际为 {other:?}"),
        }
    }

    #[test]
    fn android_rejects_short_input() {
        let outcome = XimalayaAndroidDecipher::new(XimalayaAndroidKind::X2M)
            .decrypt(&[0u8; 16], &DecryptOptions::default());
        assert!(matches!(outcome, DecipherOutcome::NotThisCipher));
    }

    #[test]
    fn pc_rejects_data_without_id3_header() {
        let outcome = XimalayaPCDecipher.decrypt(&[0u8; 2048], &DecryptOptions::default());
        assert!(matches!(outcome, DecipherOutcome::NotThisCipher));
    }

    #[test]
    fn pc_reports_metadata_too_small_as_not_this_cipher() {
        // 只有 ID3 头、声明了很长的元数据，但文件被截断
        let mut buffer = vec![0u8; 32];
        buffer[..3].copy_from_slice(b"ID3");
        buffer[6..10].copy_from_slice(&[0x00, 0x00, 0x10, 0x00]);

        let outcome = XimalayaPCDecipher.decrypt(&buffer, &DecryptOptions::default());
        assert!(matches!(outcome, DecipherOutcome::NotThisCipher));
    }
}
