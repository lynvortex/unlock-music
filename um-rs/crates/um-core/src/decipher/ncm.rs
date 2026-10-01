use crate::buffer::{chunk_buffer, DEFAULT_BLOCK_LEN};
use crate::decipher::{Decipher, DecipherOutcome, Deciphered, DecryptOptions};
use umc_ncm::header::NCMFile;

/// 网易云音乐 NCM 解密器（`.ncm`）。
///
/// 对应 TS 侧 `src/decrypt-worker/decipher/NetEaseCloudMusic.ts`。
pub struct NetEaseCloudMusicDecipher;

impl Decipher for NetEaseCloudMusicDecipher {
    fn cipher_name(&self) -> &'static str {
        "NCM/PC"
    }

    fn decrypt(&self, buffer: &[u8], _options: &DecryptOptions) -> DecipherOutcome {
        // 原版通过 ncm.open() 逐次喂入更多字节来解析头部，等价于直接传完整缓冲区
        let ncm = match NCMFile::new(buffer) {
            Ok(ncm) => ncm,
            // 任何解析失败都意味着「这不是 NCM 文件」
            Err(_) => return DecipherOutcome::NotThisCipher,
        };

        let mut audio = buffer[ncm.audio_data_offset..].to_vec();
        for (block, offset) in chunk_buffer(&mut audio, DEFAULT_BLOCK_LEN) {
            // 这里的 offset 相对音频起始位置，与原版的 chunkBuffer 语义一致
            ncm.decrypt(block, offset);
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

    const NCM_FIXTURE: &[u8] =
        include_bytes!("../../../../vendor/lib_um_crypto_rust/um_crypto/ncm/src/__fixture__/ncm_test1.bin");

    /// 官方 umc_ncm 测试中给出的、该样本解密后的音频数据。
    const EXPECTED_AUDIO: [u8; 15] = [
        0x49, 0x44, 0x33, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01, 0x73, 0x54, 0x50, 0x45, 0x31, 0x00,
    ];

    #[test]
    fn decrypts_official_ncm_fixture() {
        let outcome = NetEaseCloudMusicDecipher.decrypt(NCM_FIXTURE, &DecryptOptions::default());
        match outcome {
            DecipherOutcome::Ok(done) => assert_eq!(done.data, EXPECTED_AUDIO),
            other => panic!("应当解密成功，实际为 {other:?}"),
        }
    }

    #[test]
    fn rejects_non_ncm_data() {
        let outcome = NetEaseCloudMusicDecipher.decrypt(&[0u8; 64], &DecryptOptions::default());
        assert!(matches!(outcome, DecipherOutcome::NotThisCipher));
    }

    #[test]
    fn rejects_truncated_ncm_header() {
        // 只有 magic，没有后续字段
        let outcome =
            NetEaseCloudMusicDecipher.decrypt(b"CTENFDAM\x00\x01\x00\x00", &DecryptOptions::default());
        assert!(matches!(outcome, DecipherOutcome::NotThisCipher));
    }
}
