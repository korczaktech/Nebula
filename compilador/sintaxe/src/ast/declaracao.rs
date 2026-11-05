//! Modelos tipados para declarações da linguagem Nebula.

use super::tipos::TipoAst;
use super::expressao::Expressao;

/// Visibilidade declarada para uma função.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibilidade {
    Publico,
    Privado,
}

/// Parâmetro declarado por uma função.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParametroAst {
    pub nome: String,
    pub tipo: TipoAst,
}

/// Campo declarado por uma estrutura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampoAst {
    pub nome: String,
    pub tipo: TipoAst,
}

/// Variante declarada por uma enumeração.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarianteAst {
    pub nome: String,
    pub campos: Vec<TipoAst>,
}

/// Declaração de alias de tipo em sua forma estruturada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasTipoAst {
    pub nome: String,
    pub destino: TipoAst,
}

/// Atributo anexado a uma declaração.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtributoAst {
    pub nome: String,
    pub argumentos: Vec<Expressao>,
}
