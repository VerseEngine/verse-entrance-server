use aes_gcm::aead::Aead;
use aes_gcm::KeyInit;
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::Result;
#[allow(unused_imports)]
use log::{error, info, warn};

pub fn create_static_secret() -> Result<x25519_dalek::StaticSecret> {
    let sk = &mut [0u8; 32];
    getrandom::getrandom(sk)?;
    Ok(x25519_dalek::StaticSecret::from(*sk))
}

const NONCE_SIZE: usize = 12;

pub struct E2eKeySet {
    pub my_public: x25519_dalek::PublicKey,
    pub peer_public: x25519_dalek::PublicKey,
    pub shared_secret: [u8; 32],
}

impl E2eKeySet {
    pub fn new(peer_public: &x25519_dalek::PublicKey) -> Result<Self> {
        let secret = create_static_secret()?;
        Ok(E2eKeySet {
            peer_public: peer_public.to_owned(),
            my_public: x25519_dalek::PublicKey::from(&secret),
            shared_secret: secret.diffie_hellman(peer_public).to_bytes(),
        })
    }
    pub fn new_with_secret(
        peer_public: &x25519_dalek::PublicKey,
        secret: &x25519_dalek::StaticSecret,
    ) -> Result<Self> {
        let secret = secret.to_owned();
        Ok(E2eKeySet {
            peer_public: peer_public.to_owned(),
            my_public: x25519_dalek::PublicKey::from(&secret),
            shared_secret: secret.diffie_hellman(peer_public).to_bytes(),
        })
    }
    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(&self.shared_secret)?;
        let mut nonce = [0u8; NONCE_SIZE];
        getrandom::getrandom(&mut nonce)?;
        let encrypted = cipher.encrypt(Nonce::from_slice(nonce.as_ref()), data)?;
        Ok([nonce.to_vec(), encrypted].concat())
    }
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(&self.shared_secret)?;
        let nonce = Nonce::from_slice(&data[0..NONCE_SIZE]);
        let encrypted = &data[NONCE_SIZE..];
        let plain = cipher.decrypt(nonce, encrypted.as_ref())?;
        Ok(plain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_keypair0() {
        let sk = create_static_secret().unwrap();

        let sk2 = x25519_dalek::StaticSecret::from(sk.to_bytes());
        assert_eq!(&sk.to_bytes(), &sk2.to_bytes());
    }

    #[test]
    fn test_dec_enc() {
        let sk0 = create_static_secret().unwrap();
        let pk0 = x25519_dalek::PublicKey::from(&sk0);
        let e2e1 = E2eKeySet::new(&pk0).unwrap();
        let e2e0 = { E2eKeySet::new_with_secret(&e2e1.my_public, &sk0).unwrap() };

        assert_eq!(e2e0.shared_secret, e2e1.shared_secret);
        {
            let enc = e2e0.encrypt("testtest".as_bytes()).unwrap();
            let dec = e2e1.decrypt(&enc).unwrap();
            assert_eq!(dec, "testtest".as_bytes());
        }
        {
            let enc = e2e1.encrypt("testtest".as_bytes()).unwrap();
            let dec = e2e0.decrypt(&enc).unwrap();
            assert_eq!(dec, "testtest".as_bytes());
        }
    }
}
