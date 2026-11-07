//! Árvore Sintática Abstrata pública da linguagem Nebula.
//!
//! Os nós representam o conteúdo dos blocos, não seus delimitadores textuais.
//! A abertura e o fechamento por chaves são validados pelo Parser; não existe
//! nó ou marcador de encerramento de bloco na AST.

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
