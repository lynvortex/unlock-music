use crate::audio_type::is_data_looks_like_audio;
use crate::buffer::{chunk_buffer, DEFAULT_BLOCK_LEN};
use crate::decipher::{Decipher, DecipherOutcome, Deciphered, DecryptOptions};

/// 试探用的头部长度，与 TS 侧 `buffer.slice(0, 0x20)` 一致。
const PROBE_LEN: usize = 0x20;

/// QQ 音乐 QMCv1 解密器（`.qmc3` / `.qmcflac` 等）。
///
/// 对应 TS 侧 `src/decrypt-worker/decipher/QQMusic.ts` 的 `QQMusicV1Decipher`。
/// 使用固定静态密钥，因此没有头部或尾部特征，只能靠「解密后是否像音频」来判断。
pub struct QQMusicV1Decipher;

impl Decipher for QQMusicV1Decipher {
    fn cipher_name(&self) -> &'static str {
        "QQMusic/QMC1"
    }

    fn decrypt(&self, buffer: &[u8], _options: &DecryptOptions) -> DecipherOutcome {
        if buffer.len() < PROBE_LEN {
            return DecipherOutcome::NotThisCipher;
        }

        let mut probe = [0u8; PROBE_LEN];
        probe.copy_from_slice(&buffer[..PROBE_LEN]);
        umc_qmc::v1::decrypt(&mut probe, 0);
        if !is_data_looks_like_audio(&probe) {
            return DecipherOutcome::NotThisCipher;
        }

        let mut audio = buffer.to_vec();
        for (block, offset) in chunk_buffer(&mut audio, DEFAULT_BLOCK_LEN) {
            umc_qmc::v1::decrypt(block, offset);
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

    /// 官方 `umc_qmc::v1` 测试里的已知向量。
    /// 由它反推出前 11 字节的密钥流，用来构造「解密后像音频」的输入。
    const REF_CIPHER: [u8; 11] = [
        0xab, 0x2f, 0xba, 0xa6, 0xff, 0x47, 0x80, 0x3d, 0xaa, 0xcd, 0x02,
    ];
    const REF_PLAIN: &[u8; 11] = b"hello world";

    /// 构造一个前 11 字节解密后等于 `desired` 的 QMCv1 文件。
    fn make_qmc1_file(desired: &[u8; 11]) -> Vec<u8> {
        let mut file = vec![0u8; PROBE_LEN];
        for (i, byte) in file.iter_mut().take(11).enumerate() {
            let keystream = REF_CIPHER[i] ^ REF_PLAIN[i];
            *byte = desired[i] ^ keystream;
        }
        file
    }

    #[test]
    fn permissive_probe_claims_unrecognized_prefix() {
        // 与原版一致的行为：探测缓冲区只有 0x20 字节，不足以判定时
        // `detectAudioType` 会返回 NeedMoreHeader，`is_data_looks_like_audio`
        // 因此把它当作音频。这是上游既有的宽松判定，真正兜底的是
        // `decrypt_any` 对解密结果做的音频类型复查。
        let outcome = QQMusicV1Decipher.decrypt(&[0u8; 64], &DecryptOptions::default());
        assert!(matches!(outcome, DecipherOutcome::Ok(_)));
    }

    #[test]
    fn rejects_short_input() {
        let outcome = QQMusicV1Decipher.decrypt(&[0u8; 8], &DecryptOptions::default());
        assert!(matches!(outcome, DecipherOutcome::NotThisCipher));
    }

    #[test]
    fn decrypts_header_using_official_test_vector() {
        // 让解密后的前 3 字节为 "ID3"，从而通过音频试探
        let desired = b"ID3abcdefgh";
        let file = make_qmc1_file(desired);

        let outcome = QQMusicV1Decipher.decrypt(&file, &DecryptOptions::default());
        match outcome {
            DecipherOutcome::Ok(done) => assert_eq!(&done.data[..11], desired.as_slice()),
            other => panic!("应当解密成功，实际为 {other:?}"),
        }
    }
}
