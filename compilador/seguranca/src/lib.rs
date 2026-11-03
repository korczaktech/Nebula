//! Primitivas criptográficas implementadas sem dependências externas.
//!
//! Esta crate oferece SHA-256, HMAC-SHA-256 e comparação de dados em tempo
//! constante em relação ao conteúdo (o comprimento ainda é observável).

fn sha256_digest(bytes: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut estado = [
        0x6a09e667_u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (bytes.len() as u128).wrapping_mul(8) as u64;
    let mut mensagem = Vec::with_capacity(bytes.len().saturating_add(72));
    mensagem.extend_from_slice(bytes);
    mensagem.push(0x80);
    while mensagem.len() % 64 != 56 {
        mensagem.push(0);
    }
    mensagem.extend_from_slice(&bit_len.to_be_bytes());

    for bloco in mensagem.chunks_exact(64) {
        let mut agenda = [0_u32; 64];
        for (i, parte) in bloco.chunks_exact(4).enumerate() {
            agenda[i] = u32::from_be_bytes([parte[0], parte[1], parte[2], parte[3]]);
        }
        for i in 16..64 {
            let x = agenda[i - 15];
            let y = agenda[i - 2];
            let sigma0 = x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3);
            let sigma1 = y.rotate_right(17) ^ y.rotate_right(19) ^ (y >> 10);
            agenda[i] = agenda[i - 16]
                .wrapping_add(sigma0)
                .wrapping_add(agenda[i - 7])
                .wrapping_add(sigma1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = estado;
        for i in 0..64 {
            let sigma1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let escolha = (e & f) ^ ((!e) & g);
            let t1 = h
                .wrapping_add(sigma1)
                .wrapping_add(escolha)
                .wrapping_add(K[i])
                .wrapping_add(agenda[i]);
            let sigma0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maioria = (a & b) ^ (a & c) ^ (b & c);
            let t2 = sigma0.wrapping_add(maioria);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (destino, valor) in estado.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *destino = destino.wrapping_add(valor);
        }
    }
    let mut digest = [0_u8; 32];
    for (indice, palavra) in estado.iter().enumerate() {
        digest[indice * 4..indice * 4 + 4].copy_from_slice(&palavra.to_be_bytes());
    }
    digest
}


/// Calcula SHA-256 e retorna o resumo em hexadecimal minúsculo.
#[must_use]
pub fn sha256_hex(dados: &[u8]) -> String {
    digest_hex(&sha256_digest(dados))
}

/// Calcula HMAC-SHA-256 conforme RFC 2104 usando SHA-256.
#[must_use]
pub fn hmac_sha256_hex(chave: &[u8], mensagem: &[u8]) -> String {
    let mut chave_bloco = [0_u8; 64];
    if chave.len() > chave_bloco.len() {
        chave_bloco[..32].copy_from_slice(&sha256_digest(chave));
    } else {
        chave_bloco[..chave.len()].copy_from_slice(chave);
    }
    let mut interno = Vec::with_capacity(64 + mensagem.len());
    let mut externo = Vec::with_capacity(96);
    interno.extend(chave_bloco.iter().map(|byte| byte ^ 0x36));
    interno.extend_from_slice(mensagem);
    externo.extend(chave_bloco.iter().map(|byte| byte ^ 0x5c));
    externo.extend_from_slice(&sha256_digest(&interno));
    digest_hex(&sha256_digest(&externo))
}

/// Compara sequências sem encerrar ao encontrar o primeiro byte diferente.
/// O comprimento das sequências não é ocultado.
#[must_use]
pub fn comparar_seguro(a: &[u8], b: &[u8]) -> bool {
    let limite = a.len().max(b.len());
    let mut diferenca = a.len() ^ b.len();
    for indice in 0..limite {
        let x = a.get(indice).copied().unwrap_or(0);
        let y = b.get(indice).copied().unwrap_or(0);
        diferenca |= usize::from(x ^ y);
    }
    diferenca == 0
}

fn digest_hex(digest: &[u8; 32]) -> String {
    let mut hexadecimal = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(hexadecimal, "{byte:02x}");
    }
    hexadecimal
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn sha256_corresponde_a_vetores_conhecidos() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }

    #[test]
    fn hmac_sha256_corresponde_a_vetor_rfc_4231() {
        let chave = [0x0b_u8; 20];
        assert_eq!(hmac_sha256_hex(&chave, b"Hi There"), "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7");
    }

    #[test]
    fn comparacao_segura_verifica_conteudo_e_tamanho() {
        assert!(comparar_seguro(b"segredo", b"segredo"));
        assert!(!comparar_seguro(b"segredo", b"segreto"));
        assert!(!comparar_seguro(b"segredo", b"segredo!"));
    }
}
