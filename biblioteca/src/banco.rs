//! Armazenamento local de documentos JSON com transações otimistas.

use crate::dados::{ErroJson, ValorJson, analisar_json, serializar_json};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{RwLock, RwLockWriteGuard};

static PROXIMO_TEMPORARIO: AtomicU64 = AtomicU64::new(1);

/// Erros possíveis ao abrir ou atualizar um banco local.
#[derive(Debug)]
pub enum ErroBanco {
    /// Falha de leitura, escrita ou substituição de arquivo.
    Io(std::io::Error),
    /// Documento persistido não contém JSON válido.
    Json(ErroJson),
    /// Outra transação confirmou alterações desde o início desta transação.
    ConflitoTransacao,
    /// O arquivo do banco é um link simbólico e foi recusado.
    CaminhoSimbolico,
    /// O lock interno foi envenenado por panic.
    EstadoIndisponivel,
    /// A versão interna do banco excedeu o limite.
    VersaoExcedida,
}

impl fmt::Display for ErroBanco {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(erro) => write!(f, "erro de I/O: {erro}"),
            Self::Json(erro) => write!(f, "erro no documento JSON: {erro}"),
            Self::ConflitoTransacao => write!(f, "conflito: o banco foi alterado por outra transação"),
            Self::CaminhoSimbolico => write!(f, "o banco não pode ser aberto por link simbólico"),
            Self::EstadoIndisponivel => write!(f, "estado interno do banco indisponível"),
            Self::VersaoExcedida => write!(f, "versão interna do banco excedeu o limite"),
        }
    }
}

impl std::error::Error for ErroBanco {}

impl From<std::io::Error> for ErroBanco {
    fn from(erro: std::io::Error) -> Self {
        Self::Io(erro)
    }
}

impl From<ErroJson> for ErroBanco {
    fn from(erro: ErroJson) -> Self {
        Self::Json(erro)
    }
}

#[derive(Clone)]
struct Estado {
    versao: u64,
    documentos: BTreeMap<String, ValorJson>,
}

/// Banco embutido baseado em documentos JSON, sincronizado entre threads do processo.
pub struct BancoJson {
    caminho: PathBuf,
    estado: RwLock<Estado>,
}

/// Transação com snapshot; commit falha se outra transação já confirmou alterações.
pub struct Transacao<'a> {
    banco: &'a BancoJson,
    versao_base: u64,
    documentos: BTreeMap<String, ValorJson>,
}

impl BancoJson {
    /// Abre um arquivo JSON existente ou cria um banco vazio no caminho informado.
    pub fn abrir(caminho: impl AsRef<Path>) -> Result<Self, ErroBanco> {
        let caminho = caminho.as_ref().to_path_buf();
        match fs::symlink_metadata(&caminho) {
            Ok(meta) if meta.file_type().is_symlink() => return Err(ErroBanco::CaminhoSimbolico),
            Ok(_) => {}
            Err(erro) if erro.kind() == std::io::ErrorKind::NotFound => {
                persistir_atomico(&caminho, b"{}")?;
            }
            Err(erro) => return Err(ErroBanco::Io(erro)),
        }
        let bytes = fs::read(&caminho)?;
        let texto = String::from_utf8(bytes).map_err(|erro| {
            ErroBanco::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, erro))
        })?;
        let valor = analisar_json(&texto)?;
        let ValorJson::Objeto(documentos) = valor else {
            return Err(ErroBanco::Json(ErroJson {
                deslocamento: 0,
                mensagem: "A raiz do banco precisa ser um objeto JSON.".to_owned(),
            }));
        };
        Ok(Self { caminho, estado: RwLock::new(Estado { versao: 0, documentos }) })
    }

    /// Lê um documento por chave sem iniciar uma transação.
    pub fn obter(&self, chave: &str) -> Result<Option<ValorJson>, ErroBanco> {
        let estado = self.estado.read().map_err(|_| ErroBanco::EstadoIndisponivel)?;
        Ok(estado.documentos.get(chave).cloned())
    }

    /// Inicia uma transação com cópia consistente do estado atual.
    pub fn iniciar_transacao(&self) -> Result<Transacao<'_>, ErroBanco> {
        let estado = self.estado.read().map_err(|_| ErroBanco::EstadoIndisponivel)?;
        Ok(Transacao {
            banco: self,
            versao_base: estado.versao,
            documentos: estado.documentos.clone(),
        })
    }

    /// Retorna o número de documentos armazenados.
    pub fn quantidade(&self) -> Result<usize, ErroBanco> {
        let estado = self.estado.read().map_err(|_| ErroBanco::EstadoIndisponivel)?;
        Ok(estado.documentos.len())
    }

    /// Retorna o caminho persistente usado pelo banco.
    #[must_use]
    pub fn caminho(&self) -> &Path {
        &self.caminho
    }
}

impl Transacao<'_> {
    /// Define ou substitui um documento no snapshot da transação.
    pub fn definir(&mut self, chave: impl Into<String>, valor: ValorJson) -> Result<(), ErroBanco> {
        let chave = chave.into();
        if chave.is_empty() || chave.contains('\0') {
            return Err(ErroBanco::Json(ErroJson {
                deslocamento: 0,
                mensagem: "A chave do documento não pode ser vazia nem conter NUL.".to_owned(),
            }));
        }
        self.documentos.insert(chave, valor);
        Ok(())
    }

    /// Remove um documento do snapshot e retorna seu valor anterior.
    pub fn remover(&mut self, chave: &str) -> Option<ValorJson> {
        self.documentos.remove(chave)
    }

    /// Lê um documento dentro do snapshot ainda não confirmado.
    #[must_use]
    pub fn obter(&self, chave: &str) -> Option<&ValorJson> {
        self.documentos.get(chave)
    }

    /// Confirma alterações de forma atômica no arquivo e no estado em memória.
    pub fn confirmar(self) -> Result<(), ErroBanco> {
        let mut estado = self.banco.estado.write().map_err(|_| ErroBanco::EstadoIndisponivel)?;
        confirmar_snapshot(&mut estado, self.versao_base, self.documentos, &self.banco.caminho)
    }
}

fn confirmar_snapshot(
    estado: &mut Estado,
    versao_base: u64,
    documentos: BTreeMap<String, ValorJson>,
    caminho: &Path,
) -> Result<(), ErroBanco> {
    if estado.versao != versao_base {
        return Err(ErroBanco::ConflitoTransacao);
    }
    let nova_versao = estado.versao.checked_add(1).ok_or(ErroBanco::VersaoExcedida)?;
    let json = serializar_json(&ValorJson::Objeto(documentos.clone()))?;
    persistir_atomico(caminho, json.as_bytes())?;
    estado.documentos = documentos;
    estado.versao = nova_versao;
    Ok(())
}

fn persistir_atomico(caminho: &Path, bytes: &[u8]) -> Result<(), ErroBanco> {
    let pai = caminho.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let nome = caminho.file_name().ok_or_else(|| {
        ErroBanco::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, "caminho sem nome de arquivo"))
    })?;
    let id = PROXIMO_TEMPORARIO.fetch_add(1, Ordering::Relaxed);
    let temporario = pai.join(format!(".{}.{}.{}.tmp", nome.to_string_lossy(), std::process::id(), id));
    let resultado = (|| {
        let mut arquivo = OpenOptions::new().write(true).create_new(true).open(&temporario)?;
        arquivo.write_all(bytes)?;
        arquivo.sync_all()?;
        drop(arquivo);
        if fs::symlink_metadata(caminho).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(ErroBanco::CaminhoSimbolico);
        }
        fs::rename(&temporario, caminho)?;
        Ok(())
    })();
    if resultado.is_err() {
        let _ = fs::remove_file(&temporario);
    }
    resultado
}

#[cfg(test)]
mod testes {
    use super::*;

    fn caminho_temporario() -> PathBuf {
        std::env::temp_dir().join(format!(
            "nebula-banco-{}-{}.json",
            std::process::id(),
            PROXIMO_TEMPORARIO.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn persiste_documentos_e_reabre_banco() {
        let caminho = caminho_temporario();
        let banco = BancoJson::abrir(&caminho).unwrap();
        let mut tx = banco.iniciar_transacao().unwrap();
        tx.definir("cliente", ValorJson::Texto("Ana".to_owned())).unwrap();
        tx.confirmar().unwrap();
        drop(banco);
        let reaberto = BancoJson::abrir(&caminho).unwrap();
        assert_eq!(reaberto.obter("cliente").unwrap(), Some(ValorJson::Texto("Ana".to_owned())));
        let _ = fs::remove_file(caminho);
    }

    #[test]
    fn detecta_conflito_entre_transacoes() {
        let caminho = caminho_temporario();
        let banco = BancoJson::abrir(&caminho).unwrap();
        let mut primeira = banco.iniciar_transacao().unwrap();
        let mut segunda = banco.iniciar_transacao().unwrap();
        primeira.definir("a", ValorJson::Logico(true)).unwrap();
        segunda.definir("b", ValorJson::Logico(false)).unwrap();
        primeira.confirmar().unwrap();
        assert!(matches!(segunda.confirmar(), Err(ErroBanco::ConflitoTransacao)));
        let _ = fs::remove_file(caminho);
    }

    #[test]
    fn recusa_chave_vazia() {
        let caminho = caminho_temporario();
        let banco = BancoJson::abrir(&caminho).unwrap();
        let mut tx = banco.iniciar_transacao().unwrap();
        assert!(tx.definir("", ValorJson::Nulo).is_err());
        let _ = fs::remove_file(caminho);
    }
}
