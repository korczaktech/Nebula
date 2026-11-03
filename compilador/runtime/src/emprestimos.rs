//! Verificação dinâmica de aliasing compartilhado e exclusivo.

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static PROXIMO_EMPRESTIMO: AtomicU64 = AtomicU64::new(1);

/// Conflitos e falhas no controle dinâmico de empréstimos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErroEmprestimo {
    /// O recurso já possui um empréstimo exclusivo ativo.
    ExclusivoAtivo(u64),
    /// O recurso possui empréstimos compartilhados ativos.
    CompartilhadoAtivo(u64),
    /// O identificador de empréstimo foi repetido ou excedeu o limite.
    IdentificadorExcedido,
    /// O estado interno foi envenenado por panic em outra thread.
    EstadoIndisponivel,
}

impl fmt::Display for ErroEmprestimo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExclusivoAtivo(chave) => {
                write!(f, "o recurso {chave} possui empréstimo exclusivo ativo")
            }
            Self::CompartilhadoAtivo(chave) => {
                write!(
                    f,
                    "o recurso {chave} possui empréstimos compartilhados ativos"
                )
            }
            Self::IdentificadorExcedido => {
                write!(f, "não foi possível emitir identificador de empréstimo")
            }
            Self::EstadoIndisponivel => write!(f, "estado de empréstimos indisponível"),
        }
    }
}

impl std::error::Error for ErroEmprestimo {}

#[derive(Debug, Default)]
struct EstadoRecurso {
    compartilhados: HashMap<u64, ()>,
    exclusivo: Option<u64>,
}

#[derive(Debug, Default)]
struct EstadoTabela {
    recursos: HashMap<u64, EstadoRecurso>,
}

/// Tabela de empréstimos compartilháveis entre threads do mesmo processo.
///
/// As chaves identificam recursos gerenciados pelo chamador. A tabela não
/// acessa os dados do recurso; ela apenas impõe a regra de exclusão de aliasing.
#[derive(Clone, Debug, Default)]
pub struct TabelaEmprestimos {
    estado: Arc<Mutex<EstadoTabela>>,
}

/// Guard RAII que libera automaticamente o empréstimo quando descartado.
#[derive(Debug)]
pub struct GuardaEmprestimo {
    estado: Arc<Mutex<EstadoTabela>>,
    recurso: u64,
    identificador: u64,
    exclusivo: bool,
}

impl TabelaEmprestimos {
    /// Cria uma tabela de empréstimos vazia.
    #[must_use]
    pub fn nova() -> Self {
        Self::default()
    }

    /// Adquire empréstimo compartilhado; múltiplos leitores podem coexistir.
    pub fn compartilhar(&self, recurso: u64) -> Result<GuardaEmprestimo, ErroEmprestimo> {
        let mut estado = self
            .estado
            .lock()
            .map_err(|_| ErroEmprestimo::EstadoIndisponivel)?;
        let recurso_estado = estado.recursos.entry(recurso).or_default();
        if recurso_estado.exclusivo.is_some() {
            return Err(ErroEmprestimo::ExclusivoAtivo(recurso));
        }
        let identificador = PROXIMO_EMPRESTIMO
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |atual| {
                atual.checked_add(1)
            })
            .map_err(|_| ErroEmprestimo::IdentificadorExcedido)?;
        recurso_estado.compartilhados.insert(identificador, ());
        Ok(GuardaEmprestimo {
            estado: Arc::clone(&self.estado),
            recurso,
            identificador,
            exclusivo: false,
        })
    }

    /// Adquire empréstimo exclusivo somente quando não há qualquer leitor/escritor.
    pub fn exclusivo(&self, recurso: u64) -> Result<GuardaEmprestimo, ErroEmprestimo> {
        let mut estado = self
            .estado
            .lock()
            .map_err(|_| ErroEmprestimo::EstadoIndisponivel)?;
        let recurso_estado = estado.recursos.entry(recurso).or_default();
        if recurso_estado.exclusivo.is_some() {
            return Err(ErroEmprestimo::ExclusivoAtivo(recurso));
        }
        if !recurso_estado.compartilhados.is_empty() {
            return Err(ErroEmprestimo::CompartilhadoAtivo(recurso));
        }
        let identificador = PROXIMO_EMPRESTIMO
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |atual| {
                atual.checked_add(1)
            })
            .map_err(|_| ErroEmprestimo::IdentificadorExcedido)?;
        recurso_estado.exclusivo = Some(identificador);
        Ok(GuardaEmprestimo {
            estado: Arc::clone(&self.estado),
            recurso,
            identificador,
            exclusivo: true,
        })
    }

    /// Retorna o total de guards ativos para a chave informada.
    pub fn quantidade_ativos(&self, recurso: u64) -> Result<usize, ErroEmprestimo> {
        let estado = self
            .estado
            .lock()
            .map_err(|_| ErroEmprestimo::EstadoIndisponivel)?;
        Ok(estado.recursos.get(&recurso).map_or(0, |recurso| {
            recurso.compartilhados.len() + usize::from(recurso.exclusivo.is_some())
        }))
    }
}

impl GuardaEmprestimo {
    /// Informa se este guard é exclusivo.
    #[must_use]
    pub fn eh_exclusivo(&self) -> bool {
        self.exclusivo
    }

    /// Retorna a chave do recurso protegido.
    #[must_use]
    pub fn recurso(&self) -> u64 {
        self.recurso
    }
}

impl Drop for GuardaEmprestimo {
    fn drop(&mut self) {
        let Ok(mut estado) = self.estado.lock() else {
            return;
        };
        let Some(recurso) = estado.recursos.get_mut(&self.recurso) else {
            return;
        };
        if self.exclusivo {
            if recurso.exclusivo == Some(self.identificador) {
                recurso.exclusivo = None;
            }
        } else {
            recurso.compartilhados.remove(&self.identificador);
        }
        if recurso.exclusivo.is_none() && recurso.compartilhados.is_empty() {
            estado.recursos.remove(&self.recurso);
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn permite_multiplos_leitores_e_recusa_escritor() {
        let tabela = TabelaEmprestimos::nova();
        let primeiro = tabela.compartilhar(7).unwrap();
        let segundo = tabela.compartilhar(7).unwrap();
        assert_eq!(tabela.quantidade_ativos(7).unwrap(), 2);
        assert_eq!(
            tabela.exclusivo(7).unwrap_err(),
            ErroEmprestimo::CompartilhadoAtivo(7)
        );
        drop(primeiro);
        drop(segundo);
        assert_eq!(tabela.quantidade_ativos(7).unwrap(), 0);
    }

    #[test]
    fn escritor_exclusivo_bloqueia_leitor_e_outro_escritor() {
        let tabela = TabelaEmprestimos::nova();
        let escritor = tabela.exclusivo(11).unwrap();
        assert!(escritor.eh_exclusivo());
        assert_eq!(
            tabela.compartilhar(11).unwrap_err(),
            ErroEmprestimo::ExclusivoAtivo(11)
        );
        assert_eq!(
            tabela.exclusivo(11).unwrap_err(),
            ErroEmprestimo::ExclusivoAtivo(11)
        );
        drop(escritor);
        assert!(tabela.compartilhar(11).is_ok());
    }

    #[test]
    fn ids_de_recursos_distintos_nao_conflitam() {
        let tabela = TabelaEmprestimos::nova();
        let escritor = tabela.exclusivo(1).unwrap();
        let leitor = tabela.compartilhar(2).unwrap();
        assert_eq!(escritor.recurso(), 1);
        assert_eq!(leitor.recurso(), 2);
        assert_eq!(tabela.quantidade_ativos(1).unwrap(), 1);
        assert_eq!(tabela.quantidade_ativos(2).unwrap(), 1);
    }

    #[test]
    fn guard_libera_recurso_mesmo_ao_descartar() {
        let tabela = TabelaEmprestimos::nova();
        {
            let _guard = tabela.exclusivo(19).unwrap();
            assert_eq!(tabela.quantidade_ativos(19).unwrap(), 1);
        }
        assert_eq!(tabela.quantidade_ativos(19).unwrap(), 0);
    }
}
