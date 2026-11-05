//! Padrões de correspondência usados por escolha.

use super::instrucao::NoAst;

/// Caso de correspondência em uma instrução escolha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CasoEscolha {
    pub enumeracao: Option<String>,
    pub variante: Option<String>,
    pub vinculacoes: Vec<String>,
    pub expressao: Option<super::expressao::Expressao>,
    pub corpo: Vec<NoAst>,
}

/// Forma estruturada de um padrão de correspondência.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PadraoAst {
    Curinga,
    Vinculacao(String),
    Variante {
        caminho: Vec<String>,
        vinculacoes: Vec<PadraoAst>,
    },
    Literal(String),
    /// Expressão de padrão preservada integralmente quando não é literal simples.
    Expressao(super::expressao::Expressao),
}

impl CasoEscolha {
    /// Converte os campos legados do caso em um padrão estruturado.
    pub fn padrao(&self) -> PadraoAst {
        if let Some(expressao) = &self.expressao {
            return match expressao {
                super::expressao::Expressao::Literal(valor) => PadraoAst::Literal(valor.clone()),
                super::expressao::Expressao::Nome(nome) => PadraoAst::Vinculacao(nome.clone()),
                _ => PadraoAst::Expressao(expressao.clone()),
            };
        }
        match (&self.enumeracao, &self.variante) {
            (None, None) => PadraoAst::Curinga,
            (None, Some(nome)) if nome == "_" => PadraoAst::Curinga,
            (None, Some(nome)) => PadraoAst::Vinculacao(nome.clone()),
            (enumeracao, variante) => {
                let mut caminho = Vec::new();
                if let Some(nome) = enumeracao {
                    caminho.push(nome.clone());
                }
                if let Some(nome) = variante {
                    caminho.push(nome.clone());
                }
                PadraoAst::Variante {
                    caminho,
                    vinculacoes: self
                        .vinculacoes
                        .iter()
                        .cloned()
                        .map(PadraoAst::Vinculacao)
                        .collect(),
                }
            }
        }
    }
}
