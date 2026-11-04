//! Declarações e instruções representadas na AST.
use nebula_diagnosticos::Diagnostico;
use nebula_fonte::IntervaloFonte;
use super::{expressao::Expressao, padroes::CasoEscolha, declaracao::Visibilidade};

/// Declarações e instruções reconhecidas pelo parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoAst {
    Variavel { nome: String, tipo: Option<String>, valor: Expressao, constante: bool },
    Funcao { visibilidade: Option<Visibilidade>, nome: String, genericos: Vec<String>, parametros: Vec<(String, String)>, retorno: Option<String>, corpo: Vec<NoAst> },
    Tipo { nome: String, destino: String },
    Estrutura { nome: String, campos: Vec<(String, String)> },
    Enumeracao { nome: String, variantes: Vec<(String, Vec<String>)> },
    Incluir { caminho: String, apelido: Option<String> },
    Expressao(Expressao),
    Quando { condicao: Expressao, entao: Vec<NoAst>, senao: Vec<NoAst> },
    Escolha { valor: Expressao, casos: Vec<CasoEscolha> },
    Enquanto { condicao: Expressao, corpo: Vec<NoAst> },
    Cada { nome: String, iteravel: Expressao, corpo: Vec<NoAst> },
    Pulso { intervalo: Expressao, corpo: Vec<NoAst> },
    Para { inicial: Option<Box<NoAst>>, condicao: Option<Expressao>, atualizacao: Option<Expressao>, corpo: Vec<NoAst> },
    Devolver(Option<Expressao>),
    Parar,
    Seguir,
}

/// Nó da AST acompanhado do intervalo de bytes que o originou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoLocalizado {
    pub no: NoAst,
    pub intervalo: IntervaloFonte,
}

/// Programa analisado, incluindo a lista de nós e seus intervalos de origem.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Programa {
    pub itens: Vec<NoAst>,
    /// Nós localizados, incluindo instruções aninhadas nos blocos.
    pub nos_localizados: Vec<NoLocalizado>,
}

/// Resultado sintático com AST parcial e todos os diagnósticos coletados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultadoSintatico {
    pub programa: Programa,
    pub diagnosticos: Vec<Diagnostico>,
}

impl Programa {
    /// Itera sobre todos os nós com localização de origem.
    pub fn iter_localizados(&self) -> impl Iterator<Item = &NoLocalizado> {
        self.nos_localizados.iter()
    }
}
