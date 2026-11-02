//! Representação intermediária tipada e interpretador de referência da Nebula.

#![forbid(unsafe_code)]
#![allow(missing_docs)]

#[rustfmt::skip]
mod implementacao {
use nebula_semantica::{analisar_com_modulos, ModulosDisponiveis, Tipo};
use nebula_sintaxe::{Expressao, NoAst, Programa, TipoToken};
use nebula_runtime::{
    comprimento_texto, operar_decimal, operar_inteiro, subtexto,
    texto_contem, texto_para_decimal, texto_para_inteiro, ErroRuntime, Memoria,
    OperacaoDecimal, OperacaoInteira, Valor as ValorRuntime,
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Valor constante ou resultado de execução na IR.
#[derive(Debug, Clone, PartialEq)]
pub enum ValorIr {
    Inteiro(i128), Decimal(f64), Logico(bool), Texto(String), Caractere(char),
    Lista(Vec<ValorIr>), Intervalo(i128, i128), IntervaloDecimal(f64, f64), Nulo, Vazio,
}

/// Operadores normalizados da representação intermediária.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperadorIr {
    Mais, Menos, Vezes, Divide, Resto, Potencia, Igual, Diferente,
    Menor, MenorIgual, Maior, MaiorIgual, E, Ou, Intervalo, Negacao, Nao,
}

/// Expressão em forma independente da AST.
#[derive(Debug, Clone, PartialEq)]
pub enum ExpressaoIr {
    Constante(ValorIr),
    Variavel(String),
    Lista(Vec<ExpressaoIr>),
    Unaria { operador: OperadorIr, valor: Box<ExpressaoIr> },
    Binaria { esquerda: Box<ExpressaoIr>, operador: OperadorIr, direita: Box<ExpressaoIr> },
    Atribuicao { nome: String, valor: Box<ExpressaoIr> },
    Chamada { nome: String, argumentos: Vec<ExpressaoIr> },
    ChamadaQualificada { modulo: String, nome: String, argumentos: Vec<ExpressaoIr> },
    Indexacao { alvo: Box<ExpressaoIr>, indice: Box<ExpressaoIr> },
}

/// Instrução tipada da IR.
#[derive(Debug, Clone, PartialEq)]
pub enum InstrucaoIr {
    Declarar { nome: String, tipo: Tipo, valor: ExpressaoIr, constante: bool },
    Expressao(ExpressaoIr),
    Bloco(Vec<InstrucaoIr>),
    Se { condicao: ExpressaoIr, entao: Vec<InstrucaoIr>, senao: Vec<InstrucaoIr> },
    Enquanto { condicao: ExpressaoIr, corpo: Vec<InstrucaoIr> },
    Para { inicial: Option<Box<InstrucaoIr>>, condicao: Option<ExpressaoIr>, atualizacao: Option<ExpressaoIr>, corpo: Vec<InstrucaoIr> },
    Devolver(Option<ExpressaoIr>),
    Parar,
    Seguir,
}

/// Função independente da sintaxe de declaração.
#[derive(Debug, Clone, PartialEq)]
pub struct FuncaoIr {
    pub nome: String,
    pub parametros: Vec<(String, Tipo)>,
    pub retorno: Tipo,
    pub corpo: Vec<InstrucaoIr>,
}

/// Programa em IR, com funções indexadas por nome e instruções globais.
#[derive(Debug, Clone, PartialEq)]
pub struct ProgramaIr {
    pub instrucoes: Vec<InstrucaoIr>,
    pub funcoes: HashMap<String, FuncaoIr>,
    pub modulos: HashMap<String, String>,
}

/// Resultado da conversão da AST para IR.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultadoIr {
    pub programa: Option<ProgramaIr>,
    pub diagnosticos: Vec<String>,
}

/// Valida a semântica e converte a AST para IR.
#[must_use]
pub fn baixar(programa: &Programa, modulos: &ModulosDisponiveis) -> ResultadoIr {
    let semantica = analisar_com_modulos(programa, modulos);
    if !semantica.diagnosticos.is_empty() {
        return ResultadoIr {
            programa: None,
            diagnosticos: semantica.diagnosticos.iter()
                .map(|d| format!("{}: {}", d.codigo, d.mensagem)).collect(),
        };
    }
    let aliases = programa.itens.iter().filter_map(|item| {
        if let NoAst::Tipo { nome, destino } = item { Some((nome.clone(), destino.clone())) } else { None }
    }).collect();
    let modulos_importados = programa.itens.iter().filter_map(|item| {
        if let NoAst::Incluir { caminho, apelido } = item {
            let nome = apelido.clone().unwrap_or_else(|| caminho.split("::").last().unwrap_or(caminho).to_owned());
            Some((nome, caminho.clone()))
        } else { None }
    }).collect::<HashMap<_, _>>();
    let mut lowerer = Lowerer { diagnosticos: Vec::new(), aliases, modulos_importados: modulos_importados.clone() };
    let mut ir = ProgramaIr { instrucoes: Vec::new(), funcoes: HashMap::new(), modulos: modulos_importados };
    for item in &programa.itens {
        match item {
            NoAst::Funcao { nome, parametros, retorno, corpo, .. } => {
                let funcao = FuncaoIr {
                    nome: nome.clone(),
                    parametros: parametros.iter().map(|(n, t)| (n.clone(), lowerer.tipo(t))).collect(),
                    retorno: retorno.as_deref().map_or(Tipo::Vazio, |t| lowerer.tipo(t)),
                    corpo: lowerer.bloco(corpo),
                };
                ir.funcoes.insert(nome.clone(), funcao);
            }
            NoAst::Tipo { .. } | NoAst::Incluir { .. } => {}
            _ => if let Some(instrucao) = lowerer.item(item) { ir.instrucoes.push(instrucao); },
        }
    }
    ResultadoIr {
        programa: if lowerer.diagnosticos.is_empty() { Some(ir) } else { None },
        diagnosticos: lowerer.diagnosticos,
    }
}

struct Lowerer { diagnosticos: Vec<String>, aliases: HashMap<String, String>, modulos_importados: HashMap<String, String> }

impl Lowerer {
    fn tipo(&self, nome: &str) -> Tipo {
        match nome {
            "vazio" => Tipo::Vazio,
            "logico" | "lógico" => Tipo::Logico,
            "inteiro" => Tipo::Inteiro,
            "inteiro8" => Tipo::Inteiro8,
            "inteiro16" => Tipo::Inteiro16,
            "inteiro32" => Tipo::Inteiro32,
            "inteiro64" => Tipo::Inteiro64,
            "inteiro128" => Tipo::Inteiro128,
            "byte" => Tipo::Byte,
            "decimal" | "decimal64" => Tipo::Decimal64,
            "decimal32" => Tipo::Decimal32,
            "texto" => Tipo::Texto,
            "caractere" => Tipo::Caractere,
            _ if nome.starts_with('[') && nome.ends_with(']') => Tipo::Lista(Box::new(self.tipo(&nome[1..nome.len()-1]))),
            _ => self.aliases.get(nome).map_or(Tipo::Desconhecido, |destino| self.tipo(destino)),
        }
    }

    fn bloco(&mut self, itens: &[NoAst]) -> Vec<InstrucaoIr> {
        itens.iter().filter_map(|item| self.item(item)).collect()
    }

    fn item(&mut self, item: &NoAst) -> Option<InstrucaoIr> {
        match item {
            NoAst::Variavel { nome, tipo, valor, constante } => {
                let valor = self.expressao(valor)?;
                let tipo = tipo.as_deref().map_or_else(|| inferir_tipo(&valor), |t| self.tipo(t));
                Some(InstrucaoIr::Declarar { nome: nome.clone(), tipo, valor, constante: *constante })
            }
            NoAst::Expressao(e) => Some(InstrucaoIr::Expressao(self.expressao(e)?)),
            NoAst::Quando { condicao, entao, senao } => Some(InstrucaoIr::Se {
                condicao: self.expressao(condicao)?,
                entao: self.bloco(entao),
                senao: self.bloco(senao),
            }),
            NoAst::Enquanto { condicao, corpo } => Some(InstrucaoIr::Enquanto {
                condicao: self.expressao(condicao)?, corpo: self.bloco(corpo),
            }),
            NoAst::Para { inicial, condicao, atualizacao, corpo } => Some(InstrucaoIr::Para {
                inicial: inicial.as_deref().and_then(|i| self.item(i)).map(Box::new),
                condicao: condicao.as_ref().and_then(|e| self.expressao(e)),
                atualizacao: atualizacao.as_ref().and_then(|e| self.expressao(e)),
                corpo: self.bloco(corpo),
            }),
            NoAst::Devolver(e) => Some(InstrucaoIr::Devolver(e.as_ref().and_then(|v| self.expressao(v)))),
            NoAst::Parar => Some(InstrucaoIr::Parar),
            NoAst::Seguir => Some(InstrucaoIr::Seguir),
            NoAst::Tipo { .. } | NoAst::Incluir { .. } | NoAst::Funcao { .. } => None,
        }
    }

    fn expressao(&mut self, expressao: &Expressao) -> Option<ExpressaoIr> {
        match expressao {
            Expressao::Nome(nome) => Some(ExpressaoIr::Variavel(nome.clone())),
            Expressao::Literal(valor) => Some(ExpressaoIr::Constante(literal(valor)?)),
            Expressao::Lista(valores) => Some(ExpressaoIr::Lista(
                valores.iter().map(|v| self.expressao(v)).collect::<Option<Vec<_>>>()?,
            )),
            Expressao::Unaria { operador, valor } => Some(ExpressaoIr::Unaria {
                operador: converter_operador(*operador)?,
                valor: Box::new(self.expressao(valor)?),
            }),
            Expressao::Binaria { esquerda, operador, direita } => Some(ExpressaoIr::Binaria {
                esquerda: Box::new(self.expressao(esquerda)?),
                operador: converter_operador(*operador)?,
                direita: Box::new(self.expressao(direita)?),
            }),
            Expressao::Atribuicao { destino, valor } => {
                if let Expressao::Nome(nome) = destino.as_ref() {
                    Some(ExpressaoIr::Atribuicao { nome: nome.clone(), valor: Box::new(self.expressao(valor)?) })
                } else {
                    self.diagnosticos.push("A IR inicial só aceita atribuição a variável nomeada.".into());
                    None
                }
            }
            Expressao::Chamada { alvo, argumentos } => {
                let args = argumentos.iter().map(|a| self.expressao(a)).collect::<Option<Vec<_>>>()?;
                match alvo.as_ref() {
                    Expressao::Nome(nome) => Some(ExpressaoIr::Chamada { nome: nome.clone(), argumentos: args }),
                    Expressao::Membro { alvo, operador: TipoToken::Escopo, nome } => {
                        if let Expressao::Nome(modulo) = alvo.as_ref() {
                            let caminho = self.modulos_importados.get(modulo).cloned().unwrap_or_else(|| modulo.clone());
                            Some(ExpressaoIr::ChamadaQualificada { modulo: caminho, nome: nome.clone(), argumentos: args })
                        } else {
                            self.diagnosticos.push("Alvo de módulo qualificado inválido na IR.".into());
                            None
                        }
                    }
                    _ => {
                        self.diagnosticos.push("A IR inicial exige chamada por nome de função.".into());
                        None
                    }
                }
            }
            Expressao::Indexacao { alvo, indice } => Some(ExpressaoIr::Indexacao {
                alvo: Box::new(self.expressao(alvo)?), indice: Box::new(self.expressao(indice)?),
            }),
            Expressao::Membro { .. } => {
                self.diagnosticos.push("Acesso de membro não é suportado pela IR inicial.".into());
                None
            }
        }
    }
}

fn converter_operador(operador: TipoToken) -> Option<OperadorIr> {
    use OperadorIr::*;
    match operador {
        TipoToken::Mais => Some(Mais), TipoToken::Menos => Some(Menos),
        TipoToken::Vezes => Some(Vezes), TipoToken::Divide => Some(Divide),
        TipoToken::Resto => Some(Resto), TipoToken::Potencia => Some(Potencia),
        TipoToken::Igual => Some(Igual), TipoToken::Diferente => Some(Diferente),
        TipoToken::Menor => Some(Menor), TipoToken::MenorIgual => Some(MenorIgual),
        TipoToken::Maior => Some(Maior), TipoToken::MaiorIgual => Some(MaiorIgual),
        TipoToken::E => Some(E), TipoToken::Ou => Some(Ou),
        TipoToken::Intervalo => Some(Intervalo), TipoToken::Negacao => Some(Negacao),
        TipoToken::Nao => Some(Nao), _ => None,
    }
}

fn inferir_tipo(expressao: &ExpressaoIr) -> Tipo {
    match expressao {
        ExpressaoIr::Constante(ValorIr::Inteiro(_)) => Tipo::Inteiro,
        ExpressaoIr::Constante(ValorIr::Decimal(_)) => Tipo::Decimal64,
        ExpressaoIr::Constante(ValorIr::Logico(_)) => Tipo::Logico,
        ExpressaoIr::Constante(ValorIr::Texto(_)) => Tipo::Texto,
        ExpressaoIr::Constante(ValorIr::Caractere(_)) => Tipo::Caractere,
        ExpressaoIr::Constante(_) => Tipo::Desconhecido,
        ExpressaoIr::Lista(v) => Tipo::Lista(Box::new(v.first().map_or(Tipo::Desconhecido, inferir_tipo))),
        ExpressaoIr::Binaria { esquerda, operador, .. } => match operador {
            OperadorIr::Igual | OperadorIr::Diferente | OperadorIr::Menor | OperadorIr::MenorIgual
            | OperadorIr::Maior | OperadorIr::MaiorIgual | OperadorIr::E | OperadorIr::Ou => Tipo::Logico,
            OperadorIr::Intervalo => Tipo::Intervalo(Box::new(inferir_tipo(esquerda))),
            _ => inferir_tipo(esquerda),
        },
        ExpressaoIr::Unaria { valor, .. } => inferir_tipo(valor),
        ExpressaoIr::Atribuicao { valor, .. } => inferir_tipo(valor),
        ExpressaoIr::Indexacao { .. } | ExpressaoIr::Chamada { .. } | ExpressaoIr::ChamadaQualificada { .. } | ExpressaoIr::Variavel(_) => Tipo::Desconhecido,
    }
}

fn literal(bruto: &str) -> Option<ValorIr> {
    match bruto {
        "verdadeiro" => Some(ValorIr::Logico(true)),
        "falso" => Some(ValorIr::Logico(false)),
        "nulo" => Some(ValorIr::Nulo),
        _ if bruto.starts_with('"') && bruto.ends_with('"') => Some(ValorIr::Texto(desescapar(&bruto[1..bruto.len()-1]))),
        _ if bruto.starts_with('\'') && bruto.ends_with('\'') => desescapar(&bruto[1..bruto.len()-1]).chars().next().map(ValorIr::Caractere),
        _ if bruto.starts_with("0x") || bruto.starts_with("0X") => i128::from_str_radix(&bruto[2..].replace('_', ""), 16).ok().map(ValorIr::Inteiro),
        _ if bruto.starts_with("0b") || bruto.starts_with("0B") => i128::from_str_radix(&bruto[2..].replace('_', ""), 2).ok().map(ValorIr::Inteiro),
        _ if bruto.starts_with("0o") || bruto.starts_with("0O") => i128::from_str_radix(&bruto[2..].replace('_', ""), 8).ok().map(ValorIr::Inteiro),
        _ if bruto.contains('.') || bruto.contains('e') || bruto.contains('E') => bruto.replace('_', "").parse().ok().map(ValorIr::Decimal),
        _ => bruto.replace('_', "").parse().ok().map(ValorIr::Inteiro),
    }
}

fn desescapar(texto: &str) -> String {
    texto.replace("\\\\", "\u{0}")
        .replace("\\n", "\n").replace("\\r", "\r").replace("\\t", "\t")
        .replace("\\0", "\0").replace("\\\"", "\"").replace("\\'", "'")
        .replace("\u{0}", "\\")
}

/// Resultado da execução de referência da IR.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultadoExecucao {
    pub saida: Vec<String>,
    pub globais: HashMap<String, ValorIr>,
}

/// Executa a IR com limite de passos para impedir laços infinitos em testes.
pub fn executar(programa: &ProgramaIr, limite_passos: usize) -> Result<ResultadoExecucao, String> {
    let mut vm = Maquina {
        programa,
        memoria: Memoria::nova(),
        saida: Vec::new(),
        passos: 0,
        limite: limite_passos.max(1),
        profundidade_chamada: 0,
    };
    let fluxo = vm.bloco_raiz(&programa.instrucoes)?;
    if matches!(&fluxo, Fluxo::Parar | Fluxo::Seguir) {
        return Err("Controle de laço escapou do seu contexto.".into());
    }
    let globais = vm.memoria.globais().into_iter()
        .map(|(nome, valor)| Ok((nome, converter_para_ir(valor)?)))
        .collect::<Result<HashMap<_, _>, String>>()?;
    Ok(ResultadoExecucao { saida: vm.saida, globais })
}

enum Fluxo { Normal, Retorno(ValorIr), Parar, Seguir }

fn formatar_erro_runtime(erro: ErroRuntime) -> String {
    format!("Erro do runtime: {erro:?}")
}

fn converter_para_runtime(valor: ValorIr) -> ValorRuntime {
    match valor {
        ValorIr::Inteiro(v) => ValorRuntime::Inteiro(v),
        ValorIr::Decimal(v) => ValorRuntime::Decimal(v),
        ValorIr::Logico(v) => ValorRuntime::Logico(v),
        ValorIr::Texto(v) => ValorRuntime::Texto(v),
        ValorIr::Caractere(v) => ValorRuntime::Caractere(v),
        ValorIr::Lista(itens) => ValorRuntime::Lista(Arc::new(RwLock::new(
            itens.into_iter().map(converter_para_runtime).collect(),
        ))),
        ValorIr::Intervalo(inicio, fim) => ValorRuntime::Intervalo { inicio, fim },
        ValorIr::IntervaloDecimal(inicio, fim) => {
            ValorRuntime::IntervaloDecimal { inicio, fim }
        }
        ValorIr::Nulo => ValorRuntime::Nulo,
        ValorIr::Vazio => ValorRuntime::Vazio,
    }
}

fn converter_para_ir(valor: ValorRuntime) -> Result<ValorIr, String> {
    match valor {
        ValorRuntime::Inteiro(v) => Ok(ValorIr::Inteiro(v)),
        ValorRuntime::Decimal(v) => Ok(ValorIr::Decimal(v)),
        ValorRuntime::Logico(v) => Ok(ValorIr::Logico(v)),
        ValorRuntime::Texto(v) => Ok(ValorIr::Texto(v)),
        ValorRuntime::Caractere(v) => Ok(ValorIr::Caractere(v)),
        ValorRuntime::Lista(itens) => {
            let itens = itens.read().map_err(|_| "Lista do runtime corrompida.")?;
            Ok(ValorIr::Lista(itens.iter().cloned().map(converter_para_ir).collect::<Result<_, _>>()?))
        }
        ValorRuntime::Intervalo { inicio, fim } => Ok(ValorIr::Intervalo(inicio, fim)),
        ValorRuntime::IntervaloDecimal { inicio, fim } => {
            Ok(ValorIr::IntervaloDecimal(inicio, fim))
        }
        ValorRuntime::Nulo => Ok(ValorIr::Nulo),
        ValorRuntime::Vazio => Ok(ValorIr::Vazio),
    }
}

struct Maquina<'a> {
    programa: &'a ProgramaIr,
    memoria: Memoria,
    saida: Vec<String>,
    passos: usize,
    limite: usize,
    profundidade_chamada: usize,
}

impl Maquina<'_> {
    fn passo(&mut self) -> Result<(), String> {
        self.passos += 1;
        if self.passos > self.limite { Err("Limite de passos da IR excedido.".into()) } else { Ok(()) }
    }

    fn bloco_raiz(&mut self, itens: &[InstrucaoIr]) -> Result<Fluxo, String> {
        let mut fluxo = Fluxo::Normal;
        for item in itens {
            self.passo()?;
            fluxo = self.instrucao(item)?;
            if !matches!(&fluxo, Fluxo::Normal) { break; }
        }
        Ok(fluxo)
    }

    fn bloco(&mut self, itens: &[InstrucaoIr]) -> Result<Fluxo, String> {
        self.memoria.abrir_escopo();
        let resultado = (|| {
            let mut fluxo = Fluxo::Normal;
            for item in itens {
                self.passo()?;
                fluxo = self.instrucao(item)?;
                if !matches!(fluxo, Fluxo::Normal) { break; }
            }
            Ok(fluxo)
        })();
        let fechamento = self.memoria.fechar_escopo().map_err(formatar_erro_runtime);
        match (resultado, fechamento) {
            (Err(erro), _) => Err(erro),
            (Ok(_), Err(erro)) => Err(erro),
            (Ok(fluxo), Ok(())) => Ok(fluxo),
        }
    }

    fn instrucao(&mut self, item: &InstrucaoIr) -> Result<Fluxo, String> {
        match item {
            InstrucaoIr::Declarar { nome, valor, constante, .. } => {
                let valor = self.expressao(valor)?;
                self.memoria
                    .declarar(nome.clone(), converter_para_runtime(valor), !constante)
                    .map_err(formatar_erro_runtime)?;
                Ok(Fluxo::Normal)
            }
            InstrucaoIr::Expressao(e) => { self.expressao(e)?; Ok(Fluxo::Normal) }
            InstrucaoIr::Bloco(itens) => self.bloco(itens),
            InstrucaoIr::Se { condicao, entao, senao } => {
                let condicao = self.expressao(condicao)?;
                if self.logico(condicao)? { self.bloco(entao) } else { self.bloco(senao) }
            }
            InstrucaoIr::Enquanto { condicao, corpo } => {
                loop {
                    self.passo()?;
                    let condicao = self.expressao(condicao)?;
                    if !self.logico(condicao)? { break; }
                    match self.bloco(corpo)? {
                        Fluxo::Normal | Fluxo::Seguir => {}
                        Fluxo::Parar => break,
                        retorno @ Fluxo::Retorno(_) => return Ok(retorno),
                    }
                }
                Ok(Fluxo::Normal)
            }
            InstrucaoIr::Para { inicial, condicao, atualizacao, corpo } => {
                self.memoria.abrir_escopo();
                let resultado = (|| {
                    if let Some(inicial) = inicial { let _ = self.instrucao(inicial)?; }
                    let mut fluxo_final = Fluxo::Normal;
                    loop {
                        self.passo()?;
                        if let Some(condicao) = condicao {
                            let condicao = self.expressao(condicao)?;
                            if !self.logico(condicao)? { break; }
                        }
                        match self.bloco(corpo)? {
                            Fluxo::Normal | Fluxo::Seguir => {}
                            Fluxo::Parar => break,
                            retorno @ Fluxo::Retorno(_) => {
                                fluxo_final = retorno;
                                break;
                            }
                        }
                        if let Some(atualizacao) = atualizacao { self.expressao(atualizacao)?; }
                    }
                    Ok(fluxo_final)
                })();
                let fechamento = self.memoria.fechar_escopo().map_err(formatar_erro_runtime);
                match (resultado, fechamento) {
                    (Err(erro), _) => Err(erro),
                    (Ok(_), Err(erro)) => Err(erro),
                    (Ok(fluxo), Ok(())) => Ok(fluxo),
                }
            }
            InstrucaoIr::Devolver(valor) => Ok(Fluxo::Retorno(match valor {
                Some(valor) => self.expressao(valor)?,
                None => ValorIr::Vazio,
            })),
            InstrucaoIr::Parar => Ok(Fluxo::Parar),
            InstrucaoIr::Seguir => Ok(Fluxo::Seguir),
        }
    }

    fn expressao(&mut self, e: &ExpressaoIr) -> Result<ValorIr, String> {
        match e {
            ExpressaoIr::Constante(v) => Ok(v.clone()),
            ExpressaoIr::Variavel(nome) => self.buscar(nome).ok_or_else(|| format!("Variável '{nome}' não encontrada na IR.")),
            ExpressaoIr::Lista(valores) => Ok(ValorIr::Lista(valores.iter().map(|v| self.expressao(v)).collect::<Result<_,_>>()?)),
            ExpressaoIr::Unaria { operador, valor } => {
                let valor = self.expressao(valor)?;
                match (*operador, valor) {
                    (OperadorIr::Negacao | OperadorIr::Nao, ValorIr::Logico(v)) => Ok(ValorIr::Logico(!v)),
                    (OperadorIr::Menos, ValorIr::Inteiro(v)) => v.checked_neg().map(ValorIr::Inteiro).ok_or_else(|| "Overflow em negação inteira.".into()),
                    (OperadorIr::Menos, ValorIr::Decimal(v)) => Ok(ValorIr::Decimal(-v)),
                    (OperadorIr::Mais, v @ (ValorIr::Inteiro(_) | ValorIr::Decimal(_))) => Ok(v),
                    _ => Err("Operador unário incompatível na IR.".into()),
                }
            }
            ExpressaoIr::Binaria { esquerda, operador, direita } => {
                let a = self.expressao(esquerda)?;
                if *operador == OperadorIr::E && a == ValorIr::Logico(false) { return Ok(ValorIr::Logico(false)); }
                if *operador == OperadorIr::Ou && a == ValorIr::Logico(true) { return Ok(ValorIr::Logico(true)); }
                let b = self.expressao(direita)?;
                binaria(a, *operador, b)
            }
            ExpressaoIr::Atribuicao { nome, valor } => {
                let valor = self.expressao(valor)?;
                self.memoria
                    .atribuir(nome, converter_para_runtime(valor.clone()))
                    .map_err(formatar_erro_runtime)?;
                Ok(valor)
            }
            ExpressaoIr::Chamada { nome, argumentos } => {
                let valores = argumentos.iter().map(|a| self.expressao(a)).collect::<Result<Vec<_>,_>>()?;
                if nome == "exibir" {
                    self.saida.push(valores.iter().map(formatar).collect::<Vec<_>>().join(" "));
                    return Ok(ValorIr::Vazio);
                }
                if nome == "comprimento" {
                    let Some(valor) = valores.first() else {
                        return Err("'comprimento' exige um argumento.".into());
                    };
                    return match valor {
                        ValorIr::Texto(texto) => Ok(ValorIr::Inteiro(comprimento_texto(texto) as i128)),
                        ValorIr::Lista(itens) => Ok(ValorIr::Inteiro(itens.len() as i128)),
                        _ => Err("'comprimento' exige texto ou lista.".into()),
                    };
                }
                if nome == "subtexto" {
                    if valores.len() != 3 {
                        return Err("'subtexto' exige texto, início e fim.".into());
                    }
                    let (ValorIr::Texto(texto), ValorIr::Inteiro(inicio), ValorIr::Inteiro(fim)) =
                        (&valores[0], &valores[1], &valores[2])
                    else {
                        return Err("'subtexto' exige texto e dois inteiros.".into());
                    };
                    let inicio = usize::try_from(*inicio).map_err(|_| "Índice inicial inválido.")?;
                    let fim = usize::try_from(*fim).map_err(|_| "Índice final inválido.")?;
                    return subtexto(texto, inicio, fim)
                        .map(ValorIr::Texto)
                        .map_err(|erro| format!("Falha em 'subtexto': {erro:?}"));
                }
                if nome == "contem" {
                    if let [ValorIr::Texto(texto), ValorIr::Texto(trecho)] = valores.as_slice() {
                        return Ok(ValorIr::Logico(texto_contem(texto, trecho)));
                    }
                    return Err("'contem' exige dois textos.".into());
                }
                if nome == "maiusculas" || nome == "minusculas" {
                    if let [ValorIr::Texto(texto)] = valores.as_slice() {
                        let resultado = if nome == "maiusculas" {
                            texto_maiusculas(texto)
                        } else {
                            texto_minusculas(texto)
                        };
                        return Ok(ValorIr::Texto(resultado));
                    }
                    return Err(format!("'{nome}' exige um texto."));
                }
                if nome == "caractere" {
                    if let [ValorIr::Texto(texto), ValorIr::Inteiro(indice)] = valores.as_slice() {
                        let indice = usize::try_from(*indice)
                            .map_err(|_| "Índice inválido para 'caractere'.")?;
                        return caractere_texto(texto, indice)
                            .map(ValorIr::Caractere)
                            .map_err(|erro| format!("Falha em 'caractere': {erro:?}"));
                    }
                    return Err("'caractere' exige texto e índice inteiro.".into());
                }
                if nome == "texto" {
                    if let [valor] = valores.as_slice() {
                        return Ok(ValorIr::Texto(formatar(valor)));
                    }
                    return Err("'texto' exige exatamente um argumento.".into());
                }
                if nome == "inteiro" {
                    if let [ValorIr::Texto(texto)] = valores.as_slice() {
                        return texto_para_inteiro(texto)
                            .map(ValorIr::Inteiro)
                            .map_err(|erro| format!("Falha na conversão para inteiro: {erro:?}"));
                    }
                    return Err("'inteiro' exige um texto.".into());
                }
                if nome == "decimal" {
                    if let [ValorIr::Texto(texto)] = valores.as_slice() {
                        return texto_para_decimal(texto)
                            .map(ValorIr::Decimal)
                            .map_err(|erro| format!("Falha na conversão para decimal: {erro:?}"));
                    }
                    return Err("'decimal' exige um texto.".into());
                }
                if nome == "adicionar" {
                    if argumentos.len() != 2 {
                        return Err("'adicionar' exige uma lista e um valor.".into());
                    }
                    let ExpressaoIr::Variavel(variavel) = &argumentos[0] else {
                        return Err("'adicionar' exige uma variável de lista modificável.".into());
                    };
                    let valor = self.expressao(&argumentos[1])?;
                    self.memoria
                        .adicionar_elemento_lista(variavel, converter_para_runtime(valor))
                        .map_err(formatar_erro_runtime)?;
                    return Ok(ValorIr::Vazio);
                }
                if nome == "remover" {
                    if argumentos.len() != 2 {
                        return Err("'remover' exige uma lista e um índice.".into());
                    }
                    let ExpressaoIr::Variavel(variavel) = &argumentos[0] else {
                        return Err("'remover' exige uma variável de lista modificável.".into());
                    };
                    let indice = self.expressao(&argumentos[1])?;
                    let ValorIr::Inteiro(indice) = indice else {
                        return Err("O índice de 'remover' deve ser inteiro.".into());
                    };
                    let indice = usize::try_from(indice).map_err(|_| "Índice inválido para 'remover'.")?;
                    let valor = self.memoria
                        .remover_elemento_lista(variavel, indice)
                        .map_err(formatar_erro_runtime)?;
                    return converter_para_ir(valor);
                }
                if nome == "ler_texto" {
                    if !argumentos.is_empty() {
                        return Err("'ler_texto' não aceita argumentos.".into());
                    }
                    let mut texto = String::new();
                    std::io::stdin()
                        .read_line(&mut texto)
                        .map_err(|erro| format!("Falha ao ler texto: {erro}"))?;
                    if texto.ends_with('\n') { texto.pop(); }
                    if texto.ends_with('\r') { texto.pop(); }
                    return Ok(ValorIr::Texto(texto));
                }
                let funcao = self.programa.funcoes.get(nome).cloned().ok_or_else(|| format!("Função '{nome}' não encontrada na IR."))?;
                if funcao.parametros.len() != valores.len() { return Err(format!("Quantidade incorreta de argumentos para '{nome}'.")); }
                if self.profundidade_chamada >= 32 {
                    return Err("Limite de profundidade de chamadas da IR excedido.".into());
                }
                self.profundidade_chamada += 1;
                self.memoria.abrir_escopo();
                let fluxo = (|| {
                    for ((parametro, _), valor) in funcao.parametros.iter().zip(valores) {
                        self.memoria
                            .declarar(parametro.clone(), converter_para_runtime(valor), true)
                            .map_err(formatar_erro_runtime)?;
                    }
                    self.bloco(&funcao.corpo)
                })();
                let fechamento = self.memoria.fechar_escopo().map_err(formatar_erro_runtime);
                self.profundidade_chamada -= 1;
                let fluxo = match (fluxo, fechamento) {
                    (Err(erro), _) => return Err(erro),
                    (Ok(_), Err(erro)) => return Err(erro),
                    (Ok(fluxo), Ok(())) => fluxo,
                };
                match fluxo { Fluxo::Retorno(v) => Ok(v), _ => Ok(ValorIr::Vazio) }
            }
            ExpressaoIr::ChamadaQualificada { modulo, nome, .. } => Err(format!("A função '{modulo}::{nome}' precisa de uma implementação de módulo em tempo de execução.")),
            ExpressaoIr::Indexacao { alvo, indice } => {
                let alvo = self.expressao(alvo)?;
                let indice = self.expressao(indice)?;
                if let (ValorIr::Lista(valores), ValorIr::Inteiro(i)) = (alvo, indice) {
                    usize::try_from(i).ok().and_then(|i| valores.get(i).cloned()).ok_or_else(|| "Índice fora dos limites da lista.".into())
                } else { Err("Indexação exige lista e índice inteiro.".into()) }
            }
        }
    }

    fn buscar(&self, nome: &str) -> Option<ValorIr> {
        self.memoria.obter(nome).ok().and_then(|valor| converter_para_ir(valor).ok())
    }

    fn logico(&self, valor: ValorIr) -> Result<bool, String> {
        if let ValorIr::Logico(v) = valor { Ok(v) } else { Err("Condição da IR não é lógica.".into()) }
    }
}

fn binaria(a: ValorIr, op: OperadorIr, b: ValorIr) -> Result<ValorIr, String> {
    use OperadorIr::*;
    match (a, op, b) {
        (ValorIr::Inteiro(a), Mais, ValorIr::Inteiro(b)) => operar_inteiro(OperacaoInteira::Soma, a, b)
            .map(ValorIr::Inteiro)
            .map_err(|_| "Overflow em soma inteira.".into()),
        (ValorIr::Inteiro(a), Menos, ValorIr::Inteiro(b)) => operar_inteiro(OperacaoInteira::Subtracao, a, b)
            .map(ValorIr::Inteiro)
            .map_err(|_| "Overflow em subtração inteira.".into()),
        (ValorIr::Inteiro(a), Vezes, ValorIr::Inteiro(b)) => operar_inteiro(OperacaoInteira::Multiplicacao, a, b)
            .map(ValorIr::Inteiro)
            .map_err(|_| "Overflow em multiplicação inteira.".into()),
        (ValorIr::Inteiro(_), Divide, ValorIr::Inteiro(0)) | (ValorIr::Inteiro(_), Resto, ValorIr::Inteiro(0)) => Err("Divisão por zero na IR.".into()),
        (ValorIr::Inteiro(a), Divide, ValorIr::Inteiro(b)) => operar_inteiro(OperacaoInteira::Divisao, a, b)
            .map(ValorIr::Inteiro)
            .map_err(|_| "Overflow em divisão inteira.".into()),
        (ValorIr::Inteiro(a), Resto, ValorIr::Inteiro(b)) => operar_inteiro(OperacaoInteira::Resto, a, b)
            .map(ValorIr::Inteiro)
            .map_err(|_| "Overflow em resto inteiro.".into()),
        (ValorIr::Inteiro(a), Potencia, ValorIr::Inteiro(b)) if (0..=u32::MAX as i128).contains(&b) => operar_inteiro(OperacaoInteira::Potencia, a, b)
            .map(ValorIr::Inteiro)
            .map_err(|_| "Overflow em potência inteira.".into()),
        (ValorIr::Decimal(a), Mais, ValorIr::Decimal(b)) => operar_decimal(OperacaoDecimal::Soma, a, b)
            .map(ValorIr::Decimal)
            .map_err(|_| "Resultado decimal não finito na IR.".into()),
        (ValorIr::Decimal(a), Menos, ValorIr::Decimal(b)) => operar_decimal(OperacaoDecimal::Subtracao, a, b)
            .map(ValorIr::Decimal)
            .map_err(|_| "Resultado decimal não finito na IR.".into()),
        (ValorIr::Decimal(a), Vezes, ValorIr::Decimal(b)) => operar_decimal(OperacaoDecimal::Multiplicacao, a, b)
            .map(ValorIr::Decimal)
            .map_err(|_| "Resultado decimal não finito na IR.".into()),
        (ValorIr::Decimal(_), Divide, ValorIr::Decimal(0.0)) => Err("Divisão por zero na IR.".into()),
        (ValorIr::Decimal(a), Divide, ValorIr::Decimal(b)) => operar_decimal(OperacaoDecimal::Divisao, a, b)
            .map(ValorIr::Decimal)
            .map_err(|erro| match erro {
                ErroRuntime::DivisaoPorZero => "Divisão por zero na IR.".into(),
                _ => "Resultado decimal não finito na IR.".into(),
            }),
        (ValorIr::Decimal(a), Resto, ValorIr::Decimal(b)) => operar_decimal(OperacaoDecimal::Resto, a, b)
            .map(ValorIr::Decimal)
            .map_err(|erro| match erro {
                ErroRuntime::DivisaoPorZero => "Divisão por zero na IR.".into(),
                _ => "Resultado decimal não finito na IR.".into(),
            }),
        (ValorIr::Decimal(a), Potencia, ValorIr::Decimal(b)) => operar_decimal(OperacaoDecimal::Potencia, a, b)
            .map(ValorIr::Decimal)
            .map_err(|_| "Resultado decimal não finito na IR.".into()),
        (ValorIr::Decimal(a), Intervalo, ValorIr::Decimal(b)) => Ok(ValorIr::IntervaloDecimal(a,b)),
        (ValorIr::Inteiro(a), op, ValorIr::Decimal(b)) => binaria(ValorIr::Decimal(a as f64), op, ValorIr::Decimal(b)),
        (ValorIr::Decimal(a), op, ValorIr::Inteiro(b)) => binaria(ValorIr::Decimal(a), op, ValorIr::Decimal(b as f64)),
        (ValorIr::Texto(a), Mais, ValorIr::Texto(b)) => Ok(ValorIr::Texto(a + &b)),
        (ValorIr::Logico(a), E, ValorIr::Logico(b)) => Ok(ValorIr::Logico(a && b)),
        (ValorIr::Logico(a), Ou, ValorIr::Logico(b)) => Ok(ValorIr::Logico(a || b)),
        (a, Igual, b) => Ok(ValorIr::Logico(a == b)),
        (a, Diferente, b) => Ok(ValorIr::Logico(a != b)),
        (ValorIr::Inteiro(a), Menor, ValorIr::Inteiro(b)) => Ok(ValorIr::Logico(a < b)),
        (ValorIr::Inteiro(a), MenorIgual, ValorIr::Inteiro(b)) => Ok(ValorIr::Logico(a <= b)),
        (ValorIr::Inteiro(a), Maior, ValorIr::Inteiro(b)) => Ok(ValorIr::Logico(a > b)),
        (ValorIr::Inteiro(a), MaiorIgual, ValorIr::Inteiro(b)) => Ok(ValorIr::Logico(a >= b)),
        (ValorIr::Decimal(a), Menor, ValorIr::Decimal(b)) => Ok(ValorIr::Logico(a < b)),
        (ValorIr::Decimal(a), MenorIgual, ValorIr::Decimal(b)) => Ok(ValorIr::Logico(a <= b)),
        (ValorIr::Decimal(a), Maior, ValorIr::Decimal(b)) => Ok(ValorIr::Logico(a > b)),
        (ValorIr::Decimal(a), MaiorIgual, ValorIr::Decimal(b)) => Ok(ValorIr::Logico(a >= b)),
        (ValorIr::Inteiro(a), Intervalo, ValorIr::Inteiro(b)) => Ok(ValorIr::Intervalo(a,b)),
        _ => Err("Operandos incompatíveis para operação na IR.".into()),
    }
}

fn formatar(valor: &ValorIr) -> String {
    match valor {
        ValorIr::Inteiro(v) => v.to_string(), ValorIr::Decimal(v) => v.to_string(),
        ValorIr::Logico(v) => if *v { "verdadeiro".into() } else { "falso".into() },
        ValorIr::Texto(v) => v.clone(), ValorIr::Caractere(v) => v.to_string(),
        ValorIr::Lista(v) => format!("[{}]", v.iter().map(formatar).collect::<Vec<_>>().join(", ")),
        ValorIr::Intervalo(a,b) => format!("{a}..{b}"),
        ValorIr::IntervaloDecimal(a,b) => format!("{a}..{b}"),
        ValorIr::Nulo => "nulo".into(), ValorIr::Vazio => String::new(),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use nebula_fonte::ArquivoFonte;
    use nebula_sintaxe::analisar;

    fn compilar(codigo: &str) -> ProgramaIr {
        let fonte = ArquivoFonte::novo("teste.nbx", codigo);
        let sintaxe = analisar(&fonte);
        assert!(sintaxe.diagnosticos.is_empty(), "{:?}", sintaxe.diagnosticos);
        let resultado = baixar(&sintaxe.programa, &ModulosDisponiveis::new());
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
        resultado.programa.expect("IR válida")
    }

    #[test]
    fn executor_usa_runtime_e_rejeita_atribuicao_em_constante() {
        let mut ir = compilar("constante limite = 9");
        ir.instrucoes.push(InstrucaoIr::Expressao(ExpressaoIr::Atribuicao {
            nome: "limite".into(),
            valor: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(10))),
        }));
        let erro = executar(&ir, 100).expect_err("constante não pode ser alterada");
        assert!(erro.contains("VariavelImutavel"), "{erro}");
    }

    #[test]
    fn baixa_declaracoes_e_expressoes() {
        let ir = compilar("variavel total = 1 + 2");
        assert_eq!(ir.instrucoes.len(), 1);
        assert!(matches!(ir.instrucoes[0], InstrucaoIr::Declarar { .. }));
    }

    #[test]
    fn executa_biblioteca_basica_de_texto_numeros_e_listas() {
        let ir = compilar(
            "variavel itens = [1, 2]\nadicionar(itens, 3)\nexibir(comprimento(itens))\nexibir(remover(itens, 0))\nexibir(subtexto(\"ação\", 0, 2))\nexibir(contem(\"Nebula\", \"bul\"))\nexibir(inteiro(\"42\") + 1)\nexibir(decimal(\"1.5\") + 0.5)",
        );
        let resultado = executar(&ir, 1000).expect("runtime válido");
        assert_eq!(
            resultado.saida,
            vec!["3", "1", "aç", "verdadeiro", "43", "2"]
        );
    }

    #[test]
    fn executa_expressao_e_chamada_exibir() {
        let ir = compilar("variavel total = 1 + 2\nexibir(total)");
        let r = executar(&ir, 100).expect("execução válida");
        assert_eq!(r.saida, vec!["3"]);
        assert_eq!(r.globais.get("total"), Some(&ValorIr::Inteiro(3)));
    }

    #[test]
    fn executa_funcao_com_retorno() {
        let ir = compilar("funcao soma(a: inteiro, b: inteiro): inteiro { devolver a + b }\nvariavel total = soma(2, 3)");
        let r = executar(&ir, 100).expect("execução válida");
        assert_eq!(r.globais.get("total"), Some(&ValorIr::Inteiro(5)));
    }

    #[test]
    fn executa_condicional_e_laco() {
        let ir = compilar("variavel x = 0\nenquanto x < 3 { x = x + 1 }\nquando x == 3 { exibir(x) }");
        let r = executar(&ir, 100).expect("execução válida");
        assert_eq!(r.saida, vec!["3"]);
    }

    #[test]
    fn interrompe_laco_infinito_por_limite() {
        let ir = compilar("enquanto verdadeiro { exibir(1) }");
        assert!(executar(&ir, 5).is_err());
    }

    #[test]
    fn executa_operacoes_decimais_e_mistas() {
        let ir = compilar("variavel total = 2 + 0.5\nvariavel comparacao = total > 2");
        let r = executar(&ir, 100).expect("execução válida");
        assert_eq!(r.globais.get("total"), Some(&ValorIr::Decimal(2.5)));
        assert_eq!(r.globais.get("comparacao"), Some(&ValorIr::Logico(true)));
    }

    #[test]
    fn rejeita_divisao_por_zero_em_tempo_de_execucao() {
        let ir = compilar("variavel resultado = 10 / 0");
        assert!(executar(&ir, 100).is_err());
    }

    #[test]
    fn limita_recursao_da_maquina() {
        let ir = compilar("funcao recursiva(n: inteiro): inteiro { devolver recursiva(n + 1) }\nvariavel x = recursiva(0)");
        assert!(executar(&ir, 10_000).is_err());
    }

    #[test]
    fn executa_intervalo_decimal() {
        let ir = compilar("variavel faixa = 1.5..3.5");
        let r = executar(&ir, 100).expect("execução válida");
        assert_eq!(r.globais.get("faixa"), Some(&ValorIr::IntervaloDecimal(1.5, 3.5)));
    }


    #[test]
    fn executa_lista_e_indexacao() {
        let ir = compilar("variavel itens = [4, 5, 6]\nvariavel primeiro = itens[0]");
        let r = executar(&ir, 100).expect("execução válida");
        assert_eq!(r.globais.get("primeiro"), Some(&ValorIr::Inteiro(4)));
    }

    #[test]
    fn concatena_texto() {
        let ir = compilar("variavel saudacao = \"Ola\" + \" Nebula\"");
        let r = executar(&ir, 100).expect("execução válida");
        assert_eq!(r.globais.get("saudacao"), Some(&ValorIr::Texto("Ola Nebula".into())));
    }


    #[test]
    fn executa_laco_para() {
        let ir = compilar("variavel soma = 0\npara (variavel i = 0; i < 3; i = i + 1) { soma = soma + i }\nexibir(soma)");
        let r = executar(&ir, 200).expect("execução válida");
        assert_eq!(r.saida, vec!["3"]);
    }

    #[test]
    fn executa_parar_e_seguir_em_lacos() {
        let ir = compilar("variavel x = 0\nenquanto x < 5 { x = x + 1\nquando x == 2 { seguir }\nquando x == 4 { parar }\nexibir(x) }");
        let r = executar(&ir, 200).expect("execução válida");
        assert_eq!(r.saida, vec!["1", "3"]);
    }


    #[test]
    fn preserva_caminho_real_do_modulo_na_ir() {
        let mut modulos = ModulosDisponiveis::new();
        modulos.insert("Matematica".into(), HashMap::from([(
            "somar".into(),
            nebula_semantica::AssinaturaFuncao {
                parametros: vec![Tipo::Inteiro, Tipo::Inteiro],
                retorno: Tipo::Inteiro,
            },
        )]));
        let fonte = ArquivoFonte::novo(
            "teste.nbx",
            "incluir Matematica como Mat\nvariavel total = Mat::somar(1, 2)",
        );
        let sintaxe = analisar(&fonte);
        assert!(sintaxe.diagnosticos.is_empty(), "{:?}", sintaxe.diagnosticos);
        let resultado = baixar(&sintaxe.programa, &modulos);
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
        let ir = resultado.programa.expect("IR válida");
        assert_eq!(ir.modulos.get("Mat").map(String::as_str), Some("Matematica"));
        let InstrucaoIr::Declarar { valor, .. } = &ir.instrucoes[0] else {
            panic!("esperava declaração global");
        };
        assert!(matches!(
            valor,
            ExpressaoIr::ChamadaQualificada { modulo, nome, .. }
                if modulo.as_str() == "Matematica" && nome.as_str() == "somar"
        ));
    }

}
}

pub use implementacao::*;

/// Passes de otimização conservadores sobre a IR tipada.
#[rustfmt::skip]
pub mod otimizador;
