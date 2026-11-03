//! Manifestos e resolução reproduzível de dependências locais da Nebula.
use nebula_seguranca::sha256_hex;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static PROXIMO_LOCKFILE: AtomicU64 = AtomicU64::new(1);

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

/// Confere se o lockfile existente corresponde exatamente ao grafo resolvido.
pub fn validar_lockfile(conteudo: &str, resolucao: &Resolucao) -> Result<(), String> {
    if conteudo == resolucao.lockfile {
        Ok(())
    } else {
        Err("nebula.lock está desatualizado ou foi alterado; execute 'nebula dependencias resolver'".to_owned())
    }
}

/// Resolve o projeto e confere o lockfile sem modificá-lo.
pub fn verificar_lockfile(diretorio: &Path) -> Result<Resolucao, String> {
    let resolucao = resolver(diretorio)?;
    let caminho = diretorio.join("nebula.lock");
    let conteudo = fs::read_to_string(&caminho)
        .map_err(|erro| format!("não foi possível ler '{}': {erro}", caminho.display()))?;
    validar_lockfile(&conteudo, &resolucao)?;
    Ok(resolucao)
}

/// Publica o lockfile por arquivo temporário e substituição atômica.
pub fn gravar_lockfile(diretorio: &Path, resolucao: &Resolucao) -> Result<PathBuf, String> {
    let diretorio = fs::canonicalize(diretorio)
        .map_err(|erro| format!("diretório do projeto indisponível: {erro}"))?;
    let caminho = diretorio.join("nebula.lock");
    if let Ok(meta) = fs::symlink_metadata(&caminho) {
        if meta.file_type().is_symlink() || !meta.is_file() {
            return Err("nebula.lock existente não é um arquivo regular".to_owned());
        }
    }
    let id = PROXIMO_LOCKFILE.fetch_add(1, Ordering::Relaxed);
    let temporario = diretorio.join(format!(".nebula.lock.{}.{}.tmp", std::process::id(), id));
    let resultado = (|| {
        let mut arquivo = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporario)
            .map_err(|erro| format!("não foi possível criar lockfile temporário: {erro}"))?;
        arquivo
            .write_all(resolucao.lockfile.as_bytes())
            .map_err(|erro| format!("não foi possível gravar lockfile: {erro}"))?;
        arquivo
            .sync_all()
            .map_err(|erro| format!("não foi possível sincronizar lockfile: {erro}"))?;
        drop(arquivo);
        if fs::symlink_metadata(&caminho)
            .is_ok_and(|meta| meta.file_type().is_symlink() || !meta.is_file())
        {
            return Err("nebula.lock foi substituído por um objeto não regular".to_owned());
        }
        fs::rename(&temporario, &caminho)
            .map_err(|erro| format!("não foi possível publicar nebula.lock: {erro}"))?;
        Ok(caminho.clone())
    })();
    if resultado.is_err() {
        let _ = fs::remove_file(&temporario);
    }
    resultado
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
        let impressao = sha256_hex(conteudo.as_bytes());
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
    fn manifestos_resistem_a_entradas_adversariais_deterministicas() {
        for semente_inicial in 0..512_u32 {
            let mut estado = semente_inicial.wrapping_add(0x85eb_ca6b);
            let tamanho = semente_inicial as usize % 128 + 1;
            let mut entrada = String::with_capacity(tamanho);
            for _ in 0..tamanho {
                estado = estado.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                let byte = b' ' + ((estado >> 24) % 95) as u8;
                entrada.push(char::from(byte));
            }
            let _ = Manifesto::analisar(&entrada);
        }
    }

    fn projeto_com_dependencia_local(rotulo: &str) -> PathBuf {
        let raiz = std::env::temp_dir().join(format!(
            "nebula-lock-{rotulo}-{}-{}",
            std::process::id(),
            PROXIMO_LOCKFILE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(raiz.join("texto")).unwrap();
        fs::write(
            raiz.join("nebula.toml"),
            "[pacote]\nnome = \"app\"\nversao = \"1.0.0\"\n[dependencias]\ntexto = \"texto\"",
        )
        .unwrap();
        fs::write(
            raiz.join("texto/nebula.toml"),
            "[pacote]\nnome = \"texto\"\nversao = \"1.2.3\"",
        )
        .unwrap();
        raiz
    }

    #[test]
    fn lockfile_verifica_grafo_e_detecta_alteracao() {
        let raiz = projeto_com_dependencia_local("verificacao");
        let resolucao = resolver(&raiz).unwrap();
        gravar_lockfile(&raiz, &resolucao).unwrap();
        assert!(verificar_lockfile(&raiz).is_ok());
        fs::write(raiz.join("nebula.lock"), "nebula-lock-v1\ncorrompido").unwrap();
        assert!(verificar_lockfile(&raiz).is_err());
        let _ = fs::remove_dir_all(raiz);
    }

    #[test]
    fn lockfile_recusa_link_simbolico_no_destino() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let raiz = projeto_com_dependencia_local("symlink");
            let resolucao = resolver(&raiz).unwrap();
            let externo = raiz.join("externo");
            fs::write(&externo, "nao tocar").unwrap();
            symlink(&externo, raiz.join("nebula.lock")).unwrap();
            assert!(gravar_lockfile(&raiz, &resolucao).is_err());
            assert_eq!(fs::read_to_string(externo).unwrap(), "nao tocar");
            let _ = fs::remove_dir_all(raiz);
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
