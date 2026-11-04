//! Estruturas públicas da Árvore Sintática Abstrata da linguagem Nebula.

use crate::lexer::TipoToken;
use nebula_diagnosticos::Diagnostico;

/// Tipos de nós da AST, independentes de detalhes de formatação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expressao {
    Nome(String),
    Literal(String),
    Lista(Vec<Expressao>),
    Tupla(Vec<Expressao>),
    Unaria {
        operador: TipoToken,
        valor: Box<Expressao>,
    },
    Binaria {
        esquerda: Box<Expressao>,
        operador: TipoToken,
        direita: Box<Expressao>,
    },
    Atribuicao {
        destino: Box<Expressao>,
        valor: Box<Expressao>,
    },
    Chamada {
        alvo: Box<Expressao>,
        argumentos: Vec<Expressao>,
    },
    Indexacao {
        alvo: Box<Expressao>,
        indice: Box<Expressao>,
    },
    Membro {
        alvo: Box<Expressao>,
        operador: TipoToken,
        nome: String,
    },
    ConstrucaoEstrutura {
        nome: String,
        campos: Vec<(String, Expressao)>,
    },
}

/// Visibilidade declarada para uma função.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibilidade {
    Publico,
    Privado,
}

/// Caso de correspondência em uma instrução escolha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CasoEscolha {
    pub enumeracao: Option<String>,
    pub variante: Option<String>,
    pub vinculacoes: Vec<String>,
    pub corpo: Vec<NoAst>,
}

/// Declarações e instruções representadas na AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoAst {
    Variavel {
        nome: String,
        tipo: Option<String>,
        valor: Expressao,
        constante: bool,
    },
    Funcao {
        visibilidade: Option<Visibilidade>,
        nome: String,
        genericos: Vec<String>,
        parametros: Vec<(String, String)>,
        retorno: Option<String>,
        corpo: Vec<NoAst>,
    },
    Tipo {
        nome: String,
        destino: String,
    },
    Estrutura {
        nome: String,
        campos: Vec<(String, String)>,
    },
    Enumeracao {
        nome: String,
        variantes: Vec<(String, Vec<String>)>,
    },
    Incluir {
        caminho: String,
        apelido: Option<String>,
    },
    Expressao(Expressao),
    Quando {
        condicao: Expressao,
        entao: Vec<NoAst>,
        senao: Vec<NoAst>,
    },
    Escolha {
        valor: Expressao,
        casos: Vec<CasoEscolha>,
    },
    Enquanto {
        condicao: Expressao,
        corpo: Vec<NoAst>,
    },
    Cada {
        nome: String,
        iteravel: Expressao,
        corpo: Vec<NoAst>,
    },
    Pulso {
        intervalo: Expressao,
        corpo: Vec<NoAst>,
    },
    Para {
        inicial: Option<Box<NoAst>>,
        condicao: Option<Expressao>,
        atualizacao: Option<Expressao>,
        corpo: Vec<NoAst>,
    },
    Devolver(Option<Expressao>),
    Parar,
    Seguir,
}

/// Programa analisado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Programa {
    pub itens: Vec<NoAst>,
}

/// Resultado sintático com AST parcial e todos os diagnósticos coletados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultadoSintatico {
    pub programa: Programa,
    pub diagnosticos: Vec<Diagnostico>,
}
