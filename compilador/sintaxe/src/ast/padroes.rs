//! Padrões de correspondência usados por escolha.
use super::instrucao::NoAst;
/// Caso de correspondência em uma instrução escolha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CasoEscolha { pub enumeracao: Option<String>, pub variante: Option<String>, pub vinculacoes: Vec<String>, pub corpo: Vec<NoAst> }
/// Forma estruturada de um padrão de correspondência.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PadraoAst {
    Curinga,
    Vinculacao(String),
    Variante { caminho: Vec<String>, vinculacoes: Vec<PadraoAst> },
    Literal(String),
}
