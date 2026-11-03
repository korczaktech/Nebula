//! Cache incremental e verificável de artefatos do compilador Nebula.

use nebula_seguranca::sha256_hex;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MAGIC: &[u8; 8] = b"NEBCACH1";
const TAMANHO_CABECALHO: usize = 8 + 64 + 64 + 8;
const MAX_ARTEFATO: u64 = 256 * 1024 * 1024;
static PROXIMO_TEMPORARIO: AtomicU64 = AtomicU64::new(1);

/// Cache local de artefatos compilados, isolado em `.nebula/cache`.
pub struct CacheCompilacao {
    diretorio: PathBuf,
}

/// Estatísticas de limpeza do cache.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResultadoLimpeza {
    /// Arquivos regulares removidos.
    pub removidos: usize,
    /// Quantidade de bytes removidos.
    pub bytes_liberados: u64,
}

impl CacheCompilacao {
    /// Abre ou prepara o cache para um diretório de projeto existente.
    ///
    /// Recusa componentes `.nebula` ou `cache` que sejam links simbólicos.
    pub fn abrir(projeto: &Path) -> Result<Self, String> {
        let projeto = fs::canonicalize(projeto).map_err(|erro| {
            format!(
                "não foi possível resolver o projeto '{}': {erro}",
                projeto.display()
            )
        })?;
        if !fs::metadata(&projeto)
            .map_err(|erro| format!("não foi possível inspecionar o projeto: {erro}"))?
            .is_dir()
        {
            return Err("o projeto do cache precisa ser um diretório".to_owned());
        }

        let nebula = projeto.join(".nebula");
        criar_diretorio_seguro(&nebula)?;
        let diretorio = nebula.join("cache");
        criar_diretorio_seguro(&diretorio)?;
        Ok(Self { diretorio })
    }

    /// Calcula uma chave SHA-256 sem ambiguidades a partir de partes ordenadas.
    ///
    /// Cada parte recebe prefixo com seu comprimento em big-endian.
    #[must_use]
    pub fn calcular_chave(componentes: &[&[u8]]) -> String {
        let mut entrada = Vec::new();
        entrada.extend_from_slice(b"NebulaBuildCache\0v1");
        for componente in componentes {
            entrada.extend_from_slice(&(componente.len() as u64).to_be_bytes());
            entrada.extend_from_slice(componente);
        }
        sha256_hex(&entrada)
    }

    /// Calcula SHA-256 de um arquivo regular, sem seguir links simbólicos.
    ///
    /// Usado para invalidar artefatos quando o próprio compilador muda.
    pub fn fingerprint_arquivo(caminho: &Path) -> Result<String, String> {
        let metadados = fs::symlink_metadata(caminho)
            .map_err(|erro| format!("não foi possível inspecionar arquivo de fingerprint: {erro}"))?;
        if metadados.file_type().is_symlink() || !metadados.is_file() {
            return Err("fingerprint exige um arquivo regular, não um link simbólico".to_owned());
        }
        if metadados.len() > 512 * 1024 * 1024 {
            return Err("arquivo de fingerprint excede o limite de 512 MiB".to_owned());
        }
        let bytes = fs::read(caminho)
            .map_err(|erro| format!("não foi possível ler arquivo de fingerprint: {erro}"))?;
        Ok(sha256_hex(&bytes))
    }

    /// Retorna um artefato válido; conteúdo truncado ou adulterado é descartado.
    pub fn obter(&self, chave: &str) -> Result<Option<Vec<u8>>, String> {
        validar_chave(chave)?;
        let caminho = self.diretorio.join(format!("{chave}.nbc"));
        let metadados = match fs::symlink_metadata(&caminho) {
            Ok(metadados) => metadados,
            Err(erro) if erro.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(erro) => {
                return Err(format!(
                    "não foi possível inspecionar entrada do cache: {erro}"
                ));
            }
        };
        if metadados.file_type().is_symlink() || !metadados.is_file() {
            return Err("entrada do cache não é um arquivo regular".to_owned());
        }
        if metadados.len() > MAX_ARTEFATO + TAMANHO_CABECALHO as u64
            || metadados.len() < TAMANHO_CABECALHO as u64
        {
            descartar_arquivo_regular(&caminho)?;
            return Ok(None);
        }

        let arquivo = fs::File::open(&caminho)
            .map_err(|erro| format!("não foi possível ler entrada do cache: {erro}"))?;
        let mut bytes = Vec::with_capacity(metadados.len() as usize);
        arquivo
            .take(MAX_ARTEFATO + TAMANHO_CABECALHO as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|erro| format!("falha ao ler entrada do cache: {erro}"))?;

        let cabecalho_valido = bytes.get(..8) == Some(MAGIC.as_slice())
            && bytes.get(8..72) == Some(chave.as_bytes())
            && bytes.len() >= TAMANHO_CABECALHO;
        if !cabecalho_valido {
            descartar_arquivo_regular(&caminho)?;
            return Ok(None);
        }
        let resumo = &bytes[72..136];
        let tamanho = u64::from_be_bytes(
            bytes[136..144]
                .try_into()
                .map_err(|_| "cabeçalho de cache incompleto")?,
        );
        let conteudo = &bytes[TAMANHO_CABECALHO..];
        if tamanho != conteudo.len() as u64 || tamanho > MAX_ARTEFATO {
            descartar_arquivo_regular(&caminho)?;
            return Ok(None);
        }
        let resumo_atual = sha256_hex(conteudo);
        if resumo != resumo_atual.as_bytes() {
            descartar_arquivo_regular(&caminho)?;
            return Ok(None);
        }
        Ok(Some(conteudo.to_vec()))
    }

    /// Armazena um artefato por substituição atômica e checksum SHA-256.
    pub fn armazenar(&self, chave: &str, conteudo: &[u8]) -> Result<(), String> {
        validar_chave(chave)?;
        if conteudo.len() as u64 > MAX_ARTEFATO {
            return Err("artefato excede o limite de 256 MiB do cache".to_owned());
        }
        let destino = self.diretorio.join(format!("{chave}.nbc"));
        if let Ok(meta) = fs::symlink_metadata(&destino) {
            if meta.file_type().is_symlink() || !meta.is_file() {
                return Err("destino do cache não é um arquivo regular".to_owned());
            }
        }

        let resumo = sha256_hex(conteudo);
        let temporario_id = PROXIMO_TEMPORARIO.fetch_add(1, Ordering::Relaxed);
        let temporario = self.diretorio.join(format!(
            ".{chave}.{}.{}.tmp",
            std::process::id(),
            temporario_id
        ));
        let resultado = (|| {
            let mut arquivo = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporario)
                .map_err(|erro| {
                    format!("não foi possível criar arquivo temporário do cache: {erro}")
                })?;
            arquivo.write_all(MAGIC).map_err(|erro| erro.to_string())?;
            arquivo
                .write_all(chave.as_bytes())
                .map_err(|erro| erro.to_string())?;
            arquivo
                .write_all(resumo.as_bytes())
                .map_err(|erro| erro.to_string())?;
            arquivo
                .write_all(&(conteudo.len() as u64).to_be_bytes())
                .map_err(|erro| erro.to_string())?;
            arquivo
                .write_all(conteudo)
                .map_err(|erro| erro.to_string())?;
            arquivo.sync_all().map_err(|erro| erro.to_string())?;
            drop(arquivo);

            if fs::symlink_metadata(&destino)
                .is_ok_and(|meta| meta.file_type().is_symlink() || !meta.is_file())
            {
                return Err("destino do cache foi substituído por objeto não regular".to_owned());
            }
            fs::rename(&temporario, &destino)
                .map_err(|erro| format!("não foi possível publicar entrada do cache: {erro}"))?;
            Ok(())
        })();
        if resultado.is_err() {
            let _ = fs::remove_file(&temporario);
        }
        resultado
    }

    /// Remove somente arquivos regulares diretamente dentro de `.nebula/cache`.
    pub fn limpar(&self) -> Result<ResultadoLimpeza, String> {
        let mut resultado = ResultadoLimpeza::default();
        let entradas = fs::read_dir(&self.diretorio)
            .map_err(|erro| format!("não foi possível listar o cache: {erro}"))?;
        for entrada in entradas {
            let entrada = entrada.map_err(|erro| format!("falha ao listar o cache: {erro}"))?;
            let caminho = entrada.path();
            let meta = fs::symlink_metadata(&caminho).map_err(|erro| {
                format!(
                    "não foi possível inspecionar '{}': {erro}",
                    caminho.display()
                )
            })?;
            if meta.file_type().is_symlink() {
                fs::remove_file(&caminho)
                    .map_err(|erro| format!("não foi possível remover link do cache: {erro}"))?;
            } else if meta.is_file() {
                fs::remove_file(&caminho)
                    .map_err(|erro| format!("não foi possível remover entrada do cache: {erro}"))?;
                resultado.removidos += 1;
                resultado.bytes_liberados = resultado.bytes_liberados.saturating_add(meta.len());
            } else {
                return Err(format!(
                    "o cache contém uma subpasta inesperada '{}'; ela não foi percorrida",
                    caminho.display()
                ));
            }
        }
        Ok(resultado)
    }

    /// Retorna o diretório físico onde as entradas são armazenadas.
    #[must_use]
    pub fn diretorio(&self) -> &Path {
        &self.diretorio
    }
}

fn criar_diretorio_seguro(caminho: &Path) -> Result<(), String> {
    match fs::symlink_metadata(caminho) {
        Ok(meta) if meta.file_type().is_symlink() => Err(format!(
            "o caminho '{}' não pode ser link simbólico",
            caminho.display()
        )),
        Ok(meta) if meta.is_dir() => Ok(()),
        Ok(_) => Err(format!("'{}' existe e não é diretório", caminho.display())),
        Err(erro) if erro.kind() == std::io::ErrorKind::NotFound => match fs::create_dir(caminho) {
            Ok(()) => Ok(()),
            Err(erro) if erro.kind() == std::io::ErrorKind::AlreadyExists => {
                let meta = fs::symlink_metadata(caminho).map_err(|falha| {
                    format!(
                        "não foi possível inspecionar '{}': {falha}",
                        caminho.display()
                    )
                })?;
                if meta.is_dir() && !meta.file_type().is_symlink() {
                    Ok(())
                } else {
                    Err(format!(
                        "'{}' foi criado como objeto não permitido",
                        caminho.display()
                    ))
                }
            }
            Err(erro) => Err(format!(
                "não foi possível criar '{}': {erro}",
                caminho.display()
            )),
        },
        Err(erro) => Err(format!(
            "não foi possível inspecionar '{}': {erro}",
            caminho.display()
        )),
    }
}

fn validar_chave(chave: &str) -> Result<(), String> {
    if chave.len() != 64
        || !chave
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("chave de cache inválida; esperado SHA-256 hexadecimal minúsculo".to_owned());
    }
    Ok(())
}

fn descartar_arquivo_regular(caminho: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(caminho).map_err(|erro| {
        format!("não foi possível inspecionar entrada inválida do cache: {erro}")
    })?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err("entrada inválida não é um arquivo regular; remoção recusada".to_owned());
    }
    fs::remove_file(caminho)
        .map_err(|erro| format!("não foi possível descartar entrada inválida do cache: {erro}"))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn projeto_temporario(rotulo: &str) -> PathBuf {
        let caminho = std::env::temp_dir().join(format!(
            "nebula-cache-{rotulo}-{}-{}",
            std::process::id(),
            PROXIMO_TEMPORARIO.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&caminho).unwrap();
        caminho
    }

    #[test]
    fn chaves_sao_deterministicas_e_separadas_por_comprimento() {
        let primeira = CacheCompilacao::calcular_chave(&[b"ab", b"c"]);
        let repetida = CacheCompilacao::calcular_chave(&[b"ab", b"c"]);
        let diferente = CacheCompilacao::calcular_chave(&[b"a", b"bc"]);
        assert_eq!(primeira, repetida);
        assert_ne!(primeira, diferente);
        assert_eq!(primeira.len(), 64);
    }

    #[test]
    fn armazena_e_recupera_artefato_com_checksum() {
        let projeto = projeto_temporario("roundtrip");
        let cache = CacheCompilacao::abrir(&projeto).unwrap();
        let chave = CacheCompilacao::calcular_chave(&[b"fonte", b"O1"]);
        cache.armazenar(&chave, b"ELF64 teste").unwrap();
        assert_eq!(cache.obter(&chave).unwrap(), Some(b"ELF64 teste".to_vec()));
        let limpeza = cache.limpar().unwrap();
        assert_eq!(limpeza.removidos, 1);
        assert_eq!(cache.obter(&chave).unwrap(), None);
        let _ = fs::remove_dir_all(projeto);
    }

    #[test]
    fn descarta_conteudo_corrompido_sem_retornar_cache() {
        let projeto = projeto_temporario("corrompido");
        let cache = CacheCompilacao::abrir(&projeto).unwrap();
        let chave = CacheCompilacao::calcular_chave(&[b"fonte"]);
        cache.armazenar(&chave, b"original").unwrap();
        let caminho = cache.diretorio.join(format!("{chave}.nbc"));
        let mut arquivo = OpenOptions::new().write(true).open(&caminho).unwrap();
        arquivo.write_all(b"adulterado").unwrap();
        arquivo.sync_all().unwrap();
        assert_eq!(cache.obter(&chave).unwrap(), None);
        assert!(!caminho.exists());
        let _ = fs::remove_dir_all(projeto);
    }

    #[test]
    fn recusa_diretorio_de_cache_simbolico() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let projeto = projeto_temporario("symlink");
            let externo = projeto.join("externo");
            fs::create_dir(&externo).unwrap();
            symlink(&externo, projeto.join(".nebula")).unwrap();
            assert!(CacheCompilacao::abrir(&projeto).is_err());
            let _ = fs::remove_dir_all(projeto);
        }
    }

    #[test]
    fn fingerprint_identifica_conteudo_de_arquivo_regular() {
        let projeto = projeto_temporario("fingerprint");
        let arquivo = projeto.join("compilador.bin");
        fs::write(&arquivo, b"compilador A").unwrap();
        let primeira = CacheCompilacao::fingerprint_arquivo(&arquivo).unwrap();
        fs::write(&arquivo, b"compilador B").unwrap();
        let segunda = CacheCompilacao::fingerprint_arquivo(&arquivo).unwrap();
        assert_ne!(primeira, segunda);
        let _ = fs::remove_dir_all(projeto);
    }

    #[test]
    fn recusa_chaves_malformadas() {
        let projeto = projeto_temporario("chave");
        let cache = CacheCompilacao::abrir(&projeto).unwrap();
        assert!(cache.obter("../fora").is_err());
        assert!(cache.armazenar("not-a-digest", b"x").is_err());
        let _ = fs::remove_dir_all(projeto);
    }
}
