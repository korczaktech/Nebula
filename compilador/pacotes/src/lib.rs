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
    pub impressao: u64,
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
            "pacote\t{}\t{}\t{:016x}\n",
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
    let nucleo = versao.split(['-', '+']).next().unwrap_or("");
    let partes = nucleo.split('.').collect::<Vec<_>>();
    partes.len() == 3
        && partes.iter().all(|p| {
            !p.is_empty()
                && (*p == "0" || (!p.starts_with('0') && p.bytes().all(|b| b.is_ascii_digit())))
        })
        && versao
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+'))
}

fn fingerprint(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
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
    fn rejeita_versao_invalida_e_chave_duplicada() {
        assert!(Manifesto::analisar("[pacote]\nnome = \"app\"\nversao = \"01.0.0\"").is_err());
        assert!(
            Manifesto::analisar("[pacote]\nnome = \"app\"\nnome = \"outro\"\nversao = \"1.0.0\"")
                .is_err()
        );
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
