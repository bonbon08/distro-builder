//! SHA-512-crypt (`$6$…`) für Passwort-Hashes in `/etc/shadow`.
//!
//! Umsetzung der Spezifikation von Ulrich Drepper ("Unix crypt using SHA-256 and
//! SHA-512"), wie sie glibc, `chpasswd` und `openssl passwd -6` verwenden. So landet
//! im erzeugten System nie das Passwort im Klartext, sondern nur der Hash.

use std::io::Read;

use sha2::{Digest, Sha512};

const ROUNDS_DEFAULT: u32 = 5000;
const ROUNDS_MIN: u32 = 1000;
const ROUNDS_MAX: u32 = 999_999_999;
const SALT_MAX: usize = 16;
const B64: &[u8] = b"./0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// Hash mit zufälligem Salt und Standard-Runden, z. B. für `usermod -p`.
pub fn hash_password(password: &str) -> String {
    sha512_crypt(password, &random_salt(), None)
}

/// Prüft, ob ein String wie ein crypt-Hash aussieht (`$id$…`), den `usermod -p` akzeptiert.
pub fn looks_like_hash(s: &str) -> bool {
    let parts: Vec<&str> = s.split('$').collect();
    parts.len() >= 4
        && parts[0].is_empty()
        && !parts[1].is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "./$=".contains(c))
}

/// Prüft ein Passwort gegen einen `$6$`-Hash.
pub fn verify(password: &str, hash: &str) -> bool {
    let Some(rest) = hash.strip_prefix("$6$") else {
        return false;
    };
    let (rounds, rest) = match rest.strip_prefix("rounds=") {
        Some(r) => match r.split_once('$') {
            Some((n, rest)) => (n.parse().ok(), rest),
            None => return false,
        },
        None => (None, rest),
    };
    let Some((salt, _)) = rest.rsplit_once('$') else {
        return false;
    };
    sha512_crypt(password, salt, rounds) == hash
}

fn random_salt() -> String {
    let mut bytes = [0u8; SALT_MAX];
    let filled = std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .is_ok();
    if !filled {
        // Fallback ohne /dev/urandom: Zeit und Prozess-ID mischen
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
            ^ std::process::id() as u128;
        let digest = Sha512::digest(seed.to_le_bytes());
        bytes.copy_from_slice(&digest[..SALT_MAX]);
    }
    bytes
        .iter()
        .map(|b| B64[(*b & 0x3f) as usize] as char)
        .collect()
}

/// `$6$[rounds=N$]salt$hash`. Ist `rounds` gesetzt, erscheint es im Ergebnis.
pub fn sha512_crypt(password: &str, salt: &str, rounds: Option<u32>) -> String {
    let p = password.as_bytes();
    let s: &[u8] = &salt.as_bytes()[..salt.len().min(SALT_MAX)];
    let r = rounds
        .unwrap_or(ROUNDS_DEFAULT)
        .clamp(ROUNDS_MIN, ROUNDS_MAX);

    // Schritte 4–8: B = SHA512(P S P)
    let b = Sha512::new()
        .chain_update(p)
        .chain_update(s)
        .chain_update(p)
        .finalize();

    // Schritte 1–3, 9–12: A
    let mut a_ctx = Sha512::new().chain_update(p).chain_update(s);
    let mut cnt = p.len();
    while cnt > 64 {
        a_ctx.update(b);
        cnt -= 64;
    }
    a_ctx.update(&b[..cnt]);
    let mut i = p.len();
    while i > 0 {
        if i & 1 != 0 {
            a_ctx.update(b);
        } else {
            a_ctx.update(p);
        }
        i >>= 1;
    }
    let mut a = a_ctx.finalize();

    // Schritte 13–16: P-Folge
    let mut dp_ctx = Sha512::new();
    for _ in 0..p.len() {
        dp_ctx.update(p);
    }
    let dp = dp_ctx.finalize();
    let p_seq: Vec<u8> = dp.iter().cycle().take(p.len()).copied().collect();

    // Schritte 17–20: S-Folge
    let mut ds_ctx = Sha512::new();
    for _ in 0..(16 + a[0] as usize) {
        ds_ctx.update(s);
    }
    let ds = ds_ctx.finalize();
    let s_seq: Vec<u8> = ds.iter().cycle().take(s.len()).copied().collect();

    // Schritt 21: Runden
    for round in 0..r {
        let mut c = Sha512::new();
        if round & 1 != 0 {
            c.update(&p_seq);
        } else {
            c.update(a);
        }
        if round % 3 != 0 {
            c.update(&s_seq);
        }
        if round % 7 != 0 {
            c.update(&p_seq);
        }
        if round & 1 != 0 {
            c.update(a);
        } else {
            c.update(&p_seq);
        }
        a = c.finalize();
    }

    // Schritt 22: Base64 in der festgelegten Byte-Reihenfolge
    let mut out = String::from("$6$");
    if let Some(n) = rounds {
        out.push_str(&format!("rounds={}$", n.clamp(ROUNDS_MIN, ROUNDS_MAX)));
    }
    out.push_str(std::str::from_utf8(s).unwrap_or(""));
    out.push('$');
    const ORDER: [(usize, usize, usize); 21] = [
        (0, 21, 42),
        (22, 43, 1),
        (44, 2, 23),
        (3, 24, 45),
        (25, 46, 4),
        (47, 5, 26),
        (6, 27, 48),
        (28, 49, 7),
        (50, 8, 29),
        (9, 30, 51),
        (31, 52, 10),
        (53, 11, 32),
        (12, 33, 54),
        (34, 55, 13),
        (56, 14, 35),
        (15, 36, 57),
        (37, 58, 16),
        (59, 17, 38),
        (18, 39, 60),
        (40, 61, 19),
        (62, 20, 41),
    ];
    for (x, y, z) in ORDER {
        push_b64(&mut out, a[x], a[y], a[z], 4);
    }
    push_b64(&mut out, 0, 0, a[63], 2);
    out
}

fn push_b64(out: &mut String, b2: u8, b1: u8, b0: u8, n: usize) {
    let mut w = ((b2 as u32) << 16) | ((b1 as u32) << 8) | b0 as u32;
    for _ in 0..n {
        out.push(B64[(w & 0x3f) as usize] as char);
        w >>= 6;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Testvektoren aus der Spezifikation ("Unix crypt using SHA-256 and SHA-512").
    #[test]
    fn spec_vectors() {
        assert_eq!(
            sha512_crypt("Hello world!", "saltstring", None),
            "$6$saltstring$svn8UoSVapNtMuq1ukKS4tPQd8iKwSMHWjl/O817G3uBnIFNjnQJuesI68u4OTLiBFdcbYEdFCoEOfaS35inz1"
        );
        assert_eq!(
            sha512_crypt("Hello world!", "saltstringsaltstring", Some(10000)),
            "$6$rounds=10000$saltstringsaltst$OW1/O6BYHV6BcXZu8QVeXbDWra3Oeqh0sbHbbMCVNSnCM/UrjmM0Dp8vOuZeHBy/YTBmSK6H9qs/y3RnOaw5v."
        );
        assert_eq!(
            sha512_crypt("This is just a test", "toolongsaltstring", Some(5000)),
            "$6$rounds=5000$toolongsaltstrin$lQ8jolhgVRVhY4b5pZKaysCLi0QBxGoNeKQzQ3glMhwllF7oGDZxUhx1yxdYcz/e1JSbq3y6JMxxl8audkUEm0"
        );
        assert_eq!(
            sha512_crypt(
                "a very much longer text to encrypt.  This one even stretches over morethan one line.",
                "anotherlongsaltstring",
                Some(1400)
            ),
            "$6$rounds=1400$anotherlongsalts$POfYwTEok97VWcjxIiSOjiykti.o/pQs.wPvMxQ6Fm7I6IoYN3CmLs66x9t0oSwbtEW7o7UmJEiDwGqd8p4ur1"
        );
    }

    #[test]
    fn random_hash_roundtrip() {
        let h1 = hash_password("geheim");
        let h2 = hash_password("geheim");
        assert!(h1.starts_with("$6$"));
        assert_ne!(h1, h2, "Salt muss zufällig sein");
        assert!(verify("geheim", &h1));
        assert!(!verify("falsch", &h1));
        assert!(looks_like_hash(&h1));
        assert!(!looks_like_hash("klartext"));
    }
}
