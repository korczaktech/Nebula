//! Nós que representam expressões da linguagem Nebula.
use crate::lexer::TipoToken;
/// Expressões reconhecidas pelo parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expressao {
    Nome(String), Literal(String), Lista(Vec<Expressao>), Tupla(Vec<Expressao>),
    Unaria { operador: TipoToken, valor: Box<Expressao> },
    Binaria { esquerda: Box<Expressao>, operador: TipoToken, direita: Box<Expressao> },
    Atribuicao { destino: Box<Expressao>, valor: Box<Expressao> },
    Chamada { alvo: Box<Expressao>, argumentos: Vec<Expressao> },
    Indexacao { alvo: Box<Expressao>, indice: Box<Expressao> },
    Membro { alvo: Box<Expressao>, operador: TipoToken, nome: String },
    ConstrucaoEstrutura { nome: String, campos: Vec<(String, Expressao)> },
}
