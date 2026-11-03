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
    /// Restrições SemVer opcionais associadas às dependências.
    pub restricoes_versao: BTreeMap<String, String>,
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

fn remover_comentario(linha: &str) -> &str {
    let mut em_texto = false;
    let mut escape = false;
    for (indice, caractere) in linha.char_indices() {
        if em_texto {
            if escape {
                escape = false;
            } else if caractere == '\\' {
                escape = true;
            } else if caractere == '"' {
                em_texto = false;
            }
        } else if caractere == '"' {
            em_texto = true;
        } else if caractere == '#' {
            return &linha[..indice];
        }
    }
    linha
}

fn decodificar_string_toml(valor: &str) -> Result<String, String> {
    let mut caracteres = valor.chars();
    if caracteres.next() != Some('"') {
        return Err("esperada string entre aspas duplas".to_owned());
    }
    let mut resultado = String::new();
    loop {
        match caracteres.next() {
            Some('"') => {
                if caracteres.next().is_some() {
                    return Err("conteúdo após o fechamento da string".to_owned());
                }
                return Ok(resultado);
            }
            Some('\\') => {
                let escape = caracteres
                    .next()
                    .ok_or_else(|| "escape incompleto na string".to_owned())?;
                match escape {
                    '"' => resultado.push('"'),
                    '\\' => resultado.push('\\'),
                    'b' => resultado.push('\u{0008}'),
                    'f' => resultado.push('\u{000c}'),
                    'n' => resultado.push('\n'),
                    'r' => resultado.push('\r'),
                    't' => resultado.push('\t'),
                    'u' | 'U' => {
                        let quantidade = if escape == 'u' { 4 } else { 8 };
                        let mut hexadecimal = String::with_capacity(quantidade);
                        for _ in 0..quantidade {
                            hexadecimal.push(
                                caracteres
                                    .next()
                                    .ok_or_else(|| "escape Unicode incompleto".to_owned())?,
                            );
                        }
                        let codigo = u32::from_str_radix(&hexadecimal, 16)
                            .map_err(|_| "escape Unicode inválido".to_owned())?;
                        let caractere = char::from_u32(codigo)
                            .ok_or_else(|| "escape Unicode não representa um escalar válido".to_owned())?;
                        resultado.push(caractere);
                    }
                    _ => return Err(format!("escape TOML desconhecido: \\{escape}")),
                }
            }
            Some(caractere) if caractere.is_control() => {
                return Err("caractere de controle não escapado na string".to_owned());
            }
            Some(caractere) => resultado.push(caractere),
            None => return Err("string não foi fechada".to_owned()),
        }
    }
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
    /// [restricoes]
    /// biblioteca = "^1.2.0"
    pub fn analisar(conteudo: &str) -> Result<Self, String> {
        let mut secao = "";
        let mut nome = None;
        let mut versao = None;
        let mut dependencias = BTreeMap::new();
        let mut restricoes_versao = BTreeMap::new();
        for (indice, original) in conteudo.lines().enumerate() {
            let linha = remover_comentario(original).trim();
            if linha.is_empty() {
                continue;
            }
            if linha.starts_with('[') && linha.ends_with(']') {
                secao = &linha[1..linha.len() - 1];
                if secao != "pacote" && secao != "dependencias" && secao != "restricoes" {
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
            let valor = decodificar_string_toml(valor)
                .map_err(|erro| format!("String inválida na linha {}: {erro}.", indice + 1))?;
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
                "restricoes" => {
                    if !nome_valido(chave) {
                        return Err(format!(
                            "Nome de dependência inválido em restrições: {chave}"
                        ));
                    }
                    if !restricao_semver_valida(&valor) {
                        return Err(format!("Restrição SemVer inválida para '{chave}': {valor}"));
                    }
                    if restricoes_versao
                        .insert(chave.to_owned(), valor.to_owned())
                        .is_some()
                    {
                        return Err(format!("Restrição duplicada para dependência: {chave}"));
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
        for nome_restricao in restricoes_versao.keys() {
            if !dependencias.contains_key(nome_restricao) {
                return Err(format!(
                    "Restrição SemVer declarada para dependência inexistente: {nome_restricao}"
                ));
            }
        }
        Ok(Self {
            nome,
            versao,
            dependencias,
            restricoes_versao,
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
    let impressao_raiz = sha256_hex(conteudo_raiz.as_bytes());
    let mut lockfile = format!(
        "nebula-lock-v2\nraiz\t{}\t{}\t{}\n",
        raiz.nome, raiz.versao, impressao_raiz
    );
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
        if let Some(restricao) = manifesto.restricoes_versao.get(nome) {
            if !versao_satisfaz(&dependencia.versao, restricao) {
                return Err(format!(
                    "A dependência '{nome}' resolveu a versão {}, que não satisfaz '{restricao}'.",
                    dependencia.versao
                ));
            }
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

fn versao_core(versao: &str) -> Option<(&str, &str, &str)> {
    let nucleo = versao.split(['-', '+']).next()?;
    let mut partes = nucleo.split('.');
    let major = partes.next()?;
    let minor = partes.next()?;
    let patch = partes.next()?;
    if partes.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

fn comparar_identificador_numerico(a: &str, b: &str) -> std::cmp::Ordering {
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

fn comparar_core(a: &(&str, &str, &str), b: &(&str, &str, &str)) -> std::cmp::Ordering {
    comparar_identificador_numerico(a.0, b.0)
        .then_with(|| comparar_identificador_numerico(a.1, b.1))
        .then_with(|| comparar_identificador_numerico(a.2, b.2))
}

fn incrementar_identificador_numerico(valor: &str) -> String {
    let mut digitos = valor.as_bytes().to_vec();
    let mut indice = digitos.len();
    while indice > 0 {
        indice -= 1;
        if digitos[indice] < b'9' {
            digitos[indice] += 1;
            return String::from_utf8(digitos).expect("dígitos ASCII");
        }
        digitos[indice] = b'0';
    }
    let mut resultado = String::with_capacity(valor.len() + 1);
    resultado.push('1');
    resultado.push_str(&String::from_utf8(digitos).expect("dígitos ASCII"));
    resultado
}

/// Valida restrições simples de SemVer: exata, ^, ~, comparadores e conjunção.
fn restricao_semver_valida(restricao: &str) -> bool {
    !restricao.is_empty()
        && restricao.split("||").all(|grupo| {
            let grupo = grupo.trim();
            if grupo.is_empty() {
                return false;
            }
            if grupo == "*" {
                return true;
            }
            if let Some((inicio, fim)) = grupo.split_once(" - ") {
                return versao_semantica_valida(inicio.trim())
                    && versao_semantica_valida(fim.trim())
                    && versao_core(inicio.trim())
                        .zip(versao_core(fim.trim()))
                        .is_some_and(|(inicio, fim)| {
                            comparar_core(&inicio, &fim) != std::cmp::Ordering::Greater
                        });
            }
            grupo.split(',').all(|parte| {
                let parte = parte.trim();
                let versao = parte
                    .strip_prefix(">=")
                    .or_else(|| parte.strip_prefix("<="))
                    .or_else(|| parte.strip_prefix('>'))
                    .or_else(|| parte.strip_prefix('<'))
                    .or_else(|| parte.strip_prefix('^'))
                    .or_else(|| parte.strip_prefix('~'))
                    .or_else(|| parte.strip_prefix('='))
                    .unwrap_or(parte);
                !versao.is_empty() && versao_semantica_valida(versao)
            })
        })
}

fn versao_satisfaz(versao: &str, restricao: &str) -> bool {
    if !versao_semantica_valida(versao) || !restricao_semver_valida(restricao) {
        return false;
    }
    restricao
        .split("||")
        .any(|grupo| versao_satisfaz_conjuncao(versao, grupo.trim()))
}

fn versao_satisfaz_conjuncao(versao: &str, restricao: &str) -> bool {
    if !versao_semantica_valida(versao) || !restricao_semver_valida(restricao) {
        return false;
    }
    if restricao.trim() == "*" {
        return !versao.contains('-');
    }
    if restricao.trim() == versao {
        return true;
    }
    if let Some((inicio, fim)) = restricao.split_once(" - ") {
        let inicio = inicio.trim();
        let fim = fim.trim();
        if versao.contains('-') {
            let Some(core_candidato) = versao_core(versao) else {
                return false;
            };
            let prerelease_explicito = [inicio, fim].iter().any(|base| {
                base.contains('-')
                    && versao_core(base).is_some_and(|core| {
                        comparar_core(&core, &core_candidato) == std::cmp::Ordering::Equal
                    })
            });
            if !prerelease_explicito {
                return false;
            }
        }
        return comparar_semver(versao, inicio)
            .is_some_and(|ordem| ordem != std::cmp::Ordering::Less)
            && comparar_semver(versao, fim)
                .is_some_and(|ordem| ordem != std::cmp::Ordering::Greater);
    }

    let candidata = versao;
    let clausulas = restricao.split(',').map(str::trim).collect::<Vec<_>>();
    if candidata.contains('-') {
        let Some(core_candidato) = versao_core(candidata) else {
            return false;
        };
        let prerelease_explicitamente_referenciado = clausulas.iter().any(|clausula| {
            let base = clausula
                .strip_prefix(">=")
                .or_else(|| clausula.strip_prefix("<="))
                .or_else(|| clausula.strip_prefix('>'))
                .or_else(|| clausula.strip_prefix('<'))
                .or_else(|| clausula.strip_prefix('^'))
                .or_else(|| clausula.strip_prefix('~'))
                .or_else(|| clausula.strip_prefix('='))
                .unwrap_or(clausula);
            base.contains('-')
                && versao_core(base).is_some_and(|core| {
                    comparar_core(&core, &core_candidato) == std::cmp::Ordering::Equal
                })
        });
        if !prerelease_explicitamente_referenciado {
            return false;
        }
    }

    clausulas.iter().all(|clausula| {
        let comparar = |base: &str| comparar_semver(candidata, base);
        if let Some(base) = clausula.strip_prefix(">=") {
            return comparar(base).is_some_and(|ordem| ordem != std::cmp::Ordering::Less);
        }
        if let Some(base) = clausula.strip_prefix("<=") {
            return comparar(base).is_some_and(|ordem| ordem != std::cmp::Ordering::Greater);
        }
        if let Some(base) = clausula.strip_prefix('>') {
            return comparar(base).is_some_and(|ordem| ordem == std::cmp::Ordering::Greater);
        }
        if let Some(base) = clausula.strip_prefix('<') {
            return comparar(base).is_some_and(|ordem| ordem == std::cmp::Ordering::Less);
        }
        if let Some(base) = clausula.strip_prefix('=') {
            return comparar(base).is_some_and(|ordem| ordem == std::cmp::Ordering::Equal);
        }
        if let Some(base) = clausula.strip_prefix('^') {
            let Some(limite) = versao_core(base) else {
                return false;
            };
            let teto = if limite.0 != "0" {
                (
                    incrementar_identificador_numerico(limite.0),
                    "0".to_owned(),
                    "0".to_owned(),
                )
            } else if limite.1 != "0" {
                (
                    "0".to_owned(),
                    incrementar_identificador_numerico(limite.1),
                    "0".to_owned(),
                )
            } else {
                (
                    "0".to_owned(),
                    "0".to_owned(),
                    incrementar_identificador_numerico(limite.2),
                )
            };
            let teto_refs = (teto.0.as_str(), teto.1.as_str(), teto.2.as_str());
            return comparar(base).is_some_and(|ordem| ordem != std::cmp::Ordering::Less)
                && versao_core(candidata).is_some_and(|core| {
                    comparar_core(&core, &teto_refs) == std::cmp::Ordering::Less
                });
        }
        if let Some(base) = clausula.strip_prefix('~') {
            let Some(limite) = versao_core(base) else {
                return false;
            };
            let teto = (
                limite.0,
                incrementar_identificador_numerico(limite.1),
                "0".to_owned(),
            );
            let teto_refs = (teto.0, teto.1.as_str(), teto.2.as_str());
            return comparar(base).is_some_and(|ordem| ordem != std::cmp::Ordering::Less)
                && versao_core(candidata).is_some_and(|core| {
                    comparar_core(&core, &teto_refs) == std::cmp::Ordering::Less
                });
        }
        comparar(clausula).is_some_and(|ordem| ordem == std::cmp::Ordering::Equal)
    })
}

fn comparar_semver(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering;
    let core_a = versao_core(a)?;
    let core_b = versao_core(b)?;
    let ordem_core = comparar_core(&core_a, &core_b);
    if ordem_core != Ordering::Equal {
        return Some(ordem_core);
    }
    let pre_a = parte_prerelease(a)?;
    let pre_b = parte_prerelease(b)?;
    Some(match (pre_a, pre_b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(a), Some(b)) => comparar_identificadores_prerelease(a, b),
    })
}

fn parte_prerelease(versao: &str) -> Option<Option<&str>> {
    let sem_build = versao.split('+').next()?;
    Some(sem_build.split_once('-').map(|(_, prerelease)| prerelease))
}

fn comparar_identificadores_prerelease(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let mut a = a.split('.');
    let mut b = b.split('.');
    loop {
        match (a.next(), b.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(a), Some(b)) => {
                let a_numerico = a.bytes().all(|byte| byte.is_ascii_digit());
                let b_numerico = b.bytes().all(|byte| byte.is_ascii_digit());
                let ordem = match (a_numerico, b_numerico) {
                    (true, true) => comparar_identificador_numerico(a, b),
                    (true, false) => Ordering::Less,
                    (false, true) => Ordering::Greater,
                    (false, false) => a.cmp(b),
                };
                if ordem != Ordering::Equal {
                    return ordem;
                }
            }
        }
    }
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
    fn manifesto_preserva_hashes_e_decodifica_escapes_toml() {
        let manifesto = Manifesto::analisar(
            "[pacote]\nnome = \"app\"\nversao = \"1.0.0\"\n[dependencias]\nlib = \"../lib#local\\u0020com\\u0020espaço\" # comentário",
        )
        .expect("manifesto com comentário e escapes");
        assert_eq!(
            manifesto.dependencias.get("lib").map(String::as_str),
            Some("../lib#local com espaço")
        );
    }

    #[test]
    fn rejeita_escapes_toml_invalidos() {
        let base = "[pacote]\nnome = \"app\"\nversao = \"1.0.0\"\n[dependencias]\nlib = ";
        for valor in [r#""\q""#, r#""\uD800""#, r#""texto" sobra"#] {
            assert!(Manifesto::analisar(&format!("{base}{valor}")).is_err(), "{valor}");
        }
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
    fn analisa_restricoes_semver_locais() {
        let manifesto = Manifesto::analisar(
            "[pacote]\nnome = \"app\"\nversao = \"1.0.0\"\n[dependencias]\ntexto = \"../texto\"\n[restricoes]\ntexto = \"^1.2.0\"",
        )
        .unwrap();
        assert_eq!(manifesto.restricoes_versao.get("texto").unwrap(), "^1.2.0");
        assert!(restricao_semver_valida(">=1.2.0, <2.0.0"));
        assert!(!restricao_semver_valida("=>1.2.0"));
        assert!(versao_satisfaz("1.9.0", "^1.2.0"));
        assert!(!versao_satisfaz("2.0.0", "^1.2.0"));
        assert!(versao_satisfaz("1.2.9", "~1.2.0"));
        assert!(!versao_satisfaz("1.3.0", "~1.2.0"));
        assert!(versao_satisfaz(
            "18446744073709551615.0.0",
            "^18446744073709551615.0.0"
        ));
        assert!(versao_satisfaz("1.5.0", ">=1.2.0, <2.0.0"));
    }

    #[test]
    fn rejeita_restricao_semver_sem_dependencia() {
        let manifesto = Manifesto::analisar(
            "[pacote]\nnome = \"app\"\nversao = \"1.0.0\"\n[restricoes]\ntexto = \"^1.0.0\"",
        );
        assert!(manifesto.is_err());
    }

    #[test]
    fn compara_identificadores_semver_maiores_que_u64() {
        assert_eq!(
            comparar_semver("184467440737095516160.0.0", "184467440737095516159.9.9"),
            Some(std::cmp::Ordering::Greater)
        );
        assert_eq!(
            comparar_semver("1.0.0-alpha.184467440737095516160", "1.0.0-alpha.9"),
            Some(std::cmp::Ordering::Greater)
        );
        assert!(versao_satisfaz(
            "184467440737095516160.0.0",
            ">=184467440737095516159.0.0"
        ));
        assert!(!versao_satisfaz(
            "184467440737095516160.0.0",
            "^184467440737095516159.0.0"
        ));
    }

    #[test]
    fn aceita_alternativas_e_intervalos_semver() {
        assert!(restricao_semver_valida("^1.0.0 || ^2.0.0"));
        assert!(versao_satisfaz("2.4.0", "^1.0.0 || ^2.0.0"));
        assert!(!versao_satisfaz("3.0.0", "^1.0.0 || ^2.0.0"));
        assert!(restricao_semver_valida("1.2.0 - 2.3.4"));
        assert!(versao_satisfaz("1.9.0", "1.2.0 - 2.3.4"));
        assert!(!versao_satisfaz("2.4.0", "1.2.0 - 2.3.4"));
        assert!(!restricao_semver_valida("^1.0.0 ||"));
    }

    #[test]
    fn compara_pre_lancamentos_semver_conforme_precedencia() {
        let sequencia = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
        ];
        for par in sequencia.windows(2) {
            assert_eq!(
                comparar_semver(par[0], par[1]),
                Some(std::cmp::Ordering::Less)
            );
        }
        assert_eq!(
            comparar_semver("1.0.0+build.1", "1.0.0+build.9"),
            Some(std::cmp::Ordering::Equal)
        );
    }

    #[test]
    fn restricoes_semver_comparam_pre_lancamentos_e_limites() {
        assert!(versao_satisfaz("1.2.3-rc.2", ">=1.2.3-rc.1, <1.2.3"));
        assert!(!versao_satisfaz("1.2.3-rc.2", ">=1.2.0, <2.0.0"));
        assert!(versao_satisfaz("1.2.3", ">=1.2.0, <2.0.0"));
        assert!(!versao_satisfaz("2.0.0", "^1.2.0"));
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
        assert!(primeira.lockfile.contains("nebula-lock-v2"));
        let _ = fs::remove_dir_all(&raiz);
    }
}
