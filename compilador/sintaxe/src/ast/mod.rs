//! Árvore Sintática Abstrata pública da linguagem Nebula.

pub mod declaracao;
pub mod expressao;
pub mod instrucao;
pub mod operadores;
pub mod padroes;
pub mod tipos;

pub use declaracao::{
    AliasTipoAst, AtributoAst, CampoAst, ParametroAst, VarianteAst, Visibilidade,
};
pub use expressao::Expressao;
pub use instrucao::{
    CapturaAst, CasoEscolhaLocalizado, ExpressaoLocalizada, NoAst, NoLocalizado, Programa,
    ResultadoSintatico, TipoLocalizado,
};
pub use operadores::OperadorAst;
pub use padroes::{CasoEscolha, PadraoAst};
pub use tipos::TipoAst;

#[cfg(test)]
mod testes;
