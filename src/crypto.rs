use aes_gcm::{
    self,
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::prelude::*;
use rand;
use sha1::{Digest, Sha1};
use std::error::Error;

/// 校验微信服务器的请求是否合规
pub fn check_signature(
    signature: &str,
    timestamp: &str,
    nonce: &str,
    token: &str,
    msg: &str,
) -> bool {
    let mut raw = [timestamp, nonce, token, msg];
    raw.sort_unstable();
    let digest = Sha1::digest(raw.concat().as_bytes());
    let hex_digest = base16ct::lower::encode_string(&digest);
    signature == hex_digest
}

#[derive(PartialEq, Debug)]
pub struct CryptoSource {
    text: String,
    receive_id: String,
}

pub struct CryptoAgent {
    cipher: Aes256Gcm,
}

impl CryptoAgent {
    pub fn new(key: &str) -> Self {
        let aes_key = Key::<Aes256Gcm>::from_slice(key.as_bytes());
        let cipher = Aes256Gcm::new(&aes_key);
        Self { cipher }
    }

    /// 加密
    pub fn encrypt(&self, input: &CryptoSource) -> (String, String) {
        // 待加密数据
        let mut block: Vec<u8> = Vec::new();

        // 16字节随机字符串
        block.extend(rand::random::<[u8; 16]>());

        // 明文字符串长度
        block.extend((input.text.len() as u32).to_be_bytes());

        // 明文字符串
        block.extend(input.text.as_bytes());

        // Receive ID
        block.extend(input.receive_id.as_bytes());

        // 加密
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let cipher_bytes = self.cipher.encrypt(&nonce, block.as_ref()).unwrap();
        (
            BASE64_STANDARD.encode(cipher_bytes),
            BASE64_STANDARD.encode(nonce),
        )
    }

    /// 解密
    pub fn decrypt(&self, encoded: &str, nonce: &str) -> Result<CryptoSource, Box<dyn Error>> {
        let cipher_bytes = BASE64_STANDARD.decode(encoded)?;
        let nonce_bytes = BASE64_STANDARD.decode(nonce)?;
        let nonce = Nonce::from_slice(&nonce_bytes);
        let block = self.cipher.decrypt(nonce, &cipher_bytes[..]).unwrap();
        let buf = block.as_slice();
        let msg_len: usize = u32::from_be_bytes(buf[16..20].try_into().unwrap()) as usize;
        let text = String::from_utf8(buf[20..20 + msg_len].to_vec())?;
        let receive_id = String::from_utf8(buf[20 + msg_len..].to_vec())?;
        Ok(CryptoSource { text, receive_id })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_check_signature() {
        assert_eq!(
            check_signature(
                "a8addbc99f8b3f51d2adbceb605d650b9a8940e2",
                "0",
                "c",
                "a",
                "b"
            ),
            true
        );
    }

    #[test]
    fn test_encrypt_decrypt() {
        let key: String = String::from_utf8(Vec::from([42; 32])).unwrap();
        let agent = CryptoAgent::new(key.as_ref());
        let source = CryptoSource {
            text: "abcd".to_string(),
            receive_id: "xyz".to_string(),
        };
        let (enc, nonce) = agent.encrypt(&source);
        let dec = agent.decrypt(enc.as_str(), nonce.as_str()).unwrap();
        assert_eq!(source, dec);
    }
}
