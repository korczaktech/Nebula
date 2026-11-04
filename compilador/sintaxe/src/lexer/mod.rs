//! Análise léxica da linguagem Nebula.
//!
//! A API pública deste módulo recebe um arquivo-fonte e devolve tokens e
//! diagnósticos. Os detalhes de varredura permanecem privados.

mod scanner;
pub mod token;

#[cfg(test)]
mod testes;

pub use scanner::analisar_lexico;
pub use token::{ResultadoLexico, TipoToken, Token};
