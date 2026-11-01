//! Estruturas mínimas de diagnóstico compartilhadas pela Nebula.

#![forbid(unsafe_code)]

use nebula_fonte::IntervaloFonte;

/// Severidade de um diagnóstico.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severidade {
    /// Erro que impede a operação atual.
    Erro,
    /// Situação suspeita que não impede necessariamente a operação.
    Aviso,
    /// Informação complementar.
    Informacao,
}

/// Diagnóstico independente de interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostico {
    /// Código estável, por exemplo N0001.
    pub codigo: &'static str,
    /// Severidade.
    pub severidade: Severidade,
    /// Mensagem em português.
    pub mensagem: String,
    /// Intervalo de origem, quando conhecido.
    pub intervalo: Option<IntervaloFonte>,
}

impl Diagnostico {
    /// Cria um diagnóstico associado a um trecho de origem.
    pub fn novo(
        codigo: &'static str,
        severidade: Severidade,
        mensagem: impl Into<String>,
        intervalo: Option<IntervaloFonte>,
    ) -> Self {
        Self {
            codigo,
            severidade,
            mensagem: mensagem.into(),
            intervalo,
        }
    }
}

#[cfg(test)]
mod testes {
    use super::{Diagnostico, Severidade};

    #[test]
    fn preserva_codigo_e_mensagem() {
        let diagnostico =
            Diagnostico::novo("N0001", Severidade::Erro, "Caractere inesperado.", None);
        assert_eq!(diagnostico.codigo, "N0001");
        assert_eq!(diagnostico.mensagem, "Caractere inesperado.");
        assert_eq!(diagnostico.severidade, Severidade::Erro);
    }
}
