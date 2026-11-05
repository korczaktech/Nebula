//! Declarações e instruções representadas na AST.

use super::{
    declaracao::{AtributoAst, Visibilidade},
    expressao::Expressao,
    padroes::CasoEscolha,
};
use nebula_diagnosticos::Diagnostico;
use nebula_fonte::IntervaloFonte;
use std::path::PathBuf;

/// Declarações e instruções reconhecidas pelo parser.
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
        assincrona: bool,
        nome: String,
        genericos: Vec<String>,
        parametros: Vec<(String, String)>,
        parametros_padrao: Vec<Option<Expressao>>,
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
    Passar,
    Ceder(Option<Expressao>),
    Atributada {
        atributos: Vec<AtributoAst>,
        declaracao: Box<NoAst>,
    },
    BlocoContextual {
        contexto: String,
        argumento: Option<String>,
        argumentos: Vec<Expressao>,
        corpo: Vec<NoAst>,
    },
    Classe {
        nome: String,
        herda: Option<String>,
        corpo: Vec<NoAst>,
    },
}

/// Nó da AST acompanhado do intervalo de bytes que o originou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoLocalizado {
    pub no: NoAst,
    pub intervalo: IntervaloFonte,
}

/// Caso de escolha acompanhado do intervalo de bytes que o originou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CasoEscolhaLocalizado {
    pub caso: CasoEscolha,
    pub intervalo: IntervaloFonte,
}

/// Expressão da AST acompanhada do intervalo de bytes que a originou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressaoLocalizada {
    pub expressao: Expressao,
    pub intervalo: IntervaloFonte,
}

/// Tipo da AST acompanhado do intervalo de bytes que o originou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TipoLocalizado {
    pub tipo: super::tipos::TipoAst,
    pub intervalo: IntervaloFonte,
}

/// Programa analisado, incluindo nós, expressões e tipos com seus intervalos de origem.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Programa {
    /// Caminho do arquivo que originou o programa, quando disponível.
    pub caminho: Option<PathBuf>,
    pub itens: Vec<NoAst>,
    /// Nós localizados, incluindo instruções aninhadas nos blocos.
    pub nos_localizados: Vec<NoLocalizado>,
    /// Expressões localizadas, incluindo subexpressões.
    pub expressoes_localizadas: Vec<ExpressaoLocalizada>,
    /// Tipos localizados, incluindo tipos genéricos aninhados.
    pub tipos_localizados: Vec<TipoLocalizado>,
    /// Casos de escolha localizados na fonte.
    pub casos_localizados: Vec<CasoEscolhaLocalizado>,
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

    /// Itera sobre todas as expressões com localização de origem.
    pub fn iter_expressoes_localizadas(&self) -> impl Iterator<Item = &ExpressaoLocalizada> {
        self.expressoes_localizadas.iter()
    }

    /// Itera sobre todos os tipos com localização de origem.
    pub fn iter_tipos_localizados(&self) -> impl Iterator<Item = &TipoLocalizado> {
        self.tipos_localizados.iter()
    }

    /// Itera sobre todos os casos de escolha com localização de origem.
    pub fn iter_casos_localizados(&self) -> impl Iterator<Item = &CasoEscolhaLocalizado> {
        self.casos_localizados.iter()
    }
}

impl NoAst {
    /// Retorna o tipo declarado de uma variável, quando informado explicitamente.
    pub fn tipo_variavel(&self) -> Option<super::tipos::TipoAst> {
        match self {
            Self::Variavel {
                tipo: Some(tipo), ..
            } => Some(super::tipos::TipoAst::analisar(tipo)),
            _ => None,
        }
    }

    /// Retorna o destino estruturado de um alias de tipo.
    pub fn destino_alias(&self) -> Option<super::declaracao::AliasTipoAst> {
        match self {
            Self::Tipo { nome, destino } => Some(super::declaracao::AliasTipoAst {
                nome: nome.clone(),
                destino: super::tipos::TipoAst::analisar(destino),
            }),
            _ => None,
        }
    }

    /// Retorna os parâmetros de função com seus tipos estruturados.
    pub fn parametros_tipados(&self) -> Option<Vec<super::declaracao::ParametroAst>> {
        match self {
            Self::Funcao { parametros, .. } => Some(
                parametros
                    .iter()
                    .map(|(nome, tipo)| super::declaracao::ParametroAst {
                        nome: nome.clone(),
                        tipo: super::tipos::TipoAst::analisar(tipo),
                    })
                    .collect(),
            ),
            _ => None,
        }
    }

    /// Retorna os campos de uma estrutura com seus tipos estruturados.
    pub fn campos_tipados(&self) -> Option<Vec<super::declaracao::CampoAst>> {
        match self {
            Self::Estrutura { campos, .. } => Some(
                campos
                    .iter()
                    .map(|(nome, tipo)| super::declaracao::CampoAst {
                        nome: nome.clone(),
                        tipo: super::tipos::TipoAst::analisar(tipo),
                    })
                    .collect(),
            ),
            _ => None,
        }
    }

    /// Retorna as variantes de uma enumeração com seus tipos estruturados.
    pub fn variantes_tipadas(&self) -> Option<Vec<super::declaracao::VarianteAst>> {
        match self {
            Self::Enumeracao { variantes, .. } => Some(
                variantes
                    .iter()
                    .map(|(nome, campos)| super::declaracao::VarianteAst {
                        nome: nome.clone(),
                        campos: campos
                            .iter()
                            .map(|tipo| super::tipos::TipoAst::analisar(tipo))
                            .collect(),
                    })
                    .collect(),
            ),
            _ => None,
        }
    }
}
