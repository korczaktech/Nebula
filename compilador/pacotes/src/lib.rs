//! Manifestos e resolução reproduzível de dependências locais da Nebula.
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Manifesto de um pacote Nebula.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifesto {
    /// Nome único do pacote.
    pub nome: String,
    /// Versão SemVer exata.
    pub versao: String,
    /// Dependências locais, indexadas pelo nome declarado.
    pub dependencias: BTreeMap<String, String>,
}

/// Pacote local encontrado pelo resolvedor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PacoteResolvido {
    /// Nome do pacote.
    pub nome: String,
    /// Versão encontrada.
    pub versao: String,
    /// Caminho canônico do manifesto.
    pub manifesto: PathBuf,
    /// Impressão digital determinística do manifesto.
    pub impressao: String,
}

/// Resultado completo de uma resolução local.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolucao {
    /// Manifesto raiz.
    pub raiz: Manifesto,
    /// Dependências transitivas ordenadas por nome.
    pub pacotes: Vec<PacoteResolvido>,
    /// Conteúdo determinístico do lockfile.
    pub lockfile: String,
}

impl Manifesto {
    /// Analisa o subconjunto estrito de nebula.toml suportado.
    ///
    /// Formato:
    /// [pacote]
    /// nome = "aplicacao"
    /// versao = "1.0.0"
    /// [dependencias]
    /// biblioteca = "../biblioteca"
    pub fn analisar(conteudo: &str) -> Result<Self, String> {
        let mut secao = "";
        let mut nome = None;
        let mut versao = None;
        let mut dependencias = BTreeMap::new();
        for (indice, original) in conteudo.lines().enumerate() {
            let linha = original.split('#').next().unwrap_or("").trim();
            if linha.is_empty() {
                continue;
            }
            if linha.starts_with('[') && linha.ends_with(']') {
                secao = &linha[1..linha.len() - 1];
                if secao != "pacote" && secao != "dependencias" {
                    return Err(format!(
                        "Seção desconhecida na linha {}: {secao}",
                        indice + 1
                    ));
                }
                continue;
            }
            let (chave, valor) = linha
                .split_once('=')
                .ok_or_else(|| format!("Esperado '=' na linha {}.", indice + 1))?;
            let chave = chave.trim();
            let valor = valor.trim();
            let valor = valor
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .ok_or_else(|| {
                    format!(
                        "Valor da linha {} deve estar entre aspas duplas.",
                        indice + 1
                    )
                })?;
            if valor.is_empty() {
                return Err(format!("Valor vazio na linha {}.", indice + 1));
            }
            match secao {
                "pacote" => match chave {
                    "nome" if nome.is_none() => nome = Some(valor.to_owned()),
                    "versao" if versao.is_none() => versao = Some(valor.to_owned()),
                    "nome" | "versao" => {
                        return Err(format!("Chave duplicada na linha {}: {chave}", indice + 1));
                    }
                    _ => return Err(format!("Chave desconhecida na seção pacote: {chave}")),
                },
                "dependencias" => {
                    if !nome_valido(chave) {
                        return Err(format!("Nome de dependência inválido: {chave}"));
                    }
                    if dependencias
                        .insert(chave.to_owned(), valor.to_owned())
                        .is_some()
                    {
                        return Err(format!("Dependência duplicada: {chave}"));
                    }
                }
                _ => return Err(format!("A chave {chave} está fora de uma seção válida.")),
            }
        }
        let nome = nome.ok_or("Manifesto sem pacote.nome.")?;
        if !nome_valido(&nome) {
            return Err(format!("Nome de pacote inválido: {nome}"));
        }
        let versao = versao.ok_or("Manifesto sem pacote.versao.")?;
        if !versao_semantica_valida(&versao) {
            return Err(format!("Versão não é SemVer válida: {versao}"));
        }
        Ok(Self {
            nome,
            versao,
            dependencias,
        })
    }
}

/// Lê e resolve o grafo de dependências locais a partir de um diretório.
pub fn resolver(diretorio: &Path) -> Result<Resolucao, String> {
    let raiz_dir = fs::canonicalize(diretorio)
        .map_err(|e| format!("Diretório do projeto indisponível: {e}"))?;
    let caminho_raiz = raiz_dir.join("nebula.toml");
    let conteudo_raiz = fs::read_to_string(&caminho_raiz)
        .map_err(|e| format!("Não foi possível ler {}: {e}", caminho_raiz.display()))?;
    let raiz = Manifesto::analisar(&conteudo_raiz)?;
    let mut encontrados = BTreeMap::new();
    let mut visitando = BTreeSet::new();
    resolver_dependencias(&raiz, &raiz_dir, &mut encontrados, &mut visitando)?;
    let pacotes = encontrados.into_values().collect::<Vec<_>>();
    let mut lockfile = format!("nebula-lock-v1\nraiz\t{}\t{}\n", raiz.nome, raiz.versao);
    for pacote in &pacotes {
        lockfile.push_str(&format!(
            "pacote\t{}\t{}\t{}\n",
            pacote.nome, pacote.versao, pacote.impressao
        ));
    }
    Ok(Resolucao {
        raiz,
        pacotes,
        lockfile,
    })
}

fn resolver_dependencias(
    manifesto: &Manifesto,
    diretorio: &Path,
    encontrados: &mut BTreeMap<String, PacoteResolvido>,
    visitando: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    for (nome, relativo) in &manifesto.dependencias {
        let caminho = Path::new(relativo);
        if caminho.is_absolute() {
            return Err(format!("Dependência '{nome}' deve usar caminho relativo."));
        }
        let diretorio_dep = fs::canonicalize(diretorio.join(caminho))
            .map_err(|e| format!("Dependência local '{nome}' não encontrada: {e}"))?;
        let arquivo = diretorio_dep.join("nebula.toml");
        let arquivo = fs::canonicalize(&arquivo)
            .map_err(|e| format!("Manifesto da dependência '{nome}' não encontrado: {e}"))?;
        if !visitando.insert(arquivo.clone()) {
            return Err(format!("Ciclo de dependências detectado em '{nome}'."));
        }
        let conteudo = fs::read_to_string(&arquivo)
            .map_err(|e| format!("Não foi possível ler pacote '{nome}': {e}"))?;
        let dependencia = Manifesto::analisar(&conteudo)?;
        if dependencia.nome != *nome {
            return Err(format!(
                "A dependência '{nome}' aponta para o pacote '{}'.",
                dependencia.nome
            ));
        }
        let impressao = fingerprint(conteudo.as_bytes());
        let pacote = PacoteResolvido {
            nome: dependencia.nome.clone(),
            versao: dependencia.versao.clone(),
            manifesto: arquivo.clone(),
            impressao,
        };
        if let Some(anterior) = encontrados.get(nome) {
            if anterior != &pacote {
                return Err(format!("Conflito de dependência para '{nome}'."));
            }
        } else {
            encontrados.insert(nome.clone(), pacote);
        }
        resolver_dependencias(
            &dependencia,
            diretorio_dep.as_path(),
            encontrados,
            visitando,
        )?;
        visitando.remove(&arquivo);
    }
    Ok(())
}

fn nome_valido(nome: &str) -> bool {
    !nome.is_empty()
        && nome
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn versao_semantica_valida(versao: &str) -> bool {
    let mut partes_build = versao.split('+');
    let versao_pre_build = partes_build.next().unwrap_or("");
    let build = partes_build.next();
    if partes_build.next().is_some() {
        return false;
    }

    let (nucleo, prerelease) = versao_pre_build
        .split_once('-')
        .map_or((versao_pre_build, None), |(nucleo, prerelease)| {
            (nucleo, Some(prerelease))
        });
    let partes = nucleo.split('.').collect::<Vec<_>>();
    if partes.len() != 3
        || !partes.iter().all(|parte| {
            !parte.is_empty()
                && (parte == &"0"
                    || (!parte.starts_with('0') && parte.bytes().all(|byte| byte.is_ascii_digit())))
        })
    {
        return false;
    }

    if prerelease.is_some_and(|valor| !identificadores_semver_validos(valor, true)) {
        return false;
    }
    if build.is_some_and(|valor| !identificadores_semver_validos(valor, false)) {
        return false;
    }
    true
}

fn identificadores_semver_validos(valor: &str, prerelease: bool) -> bool {
    !valor.is_empty()
        && valor.split('.').all(|identificador| {
            !identificador.is_empty()
                && identificador
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                && (!prerelease
                    || !identificador.bytes().all(|byte| byte.is_ascii_digit())
                    || identificador == "0"
                    || !identificador.starts_with('0'))
        })
}

/// Calcula SHA-256 para identificar o conteúdo exato de um manifesto.
fn fingerprint(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98,
        0x71374491,
        0xb5c0fbcf,
        0xe9b5dba5,
        0x3956c25b,
        0x59f111f1,
        0x923f82a4,
        0xab1c5ed5,
        0xd807aa98,
        0x12835b01,
        0x243185be,
        0x550c7dc3,
        0x72be5d74,
        0x80deb1fe,
        0x9bdc06a7,
        0xc19bf174,
        0xe49b69c1,
        0xefbe4786,
        0x0fc19dc6,
        0x240ca1cc,
        0x2de92c6f,
        0x4a7484aa,
        0x5cb0a9dc,
        0x76f988da,
        0x983e5152,
        0xa831c66d,
        0xb00327c8,
        0xbf597fc7,
        0xc6e00bf3,
        0xd5a79147,
        0x06ca6351,
        0x14292967,
        0x27b70a85,
        0x2e1b2138,
        0x4d2c6dfc,
        0x53380d13,
        0x650a7354,
        0x766a0abb,
        0x81c2c92e,
        0x92722c85,
        0xa2bfe8a1,
        0xa81a664b,
        0xc24b8b70,
        0xc76c51a3,
        0xd192e819,
        0xd6990624,
        0xf40e3585,
        0x106aa070,
        0x19a4c116,
        0x1e376c08,
        0x2748774c,
        0x34b0bcb5,
        0x391c0cb3,
        0x4ed8aa4a,
        0x5b9cca4f,
        0x682e6ff3,
        0x748f82ee,
        0x78a5636f,
        0x84c87814,
        0x8cc70208,
        0x90befffa,
        0xa4506ceb,
        0xbef9a3f7,
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
    let mut hexadecimal = String::with_capacity(64);
    for palavra in estado {
        use std::fmt::Write as _;
        let _ = write!(hexadecimal, "{palavra:08x}");
    }
    hexadecimal
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn analisa_manifesto_com_dependencia_local() {
        let m = Manifesto::analisar(
            "[pacote]\nnome = \"app\"\nversao = \"1.2.3\"\n[dependencias]\ntexto = \"../texto\"",
        )
        .unwrap();
        assert_eq!(m.nome, "app");
        assert_eq!(m.versao, "1.2.3");
        assert_eq!(m.dependencias.get("texto").unwrap(), "../texto");
    }

    #[test]
    fn fingerprint_usa_sha256_conhecido() {
        assert_eq!(
            fingerprint(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            fingerprint(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn rejeita_versao_invalida_e_chave_duplicada() {
        assert!(Manifesto::analisar("[pacote]\nnome = \"app\"\nversao = \"01.0.0\"").is_err());
        assert!(
            Manifesto::analisar("[pacote]\nnome = \"app\"\nnome = \"outro\"\nversao = \"1.0.0\"")
                .is_err()
        );
    }

    #[test]
    fn valida_identificadores_semver_de_pre_lancamento_e_build() {
        for versao in ["1.2.3-alpha.1", "1.2.3+build.7", "1.2.3-rc.1+build.7"] {
            assert!(versao_semantica_valida(versao), "{versao}");
        }
        for versao in [
            "1.2.3-alpha.01",
            "1.2.3+",
            "1.2.3-alpha..1",
            "1.2.3+build+outro",
            "1.2.3-",
            "1.2",
        ] {
            assert!(!versao_semantica_valida(versao), "{versao}");
        }
    }

    #[test]
    fn resolve_dependencias_transitivas_com_lock_deterministico() {
        let raiz = std::env::temp_dir().join(format!("nebula-pacotes-{}", std::process::id()));
        let _ = fs::remove_dir_all(&raiz);
        fs::create_dir_all(raiz.join("a/b")).unwrap();
        fs::write(
            raiz.join("nebula.toml"),
            "[pacote]\nnome = \"app\"\nversao = \"1.0.0\"\n[dependencias]\na = \"a\"",
        )
        .unwrap();
        fs::write(
            raiz.join("a/nebula.toml"),
            "[pacote]\nnome = \"a\"\nversao = \"1.1.0\"\n[dependencias]\nb = \"b\"",
        )
        .unwrap();
        fs::write(
            raiz.join("a/b/nebula.toml"),
            "[pacote]\nnome = \"b\"\nversao = \"2.0.0\"",
        )
        .unwrap();
        let primeira = resolver(&raiz).unwrap();
        let segunda = resolver(&raiz).unwrap();
        assert_eq!(primeira, segunda);
        assert_eq!(primeira.pacotes.len(), 2);
        assert!(primeira.lockfile.contains("nebula-lock-v1"));
        let _ = fs::remove_dir_all(&raiz);
    }
}
