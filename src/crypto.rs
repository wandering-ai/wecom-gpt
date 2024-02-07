use sha1::{Digest, Sha1};

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
}
