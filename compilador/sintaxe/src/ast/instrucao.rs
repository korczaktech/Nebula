//! Declarações e instruções representadas na AST.

use super::{declaracao::Visibilidade, expressao::Expressao, padroes::CasoEscolha};
use nebula_diagnosticos::Diagnostico;
use nebula_fonte::IntervaloFonte;

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
