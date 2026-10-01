use crate::decipher::{Decipher, DecipherOutcome, Deciphered, DecryptOptions};
use umc_xiami::XiamiFile;

/// 虾米音乐 XM 头部长度。
const HEADER_LEN: usize = 0x10;

/// 虾米音乐解密器（`.xm`）。
///
/// 对应 TS 侧 `src/decrypt-worker/decipher/XiamiMusic.ts`。
/// 头部之后的前 `copy_len` 字节是明文，其余部分做逐字节替换。
pub struct XiamiDecipher;

impl Decipher for XiamiDecipher {
    fn cipher_name(&self) -> &'static str {
        "Xiami (XM)"
    }

    fn decrypt(&self, buffer: &[u8], _options: &DecryptOptions) -> DecipherOutcome {
        if buffer.len() < HEADER_LEN {
            return DecipherOutcome::NotThisCipher;
        }

        let xm = match XiamiFile::from_header(&buffer[..HEADER_LEN]) {
            Ok(xm) => xm,
            Err(_) => return DecipherOutcome::NotThisCipher,
        };

        let mut audio = buffer[HEADER_LEN..].to_vec();
        let copy_len = xm.copy_len.min(audio.len());
        xm.decrypt(&mut audio[copy_len..]);

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

    /// 构造一个合法的虾米文件：16 字节头部 + 音频段。
    /// 音频段的前 `copy_len` 字节在文件里本来就是明文，其余部分才是密文。
    fn make_xiami_file(plain: &[u8], copy_len: usize) -> Vec<u8> {
        let key: u8 = 0x11;
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(b"ifmt");
        header[4..8].copy_from_slice(b"mp3 ");
        header[8..12].copy_from_slice(&[0xfe; 4]);
        header[12] = (copy_len & 0xff) as u8;
        header[13] = ((copy_len >> 8) & 0xff) as u8;
        header[14] = ((copy_len >> 16) & 0xff) as u8;
        header[15] = key;

        let effective_key = key.wrapping_sub(1);
        let mut file = header.to_vec();
        file.extend_from_slice(&plain[..copy_len]);
        // 逐字节替换是可逆的：密文 = 密钥 - 明文
        file.extend(
            plain[copy_len..]
                .iter()
                .map(|&b| effective_key.wrapping_sub(b)),
        );
        file
    }

    #[test]
    fn rejects_non_xiami_data() {
        let outcome = XiamiDecipher.decrypt(&[0u8; 64], &DecryptOptions::default());
        assert!(matches!(outcome, DecipherOutcome::NotThisCipher));
    }

    #[test]
    fn rejects_short_input() {
        let outcome = XiamiDecipher.decrypt(b"ifmt", &DecryptOptions::default());
        assert!(matches!(outcome, DecipherOutcome::NotThisCipher));
    }

    #[test]
    fn decrypts_whole_audio_when_copy_len_is_zero() {
        let plain = b"ID3\x04\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
        let file = make_xiami_file(plain, 0);

        let outcome = XiamiDecipher.decrypt(&file, &DecryptOptions::default());
        match outcome {
            DecipherOutcome::Ok(done) => assert_eq!(done.data, plain),
            other => panic!("应当解密成功，实际为 {other:?}"),
        }
    }

    #[test]
    fn keeps_leading_plain_bytes_intact() {
        let plain = b"PLAIN-ID3\x04\x00\x00\x00\x00\x00\x00\x00";
        let file = make_xiami_file(plain, 6);

        let outcome = XiamiDecipher.decrypt(&file, &DecryptOptions::default());
        match outcome {
            DecipherOutcome::Ok(done) => {
                assert_eq!(&done.data[..6], b"PLAIN-");
                assert_eq!(done.data, plain);
            }
            other => panic!("应当解密成功，实际为 {other:?}"),
        }
    }
}
