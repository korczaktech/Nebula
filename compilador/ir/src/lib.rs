//! Representação intermediária tipada e interpretador de referência da Nebula.

#![forbid(unsafe_code)]
#![allow(missing_docs)]

#[rustfmt::skip]
mod implementacao {
use nebula_semantica::{analisar_com_modulos, ModulosDisponiveis, Tipo};
use nebula_fonte::IntervaloFonte;
use nebula_biblioteca::dados::{analisar_json, serializar_json};
use nebula_sintaxe::{CasoEscolha, Expressao, NoAst, Programa, TipoToken};
use nebula_runtime::{
    LayoutRuntime,
    arquivo_existe, caractere_texto, comprimento_texto, copiar_arquivo, criar_diretorio,
    diretorio_existe, escrever_arquivo, listar_diretorio, mover_arquivo, remover_arquivo,
    calcular_matematica, dormir_milisegundos, ler_arquivo, operar_decimal, operar_inteiro,
    subtexto, tempo_unix_milisegundos, texto_aparar, texto_comeca_com, texto_contem,
    texto_maiusculas, texto_minusculas, texto_substituir, texto_termina_com,
    texto_para_decimal, texto_para_inteiro, ContextoRuntime, ErroRuntime, FuncaoMatematica, Memoria,
    OperacaoDecimal, OperacaoInteira, Valor as ValorRuntime,
};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Valor constante ou resultado de execução na IR.
#[derive(Debug, Clone, PartialEq)]
pub enum ValorIr {
    Inteiro(i128), InteiroSemSinal(u128), Decimal(f64), Logico(bool), Texto(String), Caractere(char), Funcao(String),
    Lista(Vec<ValorIr>), Estrutura { nome: String, campos: Vec<(String, ValorIr)> }, Intervalo(i128, i128), IntervaloSemSinal(u128, u128), IntervaloDecimal(f64, f64), Nulo, Vazio,
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
    Normalizar { tipo: Tipo, valor: Box<ExpressaoIr> },
    Conversao { tipo: Tipo, origem: Tipo, valor: Box<ExpressaoIr> },
    ReferenciaFuncao { nome: String, tipo: Tipo },
    ReferenciaFuncaoQualificada { modulo: String, nome: String, tipo: Tipo },
    ChamadaIndireta { alvo: Box<ExpressaoIr>, argumentos: Vec<ExpressaoIr> },
    Variavel(String),
    Lista(Vec<ExpressaoIr>), Tupla(Vec<ExpressaoIr>),
    Unaria { operador: OperadorIr, valor: Box<ExpressaoIr> },
    Binaria { esquerda: Box<ExpressaoIr>, operador: OperadorIr, direita: Box<ExpressaoIr> },
    Atribuicao { nome: String, valor: Box<ExpressaoIr>, tipo: Tipo },
    AtribuicaoCampo { raiz: String, caminho: Vec<String>, valor: Box<ExpressaoIr>, tipo: Tipo },
    Chamada { nome: String, argumentos: Vec<ExpressaoIr> },
    ChamadaQualificada { modulo: String, nome: String, argumentos: Vec<ExpressaoIr> },
    Indexacao { alvo: Box<ExpressaoIr>, indice: Box<ExpressaoIr> },
    Membro { alvo: Box<ExpressaoIr>, campo: String },
    ConstrucaoEstrutura { nome: String, campos: Vec<(String, ExpressaoIr)> },
    ConstrucaoEnumeracao { nome: String, variante: String, valores: Vec<ExpressaoIr> },
}

/// Instrução tipada da IR.
#[derive(Debug, Clone, PartialEq)]
pub enum InstrucaoIr {
    Declarar { nome: String, tipo: Tipo, valor: ExpressaoIr, constante: bool },
    Expressao(ExpressaoIr),
    Bloco(Vec<InstrucaoIr>),
    Se { condicao: ExpressaoIr, entao: Vec<InstrucaoIr>, senao: Vec<InstrucaoIr> },
    Escolha { valor: ExpressaoIr, casos: Vec<CasoEscolhaIr> },
    Enquanto { condicao: ExpressaoIr, corpo: Vec<InstrucaoIr> },
    Cada { nome: String, iteravel: ExpressaoIr, corpo: Vec<InstrucaoIr> },
    Pulso { intervalo: ExpressaoIr, corpo: Vec<InstrucaoIr> },
    Para { inicial: Option<Box<InstrucaoIr>>, condicao: Option<ExpressaoIr>, atualizacao: Option<ExpressaoIr>, corpo: Vec<InstrucaoIr> },
    Devolver(Option<ExpressaoIr>),
    Parar,
    Seguir,
    Localizada { caminho: PathBuf, intervalo: IntervaloFonte, instrucao: Box<InstrucaoIr> },
}

/// Caso de enumeração convertido para IR.
#[derive(Debug, Clone, PartialEq)]
pub struct CasoEscolhaIr { pub enumeracao: Option<String>, pub variante: Option<String>, pub vinculacoes: Vec<String>, pub corpo: Vec<InstrucaoIr> }
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
    /// Layouts físicos validados das estruturas e enumerações declaradas.
    pub layouts_tipos: HashMap<String, LayoutRuntime>,
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
    let assinaturas_funcoes = programa.itens.iter().filter_map(|item| {
        if let NoAst::Funcao { nome, parametros, retorno, .. } = item {
            Some((nome.clone(), (parametros.iter().map(|(_, tipo)| tipo.clone()).collect::<Vec<_>>(), retorno.clone())))
        } else { None }
    }).collect::<HashMap<_, _>>();
    let genericas = programa.itens.iter().filter_map(|item| {
        if let NoAst::Funcao { nome, genericos, parametros, retorno, corpo, .. } = item {
            if !genericos.is_empty() {
                return Some((nome.clone(), FuncaoGenericaAst {
                    genericos: genericos.clone(),
                    parametros: parametros.clone(),
                    retorno: retorno.clone(),
                    corpo: corpo.clone(),
                }));
            }
        }
        None
    }).collect();
    let mut lowerer = Lowerer {
        diagnosticos: Vec::new(),
        aliases,
        tipos_agregados: programa.itens.iter().filter_map(|item| match item {
            NoAst::Estrutura { nome, .. } => Some((nome.clone(), Tipo::Estrutura(nome.clone()))),
            NoAst::Enumeracao { nome, .. } => Some((nome.clone(), Tipo::Enumeracao(nome.clone()))),
            _ => None,
        }).collect(),
        campos_estruturas: programa.itens.iter().filter_map(|item| match item {
            NoAst::Estrutura { nome, campos } => Some((nome.clone(), campos.iter().cloned().collect())),
            _ => None,
        }).collect(),
        variantes_enumeracoes: programa.itens.iter().filter_map(|item| match item {
            NoAst::Enumeracao { nome, variantes } => Some((nome.clone(), variantes.iter().map(|(variante, payload)| (variante.clone(), payload.clone())).collect())),
            _ => None,
        }).collect(),
        assinaturas_funcoes,
        modulos_importados: modulos_importados.clone(),
        genericas,
        instancias: HashMap::new(),
        fila_instancias: Vec::new(),
        escopos_tipos: vec![HashMap::new()],
        substituicoes: HashMap::new(),
        localizacoes: programa.nos_localizados.iter().fold(HashMap::<String, VecDeque<IntervaloFonte>>::new(), |mut mapa, localizado| {
            mapa.entry(format!("{:?}", localizado.no)).or_default().push_back(localizado.intervalo);
            mapa
        }),
        caminho_fonte: programa.caminho.clone().unwrap_or_default(),
    };
    let mut layouts_tipos = HashMap::new();
    for item in &programa.itens {
        let tipo = match item {
            NoAst::Estrutura { nome, .. } => Some((nome, Tipo::Estrutura(nome.clone()))),
            NoAst::Enumeracao { nome, .. } => Some((nome, Tipo::Enumeracao(nome.clone()))),
            _ => None,
        };
        if let Some((nome, tipo)) = tipo {
            match crate::layout::calcular_layout_tipo(programa, &tipo) {
                Ok(layout) => { layouts_tipos.insert(nome.clone(), layout); }
                Err(erro) => lowerer.diagnosticos.push(format!(
                    "Não foi possível definir o layout de '{nome}': {erro}"
                )),
            }
        }
    }
    let mut ir = ProgramaIr {
        instrucoes: Vec::new(),
        funcoes: HashMap::new(),
        modulos: modulos_importados,
        layouts_tipos,
    };
    for item in &programa.itens {
        match item {
            NoAst::Funcao { nome, genericos, parametros, retorno, corpo, .. } if genericos.is_empty() => {
                let tipos_parametros: Vec<_> = parametros.iter()
                    .map(|(n, t)| (n.clone(), lowerer.tipo(t)))
                    .collect();
                lowerer.escopos_tipos.push(tipos_parametros.iter()
                    .map(|(nome, tipo)| (nome.clone(), tipo.clone()))
                    .collect());
                let corpo = lowerer.bloco(corpo);
                lowerer.escopos_tipos.pop();
                let funcao = FuncaoIr {
                    nome: nome.clone(),
                    parametros: tipos_parametros,
                    retorno: retorno.as_deref().map_or(Tipo::Vazio, |t| lowerer.tipo(t)),
                    corpo,
                };
                ir.funcoes.insert(nome.clone(), funcao);
            }
            NoAst::Funcao { .. } | NoAst::Tipo { .. } | NoAst::Classe { .. } | NoAst::Estrutura { .. } | NoAst::Enumeracao { .. } | NoAst::Incluir { .. } => {}
            _ => if let Some(instrucao) = lowerer.item(item) { ir.instrucoes.push(instrucao); },
        }
    }
    while let Some((nome_instancia, nome_generica, substituicoes)) = lowerer.fila_instancias.pop() {
        let Some(modelo) = lowerer.genericas.get(&nome_generica).cloned() else { continue; };
        let substituicoes_anteriores = std::mem::replace(&mut lowerer.substituicoes, substituicoes);
        let parametros: Vec<_> = modelo.parametros.iter()
            .map(|(nome, tipo)| (nome.clone(), lowerer.tipo(tipo)))
            .collect();
        lowerer.escopos_tipos.push(parametros.iter()
            .map(|(nome, tipo)| (nome.clone(), tipo.clone()))
            .collect());
        let corpo = lowerer.bloco(&modelo.corpo);
        lowerer.escopos_tipos.pop();
        let retorno = modelo.retorno.as_deref().map_or(Tipo::Vazio, |tipo| lowerer.tipo(tipo));
        lowerer.substituicoes = substituicoes_anteriores;
        if let Some(funcao) = lowerer.instancias.get_mut(&nome_instancia) {
            funcao.parametros = parametros;
            funcao.retorno = retorno;
            funcao.corpo = corpo;
        }
    }
    ir.funcoes.extend(lowerer.instancias);
    ResultadoIr {
        programa: if lowerer.diagnosticos.is_empty() { Some(ir) } else { None },
        diagnosticos: lowerer.diagnosticos,
    }
}

#[derive(Clone)]
struct FuncaoGenericaAst {
    genericos: Vec<String>,
    parametros: Vec<(String, String)>,
    retorno: Option<String>,
    corpo: Vec<NoAst>,
}

struct Lowerer {
    diagnosticos: Vec<String>,
    aliases: HashMap<String, String>,
    tipos_agregados: HashMap<String, Tipo>,
    campos_estruturas: HashMap<String, HashMap<String, String>>,
    variantes_enumeracoes: HashMap<String, HashMap<String, Vec<String>>>,
    assinaturas_funcoes: HashMap<String, (Vec<String>, Option<String>)>,
    modulos_importados: HashMap<String, String>,
    genericas: HashMap<String, FuncaoGenericaAst>,
    instancias: HashMap<String, FuncaoIr>,
    fila_instancias: Vec<(String, String, HashMap<String, String>)>,
    escopos_tipos: Vec<HashMap<String, Tipo>>,
    substituicoes: HashMap<String, String>,
    localizacoes: HashMap<String, VecDeque<IntervaloFonte>>,
    caminho_fonte: PathBuf,
}

impl Lowerer {
    fn tipo(&self, nome: &str) -> Tipo {
        if let Some(substituido) = self.substituicoes.get(nome) {
            return self.tipo(substituido);
        }
        match nome {
            "vazio" => Tipo::Vazio,
            "logico" | "lógico" => Tipo::Logico,
            "inteiro" => Tipo::Inteiro,
            "inteiro8" => Tipo::Inteiro8,
            "inteiro16" => Tipo::Inteiro16,
            "inteiro32" => Tipo::Inteiro32,
            "inteiro64" => Tipo::Inteiro64,
            "inteiro128" => Tipo::Inteiro128,
            "inteiro_u8" => Tipo::InteiroU8,
            "inteiro_u16" => Tipo::InteiroU16,
            "inteiro_u32" => Tipo::InteiroU32,
            "inteiro_u64" => Tipo::InteiroU64,
            "inteiro_u128" => Tipo::InteiroU128,
            "byte" => Tipo::Byte,
            "decimal" | "decimal64" => Tipo::Decimal64,
            "decimal32" => Tipo::Decimal32,
            "texto" => Tipo::Texto,
            "caractere" => Tipo::Caractere,
            _ if dividir_tipo_funcao(nome).is_some() => {
                let (parametros, retorno) = dividir_tipo_funcao(nome)
                    .expect("tipo de função reconhecido");
                Tipo::Funcao(
                    parametros.iter().map(|tipo| self.tipo(tipo)).collect(),
                    Box::new(self.tipo(retorno)),
                )
            }
            _ if nome.starts_with('[') && nome.ends_with(']') => Tipo::Lista(Box::new(self.tipo(&nome[1..nome.len()-1]))),
            _ if dividir_tipo_generico(nome).is_some() => {
                let (base, argumentos) = dividir_tipo_generico(nome).expect("tipo genérico validado");
                let tipos: Vec<_> = argumentos.iter().map(|tipo| self.tipo(tipo)).collect();
                match (base, tipos.as_slice()) {
                    ("Opcional" | "opcional", [tipo]) => Tipo::Opcional(Box::new(tipo.clone())),
                    ("Intervalo" | "intervalo", [tipo]) => Tipo::Intervalo(Box::new(tipo.clone())),
                    ("Resultado" | "resultado", [ok, erro]) => Tipo::Resultado(Box::new(ok.clone()), Box::new(erro.clone())),
                    ("Tupla" | "tupla", tipos) if !tipos.is_empty() => Tipo::Tupla(tipos.to_vec()),
                    ("Vetor" | "vetor", [elemento]) => Tipo::Lista(Box::new(elemento.clone())),
                    ("Matriz" | "matriz", [elemento]) => Tipo::Lista(Box::new(Tipo::Lista(Box::new(elemento.clone())))),
                    _ => Tipo::Desconhecido,
                }
            }
            _ => self.tipos_agregados.get(nome).cloned().unwrap_or_else(|| self.aliases.get(nome).map_or(Tipo::Desconhecido, |destino| self.tipo(destino))),
        }
    }

    fn bloco(&mut self, itens: &[NoAst]) -> Vec<InstrucaoIr> {
        self.escopos_tipos.push(HashMap::new());
        let resultado = itens.iter().filter_map(|item| self.item(item)).collect();
        self.escopos_tipos.pop();
        resultado
    }

    fn item(&mut self, item: &NoAst) -> Option<InstrucaoIr> {
        let instrucao = match item {
            NoAst::Variavel { nome, tipo, valor, constante } => {
                let tipo_inferido = if tipo.is_none() { self.inferir_tipo_ast(valor) } else { Tipo::Desconhecido };
                let mut valor = self.expressao(valor)?;
                let tipo = tipo.as_deref().map_or(tipo_inferido, |t| self.tipo(t));
                if matches!(tipo,
                    Tipo::Inteiro128 | Tipo::InteiroU128 | Tipo::Estrutura(_) | Tipo::Tupla(_)
                    | Tipo::Enumeracao(_) | Tipo::Opcional(_) | Tipo::Resultado(_, _)
                ) {
                    let valor_interno = match valor {
                        ExpressaoIr::Normalizar { valor, .. } => *valor,
                        outro => outro,
                    };
                    valor = ExpressaoIr::Normalizar {
                        tipo: tipo.clone(),
                        valor: Box::new(valor_interno),
                    };
                }
                if let Some(escopo) = self.escopos_tipos.last_mut() {
                    escopo.insert(nome.clone(), tipo.clone());
                }
                Some(InstrucaoIr::Declarar { nome: nome.clone(), tipo, valor, constante: *constante })
            }
            NoAst::Expressao(e) => Some(InstrucaoIr::Expressao(self.expressao(e)?)),
            NoAst::Quando { condicao, entao, senao } => Some(InstrucaoIr::Se {
                condicao: self.expressao(condicao)?,
                entao: self.bloco(entao),
                senao: self.bloco(senao),
            }),
            NoAst::Escolha { valor, casos } => Some(InstrucaoIr::Escolha {
                valor: self.expressao(valor)?,
                casos: {
                    let tipo_escolhido = self.inferir_tipo_ast(valor);
                    casos.iter().map(|caso: &CasoEscolha| {
                    self.escopos_tipos.push(HashMap::new());
                    if let (Some(enumeracao), Some(variante)) = (&caso.enumeracao, &caso.variante) {
                        let tipos = match (&tipo_escolhido, enumeracao.as_str(), variante.as_str()) {
                            (Tipo::Opcional(tipo), "Opcional", "Algum") => Some(vec![(**tipo).clone()]),
                            (Tipo::Opcional(_), "Opcional", "Nenhum") => Some(Vec::new()),
                            (Tipo::Resultado(ok, _), "Resultado", "Sucesso") => Some(vec![(**ok).clone()]),
                            (Tipo::Resultado(_, erro), "Resultado", "Falha") => Some(vec![(**erro).clone()]),
                            _ => self.variantes_enumeracoes.get(enumeracao)
                                .and_then(|variantes| variantes.get(variante))
                                .map(|tipos| tipos.iter().map(|tipo| self.tipo(tipo)).collect()),
                        };
                        if let Some(tipos) = tipos {
                            for (nome, tipo) in caso.vinculacoes.iter().zip(tipos) {
                                if let Some(escopo) = self.escopos_tipos.last_mut() {
                                    escopo.insert(nome.clone(), tipo);
                                }
                            }
                        }
                    }
                    let corpo = self.bloco(&caso.corpo);
                    self.escopos_tipos.pop();
                    Some(CasoEscolhaIr {
                        enumeracao: caso.enumeracao.clone(),
                        variante: caso.variante.clone(),
                        vinculacoes: caso.vinculacoes.clone(),
                        corpo,
                    })
                }).collect::<Option<Vec<_>>>()?
                },
            }),
            NoAst::Enquanto { condicao, corpo } => Some(InstrucaoIr::Enquanto {
                condicao: self.expressao(condicao)?, corpo: self.bloco(corpo),
            }),
            NoAst::Cada { nome, iteravel, corpo } => {
                let tipo_elemento = match self.inferir_tipo_ast(iteravel) {
                    Tipo::Lista(tipo) | Tipo::Intervalo(tipo) => *tipo,
                    Tipo::Texto => Tipo::Caractere,
                    _ => Tipo::Desconhecido,
                };
                let iteravel_ir = self.expressao(iteravel)?;
                self.escopos_tipos.push(HashMap::from([(nome.clone(), tipo_elemento)]));
                let corpo_ir = self.bloco(corpo);
                self.escopos_tipos.pop();
                Some(InstrucaoIr::Cada {
                    nome: nome.clone(),
                    iteravel: iteravel_ir,
                    corpo: corpo_ir,
                })
            },
            NoAst::Pulso { intervalo, corpo } => Some(InstrucaoIr::Pulso {
                intervalo: self.expressao(intervalo)?,
                corpo: self.bloco(corpo),
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
            NoAst::Passar | NoAst::Ceder(_) | NoAst::BlocoContextual { .. }
            | NoAst::Tente { .. } | NoAst::Com { .. } | NoAst::Lancar(_) | NoAst::Verifique { .. } | NoAst::Escopo { .. } => None,
            NoAst::Atributada { declaracao, .. } => self.item(declaracao),
            NoAst::Tipo { .. } | NoAst::Classe { .. } | NoAst::Estrutura { .. } | NoAst::Enumeracao { .. } | NoAst::Incluir { .. } | NoAst::Funcao { .. } => None,
        };
        instrucao.map(|instrucao| {
            let chave = format!("{:?}", item);
            match self.localizacoes.get_mut(&chave).and_then(VecDeque::pop_front) {
                Some(intervalo) => InstrucaoIr::Localizada { caminho: self.caminho_fonte.clone(), intervalo, instrucao: Box::new(instrucao) },
                None => instrucao,
            }
        })
    }

    fn registrar_instancia_generica(
        &mut self,
        nome: &str,
        argumentos: &[Expressao],
    ) -> Option<String> {
        let modelo = self.genericas.get(nome)?.clone();
        if argumentos.len() != modelo.parametros.len() {
            self.diagnosticos.push(format!(
                "A função genérica '{nome}' espera {} argumentos, mas recebeu {}.",
                modelo.parametros.len(),
                argumentos.len()
            ));
            return None;
        }

        let mut substituicoes = HashMap::new();
        for ((_, tipo_esperado), argumento) in modelo.parametros.iter().zip(argumentos) {
            let obtido = self.inferir_tipo_ast(argumento);
            if obtido == Tipo::Desconhecido {
                self.diagnosticos.push(format!(
                    "Não foi possível inferir o tipo do argumento na chamada genérica de '{nome}'."
                ));
                return None;
            }
            if !self.inferir_substituicao(tipo_esperado, &obtido, &modelo.genericos, &mut substituicoes) {
                self.diagnosticos.push(format!(
                    "Tipos genéricos conflitantes na chamada de '{nome}': esperado '{tipo_esperado}', recebido '{}'.",
                    nome_tipo(&obtido)
                ));
                return None;
            }
        }

        for parametro in &modelo.genericos {
            if !substituicoes.contains_key(parametro) {
                self.diagnosticos.push(format!(
                    "O parâmetro genérico '{parametro}' da função '{nome}' não pode ser inferido."
                ));
                return None;
            }
        }

        let mut partes_sufixo = Vec::new();
        for parametro in &modelo.genericos {
            partes_sufixo.push(nome_tipo(&self.tipo(substituicoes.get(parametro)?)));
        }
        let sufixo = partes_sufixo.join("__");
        let nome_instancia = format!("{nome}__{}", sufixo.chars()
            .map(|caractere| if caractere.is_ascii_alphanumeric() || caractere == '_' { caractere } else { '_' })
            .collect::<String>());

        if !self.instancias.contains_key(&nome_instancia) {
            let anteriores = std::mem::replace(&mut self.substituicoes, substituicoes.clone());
            let parametros: Vec<_> = modelo.parametros.iter()
                .map(|(nome, tipo)| (nome.clone(), self.tipo(tipo)))
                .collect();
            let retorno = modelo.retorno.as_deref().map_or(Tipo::Vazio, |tipo| self.tipo(tipo));
            self.substituicoes = anteriores;
            self.instancias.insert(nome_instancia.clone(), FuncaoIr {
                nome: nome_instancia.clone(),
                parametros,
                retorno,
                corpo: Vec::new(),
            });
            self.fila_instancias.push((nome_instancia.clone(), nome.to_owned(), substituicoes));
        }
        Some(nome_instancia)
    }

    fn inferir_tipo_ast(&mut self, expressao: &Expressao) -> Tipo {
        match expressao {
            Expressao::Literal(valor) => literal(valor).as_ref().map_or(Tipo::Desconhecido, |valor| {
                inferir_tipo(&ExpressaoIr::Constante(valor.clone()))
            }),
            Expressao::Nome(nome) => self.escopos_tipos.iter().rev()
                .find_map(|escopo| escopo.get(nome).cloned())
                .or_else(|| self.assinaturas_funcoes.get(nome).and_then(|(parametros, retorno)| {
                    (!self.genericas.contains_key(nome)).then(|| Tipo::Funcao(
                        parametros.iter().map(|tipo| self.tipo(tipo)).collect(),
                        Box::new(retorno.as_deref().map_or(Tipo::Vazio, |tipo| self.tipo(tipo))),
                    ))
                }))
                .unwrap_or(Tipo::Desconhecido),
            Expressao::Lista(itens) => {
                let Some(primeiro) = itens.first() else { return Tipo::Lista(Box::new(Tipo::Desconhecido)); };
                let tipo = self.inferir_tipo_ast(primeiro);
                Tipo::Lista(Box::new(tipo))
            }
            Expressao::Tupla(itens) => {
                let mut tipos = Vec::with_capacity(itens.len());
                for item in itens { tipos.push(self.inferir_tipo_ast(item)); }
                Tipo::Tupla(tipos)
            }
            Expressao::ConstrucaoEstrutura { nome, .. } => self.tipo(nome),
            Expressao::Membro { alvo, operador: TipoToken::Ponto, nome } => {
                let Tipo::Estrutura(nome_estrutura) = self.inferir_tipo_ast(alvo) else { return Tipo::Desconhecido; };
                self.campos_estruturas.get(&nome_estrutura)
                    .and_then(|campos| campos.get(nome))
                    .map_or(Tipo::Desconhecido, |tipo| self.tipo(tipo))
            }
            Expressao::Membro { alvo, operador: TipoToken::Escopo, nome } => {
                if let Expressao::Nome(tipo_enum) = alvo.as_ref() {
                    if self.variantes_enumeracoes.get(tipo_enum)
                        .and_then(|variantes| variantes.get(nome)).is_some_and(Vec::is_empty) {
                        return Tipo::Enumeracao(tipo_enum.clone());
                    }
                }
                Tipo::Desconhecido
            }
            Expressao::Membro { .. } => Tipo::Desconhecido,
            Expressao::Unaria { valor, .. } => self.inferir_tipo_ast(valor),
            Expressao::Binaria { esquerda, operador, direita } => {
                let a = self.inferir_tipo_ast(esquerda);
                let b = self.inferir_tipo_ast(direita);
                if matches!(operador, TipoToken::Igual | TipoToken::Diferente | TipoToken::Menor
                    | TipoToken::MenorIgual | TipoToken::Maior | TipoToken::MaiorIgual
                    | TipoToken::E | TipoToken::Ou) {
                    Tipo::Logico
                } else if *operador == TipoToken::Intervalo {
                    Tipo::Intervalo(Box::new(a))
                } else if a == Tipo::Texto && b == Tipo::Texto && *operador == TipoToken::Mais {
                    Tipo::Texto
                } else if tipo_numerico(&a) && tipo_numerico(&b) {
                    promover_tipo_numerico(&a, &b)
                } else {
                    a
                }
            }
            Expressao::Indexacao { alvo, indice } => match self.inferir_tipo_ast(alvo) {
                Tipo::Lista(tipo) => *tipo,
                Tipo::Tupla(tipos) => {
                    let Expressao::Literal(bruto) = indice.as_ref() else { return Tipo::Desconhecido; };
                    let Some(ValorIr::Inteiro(posicao)) = literal(bruto) else { return Tipo::Desconhecido; };
                    usize::try_from(posicao).ok().and_then(|posicao| tipos.get(posicao).cloned())
                        .unwrap_or(Tipo::Desconhecido)
                }
                _ => Tipo::Desconhecido,
            },
            Expressao::Chamada { alvo, argumentos, .. } => match alvo.as_ref() {
                Expressao::Nome(nome) => {
                    if nome == "copiar" && argumentos.len() == 1 {
                        self.inferir_tipo_ast(&argumentos[0])
                    } else if let Some(Tipo::Funcao(_, retorno)) = self.escopos_tipos.iter().rev()
                        .find_map(|escopo| escopo.get(nome))
                    {
                        (**retorno).clone()
                    } else {
                        tipo_conversao_explicita(nome)
                            .unwrap_or_else(|| self.inferir_tipo_chamada(nome, argumentos))
                    }
                },
                Expressao::Membro { alvo, operador: TipoToken::Escopo, nome: variante } => {
                    let Expressao::Nome(tipo_base) = alvo.as_ref() else { return Tipo::Desconhecido; };
                    if self.variantes_enumeracoes.contains_key(tipo_base) {
                        return Tipo::Enumeracao(tipo_base.clone());
                    }
                    match (tipo_base.as_str(), variante.as_str(), argumentos.len()) {
                        ("Opcional" | "opcional", "Algum", 1) => {
                            Tipo::Opcional(Box::new(self.inferir_tipo_ast(&argumentos[0])))
                        }
                        ("Opcional" | "opcional", "Nenhum", 0) => {
                            Tipo::Opcional(Box::new(Tipo::Desconhecido))
                        },
                        ("Resultado" | "resultado", "Sucesso", 1) => Tipo::Resultado(
                            Box::new(self.inferir_tipo_ast(&argumentos[0])), Box::new(Tipo::Desconhecido)
                        ),
                        ("Resultado" | "resultado", "Falha", 1) => Tipo::Resultado(
                            Box::new(Tipo::Desconhecido), Box::new(self.inferir_tipo_ast(&argumentos[0]))
                        ),
                        _ => Tipo::Desconhecido,
                    }
                }
                _ => Tipo::Desconhecido,
            },
            _ => Tipo::Desconhecido,
        }
    }

    fn inferir_tipo_chamada(&mut self, nome: &str, argumentos: &[Expressao]) -> Tipo {
        match nome {
            "receber" | "perguntar" | "ler_texto" => return Tipo::Texto,
            "dormir" | "aguardar" | "exibir" | "adicionar" => return Tipo::Vazio,
            "tempo" | "comprimento" => return Tipo::Inteiro,
            _ => {}
        }

        if nome == "em_paralelo" && !argumentos.is_empty() {
            let mut retorno_comum: Option<Tipo> = None;
            for argumento in argumentos {
                let Tipo::Funcao(parametros, retorno) = self.inferir_tipo_ast(argumento) else {
                    return Tipo::Desconhecido;
                };
                if !parametros.is_empty() {
                    return Tipo::Desconhecido;
                }
                if retorno_comum.as_ref().is_some_and(|anterior| *retorno != *anterior) {
                    return Tipo::Desconhecido;
                }
                retorno_comum = Some(*retorno);
            }
            return Tipo::Lista(Box::new(retorno_comum.unwrap_or(Tipo::Desconhecido)));
        }
        if nome == "gerar" && argumentos.len() == 2 {
            return Tipo::Lista(Box::new(self.inferir_tipo_ast(&argumentos[0])));
        }
        if let Some(modelo) = self.genericas.get(nome).cloned() {
            if argumentos.len() != modelo.parametros.len() { return Tipo::Desconhecido; }
            let mut substituicoes = HashMap::new();
            for ((_, esperado), argumento) in modelo.parametros.iter().zip(argumentos) {
                let obtido = self.inferir_tipo_ast(argumento);
                if obtido == Tipo::Desconhecido
                    || !self.inferir_substituicao(esperado, &obtido, &modelo.genericos, &mut substituicoes) {
                    return Tipo::Desconhecido;
                }
            }
            let Some(retorno) = modelo.retorno.as_deref() else { return Tipo::Vazio; };
            let anterior = std::mem::replace(&mut self.substituicoes, substituicoes);
            let tipo = self.tipo(retorno);
            self.substituicoes = anterior;
            return tipo;
        }
        let Some((_, retorno)) = self.assinaturas_funcoes.get(nome).cloned() else {
            return Tipo::Desconhecido;
        };
        retorno.as_deref().map_or(Tipo::Vazio, |tipo| self.tipo(tipo))
    }

    fn inferir_substituicao(
        &self,
        esperado: &str,
        obtido: &Tipo,
        genericos: &[String],
        substituicoes: &mut HashMap<String, String>,
    ) -> bool {
        // Cada unificação composta é atômica: falhas em tuplas, funções ou
        // tipos genéricos não podem deixar vínculos parciais no chamador.
        let mut tentativa = substituicoes.clone();
        if self.inferir_substituicao_interno(
            esperado,
            obtido,
            genericos,
            &mut tentativa,
        ) {
            *substituicoes = tentativa;
            true
        } else {
            false
        }
    }

    fn inferir_substituicao_interno(
        &self,
        esperado: &str,
        obtido: &Tipo,
        genericos: &[String],
        substituicoes: &mut HashMap<String, String>,
    ) -> bool {
        if genericos.iter().any(|parametro| parametro == esperado) {
            let nome = nome_tipo(obtido);
            return match substituicoes.get(esperado) {
                Some(anterior) => self.tipo(anterior) == *obtido,
                None => {
                    substituicoes.insert(esperado.to_owned(), nome);
                    true
                }
            };
        }
        if esperado.starts_with('[') && esperado.ends_with(']') {
            return matches!(obtido, Tipo::Lista(interno)
                if self.inferir_substituicao(&esperado[1..esperado.len()-1], interno, genericos, substituicoes));
        }
        if let Some((parametros_esperados, retorno_esperado)) = dividir_tipo_funcao(esperado) {
            let Tipo::Funcao(parametros_obtidos, retorno_obtido) = obtido else {
                return false;
            };
            return parametros_esperados.len() == parametros_obtidos.len()
                && parametros_esperados.iter().zip(parametros_obtidos).all(|(esperado, obtido)|
                    self.inferir_substituicao(esperado, obtido, genericos, substituicoes))
                && self.inferir_substituicao(retorno_esperado, retorno_obtido, genericos, substituicoes);
        }
        if let Some((base, argumentos)) = dividir_tipo_generico(esperado) {
            return match (base, obtido) {
                ("Opcional" | "opcional", Tipo::Opcional(interno)) if argumentos.len() == 1 =>
                    self.inferir_substituicao(argumentos[0], interno, genericos, substituicoes),
                ("Vetor" | "vetor", Tipo::Lista(interno)) if argumentos.len() == 1 =>
                    self.inferir_substituicao(argumentos[0], interno, genericos, substituicoes),
                ("Matriz" | "matriz", Tipo::Lista(linha)) if argumentos.len() == 1 => {
                    matches!(linha.as_ref(), Tipo::Lista(interno)
                        if self.inferir_substituicao(argumentos[0], interno, genericos, substituicoes))
                }
                ("Resultado" | "resultado", Tipo::Resultado(ok, erro)) if argumentos.len() == 2 =>
                    self.inferir_substituicao(argumentos[0], ok, genericos, substituicoes)
                    && self.inferir_substituicao(argumentos[1], erro, genericos, substituicoes),
                ("Tupla" | "tupla", Tipo::Tupla(tipos)) if argumentos.len() == tipos.len() =>
                    argumentos.iter().zip(tipos).all(|(esperado, obtido)|
                        self.inferir_substituicao(esperado, obtido, genericos, substituicoes)),
                _ => false,
            };
        }
        self.tipo(esperado) == *obtido
    }

    fn expressao(&mut self, expressao: &Expressao) -> Option<ExpressaoIr> {
        let tipo = self.inferir_tipo_ast(expressao);
        let valor = self.expressao_sem_tipo(expressao)?;
        if matches!(
            tipo,
            Tipo::Inteiro | Tipo::Inteiro8 | Tipo::Inteiro16 | Tipo::Inteiro32
                | Tipo::Inteiro64 | Tipo::Inteiro128 | Tipo::InteiroU128 | Tipo::InteiroU8 | Tipo::InteiroU16
                | Tipo::InteiroU32 | Tipo::InteiroU64 | Tipo::Byte | Tipo::Decimal32
                | Tipo::Estrutura(_) | Tipo::Tupla(_) | Tipo::Enumeracao(_)
                | Tipo::Opcional(_) | Tipo::Resultado(_, _)
        ) {
            Some(ExpressaoIr::Normalizar {
                tipo,
                valor: Box::new(valor),
            })
        } else {
            Some(valor)
        }
    }

    fn expressao_sem_tipo(&mut self, expressao: &Expressao) -> Option<ExpressaoIr> {
        match expressao {
            Expressao::Nome(nome) => {
                if self.escopos_tipos.iter().rev().any(|escopo| escopo.contains_key(nome)) {
                    Some(ExpressaoIr::Variavel(nome.clone()))
                } else if let Some((parametros, retorno)) = self.assinaturas_funcoes.get(nome).cloned() {
                    if self.genericas.contains_key(nome) {
                        self.diagnosticos.push(format!(
                            "A função genérica '{nome}' precisa de uma instância concreta antes de ser usada como valor."
                        ));
                        None
                    } else {
                        let tipo = Tipo::Funcao(
                            parametros.iter().map(|tipo| self.tipo(tipo)).collect(),
                            Box::new(retorno.as_deref().map_or(Tipo::Vazio, |tipo| self.tipo(tipo))),
                        );
                        Some(ExpressaoIr::ReferenciaFuncao { nome: nome.clone(), tipo })
                    }
                } else {
                    Some(ExpressaoIr::Variavel(nome.clone()))
                }
            },
            Expressao::Literal(valor) => Some(ExpressaoIr::Constante(literal(valor)?)),
            Expressao::ArgumentoNomeado { valor, .. } => self.expressao(valor),
            Expressao::Lista(valores) => Some(ExpressaoIr::Lista(
                valores.iter().map(|v| self.expressao(v)).collect::<Option<Vec<_>>>()?,
            )),
            Expressao::Tupla(valores) => Some(ExpressaoIr::Tupla(
                valores.iter().map(|v| self.expressao(v)).collect::<Option<Vec<_>>>()?,
            )),
            Expressao::ConstrucaoEstrutura { nome, campos } => Some(ExpressaoIr::ConstrucaoEstrutura {
                nome: nome.clone(),
                campos: campos.iter().map(|(campo, valor)| Some((campo.clone(), self.expressao(valor)?))).collect::<Option<Vec<_>>>()?,
            }),
            Expressao::Unaria { operador, valor } => {
                let valor_ir = if *operador == TipoToken::Menos {
                    match valor.as_ref() {
                        Expressao::Literal(bruto) => match literal(bruto) {
                            Some(valor @ ValorIr::Inteiro(_)) => ExpressaoIr::Constante(valor),
                            _ => self.expressao(valor)?,
                        },
                        _ => self.expressao(valor)?,
                    }
                } else {
                    self.expressao(valor)?
                };
                Some(ExpressaoIr::Unaria {
                    operador: converter_operador(*operador)?,
                    valor: Box::new(valor_ir),
                })
            },
            Expressao::Binaria { esquerda, operador, direita } => {
                let tipo_esquerda = self.inferir_tipo_ast(esquerda);
                let tipo_direita = self.inferir_tipo_ast(direita);
                let tipo_operacao = if tipo_numerico(&tipo_esquerda) && tipo_numerico(&tipo_direita) {
                    Some(promover_tipo_numerico(&tipo_esquerda, &tipo_direita))
                } else {
                    None
                };
                let esquerda = self.expressao(esquerda)?;
                let direita = self.expressao(direita)?;
                let esquerda = if let Some(tipo) = &tipo_operacao {
                    ExpressaoIr::Normalizar {
                        tipo: tipo.clone(),
                        valor: Box::new(esquerda),
                    }
                } else {
                    esquerda
                };
                let direita = if let Some(tipo) = &tipo_operacao {
                    ExpressaoIr::Normalizar {
                        tipo: tipo.clone(),
                        valor: Box::new(direita),
                    }
                } else {
                    direita
                };
                Some(ExpressaoIr::Binaria {
                    esquerda: Box::new(esquerda),
                    operador: converter_operador(*operador)?,
                    direita: Box::new(direita),
                })
            },
            Expressao::Lambda { .. } | Expressao::LambdaBloco { .. } | Expressao::Condicional { .. } => None,
            Expressao::Atribuicao { destino, valor } => {
                let tipo = self.inferir_tipo_ast(destino);
                let valor = Box::new(self.expressao(valor)?);
                if let Expressao::Nome(nome) = destino.as_ref() {
                    Some(ExpressaoIr::Atribuicao { nome: nome.clone(), valor, tipo })
                } else if let Some((raiz, caminho)) = caminho_campos(destino) {
                    Some(ExpressaoIr::AtribuicaoCampo { raiz, caminho, valor, tipo })
                } else {
                    self.diagnosticos.push("O destino da atribuição precisa ser uma variável ou campo de estrutura.".into());
                    None
                }
            }
            Expressao::Chamada { alvo, argumentos, .. } => {
                let args = argumentos.iter().map(|a| self.expressao(a)).collect::<Option<Vec<_>>>()?;
                match alvo.as_ref() {
                    Expressao::Nome(nome) => {
                        if nome == "copiar" {
                            if args.len() != 1 {
                                self.diagnosticos.push("'copiar' exige exatamente um argumento.".into());
                                return None;
                            }
                            args.into_iter().next()
                        } else if self.escopos_tipos.iter().rev()
                            .find_map(|escopo| escopo.get(nome))
                            .is_some_and(|tipo| matches!(tipo, Tipo::Funcao(_, _)))
                        {
                            Some(ExpressaoIr::ChamadaIndireta {
                                alvo: Box::new(self.expressao(alvo)?),
                                argumentos: args,
                            })
                        } else if let Some(tipo) = tipo_conversao_explicita(nome) {
                            if args.len() != 1 {
                                self.diagnosticos.push(format!(
                                    "A conversão '{nome}' exige exatamente um argumento."
                                ));
                                return None;
                            }
                            let argumento = argumentos.first()?;
                            let origem = self.inferir_tipo_ast(argumento);
                            Some(ExpressaoIr::Conversao {
                                tipo,
                                origem,
                                valor: Box::new(args.into_iter().next()?),
                            })
                        } else if self.genericas.contains_key(nome) {
                            let especializado = self.registrar_instancia_generica(nome, argumentos)?;
                            Some(ExpressaoIr::Chamada { nome: especializado, argumentos: args })
                        } else {
                            Some(ExpressaoIr::Chamada { nome: nome.clone(), argumentos: args })
                        }
                    },
                    Expressao::Membro { alvo, operador: TipoToken::Escopo, nome: variante } => {
                        if let Expressao::Nome(tipo_base) = alvo.as_ref() {
                            let construtor = if self.variantes_enumeracoes.contains_key(tipo_base) {
                                None
                            } else {
                                match (tipo_base.as_str(), variante.as_str(), args.len()) {
                                ("Opcional" | "opcional", "Algum", 1) => Some(("Opcional", "Algum")),
                                ("Opcional" | "opcional", "Nenhum", 0) => Some(("Opcional", "Nenhum")),
                                ("Resultado" | "resultado", "Sucesso", 1) => Some(("Resultado", "Sucesso")),
                                ("Resultado" | "resultado", "Falha", 1) => Some(("Resultado", "Falha")),
                                    _ => None,
                                }
                            };
                            if let Some((nome, variante)) = construtor {
                                return Some(ExpressaoIr::ConstrucaoEnumeracao {
                                    nome: nome.into(), variante: variante.into(), valores: args,
                                });
                            }
                        }
                        if let Expressao::Nome(tipo_enum) = alvo.as_ref() {
                            if self.variantes_enumeracoes.get(tipo_enum).is_some_and(|variantes| variantes.contains_key(variante)) {
                                Some(ExpressaoIr::ConstrucaoEnumeracao { nome: tipo_enum.clone(), variante: variante.clone(), valores: args })
                            } else {
                                let caminho = self.modulos_importados.get(tipo_enum).cloned().unwrap_or_else(|| tipo_enum.clone());
                                Some(ExpressaoIr::ChamadaQualificada { modulo: caminho, nome: variante.clone(), argumentos: args })
                            }
                        } else {
                            self.diagnosticos.push("Alvo qualificado inválido na IR.".into());
                            None
                        }
                    }
                    _ => Some(ExpressaoIr::ChamadaIndireta {
                        alvo: Box::new(self.expressao(alvo)?),
                        argumentos: args,
                    })
                }
            }
            Expressao::Indexacao { alvo, indice } => Some(ExpressaoIr::Indexacao {
                alvo: Box::new(self.expressao(alvo)?), indice: Box::new(self.expressao(indice)?),
            }),
            Expressao::Membro { alvo, operador: TipoToken::Ponto, nome } => Some(ExpressaoIr::Membro {
                alvo: Box::new(self.expressao(alvo)?), campo: nome.clone(),
            }),
            Expressao::Membro { alvo, operador: TipoToken::Escopo, nome: variante } => {
                if let Expressao::Nome(tipo_enum) = alvo.as_ref() {
                    if self.variantes_enumeracoes.get(tipo_enum)
                        .and_then(|variantes| variantes.get(variante))
                        .is_some_and(Vec::is_empty) {
                        Some(ExpressaoIr::ConstrucaoEnumeracao {
                            nome: tipo_enum.clone(),
                            variante: variante.clone(),
                            valores: Vec::new(),
                        })
                    } else {
                        self.diagnosticos.push(format!("A variante '{tipo_enum}::{variante}' não existe ou exige payload."));
                        None
                    }
                } else {
                    self.diagnosticos.push("Alvo qualificado inválido na IR.".into());
                    None
                }
            },
            Expressao::Membro { .. } => {
                self.diagnosticos.push("Acesso de escopo inválido na IR.".into());
                None
            }
        }
    }
}

fn tipo_numerico(tipo: &Tipo) -> bool {
    matches!(
        tipo,
        Tipo::Inteiro | Tipo::Inteiro8 | Tipo::Inteiro16 | Tipo::Inteiro32
            | Tipo::Inteiro64 | Tipo::Inteiro128 | Tipo::InteiroU8
            | Tipo::InteiroU16 | Tipo::InteiroU32 | Tipo::InteiroU64
            | Tipo::InteiroU128 | Tipo::Decimal | Tipo::Decimal32
            | Tipo::Decimal64 | Tipo::Byte
    )
}

fn promover_tipo_numerico(a: &Tipo, b: &Tipo) -> Tipo {
    if matches!(a, Tipo::Decimal | Tipo::Decimal64)
        || matches!(b, Tipo::Decimal | Tipo::Decimal64)
    {
        return Tipo::Decimal64;
    }
    let largura = |tipo: &Tipo| match tipo {
        Tipo::Inteiro8 | Tipo::InteiroU8 | Tipo::Byte => 8,
        Tipo::Inteiro16 | Tipo::InteiroU16 => 16,
        Tipo::Inteiro32 | Tipo::InteiroU32 => 32,
        Tipo::Inteiro64 | Tipo::InteiroU64 | Tipo::Inteiro => 64,
        Tipo::Inteiro128 | Tipo::InteiroU128 => 128,
        _ => 64,
    };
    if a == &Tipo::Decimal32 && b == &Tipo::Decimal32 {
        return Tipo::Decimal32;
    }
    if a == &Tipo::Decimal32 || b == &Tipo::Decimal32 {
        let outro = if a == &Tipo::Decimal32 { b } else { a };
        return if largura(outro) > 16 { Tipo::Decimal64 } else { Tipo::Decimal32 };
    }
    let sem_sinal = |tipo: &Tipo| matches!(
        tipo,
        Tipo::InteiroU8 | Tipo::InteiroU16 | Tipo::InteiroU32
            | Tipo::InteiroU64 | Tipo::InteiroU128 | Tipo::Byte
    );
    let largura_maxima = largura(a).max(largura(b));
    if sem_sinal(a) && sem_sinal(b) {
        return match largura_maxima {
            8 => Tipo::InteiroU8,
            16 => Tipo::InteiroU16,
            32 => Tipo::InteiroU32,
            64 => Tipo::InteiroU64,
            _ => Tipo::InteiroU128,
        };
    }
    match largura_maxima {
        8 => Tipo::Inteiro8,
        16 => Tipo::Inteiro16,
        32 => Tipo::Inteiro32,
        128 => Tipo::Inteiro128,
        64 if a == &Tipo::Inteiro64 || b == &Tipo::Inteiro64 => Tipo::Inteiro64,
        _ => Tipo::Inteiro,
    }
}

fn caminho_campos(expressao: &Expressao) -> Option<(String, Vec<String>)> {
    fn coletar(expressao: &Expressao, caminho: &mut Vec<String>) -> Option<String> {
        match expressao {
            Expressao::Nome(nome) => Some(nome.clone()),
            Expressao::Membro { alvo, operador: TipoToken::Ponto, nome } => {
                let raiz = coletar(alvo, caminho)?;
                caminho.push(nome.clone());
                Some(raiz)
            }
            _ => None,
        }
    }
    let mut caminho = Vec::new();
    let raiz = coletar(expressao, &mut caminho)?;
    (!caminho.is_empty()).then_some((raiz, caminho))
}

fn dividir_tipo_funcao(nome: &str) -> Option<(Vec<&str>, &str)> {
    let conteudo = nome.strip_prefix("funcao(")?;
    let mut parenteses = 1usize;
    let mut angulos = 0usize;
    let mut colchetes = 0usize;
    let mut fechamento = None;
    for (indice, caractere) in conteudo.char_indices() {
        match caractere {
            '(' => parenteses += 1,
            ')' => {
                parenteses = parenteses.checked_sub(1)?;
                if parenteses == 0 {
                    fechamento = Some(indice);
                    break;
                }
            }
            '<' => angulos += 1,
            '>' => angulos = angulos.checked_sub(1)?,
            '[' => colchetes += 1,
            ']' => colchetes = colchetes.checked_sub(1)?,
            _ => {}
        }
    }
    let fechamento = fechamento?;
    let retorno = conteudo[fechamento + 1..].strip_prefix(':')?.trim();
    if retorno.is_empty() {
        return None;
    }
    let bruto = &conteudo[..fechamento];
    if bruto.trim().is_empty() {
        return Some((Vec::new(), retorno));
    }
    let mut parametros = Vec::new();
    let mut inicio = 0usize;
    let mut parenteses = 0usize;
    let mut angulos = 0usize;
    let mut colchetes = 0usize;
    for (indice, caractere) in bruto.char_indices() {
        match caractere {
            '(' => parenteses += 1,
            ')' => parenteses = parenteses.checked_sub(1)?,
            '<' => angulos += 1,
            '>' => angulos = angulos.checked_sub(1)?,
            '[' => colchetes += 1,
            ']' => colchetes = colchetes.checked_sub(1)?,
            ',' if parenteses == 0 && angulos == 0 && colchetes == 0 => {
                let parametro = bruto[inicio..indice].trim();
                if parametro.is_empty() {
                    return None;
                }
                parametros.push(parametro);
                inicio = indice + 1;
            }
            _ => {}
        }
    }
    let ultimo = bruto[inicio..].trim();
    if ultimo.is_empty() {
        return None;
    }
    parametros.push(ultimo);
    Some((parametros, retorno))
}

fn dividir_tipo_generico(nome: &str) -> Option<(&str, Vec<&str>)> {
    let abertura = nome.find('<')?;
    if !nome.ends_with('>') { return None; }
    let base = &nome[..abertura];
    let conteudo = &nome[abertura + 1..nome.len() - 1];
    if base.is_empty() || conteudo.is_empty() { return None; }
    let mut argumentos = Vec::new();
    let mut profundidade = 0usize;
    let mut inicio = 0usize;
    for (indice, caractere) in conteudo.char_indices() {
        match caractere {
            '<' => profundidade += 1,
            '>' => profundidade = profundidade.checked_sub(1)?,
            ',' if profundidade == 0 => {
                let argumento = conteudo[inicio..indice].trim();
                if argumento.is_empty() { return None; }
                argumentos.push(argumento);
                inicio = indice + caractere.len_utf8();
            }
            _ => {}
        }
    }
    if profundidade != 0 { return None; }
    let ultimo = conteudo[inicio..].trim();
    if ultimo.is_empty() { return None; }
    argumentos.push(ultimo);
    Some((base, argumentos))
}

fn nome_tipo(tipo: &Tipo) -> String {
    match tipo {
        Tipo::Vazio => "vazio".into(),
        Tipo::Logico => "logico".into(),
        Tipo::Inteiro => "inteiro".into(),
        Tipo::Inteiro8 => "inteiro8".into(),
        Tipo::Inteiro16 => "inteiro16".into(),
        Tipo::Inteiro32 => "inteiro32".into(),
        Tipo::Inteiro64 => "inteiro64".into(),
        Tipo::Inteiro128 => "inteiro128".into(),
        Tipo::InteiroU8 => "inteiro_u8".into(),
        Tipo::InteiroU16 => "inteiro_u16".into(),
        Tipo::InteiroU32 => "inteiro_u32".into(),
        Tipo::InteiroU64 => "inteiro_u64".into(),
        Tipo::InteiroU128 => "inteiro_u128".into(),
        Tipo::Decimal | Tipo::Decimal64 => "decimal64".into(),
        Tipo::Decimal32 => "decimal32".into(),
        Tipo::Texto => "texto".into(),
        Tipo::Caractere => "caractere".into(),
        Tipo::Byte => "byte".into(),
        Tipo::Lista(interno) => format!("[{}]", nome_tipo(interno)),
        Tipo::Opcional(interno) => format!("Opcional<{}>", nome_tipo(interno)),
        Tipo::Resultado(ok, erro) => format!("Resultado<{},{}>", nome_tipo(ok), nome_tipo(erro)),
        Tipo::Tupla(tipos) => format!("Tupla<{}>", tipos.iter().map(nome_tipo).collect::<Vec<_>>().join(",")),
        Tipo::Estrutura(nome) => nome.clone(),
        Tipo::Enumeracao(nome) => nome.clone(),
        Tipo::Intervalo(interno) => format!("Intervalo<{}>", nome_tipo(interno)),
        Tipo::Funcao(_, _) | Tipo::Parametro(_) | Tipo::Desconhecido => "desconhecido".into(),
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
        ExpressaoIr::Constante(ValorIr::InteiroSemSinal(_)) => Tipo::InteiroU128,
        ExpressaoIr::Constante(ValorIr::Decimal(_)) => Tipo::Decimal64,
        ExpressaoIr::Constante(ValorIr::Logico(_)) => Tipo::Logico,
        ExpressaoIr::Constante(ValorIr::Texto(_)) => Tipo::Texto,
        ExpressaoIr::Constante(ValorIr::Caractere(_)) => Tipo::Caractere,
        ExpressaoIr::Normalizar { tipo, .. } | ExpressaoIr::Conversao { tipo, .. } => tipo.clone(),
        ExpressaoIr::ReferenciaFuncao { tipo, .. } | ExpressaoIr::ReferenciaFuncaoQualificada { tipo, .. } => tipo.clone(),
        ExpressaoIr::ChamadaIndireta { alvo, .. } => match inferir_tipo(alvo) {
            Tipo::Funcao(_, retorno) => *retorno,
            _ => Tipo::Desconhecido,
        },
        ExpressaoIr::Constante(_) => Tipo::Desconhecido,
        ExpressaoIr::Lista(v) => Tipo::Lista(Box::new(v.first().map_or(Tipo::Desconhecido, inferir_tipo))),
        ExpressaoIr::ConstrucaoEstrutura { nome, .. } => Tipo::Estrutura(nome.clone()),
        ExpressaoIr::ConstrucaoEnumeracao { nome, .. } => Tipo::Enumeracao(nome.clone()),
        ExpressaoIr::Tupla(v) => Tipo::Tupla(v.iter().map(inferir_tipo).collect()),
        ExpressaoIr::Binaria { esquerda, operador, .. } => match operador {
            OperadorIr::Igual | OperadorIr::Diferente | OperadorIr::Menor | OperadorIr::MenorIgual
            | OperadorIr::Maior | OperadorIr::MaiorIgual | OperadorIr::E | OperadorIr::Ou => Tipo::Logico,
            OperadorIr::Intervalo => Tipo::Intervalo(Box::new(inferir_tipo(esquerda))),
            _ => inferir_tipo(esquerda),
        },
        ExpressaoIr::Unaria { valor, .. } => inferir_tipo(valor),
        ExpressaoIr::Atribuicao { valor, .. } | ExpressaoIr::AtribuicaoCampo { valor, .. } => inferir_tipo(valor),
        ExpressaoIr::Indexacao { .. } | ExpressaoIr::Membro { .. } | ExpressaoIr::Chamada { .. } | ExpressaoIr::ChamadaQualificada { .. } | ExpressaoIr::Variavel(_) => Tipo::Desconhecido,
    }
}

fn literal(bruto: &str) -> Option<ValorIr> {
    match bruto {
        "verdadeiro" => Some(ValorIr::Logico(true)),
        "falso" => Some(ValorIr::Logico(false)),
        "nulo" => Some(ValorIr::Nulo),
        _ if bruto.starts_with('"') && bruto.ends_with('"') => Some(ValorIr::Texto(desescapar(&bruto[1..bruto.len()-1]))),
        _ if bruto.starts_with('\'') && bruto.ends_with('\'') => desescapar(&bruto[1..bruto.len()-1]).chars().next().map(ValorIr::Caractere),
        _ if bruto.starts_with("0x") || bruto.starts_with("0X") => i128::from_str_radix(&bruto[2..].replace('_', ""), 16).ok().map(ValorIr::Inteiro).or_else(|| u128::from_str_radix(&bruto[2..].replace('_', ""), 16).ok().map(ValorIr::InteiroSemSinal)),
        _ if bruto.starts_with("0b") || bruto.starts_with("0B") => i128::from_str_radix(&bruto[2..].replace('_', ""), 2).ok().map(ValorIr::Inteiro).or_else(|| u128::from_str_radix(&bruto[2..].replace('_', ""), 2).ok().map(ValorIr::InteiroSemSinal)),
        _ if bruto.starts_with("0o") || bruto.starts_with("0O") => i128::from_str_radix(&bruto[2..].replace('_', ""), 8).ok().map(ValorIr::Inteiro).or_else(|| u128::from_str_radix(&bruto[2..].replace('_', ""), 8).ok().map(ValorIr::InteiroSemSinal)),
        _ if bruto.contains('.') || bruto.contains('e') || bruto.contains('E') => bruto.replace('_', "").parse().ok().map(ValorIr::Decimal),
        _ => bruto
            .replace('_', "")
            .parse::<i128>()
            .ok()
            .map(ValorIr::Inteiro)
            .or_else(|| bruto.replace('_', "").parse::<u128>().ok().map(ValorIr::InteiroSemSinal)),
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
    executar_interno(programa, limite_passos, None, None)
}

/// Executa a IR usando linhas de entrada fornecidas pelo chamador.
///
/// Esta interface permite testes determinísticos e integração com terminais
/// ou interfaces gráficas sem substituir a entrada padrão do processo.
pub fn executar_com_entrada(
    programa: &ProgramaIr,
    limite_passos: usize,
    entrada: impl IntoIterator<Item = String>,
) -> Result<ResultadoExecucao, String> {
    executar_interno(programa, limite_passos, Some(entrada.into_iter().collect()), None)
}

/// Estado observável da máquina no instante anterior à execução de uma instrução.
#[derive(Debug, Clone, PartialEq)]
pub struct EventoDepuracao {
    /// Intervalo de bytes da instrução atual na fonte original.
    pub caminho_fonte: Option<PathBuf>,
    pub intervalo_fonte: Option<IntervaloFonte>,
    /// Número sequencial do passo atual.
    pub passo: usize,
    /// Profundidade atual de chamadas.
    pub profundidade_chamada: usize,
    /// Funções ativas, da mais antiga à mais recente.
    pub pilha_chamadas: Vec<String>,
    /// Índices de início dos escopos pertencentes a cada chamada ativa.
    pub limites_escopos: Vec<usize>,
    /// Localizações em que cada função ativa foi chamada.
    pub locais_chamada: Vec<(Option<PathBuf>, Option<IntervaloFonte>)>,
    /// Escopos ativos, do global ao local.
    pub escopos: Vec<HashMap<String, ValorIr>>,
}

type ObservadorDepuracao<'a> = &'a mut (dyn FnMut(&EventoDepuracao) -> Result<(), String> + 'a);

/// Comando recebido pelo controlador de depuração.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComandoDepuracao {
    /// Continua até o próximo ponto de parada ou o fim do programa.
    Continuar,
    /// Executa uma instrução e pausa novamente.
    Avancar,
    /// Avança até uma linha diferente, sem entrar em chamadas.
    AvancarLinha(Vec<(PathBuf, usize)>),
    /// Pausa antes da próxima instrução executada.
    Pausar,
    /// Executa até sair da função atual.
    Sair,
    /// Substitui os pontos de parada por índices de instrução.
    DefinirPontos(Vec<usize>),
    /// Substitui pontos de parada por offsets de bytes na fonte original.
    DefinirPontosFonte(Vec<(PathBuf, usize)>),
    /// Encerra a execução depurada.
    Encerrar,
}

/// Evento emitido pelo controlador ao terminar a execução.
#[derive(Debug, Clone, PartialEq)]
pub enum EventoControleDepuracao {
    /// A máquina pausou antes de executar a instrução indicada.
    Pausado(EventoDepuracao),
    /// A execução terminou normalmente.
    Finalizado(Result<ResultadoExecucao, String>),
}

/// Executa a IR sob controle síncrono de um depurador externo.
///
/// O canal de eventos informa estado e escopos antes da instrução. O canal de
/// comandos aceita continuar, avançar, sair, pausar, definir pontos por índice
/// ou por offset de origem e encerrar. A entrada interativa fica isolada do
/// canal DAP e retorna erro quando o programa solicita texto.
pub fn executar_controlado(
    programa: &ProgramaIr,
    limite_passos: usize,
    comandos: std::sync::mpsc::Receiver<ComandoDepuracao>,
    eventos: std::sync::mpsc::Sender<EventoControleDepuracao>,
) -> Result<ResultadoExecucao, String> {
    use std::collections::HashSet;
    let mut pontos = HashSet::new();
    let mut pontos_fonte: HashSet<(PathBuf, usize)> = HashSet::new();
    let mut pausado = true;
    let mut pausar_no_proximo = false;
    let mut profundidade_saida = None;
    let mut offsets_linha_passo: HashSet<(PathBuf, usize)> = HashSet::new();
    let mut profundidade_passo = None;
    let mut observador = |evento: &EventoDepuracao| {
        while let Ok(comando) = comandos.try_recv() {
            match comando {
                ComandoDepuracao::Continuar => {
                    pausado = false;
                    pausar_no_proximo = false;
                    profundidade_saida = None;
                    offsets_linha_passo.clear();
                    profundidade_passo = None;
                }
                ComandoDepuracao::Avancar => {
                    pausado = false;
                    pausar_no_proximo = true;
                    profundidade_saida = None;
                    offsets_linha_passo.clear();
                    profundidade_passo = None;
                }
                ComandoDepuracao::Pausar => {
                    pausado = false;
                    pausar_no_proximo = true;
                    profundidade_saida = None;
                    offsets_linha_passo.clear();
                    profundidade_passo = None;
                }
                ComandoDepuracao::AvancarLinha(offsets) => {
                    pausado = false;
                    pausar_no_proximo = false;
                    profundidade_saida = None;
                    offsets_linha_passo = offsets.into_iter().collect();
                    profundidade_passo = Some(evento.profundidade_chamada);
                }
                ComandoDepuracao::Sair => {
                    pausado = false;
                    pausar_no_proximo = false;
                    profundidade_saida = Some(evento.profundidade_chamada);
                    offsets_linha_passo.clear();
                    profundidade_passo = None;
                }
                ComandoDepuracao::DefinirPontos(novos) => {
                    pontos = novos.into_iter().collect();
                }
                ComandoDepuracao::DefinirPontosFonte(novos) => {
                    pontos_fonte = novos.into_iter().collect();
                }
                ComandoDepuracao::Encerrar => {
                    return Err("Execução encerrada pelo depurador.".into());
                }
            }
        }
        if pausar_no_proximo {
            pausado = true;
            pausar_no_proximo = false;
        }
        let localizacao_atual = evento.intervalo_fonte.map(|intervalo| (evento.caminho_fonte.clone().unwrap_or_default(), intervalo.inicio));
        let ponto_atingido = pontos.contains(&evento.passo)
            || localizacao_atual.as_ref().is_some_and(|localizacao| pontos_fonte.contains(localizacao));
        let retorno_funcao = profundidade_saida
            .is_some_and(|profundidade| evento.profundidade_chamada < profundidade);
        if retorno_funcao {
            pausado = true;
            profundidade_saida = None;
        }
        let linha_seguinte = profundidade_passo.is_some_and(|profundidade| {
            evento.profundidade_chamada <= profundidade
                && localizacao_atual.as_ref().is_some_and(|localizacao| !offsets_linha_passo.contains(localizacao))
        });
        if linha_seguinte {
            pausado = true;
            profundidade_passo = None;
            offsets_linha_passo.clear();
        }
        if ponto_atingido || pausado {
            eventos.send(EventoControleDepuracao::Pausado(evento.clone()))
                .map_err(|_| "Canal de eventos do depurador foi encerrado.".to_owned())?;
            loop {
                match comandos.recv() {
                    Ok(ComandoDepuracao::Continuar) => {
                        pausado = false;
                        pausar_no_proximo = false;
                        profundidade_saida = None;
                        offsets_linha_passo.clear();
                        profundidade_passo = None;
                        break;
                    }
                    Ok(ComandoDepuracao::Avancar) => {
                        pausado = false;
                        pausar_no_proximo = true;
                        profundidade_saida = None;
                        offsets_linha_passo.clear();
                        profundidade_passo = None;
                        break;
                    }
                    Ok(ComandoDepuracao::Pausar) => {
                        pausado = false;
                        pausar_no_proximo = true;
                        profundidade_saida = None;
                        offsets_linha_passo.clear();
                        profundidade_passo = None;
                        break;
                    }
                    Ok(ComandoDepuracao::AvancarLinha(offsets)) => {
                        pausado = false;
                        pausar_no_proximo = false;
                        profundidade_saida = None;
                        offsets_linha_passo = offsets.into_iter().collect();
                        profundidade_passo = Some(evento.profundidade_chamada);
                        break;
                    }
                    Ok(ComandoDepuracao::Sair) => {
                        pausado = false;
                        pausar_no_proximo = false;
                        profundidade_saida = Some(evento.profundidade_chamada);
                        offsets_linha_passo.clear();
                        profundidade_passo = None;
                        break;
                    }
                    Ok(ComandoDepuracao::DefinirPontos(novos)) => {
                        pontos = novos.into_iter().collect();
                    }
                    Ok(ComandoDepuracao::DefinirPontosFonte(novos)) => {
                        pontos_fonte = novos.into_iter().collect();
                    }
                    Ok(ComandoDepuracao::Encerrar) => {
                        return Err("Execução encerrada pelo depurador.".into());
                    }
                    Err(_) => return Err("Canal de comandos do depurador foi encerrado.".into()),
                }
            }
        }
        Ok(())
    };
    let resultado = executar_interno(programa, limite_passos, Some(VecDeque::new()), Some(&mut observador));
    let _ = eventos.send(EventoControleDepuracao::Finalizado(resultado.clone()));
    resultado
}

/// Executa a IR notificando o chamador antes de cada instrução.
///
/// O observador pode bloquear para aguardar comandos de depuração. Retornar
/// erro interrompe a execução sem descartar o erro original.
pub fn executar_observado<F>(
    programa: &ProgramaIr,
    limite_passos: usize,
    mut observador: F,
) -> Result<ResultadoExecucao, String>
where
    F: FnMut(&EventoDepuracao) -> Result<(), String>,
{
    executar_interno(programa, limite_passos, None, Some(&mut observador))
}

fn executar_interno(
    programa: &ProgramaIr,
    limite_passos: usize,
    entrada: Option<VecDeque<String>>,
    observador: Option<ObservadorDepuracao<'_>>,
) -> Result<ResultadoExecucao, String> {
    let mut vm = Maquina {
        programa,
        observador,
        contexto: ContextoRuntime::novo(),
        saida: Vec::new(),
        entrada,
        passos: 0,
        limite: limite_passos.max(1),
        profundidade_chamada: 0,
        pilha_chamadas: Vec::new(),
        limites_escopos: Vec::new(),
        locais_chamada: Vec::new(),
        caminho_fonte: None,
        intervalo_fonte: None,
    };
    let resultado = (|| {
        let fluxo = vm.bloco_raiz(&programa.instrucoes)?;
        if matches!(&fluxo, Fluxo::Parar | Fluxo::Seguir) {
            return Err("Controle de laço escapou do seu contexto.".into());
        }
        let globais = vm.contexto.memoria().globais().into_iter()
            .map(|(nome, valor)| Ok((nome, converter_para_ir(valor)?)))
            .collect::<Result<HashMap<_, _>, String>>()?;
        Ok(ResultadoExecucao { saida: vm.saida.clone(), globais })
    })();
    let finalizacao = vm.contexto.finalizar().map_err(formatar_erro_runtime);
    match (resultado, finalizacao) {
        (Err(erro), _) => Err(erro),
        (Ok(_), Err(erro)) => Err(erro),
        (Ok(resultado), Ok(())) => Ok(resultado),
    }
}

enum Fluxo { Normal, Retorno(ValorIr), Parar, Seguir }

fn formatar_erro_runtime(erro: ErroRuntime) -> String {
    format!("Erro do runtime: {erro:?}")
}

fn tipo_conversao_explicita(nome: &str) -> Option<Tipo> {
    Some(match nome {
        "inteiro8" => Tipo::Inteiro8,
        "inteiro16" => Tipo::Inteiro16,
        "inteiro32" => Tipo::Inteiro32,
        "inteiro64" => Tipo::Inteiro64,
        "inteiro128" => Tipo::Inteiro128,
        "inteiro_u8" => Tipo::InteiroU8,
        "inteiro_u16" => Tipo::InteiroU16,
        "inteiro_u32" => Tipo::InteiroU32,
        "inteiro_u64" => Tipo::InteiroU64,
        "inteiro_u128" => Tipo::InteiroU128,
        "byte" => Tipo::Byte,
        "decimal32" => Tipo::Decimal32,
        "decimal64" => Tipo::Decimal64,
        _ => return None,
    })
}

fn converter_valor_explicito(tipo: &Tipo, valor: ValorIr) -> Result<ValorIr, String> {
    let erro = || format!("Conversão explícita para '{}' fora da faixa ou incompatível.", tipo.nome());
    match tipo {
        Tipo::Inteiro | Tipo::Inteiro64 | Tipo::Inteiro8 | Tipo::Inteiro16
        | Tipo::Inteiro32 | Tipo::Inteiro128 => {
            let inteiro = match valor {
                ValorIr::Inteiro(v) => v,
                ValorIr::InteiroSemSinal(v) => i128::try_from(v).map_err(|_| erro())?,
                ValorIr::Decimal(v) if v.is_finite() => {
                    let v = v.trunc();
                    if v < i128::MIN as f64 || v >= -(i128::MIN as f64) {
                        return Err(erro());
                    }
                    v as i128
                }
                _ => return Err(erro()),
            };
            normalizar_valor(tipo, ValorIr::Inteiro(inteiro)).map_err(|_| erro())
        }
        Tipo::InteiroU8 | Tipo::InteiroU16 | Tipo::InteiroU32
        | Tipo::InteiroU64 | Tipo::InteiroU128 | Tipo::Byte => {
            let inteiro = match valor {
                ValorIr::Inteiro(v) if v >= 0 => v as u128,
                ValorIr::InteiroSemSinal(v) => v,
                ValorIr::Decimal(v) if v.is_finite() => {
                    let v = v.trunc();
                    if v < 0.0 || v >= u128::MAX as f64 {
                        return Err(erro());
                    }
                    v as u128
                }
                _ => return Err(erro()),
            };
            normalizar_valor(tipo, ValorIr::InteiroSemSinal(inteiro)).map_err(|_| erro())
        }
        Tipo::Decimal32 | Tipo::Decimal | Tipo::Decimal64 => {
            let decimal = match valor {
                ValorIr::Inteiro(v) => v as f64,
                ValorIr::InteiroSemSinal(v) => v as f64,
                ValorIr::Decimal(v) => v,
                _ => return Err(erro()),
            };
            if !decimal.is_finite() {
                return Err(erro());
            }
            normalizar_valor(tipo, ValorIr::Decimal(decimal)).map_err(|_| erro())
        }
        _ => Err(erro()),
    }
}

fn normalizar_limite_inteiro_intervalo(tipo: &Tipo, valor: i128) -> Result<i128, String> {
    match normalizar_valor(tipo, ValorIr::Inteiro(valor))? {
        ValorIr::Inteiro(valor) => Ok(valor),
        ValorIr::InteiroSemSinal(valor) => i128::try_from(valor)
            .map_err(|_| "Limite de intervalo sem sinal excede a representação interna.".into()),
        _ => Err("Tipo de limite de intervalo inteiro inválido.".into()),
    }
}

fn normalizar_limite_inteiro_sem_sinal_intervalo(tipo: &Tipo, valor: u128) -> Result<u128, String> {
    match normalizar_valor(tipo, ValorIr::InteiroSemSinal(valor))? {
        ValorIr::InteiroSemSinal(valor) => Ok(valor),
        _ => Err("Tipo de limite de intervalo sem sinal inválido.".into()),
    }
}

fn normalizar_limite_decimal_intervalo(tipo: &Tipo, valor: f64) -> Result<f64, String> {
    match normalizar_valor(tipo, ValorIr::Decimal(valor))? {
        ValorIr::Decimal(valor) => Ok(valor),
        _ => Err("Tipo de limite de intervalo decimal inválido.".into()),
    }
}

fn normalizar_valor(tipo: &Tipo, valor: ValorIr) -> Result<ValorIr, String> {
    match (tipo, valor.clone()) {
        (Tipo::Inteiro | Tipo::Inteiro64, ValorIr::Inteiro(v)) =>
            i64::try_from(v).map(|v| ValorIr::Inteiro(v as i128)).map_err(|_| "Valor fora da faixa de inteiro64.".into()),
        (Tipo::Inteiro8, ValorIr::Inteiro(v)) =>
            i8::try_from(v).map(|v| ValorIr::Inteiro(v as i128)).map_err(|_| "Valor fora da faixa de inteiro8.".into()),
        (Tipo::Inteiro16, ValorIr::Inteiro(v)) =>
            i16::try_from(v).map(|v| ValorIr::Inteiro(v as i128)).map_err(|_| "Valor fora da faixa de inteiro16.".into()),
        (Tipo::Inteiro32, ValorIr::Inteiro(v)) =>
            i32::try_from(v).map(|v| ValorIr::Inteiro(v as i128)).map_err(|_| "Valor fora da faixa de inteiro32.".into()),
        (Tipo::Inteiro128, ValorIr::Inteiro(v)) => Ok(ValorIr::Inteiro(v)),
        (Tipo::InteiroU8 | Tipo::Byte, ValorIr::Inteiro(v)) if v >= 0 =>
            u8::try_from(v as u128).map(|v| ValorIr::InteiroSemSinal(v as u128)).map_err(|_| "Valor fora da faixa de inteiro_u8/byte.".into()),
        (Tipo::InteiroU16, ValorIr::Inteiro(v)) if v >= 0 =>
            u16::try_from(v as u128).map(|v| ValorIr::InteiroSemSinal(v as u128)).map_err(|_| "Valor fora da faixa de inteiro_u16.".into()),
        (Tipo::InteiroU32, ValorIr::Inteiro(v)) if v >= 0 =>
            u32::try_from(v as u128).map(|v| ValorIr::InteiroSemSinal(v as u128)).map_err(|_| "Valor fora da faixa de inteiro_u32.".into()),
        (Tipo::InteiroU64, ValorIr::Inteiro(v)) if v >= 0 =>
            u64::try_from(v as u128).map(|v| ValorIr::InteiroSemSinal(v as u128)).map_err(|_| "Valor fora da faixa de inteiro_u64.".into()),
        (Tipo::InteiroU8 | Tipo::Byte, ValorIr::InteiroSemSinal(v)) =>
            u8::try_from(v).map(|v| ValorIr::InteiroSemSinal(v as u128)).map_err(|_| "Valor fora da faixa de inteiro_u8/byte.".into()),
        (Tipo::InteiroU16, ValorIr::InteiroSemSinal(v)) =>
            u16::try_from(v).map(|v| ValorIr::InteiroSemSinal(v as u128)).map_err(|_| "Valor fora da faixa de inteiro_u16.".into()),
        (Tipo::InteiroU32, ValorIr::InteiroSemSinal(v)) =>
            u32::try_from(v).map(|v| ValorIr::InteiroSemSinal(v as u128)).map_err(|_| "Valor fora da faixa de inteiro_u32.".into()),
        (Tipo::InteiroU64, ValorIr::InteiroSemSinal(v)) =>
            u64::try_from(v).map(|v| ValorIr::InteiroSemSinal(v as u128)).map_err(|_| "Valor fora da faixa de inteiro_u64.".into()),
        (Tipo::InteiroU128, ValorIr::InteiroSemSinal(v)) => Ok(ValorIr::InteiroSemSinal(v)),
        (Tipo::Decimal32, ValorIr::Decimal(v)) => {
            let convertido = v as f32;
            if v.is_finite() && !convertido.is_finite() {
                Err("Valor fora da faixa de decimal32.".into())
            } else {
                Ok(ValorIr::Decimal(convertido as f64))
            }
        }
        (Tipo::Decimal32, ValorIr::Inteiro(v)) => {
            let convertido = v as f32;
            if !convertido.is_finite() {
                Err("Valor inteiro fora da faixa de decimal32.".into())
            } else {
                Ok(ValorIr::Decimal(convertido as f64))
            }
        }
        (Tipo::Decimal32, ValorIr::InteiroSemSinal(v)) => {
            let convertido = v as f32;
            if !convertido.is_finite() {
                Err("Valor inteiro sem sinal fora da faixa de decimal32.".into())
            } else {
                Ok(ValorIr::Decimal(convertido as f64))
            }
        }
        (Tipo::Decimal | Tipo::Decimal64, ValorIr::Inteiro(v)) =>
            Ok(ValorIr::Decimal(v as f64)),
        (Tipo::Decimal | Tipo::Decimal64, ValorIr::InteiroSemSinal(v)) =>
            Ok(ValorIr::Decimal(v as f64)),
        (Tipo::Decimal | Tipo::Decimal64, valor @ ValorIr::Decimal(_)) => Ok(valor),
        (Tipo::Logico, valor @ ValorIr::Logico(_))
        | (Tipo::Texto, valor @ ValorIr::Texto(_))
        | (Tipo::Caractere, valor @ ValorIr::Caractere(_))
        | (Tipo::Vazio, valor @ ValorIr::Vazio) => Ok(valor),
        (Tipo::Lista(elemento), ValorIr::Lista(valores)) => Ok(ValorIr::Lista(
            valores.into_iter().map(|valor| normalizar_valor(elemento, valor))
                .collect::<Result<_, _>>()?
        )),
        (Tipo::Tupla(tipos), ValorIr::Lista(valores)) if tipos.len() == valores.len() => {
            Ok(ValorIr::Lista(valores.into_iter().zip(tipos)
                .map(|(valor, tipo)| normalizar_valor(tipo, valor))
                .collect::<Result<_, _>>()?))
        }
        (Tipo::Estrutura(nome), ValorIr::Estrutura { nome: nome_valor, campos })
            if nome == &nome_valor => Ok(ValorIr::Estrutura { nome: nome_valor, campos }),
        (Tipo::Enumeracao(nome), ValorIr::Estrutura { nome: nome_valor, campos })
            if nome_valor.starts_with(&format!("{nome}::")) => {
                Ok(ValorIr::Estrutura { nome: nome_valor, campos })
            }
        (Tipo::Opcional(_), ValorIr::Estrutura { nome, campos })
            if nome == "Opcional::Nenhum" && campos.is_empty() => {
                Ok(ValorIr::Estrutura { nome, campos })
            }
        (Tipo::Opcional(interno), ValorIr::Estrutura { nome, campos })
            if nome == "Opcional::Algum" && campos.len() == 1 => {
                let valor = normalizar_valor(interno, campos.into_iter().next().expect("um payload").1)?;
                Ok(ValorIr::Estrutura { nome, campos: vec![("0".into(), valor)] })
            }
        (Tipo::Resultado(sucesso, _), ValorIr::Estrutura { nome, campos })
            if nome == "Resultado::Sucesso" && campos.len() == 1 => {
                let valor = normalizar_valor(sucesso, campos.into_iter().next().expect("um payload").1)?;
                Ok(ValorIr::Estrutura { nome, campos: vec![("0".into(), valor)] })
            }
        (Tipo::Resultado(_, erro), ValorIr::Estrutura { nome, campos })
            if nome == "Resultado::Falha" && campos.len() == 1 => {
                let valor = normalizar_valor(erro, campos.into_iter().next().expect("um payload").1)?;
                Ok(ValorIr::Estrutura { nome, campos: vec![("0".into(), valor)] })
            }
        (Tipo::Funcao(_, _), valor @ ValorIr::Funcao(_)) => Ok(valor),
        (Tipo::Intervalo(interno), ValorIr::Intervalo(inicio, fim))
            if matches!(interno.as_ref(),
                Tipo::Inteiro | Tipo::Inteiro8 | Tipo::Inteiro16 | Tipo::Inteiro32
                | Tipo::Inteiro64 | Tipo::Inteiro128) => {
                let inicio = normalizar_limite_inteiro_intervalo(interno, inicio)?;
                let fim = normalizar_limite_inteiro_intervalo(interno, fim)?;
                Ok(ValorIr::Intervalo(inicio, fim))
            }
        (Tipo::Intervalo(interno), ValorIr::IntervaloSemSinal(inicio, fim))
            if matches!(interno.as_ref(),
                Tipo::InteiroU8 | Tipo::InteiroU16 | Tipo::InteiroU32
                | Tipo::InteiroU64 | Tipo::InteiroU128 | Tipo::Byte) => {
                let inicio = normalizar_limite_inteiro_sem_sinal_intervalo(interno, inicio)?;
                let fim = normalizar_limite_inteiro_sem_sinal_intervalo(interno, fim)?;
                Ok(ValorIr::IntervaloSemSinal(inicio, fim))
            }
        (Tipo::Intervalo(interno), ValorIr::IntervaloDecimal(inicio, fim))
            if matches!(interno.as_ref(), Tipo::Decimal | Tipo::Decimal32 | Tipo::Decimal64) => {
                let inicio = normalizar_limite_decimal_intervalo(interno, inicio)?;
                let fim = normalizar_limite_decimal_intervalo(interno, fim)?;
                Ok(ValorIr::IntervaloDecimal(inicio, fim))
            }
        (Tipo::Desconhecido, valor) => Ok(valor),
        _ => Err(format!("Valor IR incompatível com o tipo '{}'.", tipo.nome())),
    }
}

fn converter_para_runtime(valor: ValorIr) -> ValorRuntime {
    match valor {
        ValorIr::Inteiro(v) => ValorRuntime::Inteiro(v),
        ValorIr::InteiroSemSinal(v) => ValorRuntime::InteiroSemSinal(v),
        ValorIr::Decimal(v) => ValorRuntime::Decimal(v),
        ValorIr::Logico(v) => ValorRuntime::Logico(v),
        ValorIr::Texto(v) => ValorRuntime::Texto(v),
        ValorIr::Caractere(v) => ValorRuntime::Caractere(v),
        ValorIr::Funcao(v) => ValorRuntime::Funcao(v),
        ValorIr::Lista(itens) => ValorRuntime::Lista(Arc::new(RwLock::new(
            itens.into_iter().map(converter_para_runtime).collect(),
        ))),
        ValorIr::Estrutura { nome, campos } => ValorRuntime::Estrutura { nome, campos: Arc::new(RwLock::new(
            campos.into_iter().map(|(nome, valor)| (nome, converter_para_runtime(valor))).collect(),
        )) },
        ValorIr::Intervalo(inicio, fim) => ValorRuntime::Intervalo { inicio, fim },
        ValorIr::IntervaloSemSinal(inicio, fim) => ValorRuntime::IntervaloSemSinal { inicio, fim },
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
        ValorRuntime::InteiroSemSinal(v) => Ok(ValorIr::InteiroSemSinal(v)),
        ValorRuntime::Decimal(v) => Ok(ValorIr::Decimal(v)),
        ValorRuntime::Logico(v) => Ok(ValorIr::Logico(v)),
        ValorRuntime::Texto(v) => Ok(ValorIr::Texto(v)),
        ValorRuntime::Caractere(v) => Ok(ValorIr::Caractere(v)),
        ValorRuntime::Funcao(v) => Ok(ValorIr::Funcao(v)),
        ValorRuntime::Estrutura { nome, campos } => {
            let campos = campos.read().map_err(|_| "Estrutura do runtime corrompida.")?;
            Ok(ValorIr::Estrutura { nome: nome.clone(), campos: campos.iter().map(|(nome, valor)| Ok((nome.clone(), converter_para_ir(valor.clone())?))).collect::<Result<_, String>>()? })
        }
        ValorRuntime::Lista(itens) => {
            let itens = itens.read().map_err(|_| "Lista do runtime corrompida.")?;
            Ok(ValorIr::Lista(itens.iter().cloned().map(converter_para_ir).collect::<Result<_, _>>()?))
        }
        ValorRuntime::Intervalo { inicio, fim } => Ok(ValorIr::Intervalo(inicio, fim)),
        ValorRuntime::IntervaloSemSinal { inicio, fim } => Ok(ValorIr::IntervaloSemSinal(inicio, fim)),
        ValorRuntime::IntervaloDecimal { inicio, fim } => {
            Ok(ValorIr::IntervaloDecimal(inicio, fim))
        }
        ValorRuntime::Nulo => Ok(ValorIr::Nulo),
        ValorRuntime::Vazio => Ok(ValorIr::Vazio),
    }
}

struct Maquina<'p, 'o> {
    programa: &'p ProgramaIr,
    observador: Option<ObservadorDepuracao<'o>>,
    contexto: ContextoRuntime,
    saida: Vec<String>,
    entrada: Option<VecDeque<String>>,
    passos: usize,
    limite: usize,
    profundidade_chamada: usize,
    pilha_chamadas: Vec<String>,
    limites_escopos: Vec<usize>,
    locais_chamada: Vec<(Option<PathBuf>, Option<IntervaloFonte>)>,
    caminho_fonte: Option<PathBuf>,
    intervalo_fonte: Option<IntervaloFonte>,
}

impl Maquina<'_, '_> {
    fn marcar_localizacao(&mut self, item: &InstrucaoIr) {
        if let InstrucaoIr::Localizada { caminho, intervalo, .. } = item {
            self.caminho_fonte = Some(caminho.clone());
            self.intervalo_fonte = Some(*intervalo);
        }
    }

    fn memoria_mutavel(&mut self) -> Result<&mut Memoria, String> {
        self.contexto.memoria_mutavel().map_err(formatar_erro_runtime)
    }

    fn passo(&mut self) -> Result<(), String> {
        self.passos += 1;
        if self.passos > self.limite {
            return Err("Limite de passos da IR excedido.".into());
        }
        if self.observador.is_some() {
            let escopos = self.contexto.memoria().escopos().into_iter()
                .map(|escopo| escopo.into_iter()
                    .map(|(nome, valor)| Ok((nome, converter_para_ir(valor)?)))
                    .collect::<Result<HashMap<_, _>, String>>())
                .collect::<Result<Vec<_>, String>>()?;
            let evento = EventoDepuracao {
                caminho_fonte: self.caminho_fonte.clone(),
                intervalo_fonte: self.intervalo_fonte,
                passo: self.passos,
                profundidade_chamada: self.profundidade_chamada,
                pilha_chamadas: self.pilha_chamadas.clone(),
                limites_escopos: self.limites_escopos.clone(),
                locais_chamada: self.locais_chamada.clone(),
                escopos,
            };
            if let Some(observador) = self.observador.as_mut() {
                observador(&evento)?;
            }
        }
        Ok(())
    }

    fn bloco_raiz(&mut self, itens: &[InstrucaoIr]) -> Result<Fluxo, String> {
        let mut fluxo = Fluxo::Normal;
        for item in itens {
            self.marcar_localizacao(item);
            self.passo()?;
            fluxo = self.instrucao(item)?;
            if !matches!(&fluxo, Fluxo::Normal) { break; }
        }
        Ok(fluxo)
    }

    fn bloco(&mut self, itens: &[InstrucaoIr]) -> Result<Fluxo, String> {
        self.memoria_mutavel()?.abrir_escopo();
        let resultado = (|| {
            let mut fluxo = Fluxo::Normal;
            for item in itens {
                self.marcar_localizacao(item);
                self.passo()?;
                fluxo = self.instrucao(item)?;
                if !matches!(fluxo, Fluxo::Normal) { break; }
            }
            Ok(fluxo)
        })();
        let fechamento = self.memoria_mutavel()?.fechar_escopo().map_err(formatar_erro_runtime);
        match (resultado, fechamento) {
            (Err(erro), _) => Err(erro),
            (Ok(_), Err(erro)) => Err(erro),
            (Ok(fluxo), Ok(())) => Ok(fluxo),
        }
    }

    fn instrucao(&mut self, item: &InstrucaoIr) -> Result<Fluxo, String> {
        match item {
            InstrucaoIr::Localizada { caminho, intervalo, instrucao } => {
                self.caminho_fonte = Some(caminho.clone());
                self.intervalo_fonte = Some(*intervalo);
                self.instrucao(instrucao)
            }
            InstrucaoIr::Declarar { nome, tipo, valor, constante } => {
                let valor = normalizar_valor(tipo, self.expressao(valor)?)?;
                self.memoria_mutavel()?
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
            InstrucaoIr::Escolha { valor, casos } => {
                let valor = self.expressao(valor)?;
                let (nome_enum, variante, campos) = match valor {
                    ValorIr::Estrutura { nome, campos } => {
                        let Some((tipo, variante)) = nome.rsplit_once("::") else {
                            return Err("Valor não contém uma variante de enumeração válida.".into());
                        };
                        (tipo.to_owned(), variante.to_owned(), campos)
                    }
                    _ => return Err("Valor de 'escolha' não é uma enumeração.".into()),
                };
                let caso = casos.iter().find(|caso| {
                    match (&caso.enumeracao, &caso.variante) {
                        (Some(tipo), Some(nome)) => tipo == &nome_enum && nome == &variante,
                        _ => true,
                    }
                });
                if let Some(caso) = caso {
                    let mut instrucoes = Vec::with_capacity(caso.vinculacoes.len() + caso.corpo.len());
                    for (indice, vinculo) in caso.vinculacoes.iter().enumerate() {
                        let campo = campos.iter().find(|(nome, _)| nome == &indice.to_string())
                            .map(|(_, valor)| valor.clone())
                            .ok_or_else(|| format!("Payload ausente na variante '{variante}'.") )?;
                        instrucoes.push(InstrucaoIr::Declarar {
                            nome: vinculo.clone(),
                            tipo: Tipo::Desconhecido,
                            valor: ExpressaoIr::Constante(campo),
                            constante: true,
                        });
                    }
                    instrucoes.extend(caso.corpo.clone());
                    self.bloco(&instrucoes)
                } else {
                    Err(format!("Nenhum caso corresponde à variante '{variante}'."))
                }
            }
            InstrucaoIr::Enquanto { condicao, corpo } => {
                let localizacao_laco = (self.caminho_fonte.clone(), self.intervalo_fonte);
                loop {
                    self.caminho_fonte = localizacao_laco.0.clone();
                    self.intervalo_fonte = localizacao_laco.1;
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
            InstrucaoIr::Cada { nome, iteravel, corpo } => {
                let localizacao_laco = (self.caminho_fonte.clone(), self.intervalo_fonte);
                let valores = match self.expressao(iteravel)? {
                    ValorIr::Lista(valores) => valores,
                    ValorIr::Texto(texto) => texto.chars().map(ValorIr::Caractere).collect(),
                    ValorIr::Intervalo(inicio, fim) => {
                        let quantidade = inicio.abs_diff(fim).saturating_add(1);
                        if quantidade > 1_000_000 {
                            return Err("O intervalo de 'cada' excede o limite de 1.000.000 elementos.".into());
                        }
                        if inicio <= fim {
                            (inicio..=fim).map(ValorIr::Inteiro).collect()
                        } else {
                            (fim..=inicio).rev().map(ValorIr::Inteiro).collect()
                        }
                    }
                    ValorIr::IntervaloSemSinal(inicio, fim) => {
                        let quantidade = fim.abs_diff(inicio).saturating_add(1);
                        if quantidade > 1_000_000 {
                            return Err("O intervalo de 'cada' excede o limite de 1.000.000 elementos.".into());
                        }
                        if inicio <= fim {
                            (inicio..=fim).map(ValorIr::InteiroSemSinal).collect()
                        } else {
                            (fim..=inicio).rev().map(ValorIr::InteiroSemSinal).collect()
                        }
                    }
                    ValorIr::IntervaloDecimal(inicio, fim) => {
                        if !inicio.is_finite() || !fim.is_finite() {
                            return Err("O intervalo decimal de 'cada' precisa ter limites finitos.".into());
                        }
                        let mut valores = Vec::new();
                        let passo = if inicio <= fim { 1.0 } else { -1.0 };
                        let mut atual = inicio;
                        while if passo > 0.0 { atual <= fim } else { atual >= fim } {
                            if valores.len() == 1_000_000 {
                                return Err("O intervalo decimal de 'cada' excede o limite de 1.000.000 elementos.".into());
                            }
                            valores.push(ValorIr::Decimal(atual));
                            let proximo = atual + passo;
                            if proximo == atual { break; }
                            atual = proximo;
                        }
                        valores
                    }
                    _ => return Err("O laço 'cada' exige lista, texto ou intervalo.".into()),
                };
                for valor in valores {
                    self.memoria_mutavel()?.abrir_escopo();
                    let resultado = (|| {
                        self.memoria_mutavel()?
                            .declarar(nome.clone(), converter_para_runtime(valor), false)
                            .map_err(formatar_erro_runtime)?;
                        self.bloco(corpo)
                    })();
                    let fechamento = self.memoria_mutavel()?.fechar_escopo().map_err(formatar_erro_runtime);
                    let fluxo = match (resultado, fechamento) {
                        (Err(erro), _) => return Err(erro),
                        (Ok(_), Err(erro)) => return Err(erro),
                        (Ok(fluxo), Ok(())) => fluxo,
                    };
                    self.caminho_fonte = localizacao_laco.0.clone();
                    self.intervalo_fonte = localizacao_laco.1;
                    self.passo()?;
                    match fluxo {
                        Fluxo::Normal | Fluxo::Seguir => {}
                        Fluxo::Parar => break,
                        retorno @ Fluxo::Retorno(_) => return Ok(retorno),
                    }
                }
                Ok(Fluxo::Normal)
            }
            InstrucaoIr::Pulso { intervalo, corpo } => {
                let localizacao_laco = (self.caminho_fonte.clone(), self.intervalo_fonte);
                let intervalo = match self.expressao(intervalo)? {
                    ValorIr::Inteiro(valor) => u64::try_from(valor),
                    ValorIr::InteiroSemSinal(valor) => u64::try_from(valor),
                    _ => return Err("'pulso' exige um intervalo inteiro não negativo.".into()),
                }
                .map_err(|_| "Intervalo de 'pulso' fora do limite suportado.")?;
                loop {
                    self.caminho_fonte = localizacao_laco.0.clone();
                    self.intervalo_fonte = localizacao_laco.1;
                    self.passo()?;
                    match self.bloco(corpo)? {
                        Fluxo::Normal | Fluxo::Seguir => dormir_milisegundos(intervalo),
                        Fluxo::Parar => break,
                        retorno @ Fluxo::Retorno(_) => return Ok(retorno),
                    }
                }
                Ok(Fluxo::Normal)
            }
            InstrucaoIr::Para { inicial, condicao, atualizacao, corpo } => {
                let localizacao_laco = (self.caminho_fonte.clone(), self.intervalo_fonte);
                self.memoria_mutavel()?.abrir_escopo();
                let resultado = (|| {
                    if let Some(inicial) = inicial { let _ = self.instrucao(inicial)?; }
                    let mut fluxo_final = Fluxo::Normal;
                    loop {
                        self.caminho_fonte = localizacao_laco.0.clone();
                    self.intervalo_fonte = localizacao_laco.1;
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
                let fechamento = self.memoria_mutavel()?.fechar_escopo().map_err(formatar_erro_runtime);
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
            ExpressaoIr::ReferenciaFuncao { nome, .. } => Ok(ValorIr::Funcao(nome.clone())),
            ExpressaoIr::ReferenciaFuncaoQualificada { modulo, nome, .. } => Ok(ValorIr::Funcao(format!("{modulo}::{nome}"))),
            ExpressaoIr::ChamadaIndireta { alvo, argumentos } => {
                let ValorIr::Funcao(nome) = self.expressao(alvo)? else {
                    return Err("A chamada indireta exige um valor de função válido.".into());
                };
                let valores = argumentos.iter().map(|argumento| self.expressao(argumento))
                    .collect::<Result<Vec<_>, _>>()?;
                let funcao = self.programa.funcoes.get(&nome).cloned()
                    .ok_or_else(|| format!("Função referenciada '{nome}' não foi encontrada na IR."))?;
                self.chamar_funcao(&nome, funcao, valores)
            }
            ExpressaoIr::Normalizar { tipo, valor } => normalizar_valor(tipo, self.expressao(valor)?),
            ExpressaoIr::Conversao { tipo, valor, .. } => converter_valor_explicito(tipo, self.expressao(valor)?),
            ExpressaoIr::Variavel(nome) => self.buscar(nome).ok_or_else(|| format!("Variável '{nome}' não encontrada na IR.")),
            ExpressaoIr::Lista(valores) | ExpressaoIr::Tupla(valores) => Ok(ValorIr::Lista(valores.iter().map(|v| self.expressao(v)).collect::<Result<_,_>>()?)),
            ExpressaoIr::ConstrucaoEstrutura { nome, campos } => Ok(ValorIr::Estrutura {
                nome: nome.clone(),
                campos: campos.iter().map(|(campo, valor)| Ok((campo.clone(), self.expressao(valor)?))).collect::<Result<_, String>>()?,
            }),
            ExpressaoIr::ConstrucaoEnumeracao { nome, variante, valores } => {
                let campos = valores.iter().enumerate()
                    .map(|(indice, valor)| Ok((indice.to_string(), self.expressao(valor)?)))
                    .collect::<Result<Vec<_>, String>>()?;
                Ok(ValorIr::Estrutura { nome: format!("{nome}::{variante}"), campos })
            },
            ExpressaoIr::Membro { alvo, campo } => {
                let ValorIr::Estrutura { campos, .. } = self.expressao(alvo)? else { return Err("Acesso de campo exige uma estrutura.".into()); };
                campos.into_iter().find(|(nome, _)| nome == campo).map(|(_, valor)| valor)
                    .ok_or_else(|| format!("Campo '{campo}' não encontrado na estrutura."))
            }
            ExpressaoIr::Unaria { operador, valor } => {
                let valor = self.expressao(valor)?;
                match (*operador, valor) {
                    (OperadorIr::Negacao | OperadorIr::Nao, ValorIr::Logico(v)) => Ok(ValorIr::Logico(!v)),
                    (OperadorIr::Menos, ValorIr::Inteiro(v)) => v.checked_neg().map(ValorIr::Inteiro).ok_or_else(|| "Overflow em negação inteira.".into()),
                    (OperadorIr::Menos, ValorIr::Decimal(v)) => Ok(ValorIr::Decimal(-v)),
                    (OperadorIr::Mais, v @ (ValorIr::Inteiro(_) | ValorIr::InteiroSemSinal(_) | ValorIr::Decimal(_))) => Ok(v),
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
            ExpressaoIr::Atribuicao { nome, valor, tipo } => {
                let valor = normalizar_valor(tipo, self.expressao(valor)?)?;
                self.memoria_mutavel()?
                    .atribuir(nome, converter_para_runtime(valor.clone()))
                    .map_err(formatar_erro_runtime)?;
                Ok(valor)
            }
            ExpressaoIr::AtribuicaoCampo { raiz, caminho, valor, tipo } => {
                let valor = normalizar_valor(tipo, self.expressao(valor)?)?;
                self.memoria_mutavel()?
                    .definir_campo_estrutura(raiz, caminho, converter_para_runtime(valor.clone()))
                    .map_err(formatar_erro_runtime)?;
                Ok(valor)
            }
            ExpressaoIr::Chamada { nome, argumentos } => {
                let valores = argumentos.iter().map(|a| self.expressao(a)).collect::<Result<Vec<_>,_>>()?;
                if let Some(funcao) = self.programa.funcoes.get(nome).cloned() {
                    return self.chamar_funcao(nome, funcao, valores);
                }
                if nome == "em_paralelo" {
                    if valores.is_empty() {
                        return Err("'em_paralelo' exige pelo menos uma função sem parâmetros.".into());
                    }
                    let nomes = valores.iter().map(|valor| {
                        if let ValorIr::Funcao(nome) = valor {
                            Ok(nome.clone())
                        } else {
                            Err("'em_paralelo' aceita apenas referências a funções.".to_owned())
                        }
                    }).collect::<Result<Vec<_>, _>>()?;
                    let programa = self.programa;
                    let memoria = self.contexto.memoria().clone();
                    let entrada = self.entrada.clone();
                    let limite = self.limite;
                    let resultados = std::thread::scope(|escopo| {
                        let tarefas = nomes.into_iter().map(|nome| {
                            let memoria = memoria.clone();
                            let entrada = entrada.clone();
                            escopo.spawn(move || -> Result<(ValorIr, Vec<String>), String> {
                                let funcao = programa.funcoes.get(&nome).cloned()
                                    .ok_or_else(|| format!("Função paralela '{nome}' não encontrada."))?;
                                if !funcao.parametros.is_empty() {
                                    return Err(format!("A função paralela '{nome}' precisa não receber parâmetros."));
                                }
                                let mut contexto = ContextoRuntime::novo();
                                *contexto.memoria_mutavel().map_err(formatar_erro_runtime)? = memoria;
                                let mut maquina = Maquina {
                                    programa,
                                    observador: None,
                                    contexto,
                                    saida: Vec::new(),
                                    entrada: entrada.clone(),
                                    passos: 0,
                                    limite,
                                    profundidade_chamada: 0,
                                    pilha_chamadas: Vec::new(),
                                    limites_escopos: Vec::new(),
                                    locais_chamada: Vec::new(),
                                    caminho_fonte: None,
                                    intervalo_fonte: None,
                                };
                                let retorno = maquina.chamar_funcao(&nome, funcao, Vec::new())?;
                                let saida = maquina.saida.clone();
                                maquina.contexto.finalizar().map_err(formatar_erro_runtime)?;
                                Ok((retorno, saida))
                            })
                        }).collect::<Vec<_>>();
                        tarefas.into_iter().map(|tarefa| {
                            tarefa.join().map_err(|_| "Uma tarefa de 'em_paralelo' falhou inesperadamente.".to_owned())?
                        }).collect::<Result<Vec<_>, String>>()
                    })?;
                    let mut retornos = Vec::with_capacity(resultados.len());
                    for (retorno, saida) in resultados {
                        retornos.push(retorno);
                        self.saida.extend(saida);
                    }
                    return Ok(ValorIr::Lista(retornos));
                }
                if nome == "gerar" {
                    let [valor, quantidade] = valores.as_slice() else {
                        return Err("'gerar' exige um valor e uma quantidade.".into());
                    };
                    let quantidade = match quantidade {
                        ValorIr::Inteiro(valor) => usize::try_from(*valor),
                        ValorIr::InteiroSemSinal(valor) => usize::try_from(*valor),
                        _ => return Err("'gerar' exige quantidade inteira.".into()),
                    }.map_err(|_| "Quantidade negativa ou fora do limite suportado em 'gerar'.")?;
                    const LIMITE_GERAR: usize = 1_000_000;
                    if quantidade > LIMITE_GERAR {
                        return Err(format!("'gerar' aceita no máximo {LIMITE_GERAR} elementos por chamada."));
                    }
                    return Ok(ValorIr::Lista(vec![valor.clone(); quantidade]));
                }
                if nome == "exibir" {
                    self.saida.push(valores.iter().map(formatar).collect::<Vec<_>>().join(" "));
                    return Ok(ValorIr::Vazio);
                }
                if nome == "json_valido" {
                    if let [ValorIr::Texto(texto)] = valores.as_slice() {
                        return Ok(ValorIr::Logico(analisar_json(texto).is_ok()));
                    }
                    return Err("'json_valido' exige exatamente um texto.".into());
                }
                if nome == "json_canonico" {
                    if let [ValorIr::Texto(texto)] = valores.as_slice() {
                        let valor = analisar_json(texto)
                            .map_err(|erro| format!("JSON inválido: {erro}"))?;
                        return serializar_json(&valor)
                            .map(ValorIr::Texto)
                            .map_err(|erro| format!("Falha ao serializar JSON: {erro}"));
                    }
                    return Err("'json_canonico' exige exatamente um texto.".into());
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
                if nome == "aparar" {
                    if let [ValorIr::Texto(texto)] = valores.as_slice() {
                        return Ok(ValorIr::Texto(texto_aparar(texto)));
                    }
                    return Err("'aparar' exige um texto.".into());
                }
                if nome == "comeca_com" || nome == "termina_com" {
                    if let [ValorIr::Texto(texto), ValorIr::Texto(trecho)] = valores.as_slice() {
                        let resultado = if nome == "comeca_com" {
                            texto_comeca_com(texto, trecho)
                        } else {
                            texto_termina_com(texto, trecho)
                        };
                        return Ok(ValorIr::Logico(resultado));
                    }
                    return Err(format!("'{nome}' exige dois textos."));
                }
                if nome == "substituir" {
                    if let [
                        ValorIr::Texto(texto),
                        ValorIr::Texto(alvo),
                        ValorIr::Texto(substituto),
                    ] = valores.as_slice()
                    {
                        return Ok(ValorIr::Texto(texto_substituir(texto, alvo, substituto)));
                    }
                    return Err("'substituir' exige texto, trecho e substituição.".into());
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
                if nome == "ler_arquivo" {
                    if let [ValorIr::Texto(caminho)] = valores.as_slice() {
                        return ler_arquivo(caminho)
                            .map(ValorIr::Texto)
                            .map_err(|erro| format!("Falha ao ler arquivo: {erro:?}"));
                    }
                    return Err("'ler_arquivo' exige um caminho de texto.".into());
                }
                if nome == "escrever_arquivo" {
                    if let [ValorIr::Texto(caminho), ValorIr::Texto(conteudo)] = valores.as_slice() {
                        escrever_arquivo(caminho, conteudo)
                            .map_err(|erro| format!("Falha ao escrever arquivo: {erro:?}"))?;
                        return Ok(ValorIr::Logico(true));
                    }
                    return Err("'escrever_arquivo' exige caminho e conteúdo de texto.".into());
                }
                if nome == "existe_arquivo" {
                    if let [ValorIr::Texto(caminho)] = valores.as_slice() {
                        return Ok(ValorIr::Logico(arquivo_existe(caminho)));
                    }
                    return Err("'existe_arquivo' exige um caminho de texto.".into());
                }
                if nome == "criar_diretorio" {
                    if let [ValorIr::Texto(caminho)] = valores.as_slice() {
                        criar_diretorio(caminho)
                            .map_err(|erro| format!("Falha ao criar diretório: {erro:?}"))?;
                        return Ok(ValorIr::Logico(true));
                    }
                    return Err("'criar_diretorio' exige um caminho de texto.".into());
                }
                if nome == "remover_arquivo" {
                    if let [ValorIr::Texto(caminho)] = valores.as_slice() {
                        remover_arquivo(caminho)
                            .map_err(|erro| format!("Falha ao remover arquivo: {erro:?}"))?;
                        return Ok(ValorIr::Logico(true));
                    }
                    return Err("'remover_arquivo' exige um caminho de texto.".into());
                }
                if nome == "copiar_arquivo" {
                    if let [ValorIr::Texto(origem), ValorIr::Texto(destino)] = valores.as_slice() {
                        let bytes = copiar_arquivo(origem, destino)
                            .map_err(|erro| format!("Falha ao copiar arquivo: {erro:?}"))?;
                        return Ok(ValorIr::Inteiro(i128::from(bytes)));
                    }
                    return Err("'copiar_arquivo' exige origem e destino de texto.".into());
                }
                if nome == "mover_arquivo" {
                    if let [ValorIr::Texto(origem), ValorIr::Texto(destino)] = valores.as_slice() {
                        mover_arquivo(origem, destino)
                            .map_err(|erro| format!("Falha ao mover arquivo: {erro:?}"))?;
                        return Ok(ValorIr::Logico(true));
                    }
                    return Err("'mover_arquivo' exige origem e destino de texto.".into());
                }
                if nome == "diretorio_existe" {
                    if let [ValorIr::Texto(caminho)] = valores.as_slice() {
                        return Ok(ValorIr::Logico(diretorio_existe(caminho)));
                    }
                    return Err("'diretorio_existe' exige um caminho de texto.".into());
                }
                if nome == "listar_diretorio" {
                    if let [ValorIr::Texto(caminho)] = valores.as_slice() {
                        let nomes = listar_diretorio(caminho)
                            .map_err(|erro| format!("Falha ao listar diretório: {erro:?}"))?;
                        return Ok(ValorIr::Lista(
                            nomes.into_iter().map(ValorIr::Texto).collect(),
                        ));
                    }
                    return Err("'listar_diretorio' exige um caminho de texto.".into());
                }
                if nome == "tempo" {
                    if !valores.is_empty() {
                        return Err("'tempo' não aceita argumentos.".into());
                    }
                    let milissegundos = tempo_unix_milisegundos()
                        .map_err(|erro| format!("Falha ao consultar relógio: {erro:?}"))?;
                    return i128::try_from(milissegundos)
                        .map(ValorIr::Inteiro)
                        .map_err(|_| "Relógio excedeu a faixa inteira da Nebula.".into());
                }
                if nome == "dormir" || nome == "aguardar" {
                    let milissegundos = match valores.as_slice() {
                        [ValorIr::Inteiro(valor)] => u64::try_from(*valor),
                        [ValorIr::InteiroSemSinal(valor)] => u64::try_from(*valor),
                        _ => return Err(format!("'{nome}' exige um inteiro não negativo.")),
                    }
                    .map_err(|_| format!("Duração inválida para '{nome}'."))?;
                    dormir_milisegundos(milissegundos);
                    return Ok(ValorIr::Vazio);
                }
                if matches!(
                    nome.as_str(),
                    "raiz" | "seno" | "cosseno" | "tangente" | "logaritmo"
                        | "exponencial" | "piso" | "teto" | "arredondar" | "absoluto"
                ) {
                    let valor = match valores.as_slice() {
                        [ValorIr::Inteiro(valor)] => *valor as f64,
                        [ValorIr::Decimal(valor)] => *valor,
                        _ => return Err(format!("'{nome}' exige um argumento numérico.")),
                    };
                    let funcao = match nome.as_str() {
                        "raiz" => FuncaoMatematica::Raiz,
                        "seno" => FuncaoMatematica::Seno,
                        "cosseno" => FuncaoMatematica::Cosseno,
                        "tangente" => FuncaoMatematica::Tangente,
                        "logaritmo" => FuncaoMatematica::Logaritmo,
                        "exponencial" => FuncaoMatematica::Exponencial,
                        "piso" => FuncaoMatematica::Piso,
                        "teto" => FuncaoMatematica::Teto,
                        "arredondar" => FuncaoMatematica::Arredondar,
                        "absoluto" => FuncaoMatematica::Absoluto,
                        _ => unreachable!("nome validado acima"),
                    };
                    return calcular_matematica(funcao, valor)
                        .map(ValorIr::Decimal)
                        .map_err(|erro| format!("Falha matemática em '{nome}': {erro:?}"));
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
                    self.memoria_mutavel()?
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
                    let valor = self.memoria_mutavel()?
                        .remover_elemento_lista(variavel, indice)
                        .map_err(formatar_erro_runtime)?;
                    return converter_para_ir(valor);
                }
                if matches!(nome.as_str(), "ler_texto" | "receber" | "perguntar") {
                    if valores.len() > 1 {
                        return Err(format!("'{nome}' aceita no máximo um texto de prompt."));
                    }
                    if let Some(prompt) = valores.first() {
                        let ValorIr::Texto(prompt) = prompt else {
                            return Err(format!("O prompt de '{nome}' precisa ser texto."));
                        };
                        print!("{prompt}");
                        let mut stdout = std::io::stdout();
                        std::io::Write::flush(&mut stdout)
                            .map_err(|erro| format!("Falha ao exibir prompt: {erro}"))?;
                    }
                    let mut texto = if let Some(entrada) = self.entrada.as_mut() {
                        entrada.pop_front().ok_or_else(|| {
                            "A entrada fornecida para 'receber' foi esgotada.".to_owned()
                        })?
                    } else {
                        let mut linha = String::new();
                        std::io::stdin()
                            .read_line(&mut linha)
                            .map_err(|erro| format!("Falha ao ler texto: {erro}"))?;
                        linha
                    };
                    if texto.ends_with('\n') { texto.pop(); }
                    if texto.ends_with('\r') { texto.pop(); }
                    return Ok(ValorIr::Texto(texto));
                }
                let funcao = self.programa.funcoes.get(nome).cloned().ok_or_else(|| format!("Função '{nome}' não encontrada na IR."))?;
                self.chamar_funcao(nome, funcao, valores)
            }
            ExpressaoIr::ChamadaQualificada { modulo, nome, argumentos } => {
                let valores = argumentos
                    .iter()
                    .map(|argumento| self.expressao(argumento))
                    .collect::<Result<Vec<_>, _>>()?;
                let chave = format!("{modulo}::{nome}");
                let funcao = self.programa.funcoes.get(&chave).cloned()
                    .ok_or_else(|| format!("Função qualificada '{chave}' não foi ligada ao runtime."))?;
                self.chamar_funcao(&chave, funcao, valores)
            }
            ExpressaoIr::Indexacao { alvo, indice } => {
                let alvo = self.expressao(alvo)?;
                let indice = self.expressao(indice)?;
                if let (ValorIr::Lista(valores), ValorIr::Inteiro(i)) = (alvo, indice) {
                    usize::try_from(i).ok().and_then(|i| valores.get(i).cloned()).ok_or_else(|| "Índice fora dos limites da lista.".into())
                } else { Err("Indexação exige lista e índice inteiro.".into()) }
            }
        }
    }

    fn chamar_funcao(
        &mut self,
        nome: &str,
        funcao: FuncaoIr,
        valores: Vec<ValorIr>,
    ) -> Result<ValorIr, String> {
        if funcao.parametros.len() != valores.len() {
            return Err(format!("Quantidade incorreta de argumentos para '{nome}'."));
        }
        if self.profundidade_chamada >= 32 {
            return Err("Limite de profundidade de chamadas da IR excedido.".into());
        }
        let localizacao_chamada = (self.caminho_fonte.clone(), self.intervalo_fonte);
        self.profundidade_chamada += 1;
        let limite_escopos = self.contexto.memoria().escopos().len();
        self.limites_escopos.push(limite_escopos);
        self.locais_chamada.push(localizacao_chamada.clone());
        self.memoria_mutavel()?.abrir_escopo();
        self.pilha_chamadas.push(nome.to_owned());
        let fluxo = (|| {
            for ((parametro, tipo), valor) in funcao.parametros.iter().zip(valores) {
                let valor = normalizar_valor(tipo, valor)?;
                self.memoria_mutavel()?
                    .declarar(parametro.clone(), converter_para_runtime(valor), true)
                    .map_err(formatar_erro_runtime)?;
            }
            self.bloco(&funcao.corpo)
        })();
        let fechamento = self.memoria_mutavel()?.fechar_escopo().map_err(formatar_erro_runtime);
        self.profundidade_chamada -= 1;
        self.pilha_chamadas.pop();
        self.limites_escopos.pop();
        self.locais_chamada.pop();
        self.caminho_fonte = localizacao_chamada.0;
        self.intervalo_fonte = localizacao_chamada.1;
        let fluxo = match (fluxo, fechamento) {
            (Err(erro), _) => return Err(erro),
            (Ok(_), Err(erro)) => return Err(erro),
            (Ok(fluxo), Ok(())) => fluxo,
        };
        match fluxo {
            Fluxo::Retorno(valor) => normalizar_valor(&funcao.retorno, valor),
            _ => Ok(ValorIr::Vazio),
        }
    }

    fn buscar(&self, nome: &str) -> Option<ValorIr> {
        self.contexto.memoria().obter(nome).ok().and_then(|valor| converter_para_ir(valor).ok())
    }

    fn logico(&self, valor: ValorIr) -> Result<bool, String> {
        if let ValorIr::Logico(v) = valor { Ok(v) } else { Err("Condição da IR não é lógica.".into()) }
    }
}

fn valores_iguais(a: &ValorIr, b: &ValorIr) -> bool {
    match (a, b) {
        (ValorIr::Lista(a), ValorIr::Lista(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| valores_iguais(a, b))
        }
        (
            ValorIr::Estrutura { nome: nome_a, campos: campos_a },
            ValorIr::Estrutura { nome: nome_b, campos: campos_b },
        ) => {
            nome_a == nome_b
                && campos_a.len() == campos_b.len()
                && campos_a.iter().all(|(nome, valor_a)| {
                    campos_b.iter().find(|(outro_nome, _)| outro_nome == nome)
                        .is_some_and(|(_, valor_b)| valores_iguais(valor_a, valor_b))
                })
        }
        _ => a == b,
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
        (ValorIr::InteiroSemSinal(a), Mais, ValorIr::InteiroSemSinal(b)) =>
            a.checked_add(b).map(ValorIr::InteiroSemSinal).ok_or_else(|| "Overflow em soma sem sinal.".into()),
        (ValorIr::InteiroSemSinal(a), Menos, ValorIr::InteiroSemSinal(b)) =>
            a.checked_sub(b).map(ValorIr::InteiroSemSinal).ok_or_else(|| "Underflow em subtração sem sinal.".into()),
        (ValorIr::InteiroSemSinal(a), Vezes, ValorIr::InteiroSemSinal(b)) =>
            a.checked_mul(b).map(ValorIr::InteiroSemSinal).ok_or_else(|| "Overflow em multiplicação sem sinal.".into()),
        (ValorIr::InteiroSemSinal(_), Divide, ValorIr::InteiroSemSinal(0))
        | (ValorIr::InteiroSemSinal(_), Resto, ValorIr::InteiroSemSinal(0)) =>
            Err("Divisão por zero na IR.".into()),
        (ValorIr::InteiroSemSinal(a), Divide, ValorIr::InteiroSemSinal(b)) =>
            Ok(ValorIr::InteiroSemSinal(a / b)),
        (ValorIr::InteiroSemSinal(a), Resto, ValorIr::InteiroSemSinal(b)) =>
            Ok(ValorIr::InteiroSemSinal(a % b)),
        (ValorIr::InteiroSemSinal(a), Potencia, ValorIr::InteiroSemSinal(b))
            if b <= u128::from(u32::MAX) =>
            a.checked_pow(b as u32).map(ValorIr::InteiroSemSinal)
                .ok_or_else(|| "Overflow em potência sem sinal.".into()),
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
        (a, Igual, b) => Ok(ValorIr::Logico(valores_iguais(&a, &b))),
        (a, Diferente, b) => Ok(ValorIr::Logico(!valores_iguais(&a, &b))),
        (ValorIr::Inteiro(a), Menor, ValorIr::Inteiro(b)) => Ok(ValorIr::Logico(a < b)),
        (ValorIr::Inteiro(a), MenorIgual, ValorIr::Inteiro(b)) => Ok(ValorIr::Logico(a <= b)),
        (ValorIr::Inteiro(a), Maior, ValorIr::Inteiro(b)) => Ok(ValorIr::Logico(a > b)),
        (ValorIr::Inteiro(a), MaiorIgual, ValorIr::Inteiro(b)) => Ok(ValorIr::Logico(a >= b)),
        (ValorIr::InteiroSemSinal(a), Menor, ValorIr::InteiroSemSinal(b)) => Ok(ValorIr::Logico(a < b)),
        (ValorIr::InteiroSemSinal(a), MenorIgual, ValorIr::InteiroSemSinal(b)) => Ok(ValorIr::Logico(a <= b)),
        (ValorIr::InteiroSemSinal(a), Maior, ValorIr::InteiroSemSinal(b)) => Ok(ValorIr::Logico(a > b)),
        (ValorIr::InteiroSemSinal(a), MaiorIgual, ValorIr::InteiroSemSinal(b)) => Ok(ValorIr::Logico(a >= b)),
        (ValorIr::Decimal(a), Menor, ValorIr::Decimal(b)) => Ok(ValorIr::Logico(a < b)),
        (ValorIr::Decimal(a), MenorIgual, ValorIr::Decimal(b)) => Ok(ValorIr::Logico(a <= b)),
        (ValorIr::Decimal(a), Maior, ValorIr::Decimal(b)) => Ok(ValorIr::Logico(a > b)),
        (ValorIr::Decimal(a), MaiorIgual, ValorIr::Decimal(b)) => Ok(ValorIr::Logico(a >= b)),
        (ValorIr::Inteiro(a), Intervalo, ValorIr::Inteiro(b)) => Ok(ValorIr::Intervalo(a,b)),
        (ValorIr::InteiroSemSinal(a), Intervalo, ValorIr::InteiroSemSinal(b)) => Ok(ValorIr::IntervaloSemSinal(a,b)),
        _ => Err("Operandos incompatíveis para operação na IR.".into()),
    }
}

fn formatar(valor: &ValorIr) -> String {
    match valor {
        ValorIr::Inteiro(v) => v.to_string(), ValorIr::InteiroSemSinal(v) => v.to_string(), ValorIr::Decimal(v) => v.to_string(),
        ValorIr::Logico(v) => if *v { "verdadeiro".into() } else { "falso".into() },
        ValorIr::Texto(v) => v.clone(), ValorIr::Caractere(v) => v.to_string(),
        ValorIr::Funcao(nome) => format!("<função {nome}>"),
        ValorIr::Lista(v) => format!("[{}]", v.iter().map(formatar).collect::<Vec<_>>().join(", ")),
        ValorIr::Estrutura { nome, campos } => format!("{nome} {{ {} }}", campos.iter().map(|(campo, valor)| format!("{campo}: {}", formatar(valor))).collect::<Vec<_>>().join(", ")),
        ValorIr::Intervalo(a,b) => format!("{a}..{b}"),
        ValorIr::IntervaloSemSinal(a,b) => format!("{a}..{b}"),
        ValorIr::IntervaloDecimal(a,b) => format!("{a}..{b}"),
        ValorIr::Nulo => "nulo".into(), ValorIr::Vazio => String::new(),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use nebula_fonte::ArquivoFonte;
    use nebula_sintaxe::analisar;

    fn caminho_portavel(caminho: &std::path::Path) -> String {
        caminho.to_string_lossy().replace('\\', "/")
    }

    fn normalizar_blocos_legados(codigo: &str) -> String {
        if codigo.contains(": {") {
            return codigo.to_owned();
        }

        #[derive(Clone, Copy, PartialEq, Eq)]
        enum TipoBloco {
            Completo,
            Condicional,
            Escolha,
            Caso,
        }

        struct Bloco {
            indentacao: usize,
            tipo: TipoBloco,
        }

        fn indentacao(texto: &str, nivel: usize) -> String {
            format!("{}{}", " ".repeat(nivel), texto)
        }

        fn fechar(saida: &mut Vec<String>, bloco: Bloco) {
            match bloco.tipo {
                TipoBloco::Caso => {
                    saida.push(indentacao("fim", bloco.indentacao));
                    saida.push(indentacao("}", bloco.indentacao));
                }
                TipoBloco::Completo | TipoBloco::Condicional | TipoBloco::Escolha => {
                    saida.push(indentacao("fim", bloco.indentacao));
                    saida.push(indentacao("}", bloco.indentacao));
                    saida.push(indentacao("fim", bloco.indentacao));
                }
            }
        }

        let mut saida = Vec::new();
        let mut blocos: Vec<Bloco> = Vec::new();

        for linha in codigo.lines() {
            let conteudo = linha.trim();
            let nivel = linha.len() - linha.trim_start().len();

            if conteudo.is_empty() {
                saida.push(linha.to_owned());
                continue;
            }

            if conteudo.starts_with("senao") {
                if let Some(bloco) = blocos.last() {
                    if bloco.tipo == TipoBloco::Condicional && bloco.indentacao == nivel {
                        saida.push(indentacao("fim", nivel));
                        saida.push(indentacao("}", nivel));
                        saida.push(indentacao("senao: {", nivel));
                        continue;
                    }
                }
            }

            if conteudo.starts_with("caso ") || conteudo == "padrao" {
                if blocos.last().is_some_and(|bloco| {
                    bloco.tipo == TipoBloco::Caso && bloco.indentacao == nivel
                }) {
                    if let Some(bloco) = blocos.pop() {
                        fechar(&mut saida, bloco);
                    }
                }
                saida.push(indentacao(&format!("{conteudo}: {{"), nivel));
                blocos.push(Bloco {
                    indentacao: nivel,
                    tipo: TipoBloco::Caso,
                });
                continue;
            }

            if conteudo == "fim" {
                while blocos.last().is_some_and(|bloco| {
                    bloco.tipo == TipoBloco::Caso && bloco.indentacao > nivel
                }) {
                    if let Some(bloco) = blocos.pop() {
                        fechar(&mut saida, bloco);
                    }
                }

                if blocos.last().is_some_and(|bloco| bloco.indentacao == nivel) {
                    if let Some(bloco) = blocos.pop() {
                        fechar(&mut saida, bloco);
                    }
                } else {
                    saida.push(linha.to_owned());
                }
                continue;
            }

            let (tipo, cabecalho) = if conteudo.starts_with("publico funcao ")
                || conteudo.starts_with("funcao ")
            {
                let assinatura = conteudo.replace("): ", ") -> ");
                let assinatura = if assinatura.ends_with(':') {
                    format!("{assinatura} {{")
                } else {
                    format!("{assinatura}: {{")
                };
                (Some(TipoBloco::Completo), Some(assinatura))
            } else if conteudo.starts_with("estrutura ") || conteudo.starts_with("enumeracao ") {
                let assinatura = if conteudo.ends_with(':') {
                    format!("{conteudo} {{")
                } else {
                    format!("{conteudo}: {{")
                };
                (Some(TipoBloco::Completo), Some(assinatura))
            } else if conteudo.starts_with("escolha ") {
                (Some(TipoBloco::Escolha), Some(format!("{conteudo}: {{")))
            } else if conteudo.starts_with("quando ")
                || conteudo.starts_with("se ")
            {
                (Some(TipoBloco::Condicional), Some(format!("{conteudo}: {{")))
            } else if conteudo.starts_with("cada ")
                || conteudo.starts_with("enquanto ")
                || conteudo.starts_with("para ")
                || conteudo.starts_with("pulso(")
                || conteudo.starts_with("durante ")
                || conteudo.starts_with("repetir ")
            {
                (Some(TipoBloco::Completo), Some(format!("{conteudo}: {{")))
            } else {
                (None, None)
            };

            if let (Some(tipo), Some(cabecalho)) = (tipo, cabecalho) {
                saida.push(indentacao(&cabecalho, nivel));
                blocos.push(Bloco {
                    indentacao: nivel,
                    tipo,
                });
            } else {
                saida.push(linha.to_owned());
            }
        }

        saida.join("\n")
    }

    fn compilar(codigo: &str) -> ProgramaIr {
        let codigo = normalizar_blocos_legados(codigo);
        let fonte = ArquivoFonte::novo("teste.nbx", &codigo);
        let sintaxe = analisar(&fonte);
        assert!(sintaxe.diagnosticos.is_empty(), "{:?}", sintaxe.diagnosticos);
        let resultado = baixar(&sintaxe.programa, &ModulosDisponiveis::new());
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
        let mut programa = resultado.programa.expect("IR válida");
        fn deslocalizar(instrucao: InstrucaoIr) -> InstrucaoIr {
            match instrucao {
                InstrucaoIr::Localizada { instrucao, .. } => deslocalizar(*instrucao),
                InstrucaoIr::Bloco(itens) => InstrucaoIr::Bloco(itens.into_iter().map(deslocalizar).collect()),
                InstrucaoIr::Se { condicao, entao, senao } => InstrucaoIr::Se { condicao, entao: entao.into_iter().map(deslocalizar).collect(), senao: senao.into_iter().map(deslocalizar).collect() },
                InstrucaoIr::Escolha { valor, casos } => InstrucaoIr::Escolha { valor, casos: casos.into_iter().map(|mut caso| { caso.corpo = caso.corpo.into_iter().map(deslocalizar).collect(); caso }).collect() },
                InstrucaoIr::Enquanto { condicao, corpo } => InstrucaoIr::Enquanto { condicao, corpo: corpo.into_iter().map(deslocalizar).collect() },
                InstrucaoIr::Cada { nome, iteravel, corpo } => InstrucaoIr::Cada { nome, iteravel, corpo: corpo.into_iter().map(deslocalizar).collect() },
                InstrucaoIr::Pulso { intervalo, corpo } => InstrucaoIr::Pulso { intervalo, corpo: corpo.into_iter().map(deslocalizar).collect() },
                InstrucaoIr::Para { inicial, condicao, atualizacao, corpo } => InstrucaoIr::Para { inicial: inicial.map(|i| Box::new(deslocalizar(*i))), condicao, atualizacao, corpo: corpo.into_iter().map(deslocalizar).collect() },
                outra => outra,
            }
        }
        programa.instrucoes = programa.instrucoes.into_iter().map(deslocalizar).collect();
        for funcao in programa.funcoes.values_mut() {
            funcao.corpo = std::mem::take(&mut funcao.corpo).into_iter().map(deslocalizar).collect();
        }
        programa
    }

    #[test]
    fn infere_genericos_por_aliases_de_vetor_e_matriz_na_ir() {
        let lowerer = Lowerer {
            diagnosticos: Vec::new(),
            aliases: HashMap::new(),
            tipos_agregados: HashMap::new(),
            campos_estruturas: HashMap::new(),
            variantes_enumeracoes: HashMap::new(),
            assinaturas_funcoes: HashMap::new(),
            modulos_importados: HashMap::new(),
            genericas: HashMap::new(),
            instancias: HashMap::new(),
            fila_instancias: Vec::new(),
            escopos_tipos: vec![HashMap::new()],
            substituicoes: HashMap::new(),
            localizacoes: HashMap::new(),
            caminho_fonte: PathBuf::new(),
        };
        let genericos = vec!["T".to_owned()];

        for (esperado, obtido) in [
            ("Vetor<T>", Tipo::Lista(Box::new(Tipo::Inteiro))),
            ("Matriz<T>", Tipo::Lista(Box::new(Tipo::Lista(Box::new(Tipo::Decimal64))))),
        ] {
            let mut substituicoes = HashMap::new();
            assert!(lowerer.inferir_substituicao(
                esperado,
                &obtido,
                &genericos,
                &mut substituicoes,
            ));
            let tipo_inferido = lowerer.tipo(substituicoes.get("T").unwrap());
            assert_eq!(
                tipo_inferido,
                if esperado.starts_with("Vetor") { Tipo::Inteiro } else { Tipo::Decimal64 },
            );
        }
    }

    #[test]
    fn falha_de_unificacao_composta_nao_deixa_substituicoes_parciais() {
        let lowerer = Lowerer {
            diagnosticos: Vec::new(),
            aliases: HashMap::new(),
            tipos_agregados: HashMap::new(),
            campos_estruturas: HashMap::new(),
            variantes_enumeracoes: HashMap::new(),
            assinaturas_funcoes: HashMap::new(),
            modulos_importados: HashMap::new(),
            genericas: HashMap::new(),
            instancias: HashMap::new(),
            fila_instancias: Vec::new(),
            escopos_tipos: vec![HashMap::new()],
            substituicoes: HashMap::new(),
            localizacoes: HashMap::new(),
            caminho_fonte: PathBuf::new(),
        };
        let genericos = vec!["T".to_owned()];
        let mut substituicoes = HashMap::new();

        let unificou = lowerer.inferir_substituicao(
            "Tupla<T,T>",
            &Tipo::Tupla(vec![Tipo::Inteiro, Tipo::Texto]),
            &genericos,
            &mut substituicoes,
        );

        assert!(!unificou);
        assert!(substituicoes.is_empty());
    }

    #[test]
    fn infere_retorno_de_builtins_de_entrada_e_consulta() {
        let ir = compilar(
            "variavel nome = receber()\nvariavel instante = tempo()\nvariavel tamanho = comprimento(\"abc\")",
        );
        let tipos: HashMap<_, _> = ir.instrucoes.iter().filter_map(|instrucao| {
            if let InstrucaoIr::Declarar { nome, tipo, .. } = instrucao {
                Some((nome.as_str(), tipo.clone()))
            } else {
                None
            }
        }).collect();
        assert_eq!(tipos.get("nome"), Some(&Tipo::Texto));
        assert_eq!(tipos.get("instante"), Some(&Tipo::Inteiro));
        assert_eq!(tipos.get("tamanho"), Some(&Tipo::Inteiro));
    }

    #[test]
    fn receber_consumindo_entrada_fornecida_pelo_chamador() {
        let ir = compilar(
            "variavel nome = receber()\nvariavel cidade = receber()\nexibir(nome)\nexibir(cidade)",
        );
        let resultado = executar_com_entrada(
            &ir,
            100,
            ["Raphael".to_owned(), "São Paulo".to_owned()],
        )
        .expect("entrada fornecida deve ser consumida");
        assert_eq!(resultado.saida, vec!["Raphael", "São Paulo"]);
    }

    #[test]
    fn receber_falha_quando_a_entrada_fornecida_termina() {
        let ir = compilar("variavel nome = receber()\nvariavel cidade = receber()");
        let erro = executar_com_entrada(&ir, 100, ["Raphael".to_owned()])
            .expect_err("a segunda leitura deve detectar EOF determinístico");
        assert!(erro.contains("entrada fornecida"), "{erro}");
    }

    #[test]
    fn executa_comandos_de_controle_e_colecoes_em_conjunto() {
        let ir = compilar(
            "funcao incrementar(valor: inteiro): inteiro\n    devolver valor + 1\nfim\n\ndefinir contador = 0\nalterar contador <- incrementar(contador)\nquando contador == 1\n    exibir(\"quando\")\nsenao\n    exibir(\"senao\")\nfim\navaliar Opcional::Algum(contador)\n    caso Opcional::Algum(valor)\n        exibir(valor)\n    caso Opcional::Nenhum\n        exibir(0)\nfim\nrepetir contador < 3\n    alterar contador <- contador + 1\nfim\ndurante contador < 4\n    alterar contador <- contador + 1\nfim\ncada item em gerar(2, 2)\n    alterar contador <- contador + item\nfim\npulso(0)\n    alterar contador <- contador + 1\n    quando contador >= 9\n        parar\n    fim\nfim\n\naguardar(0)\nexibir(contador)",
        );
        let resultado = executar(&ir, 500).expect("comandos de controle devem executar");
        assert_eq!(resultado.saida, vec!["quando", "1", "9"]);
    }

    #[test]
    fn executa_igualdade_estrutural_de_agregados() {
        let ir = compilar(
            "estrutura Pessoa:\n    nome: texto\n    idade: inteiro\nfim\nenumeracao Estado:\n    Ativo\n    Falha(texto)\nfim\nvariavel a = Pessoa { nome: \"Lia\", idade: 20, fim }\nvariavel b = Pessoa { nome: \"Lia\", idade: 20, fim }\nvariavel c = Pessoa { nome: \"Lia\", idade: 21, fim }\nexibir(a == b)\nexibir(a != c)\nexibir((1, \"x\") == (1, \"x\"))\nexibir([1, 2] == [1, 2])\nexibir(Estado::Falha(\"erro\") == Estado::Falha(\"erro\"))\nexibir(Opcional::Algum(1) == Opcional::Algum(1))\nvariavel falha: Resultado<inteiro,texto> = Resultado::Falha(\"x\")\nvariavel sucesso: Resultado<inteiro,texto> = Resultado::Sucesso(1)\nexibir(falha != sucesso)",
        );
        let resultado = executar(&ir, 300).expect("igualdade estrutural válida");
        assert_eq!(
            resultado.saida,
            vec!["verdadeiro", "verdadeiro", "verdadeiro", "verdadeiro", "verdadeiro", "verdadeiro", "verdadeiro"]
        );
    }

    #[test]
    fn executa_copia_explicita_de_estrutura_derivada() {
        let ir = compilar(
            "estrutura Ponto:\n    x: inteiro8\nfim\nvariavel origem = Ponto { x: 7, fim }\nvariavel destino = copiar(origem)\nexibir(origem.x)\nexibir(destino.x)",
        );
        let resultado = executar(&ir, 100).expect("cópia explícita de estrutura");
        assert_eq!(resultado.saida, vec!["7", "7"]);
    }

    #[test]
    fn rejeita_inferencia_generica_com_tipo_aninhado_desconhecido() {
        let fonte = ArquivoFonte::novo(
            "teste.nbx",
            "funcao identidade<T>(valor: T) -> T: {\n    devolver valor\n    fim\n} fim\n\nvariavel ausente = identidade(Opcional::Nenhum())",
        );
        let sintaxe = analisar(&fonte);
        assert!(sintaxe.diagnosticos.is_empty(), "{:?}", sintaxe.diagnosticos);
        let resultado = baixar(&sintaxe.programa, &ModulosDisponiveis::new());
        assert!(resultado.programa.is_none());
        assert!(
            resultado.diagnosticos.iter().any(|d| d.contains("N3032")),
            "{:?}",
            resultado.diagnosticos
        );
    }

    #[test]
    fn rejeita_unificacao_generica_inconsistente() {
        let fonte = ArquivoFonte::novo(
            "teste.nbx",
            "funcao par<T>(a: T, b: T) -> T: {\n    devolver a\n    fim\n} fim\n\nvariavel resultado = par([1], [verdadeiro])",
        );
        let sintaxe = analisar(&fonte);
        assert!(sintaxe.diagnosticos.is_empty(), "{:?}", sintaxe.diagnosticos);
        let resultado = baixar(&sintaxe.programa, &ModulosDisponiveis::new());
        assert!(resultado.programa.is_none());
        assert!(
            resultado.diagnosticos.iter().any(|d| d.contains("N3032")),
            "{:?}",
            resultado.diagnosticos
        );
    }

    #[test]
    fn executa_chamada_indireta_de_campo_de_estrutura() {
        let ir = compilar(
            "estrutura Operacoes:\n    callback: funcao(inteiro): inteiro\nfim\nfuncao somar(valor: inteiro): inteiro\n    devolver valor + 2\nfim\n\nvariavel operacoes = Operacoes { callback: somar, fim }\nexibir(operacoes.callback(40))",
        );
        let resultado = executar(&ir, 200).expect("chamada por campo de função válida");
        assert_eq!(resultado.saida, vec!["42"]);
    }

    #[test]
    fn executa_chamada_indireta_de_retorno_de_funcao() {
        let ir = compilar(
            "funcao somar(valor: inteiro): inteiro\n    devolver valor + 1\nfim\nfuncao consultar(): funcao(inteiro): inteiro\n    devolver somar\nfim\n\nexibir(consultar()(41))",
        );
        let resultado = executar(&ir, 200).expect("chamada de função retornada válida");
        assert_eq!(resultado.saida, vec!["42"]);
    }

    #[test]
    fn executa_valor_de_funcao_e_chamada_indireta() {
        let ir = compilar(
            "funcao somar(valor: inteiro): inteiro\n    devolver valor + 1\nfim\n\nvariavel callback: funcao(inteiro): inteiro = somar\nexibir(callback(41))",
        );
        let resultado = executar(&ir, 200).expect("chamada indireta válida");
        assert_eq!(resultado.saida, vec!["42"]);
    }

    #[test]
    fn executa_conversoes_numericas_explicitas() {
        let ir = compilar(
            "variavel pequeno = inteiro8(127)\nvariavel sem_sinal = inteiro_u8(255)\nvariavel ampliado = inteiro8(inteiro_u16(42))\nvariavel decimal = decimal32(16777217)\nvariavel truncado = inteiro8(1.9)\nvariavel zero = inteiro_u8(-0.5)\nvariavel grande = inteiro128(42.9)\nvariavel grande_sem_sinal = inteiro_u128(42.9)\nvariavel decimal_de_128 = decimal64(inteiro128(42))\nexibir(truncado)\nexibir(zero)\nexibir(pequeno)\nexibir(sem_sinal)\nexibir(ampliado)\nexibir(decimal)\nexibir(grande)\nexibir(grande_sem_sinal)\nexibir(decimal_de_128)",
        );
        let resultado = executar(&ir, 200).expect("conversões explícitas válidas");
        assert_eq!(resultado.saida, vec!["1", "0", "127", "255", "42", "16777216", "42", "42", "42"]);
    }

    #[test]
    fn rejeita_conversao_explicita_fora_da_faixa() {
        let ir = compilar("variavel valor = inteiro8(127 + 1)");
        assert!(executar(&ir, 100).unwrap_err().contains("Conversão explícita"));
    }

    #[test]
    fn verifica_overflow_em_operacoes_de_largura_reduzida() {
        let assinado = compilar(
            "variavel a: inteiro8 = 127\nvariavel b: inteiro8 = 1\nvariavel c: inteiro8 = a + b",
        );
        assert!(executar(&assinado, 100).unwrap_err().contains("inteiro8"));

        let sem_sinal = compilar(
            "variavel a: inteiro_u8 = 255\nvariavel b: inteiro_u8 = 1\nvariavel c: inteiro_u8 = a + b",
        );
        assert!(executar(&sem_sinal, 100).unwrap_err().contains("inteiro_u8"));
    }

    #[test]
    fn arredonda_decimal32_apos_cada_operacao() {
        let ir = compilar(
            "variavel a: decimal32 = 16777216.0\nvariavel b: decimal32 = 1.0\nvariavel c: decimal32 = 1.0\nexibir(a + b + c)",
        );
        let resultado = executar(&ir, 100).expect("operações decimal32 válidas");
        assert_eq!(resultado.saida, vec!["16777216"]);
    }

    #[test]
    fn normalizacao_da_ir_rejeita_valores_de_tipo_incompativel() {
        assert!(normalizar_valor(&Tipo::Inteiro, ValorIr::Texto("42".into())).is_err());
        assert!(normalizar_valor(&Tipo::Logico, ValorIr::Inteiro(1)).is_err());
        assert!(normalizar_valor(&Tipo::Texto, ValorIr::Caractere('a')).is_err());
        assert!(normalizar_valor(&Tipo::Estrutura("Pessoa".into()), ValorIr::Estrutura {
            nome: "Endereco".into(),
            campos: Vec::new(),
        }).is_err());
    }

    #[test]
    fn normalizacao_da_ir_valida_payloads_algebricos() {
        let opcional_inteiro = Tipo::Opcional(Box::new(Tipo::Inteiro));
        assert!(normalizar_valor(&opcional_inteiro, ValorIr::Estrutura {
            nome: "Opcional::Algum".into(),
            campos: vec![("0".into(), ValorIr::Inteiro(7))],
        }).is_ok());
        assert!(normalizar_valor(&opcional_inteiro, ValorIr::Estrutura {
            nome: "Opcional::Algum".into(),
            campos: vec![("0".into(), ValorIr::Texto("sete".into()))],
        }).is_err());
        let resultado = Tipo::Resultado(Box::new(Tipo::Inteiro), Box::new(Tipo::Texto));
        assert!(normalizar_valor(&resultado, ValorIr::Estrutura {
            nome: "Resultado::Falha".into(),
            campos: vec![("0".into(), ValorIr::Texto("erro".into()))],
        }).is_ok());
        assert!(normalizar_valor(&Tipo::Enumeracao("Estado".into()), ValorIr::Estrutura {
            nome: "Estado::Ativo".into(),
            campos: Vec::new(),
        }).is_ok());
        assert!(normalizar_valor(&Tipo::Enumeracao("Estado".into()), ValorIr::Estrutura {
            nome: "Outro::Ativo".into(),
            campos: Vec::new(),
        }).is_err());
    }

    #[test]
    fn executa_literal_inteiro_u128_acima_de_i128() {
        let ir = compilar(
            "variavel maximo: inteiro_u128 = 340282366920938463463374607431768211455\nexibir(maximo)",
        );
        let resultado = executar(&ir, 100).expect("inteiro sem sinal válido");
        assert_eq!(
            resultado.saida,
            vec!["340282366920938463463374607431768211455"]
        );
    }

    #[test]
    fn opera_inteiros_u128_com_overflow_verificado() {
        assert_eq!(
            binaria(
                ValorIr::InteiroSemSinal(u128::MAX - 1),
                OperadorIr::Mais,
                ValorIr::InteiroSemSinal(1),
            ),
            Ok(ValorIr::InteiroSemSinal(u128::MAX))
        );
        assert!(binaria(
            ValorIr::InteiroSemSinal(u128::MAX),
            OperadorIr::Mais,
            ValorIr::InteiroSemSinal(1),
        )
        .is_err());
        assert!(binaria(
            ValorIr::InteiroSemSinal(0),
            OperadorIr::Menos,
            ValorIr::InteiroSemSinal(1),
        )
        .is_err());
    }

    #[test]
    fn calcula_layout_de_campo_com_tipo_de_funcao() {
        let ir = compilar(
            "estrutura Registro:\n    callback: funcao(inteiro, texto): logico\n    inicializar: funcao(funcao(inteiro): logico): vazio\nfim",
        );
        let layout = ir.layouts_tipos.get("Registro").expect("layout nominal");
        assert_eq!(layout.tamanho, 16);
        assert_eq!(layout.alinhamento, 8);
        assert_eq!(layout.campos[0].tamanho, 8);
        assert_eq!(layout.campos[1].deslocamento, 8);
    }

    #[test]
    fn baixa_assinaturas_com_tipos_nominais() {
        let ir = compilar(
            "estrutura Pessoa:\n    nome: texto\nfim\nenumeracao Estado:\n    Ativo\n    Inativo\nfim\nfuncao receber(pessoa: Pessoa, estado: Estado): Pessoa\n    devolver pessoa\nfim",
        );
        let funcao = ir.funcoes.get("receber").expect("função declarada");
        assert_eq!(funcao.parametros[0].1, Tipo::Estrutura("Pessoa".into()));
        assert_eq!(funcao.parametros[1].1, Tipo::Enumeracao("Estado".into()));
        assert_eq!(funcao.retorno, Tipo::Estrutura("Pessoa".into()));
    }

    #[test]
    fn construi_estrutura_e_le_campos_no_runtime() {
        let ir = compilar(
            "estrutura Pessoa:\n    nome: texto\n    idade: inteiro\nfim\nvariavel pessoa = Pessoa { nome: \"Lia\", idade: 28, fim }\nexibir(pessoa.nome)\nexibir(pessoa.idade)",
        );
        let resultado = executar(&ir, 100).expect("estrutura executável");
        assert_eq!(resultado.saida, vec!["Lia", "28"]);
    }

    #[test]
    fn estrutura_aninhada_le_campo_da_estrutura_interna() {
        let ir = compilar(
            "estrutura Endereco:\n    cidade: texto\nfim\nestrutura Pessoa:\n    endereco: Endereco\nfim\nvariavel pessoa = Pessoa { endereco: Endereco { cidade: \"Recife\", fim }, fim }\nexibir(pessoa.endereco.cidade)",
        );
        let resultado = executar(&ir, 100).expect("estrutura aninhada executável");
        assert_eq!(resultado.saida, vec!["Recife"]);
    }

    #[test]
    fn atribui_campo_de_estrutura_no_runtime() {
        let ir = compilar(
            "estrutura Pessoa:\n    nome: texto\n    idade: inteiro\nfim\nvariavel pessoa = Pessoa { nome: \"Lia\", idade: 28, fim }\npessoa.idade = 29\nexibir(pessoa.idade)",
        );
        let resultado = executar(&ir, 100).expect("atribuição de campo válida");
        assert_eq!(resultado.saida, vec!["29"]);
    }

    #[test]
    fn atribui_campo_de_estrutura_aninhada_no_runtime() {
        let ir = compilar(
            "estrutura Endereco:\n    cidade: texto\nfim\nestrutura Pessoa:\n    endereco: Endereco\nfim\nvariavel pessoa = Pessoa { endereco: Endereco { cidade: \"Recife\", fim }, fim }\npessoa.endereco.cidade = \"Olinda\"\nexibir(pessoa.endereco.cidade)",
        );
        let resultado = executar(&ir, 100).expect("atribuição aninhada válida");
        assert_eq!(resultado.saida, vec!["Olinda"]);
    }

    #[test]
    fn constroi_enumeracao_com_payload_no_runtime() {
        let ir = compilar(
            "enumeracao Resultado:\n    Sucesso(inteiro)\n    Falha(texto)\nfim\nvariavel resultado = Resultado::Sucesso(42)\nexibir(resultado)",
        );
        let resultado = executar(&ir, 100).expect("payload de enumeração válido");
        assert_eq!(resultado.saida, vec!["Resultado::Sucesso { 0: 42 }"]);
    }

    #[test]
    fn executa_construtores_opcional_e_resultado() {
        let ir = compilar(
            "variavel numero: Opcional<inteiro> = Opcional::Algum(42)\nvariavel ausente: Opcional<texto> = Opcional::Nenhum()\nvariavel sucesso: Resultado<inteiro,texto> = Resultado::Sucesso(7)\nvariavel falha: Resultado<inteiro,texto> = Resultado::Falha(\"erro\")\nexibir(numero)\nexibir(ausente)\nexibir(sucesso)\nexibir(falha)",
        );
        let resultado = executar(&ir, 200).expect("construtores algébricos válidos");
        assert_eq!(
            resultado.saida,
            vec![
                "Opcional::Algum { 0: 42 }",
                "Opcional::Nenhum {  }",
                "Resultado::Sucesso { 0: 7 }",
                "Resultado::Falha { 0: erro }",
            ]
        );
    }

    #[test]
    fn executa_escolha_com_payload_de_enumeracao() {
        let ir = compilar(
            "enumeracao Resultado:\n    Sucesso(inteiro)\n    Falha(texto)\nfim\nvariavel resultado = Resultado::Sucesso(42)\nescolha resultado\n    caso Resultado::Sucesso(valor)\n        exibir(valor)\n    caso Resultado::Falha(mensagem)\n        exibir(mensagem)\n    caso _\n        exibir(0)\nfim\n\nvariavel falha = Resultado::Falha(\"opa\")\nescolha falha\n    caso Resultado::Sucesso(valor)\n        exibir(valor)\n    caso Resultado::Falha(mensagem)\n        exibir(mensagem)\nfim",
        );
        let resultado = executar(&ir, 200).expect("pattern matching válido");
        assert_eq!(resultado.saida, vec!["42", "opa"]);
    }

    #[test]
    fn executa_escolha_de_opcional_e_resultado() {
        let ir = compilar(
            "variavel valor: Opcional<inteiro> = Opcional::Algum(42)\nescolha valor\n    caso Opcional::Algum(numero)\n        exibir(numero)\n    caso Opcional::Nenhum\n        exibir(0)\nfim\n\nvariavel falha: Resultado<inteiro,texto> = Resultado::Falha(\"opa\")\nescolha falha\n    caso Resultado::Sucesso(numero)\n        exibir(numero)\n    caso Resultado::Falha(mensagem)\n        exibir(mensagem)\nfim",
        );
        let resultado = executar(&ir, 200).expect("escolha algébrica válida");
        assert_eq!(resultado.saida, vec!["42", "opa"]);
    }

    #[test]
    fn executa_escolha_de_variantes_sem_payload() {
        let ir = compilar(
            "enumeracao Estado:\n    Ativo\n    Inativo\nfim\nvariavel estado = Estado::Ativo\nescolha estado\n    caso Estado::Ativo\n        exibir(\"ativo\")\n    caso Estado::Inativo\n        exibir(\"inativo\")\nfim",
        );
        let resultado = executar(&ir, 200).expect("enumeração sem payload válida");
        assert_eq!(resultado.saida, vec!["ativo"]);
    }

    #[test]
    fn executor_usa_runtime_e_rejeita_atribuicao_em_constante() {
        let mut ir = compilar("constante limite = 9");
        ir.instrucoes.push(InstrucaoIr::Expressao(ExpressaoIr::Atribuicao {
            nome: "limite".into(),
            valor: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(10))),
            tipo: Tipo::Inteiro,
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
            "variavel itens = [1, 2]\nadicionar(itens, 3)\nexibir(comprimento(itens))\nexibir(remover(itens, 0))\nexibir(subtexto(\"ação\", 0, 2))\nexibir(contem(\"Nebula\", \"bul\"))\nexibir(inteiro(\"42\") + 1)\nexibir(decimal(\"1.5\") + 0.5)\nexibir(maiusculas(\"ação\"))\nexibir(minusculas(\"NEBULA\"))\nexibir(caractere(\"ação\", 1))\nexibir(texto(123))",
        );
        let resultado = executar(&ir, 1000).expect("runtime válido");
        assert_eq!(
            resultado.saida,
            vec!["3", "1", "aç", "verdadeiro", "43", "2", "AÇÃO", "nebula", "ç", "123"]
        );
    }

    #[test]
    fn executa_biblioteca_basica_de_arquivos() {
        let caminho = std::env::temp_dir()
            .join(format!("nebula-ir-{}.txt", std::process::id()))
            .to_string_lossy()
            .replace('\\', "/");
        let codigo = format!(
            "escrever_arquivo(\"{caminho}\", \"Nebula\")\nexibir(ler_arquivo(\"{caminho}\"))\nexibir(existe_arquivo(\"{caminho}\"))"
        );
        let ir = compilar(&codigo);
        let resultado = executar(&ir, 1000).expect("operações de arquivo válidas");
        assert_eq!(resultado.saida, vec!["Nebula", "verdadeiro"]);
        std::fs::remove_file(caminho).unwrap();
    }

    #[test]
    fn executa_biblioteca_basica_de_tempo() {
        let ir = compilar("variavel inicio = tempo()\ndormir(0)\nexibir(inicio > 0)");
        let resultado = executar(&ir, 100).expect("runtime de tempo válido");
        assert_eq!(resultado.saida, vec!["verdadeiro"]);
    }

    #[rustfmt::skip]
    #[test]
    fn executa_operacoes_de_copia_movimentacao_e_remocao_de_arquivo() {
        let identificador = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let raiz = std::env::temp_dir().join(format!("nebula-ir-arquivos-{}-{identificador}", std::process::id()));
        std::fs::create_dir_all(&raiz).unwrap();
        let origem = raiz.join("origem.txt");
        let copia = raiz.join("copia.txt");
        let destino = raiz.join("destino.txt");
        std::fs::write(&origem, "Nebula").unwrap();
        let codigo = format!(
            "exibir(copiar_arquivo(\"{}\", \"{}\"))\nexibir(mover_arquivo(\"{}\", \"{}\"))\nexibir(ler_arquivo(\"{}\"))\nexibir(remover_arquivo(\"{}\"))",
            caminho_portavel(&origem),
            caminho_portavel(&copia),
            caminho_portavel(&copia),
            caminho_portavel(&destino),
            caminho_portavel(&destino),
            caminho_portavel(&destino)
        );
        let ir = compilar(&codigo);
        let resultado = executar(&ir, 100).expect("operações de arquivo válidas");
        assert_eq!(resultado.saida, vec!["6", "verdadeiro", "Nebula", "verdadeiro"]);
        std::fs::remove_dir_all(raiz).unwrap();
    }

    #[test]
    fn executa_biblioteca_de_diretorios() {
        let raiz = std::env::temp_dir().join(format!("nebula-ir-listar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        std::fs::write(raiz.join("arquivo.txt"), "").unwrap();
        let caminho = caminho_portavel(&raiz);
        let codigo = format!(
            "exibir(diretorio_existe(\"{caminho}\"))\nexibir(comprimento(listar_diretorio(\"{caminho}\")))"
        );
        let ir = compilar(&codigo);
        let resultado = executar(&ir, 100).expect("biblioteca de diretórios válida");
        assert_eq!(resultado.saida, vec!["verdadeiro", "1"]);
        std::fs::remove_dir_all(raiz).unwrap();
    }

    #[test]
    fn executa_biblioteca_matematica() {
        let ir = compilar(
            "exibir(raiz(9))\nexibir(arredondar(2.6))\nexibir(absoluto(-5))",
        );
        let resultado = executar(&ir, 100).expect("operações matemáticas válidas");
        assert_eq!(resultado.saida, vec!["3", "3", "5"]);

        let ir = compilar("exibir(raiz(-1))");
        assert!(executar(&ir, 100).unwrap_err().contains("DominioMatematico"));
    }

    #[test]
    fn executa_biblioteca_de_texto_unicode() {
        let ir = compilar(
            "exibir(aparar(\"  olá 🌌  \"))\nexibir(comeca_com(\"Nebula\", \"Neb\"))\nexibir(termina_com(\"Nebula\", \"ula\"))\nexibir(substituir(\"a-b-a\", \"a\", \"x\"))",
        );
        let resultado = executar(&ir, 100).expect("builtins de texto válidos");
        assert_eq!(resultado.saida, vec!["olá 🌌", "verdadeiro", "verdadeiro", "x-b-x"]);
    }

    #[test]
    fn executa_expressao_e_chamada_exibir() {
        let ir = compilar("variavel total = 1 + 2\nexibir(total)");
        let r = executar(&ir, 100).expect("execução válida");
        assert_eq!(r.saida, vec!["3"]);
        assert_eq!(r.globais.get("total"), Some(&ValorIr::Inteiro(3)));
    }

    #[test]
    fn executa_tuplas_heterogeneas_e_acesso_por_indice() {
        let ir = compilar(
            "variavel ponto = (10, \"norte\", verdadeiro)\nvariavel x = ponto[0]\nvariavel direcao = ponto[1]\nvariavel ativo = ponto[2]\nexibir(x)\nexibir(direcao)\nexibir(ativo)",
        );
        let resultado = executar(&ir, 100).expect("tupla válida");
        assert_eq!(resultado.saida, vec!["10", "norte", "verdadeiro"]);
        assert_eq!(ir.instrucoes.iter().filter(|instrucao| matches!(
            instrucao,
            InstrucaoIr::Declarar { nome, tipo: Tipo::Tupla(_), .. } if nome == "ponto"
        )).count(), 1);
    }

    #[test]
    fn monomorfiza_generico_com_tupla() {
        let ir = compilar(
            "funcao primeiro<T>(ponto: Tupla<T,T>): T\n    devolver ponto[0]\nfim\n\nvariavel numero = primeiro((7, 8))",
        );
        assert!(ir.funcoes.contains_key("primeiro__inteiro"));
        let resultado = executar(&ir, 100).expect("genérico com tupla válido");
        assert_eq!(resultado.globais.get("numero"), Some(&ValorIr::Inteiro(7)));
    }

    #[test]
    fn monomorfiza_funcao_generica_para_tipos_inferidos() {
        let ir = compilar(
            "funcao identidade<T>(valor: T) -> T: {\n    devolver valor\n    fim\n} fim\n\nvariavel numero = identidade(42)\nvariavel texto = identidade(\"Nebula\")",
        );
        assert!(ir.funcoes.contains_key("identidade__inteiro"));
        assert!(ir.funcoes.contains_key("identidade__texto"));
        let resultado = executar(&ir, 100).expect("instâncias genéricas válidas");
        assert_eq!(resultado.globais.get("numero"), Some(&ValorIr::Inteiro(42)));
        assert_eq!(resultado.globais.get("texto"), Some(&ValorIr::Texto("Nebula".into())));
    }

    #[test]
    fn monomorfiza_generico_com_parametro_composto() {
        let ir = compilar(
            "funcao primeiro<T>(valores: [T]): T\n    devolver valores[0]\nfim\n\nvariavel numero = primeiro([7, 8])",
        );
        assert!(ir.funcoes.contains_key("primeiro__inteiro"));
        let resultado = executar(&ir, 100).expect("instância de lista genérica válida");
        assert_eq!(resultado.globais.get("numero"), Some(&ValorIr::Inteiro(7)));
    }

    #[test]
    fn infere_retorno_de_chamada_generica_aninhada() {
        let ir = compilar(
            "funcao identidade<T>(valor: T) -> T: {\n    devolver valor\n    fim\n} fim\n\nvariavel numero = identidade(identidade(42))",
        );
        assert!(ir.funcoes.contains_key("identidade__inteiro"));
        let resultado = executar(&ir, 100).expect("chamadas genéricas aninhadas válidas");
        assert_eq!(resultado.globais.get("numero"), Some(&ValorIr::Inteiro(42)));
    }

    #[test]
    fn infere_retorno_de_funcao_nao_generica() {
        let ir = compilar(
            "funcao somar(a: inteiro, b: inteiro): inteiro\n    devolver a + b\nfim\n\nvariavel total = somar(20, 22)",
        );
        let declaracao = ir.instrucoes.iter().find_map(|instrucao| match instrucao {
            InstrucaoIr::Declarar { nome, tipo, .. } if nome == "total" => Some(tipo),
            _ => None,
        });
        assert_eq!(declaracao, Some(&Tipo::Inteiro));
        let resultado = executar(&ir, 100).expect("retorno tipado inferido");
        assert_eq!(resultado.globais.get("total"), Some(&ValorIr::Inteiro(42)));
    }

    #[test]
    fn infere_tipo_de_indice_literal_de_tupla_em_chamada_generica() {
        let ir = compilar(
            "funcao extrair<T,U>(ponto: Tupla<T,U>): T\n    devolver ponto[0]\nfim\nfuncao identidade<T>(valor: T) -> T: {\n    devolver valor\n    fim\n} fim\n\nvariavel ponto = (42, \"Nebula\")\nvariavel numero = identidade(extrair(ponto))",
        );
        assert!(ir.funcoes.contains_key("extrair__inteiro__texto"));
        assert!(ir.funcoes.contains_key("identidade__inteiro"));
        let resultado = executar(&ir, 100).expect("inferência por índice de tupla válida");
        assert_eq!(resultado.globais.get("numero"), Some(&ValorIr::Inteiro(42)));
    }

    #[test]
    fn infere_chamada_generica_em_variavel_local() {
        let ir = compilar(
            "funcao identidade<T>(valor: T) -> T: {\n    devolver valor\n    fim\n} fim\nfuncao repassar<T>(valor: T): T\n    variavel copia = identidade(valor)\n    devolver copia\nfim\n\nvariavel numero = repassar(42)",
        );
        assert!(ir.funcoes.contains_key("repassar__inteiro"));
        assert!(ir.funcoes.contains_key("identidade__inteiro"));
        let resultado = executar(&ir, 100).expect("chamada genérica local válida");
        assert_eq!(resultado.globais.get("numero"), Some(&ValorIr::Inteiro(42)));
    }

    #[test]
    fn infere_genericos_em_parametros_e_retorno_de_tipo_funcao() {
        let ir = compilar(
            "funcao identidade(valor: inteiro): inteiro\n    devolver valor\nfim\nfuncao aplicar<T>(operacao: funcao(T): T, valor: T): T\n    devolver operacao(valor)\nfim\n\nvariavel resultado = aplicar(identidade, 42)",
        );
        assert!(ir.funcoes.contains_key("aplicar__inteiro"));
        let resultado = executar(&ir, 100).expect("inferência em tipo de função válida");
        assert_eq!(resultado.globais.get("resultado"), Some(&ValorIr::Inteiro(42)));
    }

    #[test]
    fn executa_funcao_com_retorno() {
        let ir = compilar("funcao soma(a: inteiro, b: inteiro): inteiro\n    devolver a + b\nfim\n\nvariavel total = soma(2, 3)");
        let r = executar(&ir, 100).expect("execução válida");
        assert_eq!(r.globais.get("total"), Some(&ValorIr::Inteiro(5)));
    }

    #[test]
    fn em_paralelo_aguarda_todas_as_funcoes_e_preserva_ordem() {
        let ir = compilar(
            "funcao primeira(): inteiro\n    devolver 11\nfim\nfuncao segunda(): inteiro\n    devolver 22\nfim\n\nvariavel resultados = em_paralelo(primeira, segunda)\nexibir(resultados[0])\nexibir(resultados[1])",
        );
        let resultado = executar(&ir, 300).expect("tarefas paralelas devem concluir");
        assert_eq!(resultado.saida, vec!["11", "22"]);
    }

    #[test]
    fn executa_builtins_json_com_serializacao_deterministica() {
        let ir = compilar(
            "variavel valido = json_valido(\"{\\\"chave\\\":1}\")\nvariavel invalido = json_valido(\"{\\\"chave\\\":}\")\nvariavel canonico = json_canonico(\"{\\\"b\\\":2,\\\"a\\\":1}\")",
        );
        let resultado = executar(&ir, 100).expect("builtins JSON");
        assert_eq!(resultado.globais.get("valido"), Some(&ValorIr::Logico(true)));
        assert_eq!(resultado.globais.get("invalido"), Some(&ValorIr::Logico(false)));
        assert_eq!(
            resultado.globais.get("canonico"),
            Some(&ValorIr::Texto(r#"{"a":1,"b":2}"#.into()))
        );
    }

    #[test]
    fn json_canonico_retorna_erro_para_entrada_invalida() {
        let ir = compilar("variavel canonico = json_canonico(\"{\\\"chave\\\":}\")");
        assert!(executar(&ir, 100).unwrap_err().contains("JSON inválido"));
    }

    #[test]
    fn executa_condicional_e_laco() {
        let ir = compilar("variavel x = 0\nenquanto x < 3\n    x = x + 1\nfim\nquando x == 3\n    exibir(x)\nfim");
        let r = executar(&ir, 100).expect("execução válida");
        assert_eq!(r.saida, vec!["3"]);
    }

    #[test]
    fn interrompe_laco_infinito_por_limite() {
        let ir = compilar("enquanto verdadeiro\n    exibir(1)\nfim");
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
        let ir = compilar("funcao recursiva(n: inteiro): inteiro\n    devolver recursiva(n + 1)\nfim\n\nvariavel x = recursiva(0)");
        assert!(executar(&ir, 10_000).is_err());
    }

    #[test]
    fn executa_intervalo_com_tipo_generico_explicito() {
        let ir = compilar(
            "variavel faixa: Intervalo<inteiro> = 1..3\ncada numero em faixa\n    exibir(numero)\nfim",
        );
        let resultado = executar(&ir, 100).expect("intervalo tipado explícito");
        assert_eq!(resultado.saida, vec!["1", "2", "3"]);
    }

    #[test]
    fn executa_intervalo_u128_acima_do_limite_de_i128() {
        let ir = compilar(
            "variavel inicio: inteiro_u128 = 340282366920938463463374607431768211453\nvariavel limite: inteiro_u128 = 340282366920938463463374607431768211455\nvariavel faixa: Intervalo<inteiro_u128> = inicio..limite\ncada numero em faixa\n    exibir(numero)\nfim",
        );
        let resultado = executar(&ir, 100).expect("intervalo u128 válido");
        assert_eq!(
            resultado.saida,
            vec![
                "340282366920938463463374607431768211453",
                "340282366920938463463374607431768211454",
                "340282366920938463463374607431768211455",
            ],
        );
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
        let ir = compilar("variavel soma = 0\npara (variavel i = 0; i < 3; i = i + 1)\n    soma = soma + i\nfim\n\nexibir(soma)");
        let r = executar(&ir, 200).expect("execução válida");
        assert_eq!(r.saida, vec!["3"]);
    }

    #[test]
    fn executa_parar_e_seguir_em_lacos() {
        let ir = compilar("variavel x = 0\nenquanto x < 5\n    x = x + 1\n    quando x == 2\n        seguir\n    fim\n    quando x == 4\n        parar\n    fim\n    \n    exibir(x)\nfim");
        let r = executar(&ir, 200).expect("execução válida");
        assert_eq!(r.saida, vec!["1", "3"]);
    }


    #[test]
    fn baixa_layouts_nominais_para_a_ir() {
        let ir = compilar(
            "estrutura Pessoa:\n    idade: inteiro8\n    ativo: logico\n    saldo: decimal64\nfim\nenumeracao Estado:\n    Inativo\n    Ativo\n    Falha(texto)\nfim",
        );
        let pessoa = ir.layouts_tipos.get("Pessoa").expect("layout da estrutura");
        assert_eq!(pessoa.tamanho, 16);
        assert_eq!(pessoa.alinhamento, 8);
        let estado = ir.layouts_tipos.get("Estado").expect("layout da enumeração");
        assert_eq!(estado.tamanho_tag, 1);
        assert_eq!(estado.tamanho, 24);
    }

    #[test]
    fn rejeita_layout_nominal_recursivo_ao_baixar_ir() {
        let fonte = ArquivoFonte::novo("recursao.nbx", "estrutura No: {\n    proximo: No\n    fim\n}\nfim");
        let sintaxe = analisar(&fonte);
        assert!(sintaxe.diagnosticos.is_empty(), "{:?}", sintaxe.diagnosticos);
        let resultado = baixar(&sintaxe.programa, &ModulosDisponiveis::new());
        assert!(resultado.programa.is_none());
        assert!(resultado.diagnosticos.iter().any(|d| d.contains("recursivo por valor")));
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
            "incluir(biblioteca: Matematica) como Mat\nvariavel total = Mat::somar(1, 2)",
        );
        let sintaxe = analisar(&fonte);
        assert!(sintaxe.diagnosticos.is_empty(), "{:?}", sintaxe.diagnosticos);
        let resultado = baixar(&sintaxe.programa, &modulos);
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
        let ir = resultado.programa.expect("IR válida");
        assert_eq!(ir.modulos.get("Mat").map(String::as_str), Some("Matematica"));
        let instrucao = match &ir.instrucoes[0] {
            InstrucaoIr::Localizada { instrucao, .. } => instrucao.as_ref(),
            instrucao => instrucao,
        };
        let InstrucaoIr::Declarar { valor, .. } = instrucao else {
            panic!("esperava declaração global");
        };
        assert!(matches!(
            valor,
            ExpressaoIr::ChamadaQualificada { modulo, nome, .. }
                if modulo.as_str() == "Matematica" && nome.as_str() == "somar"
        ));
    }

    #[test]
    fn normalizacao_numerica_promove_inteiros_para_decimais() {
        assert_eq!(
            normalizar_valor(&Tipo::Decimal64, ValorIr::Inteiro(2)).unwrap(),
            ValorIr::Decimal(2.0),
        );
        assert_eq!(
            normalizar_valor(&Tipo::Decimal64, ValorIr::InteiroSemSinal(u128::MAX)).unwrap(),
            ValorIr::Decimal(u128::MAX as f64),
        );
        assert_eq!(
            normalizar_valor(&Tipo::Decimal32, ValorIr::Inteiro(2)).unwrap(),
            ValorIr::Decimal(2.0),
        );
        assert!(normalizar_valor(
            &Tipo::Decimal32,
            ValorIr::InteiroSemSinal(u128::MAX),
        ).is_err());
    }

    #[test]
    fn normaliza_intervalo_inteiro_u128_sem_perda_de_sinal() {
        let maximo = u128::MAX;
        let tipo = Tipo::Intervalo(Box::new(Tipo::InteiroU128));
        assert_eq!(
            normalizar_valor(&tipo, ValorIr::IntervaloSemSinal(maximo - 2, maximo)).unwrap(),
            ValorIr::IntervaloSemSinal(maximo - 2, maximo),
        );
        assert!(normalizar_valor(
            &tipo,
            ValorIr::IntervaloSemSinal(0, maximo),
        ).is_ok());
        assert!(normalizar_valor(
            &Tipo::Intervalo(Box::new(Tipo::InteiroU64)),
            ValorIr::IntervaloSemSinal(0, u64::MAX as u128 + 1),
        ).is_err());
    }

    #[test]
    fn normalizacao_de_intervalos_respeita_tipo_interno() {
        assert!(normalizar_valor(
            &Tipo::Intervalo(Box::new(Tipo::Texto)),
            ValorIr::Intervalo(1, 2),
        ).is_err());
        assert!(normalizar_valor(
            &Tipo::Intervalo(Box::new(Tipo::Inteiro8)),
            ValorIr::Intervalo(-129, 2),
        ).is_err());
        assert!(normalizar_valor(
            &Tipo::Intervalo(Box::new(Tipo::InteiroU8)),
            ValorIr::Intervalo(-1, 2),
        ).is_err());
        assert!(normalizar_valor(
            &Tipo::Intervalo(Box::new(Tipo::InteiroU8)),
            ValorIr::Intervalo(0, 256),
        ).is_err());
        assert!(normalizar_valor(
            &Tipo::Intervalo(Box::new(Tipo::Decimal)),
            ValorIr::Intervalo(1, 2),
        ).is_err());
        assert!(normalizar_valor(
            &Tipo::Intervalo(Box::new(Tipo::Inteiro)),
            ValorIr::IntervaloDecimal(1.0, 2.0),
        ).is_err());
        assert_eq!(
            normalizar_valor(
                &Tipo::Intervalo(Box::new(Tipo::Decimal32)),
                ValorIr::IntervaloDecimal(0.1, 0.2),
            ).unwrap(),
            ValorIr::IntervaloDecimal((0.1f64 as f32) as f64, (0.2f64 as f32) as f64),
        );
    }

    #[test]
    fn normaliza_larguras_e_decimal32() {
        let decimal = normalizar_valor(&Tipo::Decimal32, ValorIr::Decimal(0.1)).unwrap();
        assert_eq!(decimal, ValorIr::Decimal((0.1f64 as f32) as f64));
        assert!(normalizar_valor(&Tipo::Inteiro8, ValorIr::Inteiro(128)).is_err());
        assert!(normalizar_valor(&Tipo::InteiroU8, ValorIr::InteiroSemSinal(256)).is_err());
        assert_eq!(
            normalizar_valor(&Tipo::InteiroU8, ValorIr::InteiroSemSinal(255)).unwrap(),
            ValorIr::InteiroSemSinal(255)
        );
    }


    #[test]
    fn observador_recebe_eventos_antes_de_cada_instrucao() {
        let programa = compilar("variavel contador = 1\ncontador = contador + 1");
        let mut eventos = Vec::new();

        let resultado = executar_observado(&programa, 100, |evento| {
            eventos.push((evento.passo, evento.profundidade_chamada, evento.escopos.clone()));
            Ok(())
        }).expect("execução observada válida");

        assert_eq!(eventos.len(), 2);
        assert_eq!((eventos[0].0, eventos[0].1), (1, 0));
        assert_eq!((eventos[1].0, eventos[1].1), (2, 0));
        assert_eq!(eventos[1].2[0].get("contador"), Some(&ValorIr::Inteiro(1)));
        assert_eq!(resultado.globais.get("contador"), Some(&ValorIr::Inteiro(2)));
    }

    #[test]
    fn controlador_pausa_avanca_inspeciona_e_respeita_pontos() {
        let programa = compilar(
            "variavel contador = 1\ncontador = contador + 1\ncontador = contador + 1"
        );
        let (tx_comandos, rx_comandos) = std::sync::mpsc::channel();
        let (tx_eventos, rx_eventos) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            executar_controlado(&programa, 100, rx_comandos, tx_eventos)
        });

        let primeiro = rx_eventos.recv().expect("primeira pausa");
        let EventoControleDepuracao::Pausado(primeiro) = primeiro else {
            panic!("esperava pausa inicial");
        };
        assert_eq!(primeiro.passo, 1);

        tx_comandos.send(ComandoDepuracao::Avancar).unwrap();
        let segundo = rx_eventos.recv().expect("pausa após avançar");
        let EventoControleDepuracao::Pausado(segundo) = segundo else {
            panic!("esperava segunda pausa");
        };
        assert_eq!(segundo.passo, 2);
        assert_eq!(segundo.escopos[0].get("contador"), Some(&ValorIr::Inteiro(1)));

        tx_comandos
            .send(ComandoDepuracao::DefinirPontos(vec![3]))
            .unwrap();
        tx_comandos.send(ComandoDepuracao::Continuar).unwrap();
        let terceiro = rx_eventos.recv().expect("ponto de parada");
        let EventoControleDepuracao::Pausado(terceiro) = terceiro else {
            panic!("esperava ponto de parada");
        };
        assert_eq!(terceiro.passo, 3);

        tx_comandos.send(ComandoDepuracao::Continuar).unwrap();
        let finalizado = rx_eventos.recv().expect("evento de finalização");
        assert!(matches!(
            finalizado,
            EventoControleDepuracao::Finalizado(Ok(_))
        ));
        let resultado = worker.join().expect("thread de depuração");
        assert_eq!(
            resultado.unwrap().globais.get("contador"),
            Some(&ValorIr::Inteiro(3))
        );
    }

    #[test]
    fn preserva_intervalos_de_origem_na_ir() {
        let fonte = ArquivoFonte::novo("origem.nbx", "variavel primeiro = 1\nvariavel segundo = 2");
        let sintaxe = analisar(&fonte);
        assert!(sintaxe.diagnosticos.is_empty(), "{:?}", sintaxe.diagnosticos);
        let resultado = baixar(&sintaxe.programa, &ModulosDisponiveis::new());
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
        let ir = resultado.programa.expect("IR localizada");
        let InstrucaoIr::Localizada { intervalo, .. } = &ir.instrucoes[1] else {
            panic!("a segunda instrução precisa preservar sua origem");
        };
        assert_eq!(fonte.trecho(*intervalo), Some("variavel segundo = 2"));
        assert_eq!(fonte.linha_coluna(intervalo.inicio), Some((2, 1)));
    }

    #[test]
    fn preserva_localizacoes_em_blocos_e_lacos_aninhados() {
        let fonte = ArquivoFonte::novo(
            "aninhado.nbx",
            "quando verdadeiro: {\n    variavel interno = 7\n    fim\n}\nfim\nenquanto falso: {\n    variavel repetido = 1\n    fim\n}\nfim",
        );
        let sintaxe = analisar(&fonte);
        assert!(sintaxe.diagnosticos.is_empty(), "{:?}", sintaxe.diagnosticos);
        let resultado = baixar(&sintaxe.programa, &ModulosDisponiveis::new());
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
        let ir = resultado.programa.expect("IR aninhada");

        let InstrucaoIr::Localizada { instrucao: condicional, .. } = &ir.instrucoes[0] else {
            panic!("condicional precisa ter origem");
        };
        let InstrucaoIr::Se { entao, .. } = condicional.as_ref() else {
            panic!("esperava condicional");
        };
        let InstrucaoIr::Localizada { intervalo: interno, .. } = &entao[0] else {
            panic!("instrução interna precisa ter origem");
        };
        assert_eq!(fonte.linha_coluna(interno.inicio), Some((2, 5)));

        let InstrucaoIr::Localizada { instrucao: laco, .. } = &ir.instrucoes[1] else {
            panic!("laço precisa ter origem");
        };
        let InstrucaoIr::Enquanto { corpo, .. } = laco.as_ref() else {
            panic!("esperava enquanto");
        };
        let InstrucaoIr::Localizada { intervalo: repetido, .. } = &corpo[0] else {
            panic!("corpo do laço precisa ter origem");
        };
        assert_eq!(fonte.linha_coluna(repetido.inicio), Some((7, 5)));
    }

    #[test]
    fn controlador_interrompe_em_offset_de_linha_fonte() {
        let fonte = ArquivoFonte::novo("pontos.nbx", "variavel contador = 1\ncontador = contador + 1\ncontador = contador + 1");
        let sintaxe = analisar(&fonte);
        let resultado = baixar(&sintaxe.programa, &ModulosDisponiveis::new());
        let programa = resultado.programa.expect("IR válida");
        let offset_segunda = fonte.conteudo().find("contador = contador + 1").expect("segunda linha");
        let (tx_comandos, rx_comandos) = std::sync::mpsc::channel();
        let (tx_eventos, rx_eventos) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || executar_controlado(&programa, 100, rx_comandos, tx_eventos));

        let inicial = rx_eventos.recv().expect("pausa inicial");
        let EventoControleDepuracao::Pausado(inicial) = inicial else {
            panic!("esperava pausa inicial");
        };
        assert_eq!(fonte.linha_coluna(inicial.intervalo_fonte.expect("origem inicial").inicio), Some((1, 1)));

        tx_comandos.send(ComandoDepuracao::DefinirPontosFonte(vec![(fonte.caminho().to_path_buf(), offset_segunda)])).unwrap();
        tx_comandos.send(ComandoDepuracao::Continuar).unwrap();
        let ponto = rx_eventos.recv().expect("pausa no breakpoint de origem");
        let EventoControleDepuracao::Pausado(ponto) = ponto else {
            panic!("esperava pausa por origem");
        };
        assert_eq!(ponto.intervalo_fonte.map(|intervalo| intervalo.inicio), Some(offset_segunda));
        assert_eq!(fonte.linha_coluna(offset_segunda), Some((2, 1)));

        tx_comandos.send(ComandoDepuracao::Continuar).unwrap();
        assert!(matches!(
            rx_eventos.recv().expect("finalização"),
            EventoControleDepuracao::Finalizado(Ok(_))
        ));
        worker.join().expect("thread").expect("execução controlada");
    }

    #[test]
    fn controlador_pausa_e_encerra_execucao_em_andamento() {
        let programa = compilar("variavel primeiro = 1\nvariavel segundo = 2\nvariavel terceiro = 3");
        let (tx_comandos, rx_comandos) = std::sync::mpsc::channel();
        let (tx_eventos, rx_eventos) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || executar_controlado(&programa, 100, rx_comandos, tx_eventos));

        assert!(matches!(
            rx_eventos.recv().expect("pausa inicial"),
            EventoControleDepuracao::Pausado(_)
        ));
        tx_comandos.send(ComandoDepuracao::Pausar).unwrap();
        let pausado = rx_eventos.recv().expect("pausa solicitada");
        let EventoControleDepuracao::Pausado(pausado) = pausado else {
            panic!("esperava pausa solicitada");
        };
        assert_eq!(pausado.passo, 2);

        tx_comandos.send(ComandoDepuracao::Encerrar).unwrap();
        assert!(matches!(
            rx_eventos.recv().expect("finalização"),
            EventoControleDepuracao::Finalizado(Err(_))
        ));
        assert!(worker.join().expect("thread").is_err());
    }

    #[test]
    fn depuracao_nao_consume_o_canal_de_protocolo_com_receber() {
        let programa = compilar("variavel nome = receber()");
        let (tx_comandos, rx_comandos) = std::sync::mpsc::channel();
        let (tx_eventos, rx_eventos) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || executar_controlado(&programa, 100, rx_comandos, tx_eventos));

        assert!(matches!(
            rx_eventos.recv().expect("pausa inicial"),
            EventoControleDepuracao::Pausado(_)
        ));
        tx_comandos.send(ComandoDepuracao::Continuar).unwrap();
        let finalizado = rx_eventos.recv().expect("falha de entrada");
        let EventoControleDepuracao::Finalizado(Err(erro)) = finalizado else {
            panic!("receber sem entrada deve falhar sem ler o protocolo DAP");
        };
        assert!(erro.contains("entrada fornecida"));
        assert!(worker.join().expect("thread").is_err());
    }

    #[test]
    fn observador_pode_interromper_a_execucao_com_erro() {
        let programa = compilar("variavel contador = 1\ncontador = contador + 1");
        let resultado = executar_observado(&programa, 100, |evento| {
            if evento.passo == 2 {
                Err("pausa solicitada pelo depurador".into())
            } else {
                Ok(())
            }
        });

        assert_eq!(resultado, Err("pausa solicitada pelo depurador".into()));
    }

}
}

pub use implementacao::*;

/// Passes de otimização conservadores sobre a IR tipada.
#[rustfmt::skip]
pub mod otimizador;

/// Conversão dos tipos semânticos para o layout físico do runtime.
pub mod layout;
