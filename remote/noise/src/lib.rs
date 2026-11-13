//! The end to end layer of ByteHive Remote, shared by the app and the browser
//! (through wasm) so both run one implementation. Known peers use Noise XX. A
//! new device pairs with XXpsk3: the one-time code is the pre-shared key, so a
//! relay that swaps keys fails the handshake.

use blake2::{Blake2s256, Digest};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use snow::{Builder, HandshakeState, TransportState};

const PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
const PATTERN_PAIR: &str = "Noise_XXpsk3_25519_ChaChaPoly_BLAKE2s";
const PROLOGUE: &[u8] = b"bytehive-remote/1";
/// A Noise message is at most 65535 bytes, 16 of them the tag.
pub const MAX_PLAIN: usize = 65535 - 16;
/// Crockford base32: no I, L, O or U to misread.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

#[derive(Debug)]
pub enum Error {
    Noise(snow::Error),
    Random,
    Sealed,
    State,
}
impl From<snow::Error> for Error {
    fn from(e: snow::Error) -> Self {
        Error::Noise(e)
    }
}

pub struct Keys {
    pub private: Vec<u8>,
    pub public: Vec<u8>,
}

pub fn generate_keys() -> Result<Keys, Error> {
    let k = Builder::new(PATTERN.parse()?).generate_keypair()?;
    Ok(Keys { private: k.private, public: k.public })
}

/// 26 symbols of 5 random bits each (130 bits), grouped to be typed.
pub fn new_code() -> String {
    let mut raw = [0u8; 26];
    getrandom::fill(&mut raw).expect("the system's random source");
    let symbols: Vec<char> = raw.iter().map(|b| ALPHABET[(b & 31) as usize] as char).collect();
    symbols.chunks(4).map(|c| c.iter().collect::<String>()).collect::<Vec<_>>().join("-")
}

fn normal(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            c => c,
        })
        .collect()
}

pub fn psk_from_code(code: &str) -> [u8; 32] {
    Blake2s256::digest(normal(code).as_bytes()).into()
}

#[derive(Clone, Copy)]
pub enum Role {
    Initiator,
    Responder,
}

pub struct Handshake(HandshakeState);

impl Handshake {
    pub fn new(role: Role, private: &[u8], psk: Option<[u8; 32]>) -> Result<Self, Error> {
        if private.len() != 32 {
            return Err(Error::State);
        }
        let pattern = if psk.is_some() { PATTERN_PAIR } else { PATTERN };
        let mut b = Builder::new(pattern.parse()?).prologue(PROLOGUE)?.local_private_key(private)?;
        if let Some(k) = psk.as_ref() {
            b = b.psk(3, k)?;
        }
        Ok(Self(match role {
            Role::Initiator => b.build_initiator()?,
            Role::Responder => b.build_responder()?,
        }))
    }
    pub fn write(&mut self, payload: &[u8]) -> Result<Vec<u8>, Error> {
        let mut out = vec![0u8; payload.len() + 256];
        let n = self.0.write_message(payload, &mut out)?;
        out.truncate(n);
        Ok(out)
    }
    pub fn read(&mut self, msg: &[u8]) -> Result<Vec<u8>, Error> {
        let mut out = vec![0u8; msg.len()];
        let n = self.0.read_message(msg, &mut out)?;
        out.truncate(n);
        Ok(out)
    }
    pub fn finished(&self) -> bool {
        self.0.is_handshake_finished()
    }
    pub fn remote_static(&self) -> Option<Vec<u8>> {
        self.0.get_remote_static().map(|k| k.to_vec())
    }
    pub fn into_transport(self) -> Result<Transport, Error> {
        Ok(Transport(self.0.into_transport_mode()?))
    }
}

pub struct Transport(TransportState);

impl Transport {
    pub fn encrypt(&mut self, plain: &[u8]) -> Result<Vec<u8>, Error> {
        let mut out = vec![0u8; plain.len() + 16];
        let n = self.0.write_message(plain, &mut out)?;
        out.truncate(n);
        Ok(out)
    }
    pub fn decrypt(&mut self, cipher: &[u8]) -> Result<Vec<u8>, Error> {
        let mut out = vec![0u8; cipher.len()];
        let n = self.0.read_message(cipher, &mut out)?;
        out.truncate(n);
        Ok(out)
    }
}

/// Data left with the relay for a device to read later: nonce then ciphertext.
/// `aad` is bound to the ciphertext but not stored in it: open needs the same.
pub fn seal(key: &[u8; 32], aad: &[u8], plain: &[u8]) -> Result<Vec<u8>, Error> {
    let mut nonce = [0u8; 24];
    getrandom::fill(&mut nonce).map_err(|_| Error::Random)?;
    let c = XChaCha20Poly1305::new(Key::from_slice(key))
        .encrypt(XNonce::from_slice(&nonce), Payload { msg: plain, aad })
        .map_err(|_| Error::Sealed)?;
    Ok([nonce.as_slice(), &c].concat())
}

pub fn open(key: &[u8; 32], aad: &[u8], sealed: &[u8]) -> Result<Vec<u8>, Error> {
    if sealed.len() < 24 {
        return Err(Error::Sealed);
    }
    let (nonce, c) = sealed.split_at(24);
    XChaCha20Poly1305::new(Key::from_slice(key))
        .decrypt(XNonce::from_slice(nonce), Payload { msg: c, aad })
        .map_err(|_| Error::Sealed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(a: &Keys, b: &Keys, psk_a: Option<[u8; 32]>, psk_b: Option<[u8; 32]>) -> Result<(Transport, Transport, Vec<u8>), Error> {
        let mut i = Handshake::new(Role::Initiator, &a.private, psk_a)?;
        let mut r = Handshake::new(Role::Responder, &b.private, psk_b)?;
        let m = i.write(&[])?;
        r.read(&m)?;
        let m = r.write(b"machine")?;
        i.read(&m)?;
        let m = i.write(b"device")?;
        let said = r.read(&m)?;
        assert_eq!(i.remote_static().unwrap(), b.public);
        assert_eq!(r.remote_static().unwrap(), a.public);
        Ok((i.into_transport()?, r.into_transport()?, said))
    }

    #[test]
    fn known_peers_talk() {
        let (a, b) = (generate_keys().unwrap(), generate_keys().unwrap());
        let (mut x, mut y, said) = run(&a, &b, None, None).unwrap();
        assert_eq!(said, b"device");
        let c = x.encrypt(b"hello").unwrap();
        assert_eq!(y.decrypt(&c).unwrap(), b"hello");
    }

    #[test]
    fn pairing_needs_the_same_code() {
        let (a, b) = (generate_keys().unwrap(), generate_keys().unwrap());
        let code = new_code();
        let ok = psk_from_code(&code);
        assert!(run(&a, &b, Some(ok), Some(ok)).is_ok());
        assert!(run(&a, &b, Some(psk_from_code("WRONG")), Some(ok)).is_err());
    }

    #[test]
    fn a_changed_or_replayed_packet_is_refused() {
        let (a, b) = (generate_keys().unwrap(), generate_keys().unwrap());
        let (mut x, mut y, _) = run(&a, &b, None, None).unwrap();
        let mut c = x.encrypt(b"one").unwrap();
        c[0] ^= 1;
        assert!(y.decrypt(&c).is_err());
        let (mut x, mut y, _) = run(&a, &b, None, None).unwrap();
        let c = x.encrypt(b"one").unwrap();
        assert!(y.decrypt(&c).is_ok());
        assert!(y.decrypt(&c).is_err());
    }

    #[test]
    fn a_typed_code_matches_however_it_is_written() {
        let code = new_code();
        assert_eq!(code.len(), 26 + 6);
        let typed = code.to_lowercase().replace('-', " ");
        assert_eq!(psk_from_code(&code), psk_from_code(&typed));
        assert_eq!(psk_from_code("O1L"), psk_from_code("011"));
    }

    #[test]
    fn sealed_data_opens_only_with_its_key() {
        let key = [9u8; 32];
        let box_ = seal(&key, b"where", b"note").unwrap();
        assert_eq!(open(&key, b"where", &box_).unwrap(), b"note");
        assert!(open(&[8u8; 32], b"where", &box_).is_err());
    }

    #[test]
    fn sealed_data_is_bound_to_its_aad() {
        let key = [9u8; 32];
        let box_ = seal(&key, b"machine-a", b"note").unwrap();
        assert!(open(&key, b"machine-b", &box_).is_err());
        assert!(open(&key, b"", &box_).is_err());
        assert_eq!(open(&key, b"machine-a", &box_).unwrap(), b"note");
    }

    #[test]
    fn a_private_key_must_be_32_bytes() {
        for len in [0usize, 31, 33] {
            let key = vec![7u8; len];
            assert!(matches!(Handshake::new(Role::Initiator, &key, None), Err(Error::State)));
            assert!(matches!(Handshake::new(Role::Responder, &key, Some([1u8; 32])), Err(Error::State)));
        }
        assert!(Handshake::new(Role::Initiator, &[7u8; 32], None).is_ok());
    }
}
