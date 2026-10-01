use crate::decipher::{Decipher, DecipherOutcome, Deciphered, DecryptOptions};

/// 不做任何解密，原样返回数据。
///
/// 对应 TS 侧 `src/decrypt-worker/decipher/Transparent.ts`。
/// 它在注册表中排在最后，作为"文件本来就没加密"的兜底。
pub struct TransparentDecipher;

impl Decipher for TransparentDecipher {
    fn cipher_name(&self) -> &'static str {
        "none"
    }

    fn decrypt(&self, buffer: &[u8], _options: &DecryptOptions) -> DecipherOutcome {
        DecipherOutcome::Ok(Box::new(Deciphered {
            data: buffer.to_vec(),
            override_extension: None,
            cipher_name: "None",
        }))
    }
}
