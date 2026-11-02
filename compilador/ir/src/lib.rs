//! Representação intermediária tipada e interpretador de referência da Nebula.

#![forbid(unsafe_code)]
#![allow(missing_docs)]

#[rustfmt::skip]
mod implementacao {
use nebula_semantica::{analisar_com_modulos, ModulosDisponiveis, Tipo};
use nebula_sintaxe::{Expressao, NoAst, Programa, TipoToken};
use std::collections::HashMap;

/// Valor constante ou resultado de execução na IR.
#[derive(Debug, Clone, PartialEq)]
pub enum ValorIr {
    Inteiro(i128), Decimal(f64), Logico(bool), Texto(String), Caractere(char),
    Lista(Vec<ValorIr>), Intervalo(i128, i128), Nulo, Vazio,
}

/// Expressão em forma independente da AST.
#[derive(Debug, Clone, PartialEq)]
pub enum ExpressaoIr {
    Constante(ValorIr),
    Variavel(String),
    Lista(Vec<ExpressaoIr>),
    Unaria { operador: TipoToken, valor: Box<ExpressaoIr> },
    Binaria { esquerda: Box<ExpressaoIr>, operador: TipoToken, direita: Box<ExpressaoIr> },
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
    let mut lowerer = Lowerer { diagnosticos: Vec::new(), aliases };
    let mut ir = ProgramaIr { instrucoes: Vec::new(), funcoes: HashMap::new() };
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

struct Lowerer { diagnosticos: Vec<String>, aliases: HashMap<String, String> }

impl Lowerer {
    fn tipo(&self, nome: &str) -> Tipo {
        match nome {
            "vazio" => Tipo::Vazio,
            "logico" | "lógico" => Tipo::Logico,
            "inteiro" | "inteiro8" | "inteiro16" | "inteiro32" | "inteiro64" | "inteiro128" => Tipo::Inteiro,
            "byte" => Tipo::Byte,
            "decimal" | "decimal32" | "decimal64" => Tipo::Decimal,
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
                operador: *operador, valor: Box::new(self.expressao(valor)?),
            }),
            Expressao::Binaria { esquerda, operador, direita } => Some(ExpressaoIr::Binaria {
                esquerda: Box::new(self.expressao(esquerda)?),
                operador: *operador,
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
                            Some(ExpressaoIr::ChamadaQualificada { modulo: modulo.clone(), nome: nome.clone(), argumentos: args })
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

fn inferir_tipo(expressao: &ExpressaoIr) -> Tipo {
    match expressao {
        ExpressaoIr::Constante(ValorIr::Inteiro(_)) => Tipo::Inteiro,
        ExpressaoIr::Constante(ValorIr::Decimal(_)) => Tipo::Decimal,
        ExpressaoIr::Constante(ValorIr::Logico(_)) => Tipo::Logico,
        ExpressaoIr::Constante(ValorIr::Texto(_)) => Tipo::Texto,
        ExpressaoIr::Constante(ValorIr::Caractere(_)) => Tipo::Caractere,
        ExpressaoIr::Constante(_) => Tipo::Desconhecido,
        ExpressaoIr::Lista(v) => Tipo::Lista(Box::new(v.first().map_or(Tipo::Desconhecido, inferir_tipo))),
        ExpressaoIr::Binaria { esquerda, operador, .. } => match operador {
            TipoToken::Igual | TipoToken::Diferente | TipoToken::Menor | TipoToken::MenorIgual
            | TipoToken::Maior | TipoToken::MaiorIgual | TipoToken::E | TipoToken::Ou => Tipo::Logico,
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
#[must_use]
pub fn executar(programa: &ProgramaIr, limite_passos: usize) -> Result<ResultadoExecucao, String> {
    let mut vm = Maquina {
        programa,
        escopos: vec![HashMap::new()],
        saida: Vec::new(),
        passos: 0,
        limite: limite_passos.max(1),
    };
    let fluxo = vm.bloco_raiz(&programa.instrucoes)?;
    if matches!(&fluxo, Fluxo::Parar | Fluxo::Seguir) {
        return Err("Controle de laço escapou do seu contexto.".into());
    }
    Ok(ResultadoExecucao { saida: vm.saida, globais: vm.escopos.remove(0) })
}

enum Fluxo { Normal, Retorno(ValorIr), Parar, Seguir }

struct Maquina<'a> {
    programa: &'a ProgramaIr,
    escopos: Vec<HashMap<String, ValorIr>>,
    saida: Vec<String>,
    passos: usize,
    limite: usize,
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
        self.escopos.push(HashMap::new());
        let mut fluxo = Fluxo::Normal;
        for item in itens {
            self.passo()?;
            fluxo = self.instrucao(item)?;
            if !matches!(fluxo, Fluxo::Normal) { break; }
        }
        self.escopos.pop();
        Ok(fluxo)
    }

    fn instrucao(&mut self, item: &InstrucaoIr) -> Result<Fluxo, String> {
        match item {
            InstrucaoIr::Declarar { nome, valor, .. } => {
                let valor = self.expressao(valor)?;
                self.escopos.last_mut().expect("escopo ativo").insert(nome.clone(), valor);
                Ok(Fluxo::Normal)
            }
            InstrucaoIr::Expressao(e) => { self.expressao(e)?; Ok(Fluxo::Normal) }
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
                self.escopos.push(HashMap::new());
                if let Some(inicial) = inicial { let _ = self.instrucao(inicial)?; }
                loop {
                    self.passo()?;
                    if let Some(condicao) = condicao {
                        if !self.logico(self.expressao(condicao)?)? { break; }
                    }
                    match self.bloco(corpo)? {
                        Fluxo::Normal | Fluxo::Seguir => {}
                        Fluxo::Parar => break,
                        retorno @ Fluxo::Retorno(_) => { self.escopos.pop(); return Ok(retorno); }
                    }
                    if let Some(atualizacao) = atualizacao { self.expressao(atualizacao)?; }
                }
                self.escopos.pop();
                Ok(Fluxo::Normal)
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
            ExpressaoIr::Variavel(nome) => self.buscar(nome).cloned().ok_or_else(|| format!("Variável '{nome}' não encontrada na IR.")),
            ExpressaoIr::Lista(valores) => Ok(ValorIr::Lista(valores.iter().map(|v| self.expressao(v)).collect::<Result<_,_>>()?)),
            ExpressaoIr::Unaria { operador, valor } => {
                let valor = self.expressao(valor)?;
                match (operador, valor) {
                    (TipoToken::Negacao | TipoToken::Nao, ValorIr::Logico(v)) => Ok(ValorIr::Logico(!v)),
                    (TipoToken::Menos, ValorIr::Inteiro(v)) => v.checked_neg().map(ValorIr::Inteiro).ok_or_else(|| "Overflow em negação inteira.".into()),
                    (TipoToken::Menos, ValorIr::Decimal(v)) => Ok(ValorIr::Decimal(-v)),
                    (TipoToken::Mais, v @ (ValorIr::Inteiro(_) | ValorIr::Decimal(_))) => Ok(v),
                    _ => Err("Operador unário incompatível na IR.".into()),
                }
            }
            ExpressaoIr::Binaria { esquerda, operador, direita } => {
                let a = self.expressao(esquerda)?;
                if *operador == TipoToken::E && a == ValorIr::Logico(false) { return Ok(ValorIr::Logico(false)); }
                if *operador == TipoToken::Ou && a == ValorIr::Logico(true) { return Ok(ValorIr::Logico(true)); }
                let b = self.expressao(direita)?;
                binaria(a, *operador, b)
            }
            ExpressaoIr::Atribuicao { nome, valor } => {
                let valor = self.expressao(valor)?;
                for escopo in self.escopos.iter_mut().rev() {
                    if let Some(slot) = escopo.get_mut(nome) { *slot = valor.clone(); return Ok(valor); }
                }
                Err(format!("Variável '{nome}' não encontrada para atribuição."))
            }
            ExpressaoIr::Chamada { nome, argumentos } => {
                let valores = argumentos.iter().map(|a| self.expressao(a)).collect::<Result<Vec<_>,_>>()?;
                if nome == "exibir" {
                    self.saida.push(valores.iter().map(formatar).collect::<Vec<_>>().join(" "));
                    return Ok(ValorIr::Vazio);
                }
                let funcao = self.programa.funcoes.get(nome).cloned().ok_or_else(|| format!("Função '{nome}' não encontrada na IR."))?;
                if funcao.parametros.len() != valores.len() { return Err(format!("Quantidade incorreta de argumentos para '{nome}'.")); }
                self.escopos.push(HashMap::new());
                for ((parametro, _), valor) in funcao.parametros.iter().zip(valores) {
                    self.escopos.last_mut().expect("escopo ativo").insert(parametro.clone(), valor);
                }
                let fluxo = self.bloco(&funcao.corpo)?;
                self.escopos.pop();
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

    fn buscar(&self, nome: &str) -> Option<&ValorIr> {
        self.escopos.iter().rev().find_map(|e| e.get(nome))
    }

    fn logico(&self, valor: ValorIr) -> Result<bool, String> {
        if let ValorIr::Logico(v) = valor { Ok(v) } else { Err("Condição da IR não é lógica.".into()) }
    }
}

fn binaria(a: ValorIr, op: TipoToken, b: ValorIr) -> Result<ValorIr, String> {
    use TipoToken::*;
    match (a, op, b) {
        (ValorIr::Inteiro(a), Mais, ValorIr::Inteiro(b)) => a.checked_add(b).map(ValorIr::Inteiro).ok_or_else(|| "Overflow em soma inteira.".into()),
        (ValorIr::Inteiro(a), Menos, ValorIr::Inteiro(b)) => a.checked_sub(b).map(ValorIr::Inteiro).ok_or_else(|| "Overflow em subtração inteira.".into()),
        (ValorIr::Inteiro(a), Vezes, ValorIr::Inteiro(b)) => a.checked_mul(b).map(ValorIr::Inteiro).ok_or_else(|| "Overflow em multiplicação inteira.".into()),
        (ValorIr::Inteiro(_), Divide, ValorIr::Inteiro(0)) | (ValorIr::Inteiro(_), Resto, ValorIr::Inteiro(0)) => Err("Divisão por zero na IR.".into()),
        (ValorIr::Inteiro(a), Divide, ValorIr::Inteiro(b)) => a.checked_div(b).map(ValorIr::Inteiro).ok_or_else(|| "Overflow em divisão inteira.".into()),
        (ValorIr::Inteiro(a), Resto, ValorIr::Inteiro(b)) => a.checked_rem(b).map(ValorIr::Inteiro).ok_or_else(|| "Overflow em resto inteiro.".into()),
        (ValorIr::Inteiro(a), Potencia, ValorIr::Inteiro(b)) if (0..=u32::MAX as i128).contains(&b) => a.checked_pow(b as u32).map(ValorIr::Inteiro).ok_or_else(|| "Overflow em potência inteira.".into()),
        (ValorIr::Decimal(a), Mais, ValorIr::Decimal(b)) => Ok(ValorIr::Decimal(a+b)),
        (ValorIr::Decimal(a), Menos, ValorIr::Decimal(b)) => Ok(ValorIr::Decimal(a-b)),
        (ValorIr::Decimal(a), Vezes, ValorIr::Decimal(b)) => Ok(ValorIr::Decimal(a*b)),
        (ValorIr::Decimal(_), Divide, ValorIr::Decimal(b)) if b == 0.0 => Err("Divisão por zero na IR.".into()),
        (ValorIr::Decimal(a), Divide, ValorIr::Decimal(b)) => Ok(ValorIr::Decimal(a/b)),
        (ValorIr::Decimal(a), Resto, ValorIr::Decimal(b)) if b != 0.0 => Ok(ValorIr::Decimal(a % b)),
        (ValorIr::Decimal(a), Potencia, ValorIr::Decimal(b)) => Ok(ValorIr::Decimal(a.powf(b))),
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
        ValorIr::Intervalo(a,b) => format!("{a}..{b}"), ValorIr::Nulo => "nulo".into(), ValorIr::Vazio => String::new(),
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
    fn baixa_declaracoes_e_expressoes() {
        let ir = compilar("variavel total = 1 + 2");
        assert_eq!(ir.instrucoes.len(), 1);
        assert!(matches!(ir.instrucoes[0], InstrucaoIr::Declarar { .. }));
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
}
}

pub use implementacao::*;
