//! Análise semântica, resolução de nomes e sistema de tipos da Nebula.

#![forbid(unsafe_code)]
#![allow(missing_docs)]

#[rustfmt::skip]
mod implementacao {
use nebula_diagnosticos::{Diagnostico, Severidade};
use nebula_sintaxe::{Expressao, NoAst, Programa, TipoToken};
use std::collections::{HashMap, HashSet};

/// Tipos estáticos disponíveis no subconjunto semântico inicial.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Tipo {
    Vazio, Logico, Inteiro, Inteiro8, Inteiro16, Inteiro32, Inteiro64, Inteiro128, InteiroU8, InteiroU16, InteiroU32, InteiroU64, InteiroU128,
    Decimal, Decimal32, Decimal64, Texto, Caractere, Byte,
    Lista(Box<Tipo>), Intervalo(Box<Tipo>), Opcional(Box<Tipo>), Resultado(Box<Tipo>, Box<Tipo>), Tupla(Vec<Tipo>), Estrutura(String), Enumeracao(String), Funcao(Vec<Tipo>, Box<Tipo>), Parametro(String), Desconhecido,
}

/// Assinatura pública de função disponibilizada por um módulo importado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssinaturaFuncao {
    pub parametros: Vec<Tipo>,
    pub retorno: Tipo,
}

/// Módulos explicitamente disponibilizados pelo chamador, indexados pelo caminho.
pub type ModulosDisponiveis = HashMap<String, HashMap<String, AssinaturaFuncao>>;

/// Resultado da análise semântica.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultadoSemantico {
    pub diagnosticos: Vec<Diagnostico>,
    pub tipos_globais: HashMap<String, Tipo>,
}

/// Analisa nomes, escopos, tipos, chamadas e fluxo estrutural.
#[must_use]
pub fn analisar_semantica(programa: &Programa) -> ResultadoSemantico {
    analisar_com_modulos(programa, &ModulosDisponiveis::new())
}

/// Analisa um programa com os módulos e símbolos exportados fornecidos pelo chamador.
#[must_use]
pub fn analisar_com_modulos(programa: &Programa, modulos: &ModulosDisponiveis) -> ResultadoSemantico {
    let mut analisador = Analisador::novo(modulos);
    analisador.analisar(programa);
    ResultadoSemantico {
        diagnosticos: analisador.diagnosticos,
        tipos_globais: analisador.escopos[0].iter()
            .map(|(nome, simbolo)| (nome.clone(), simbolo.tipo.clone()))
            .collect(),
    }
}

#[derive(Debug, Clone)]
struct Simbolo { tipo: Tipo, mutavel: bool, movido: bool }

#[derive(Debug, Clone, Copy)]
enum LiteralNumerico { Inteiro(i128), InteiroSemSinal(u128), Decimal(f64) }

#[derive(Debug, Clone)]
struct Funcao { genericos: Vec<String>, parametros: Vec<Tipo>, retorno: Tipo }

struct Analisador<'a> {
    escopos: Vec<HashMap<String, Simbolo>>,
    funcoes: HashMap<String, Funcao>,
    aliases: HashMap<String, String>,
    estruturas: HashSet<String>,
    campos_estruturas: HashMap<String, HashMap<String, Tipo>>,
    enumeracoes: HashSet<String>,
    variantes_enumeracoes: HashMap<String, HashMap<String, Vec<Tipo>>>,
    modulos: &'a ModulosDisponiveis,
    modulos_importados: HashMap<String, String>,
    diagnosticos: Vec<Diagnostico>,
    retorno_atual: Option<Tipo>,
    profundidade_laco: usize,
    funcoes_vistas: HashSet<String>,
    parametros_genericos: HashSet<String>,
}

impl<'a> Analisador<'a> {
    fn novo(modulos: &'a ModulosDisponiveis) -> Self {
        Self {
            escopos: vec![HashMap::new()],
            funcoes: HashMap::new(),
            aliases: HashMap::new(),
            estruturas: HashSet::new(),
            campos_estruturas: HashMap::new(),
            enumeracoes: HashSet::new(),
            variantes_enumeracoes: HashMap::new(),
            modulos,
            modulos_importados: HashMap::new(),
            diagnosticos: Vec::new(),
            retorno_atual: None,
            profundidade_laco: 0,
            funcoes_vistas: HashSet::new(),
            parametros_genericos: HashSet::new(),
        }
    }

    fn analisar(&mut self, programa: &Programa) {
        self.predeclarar(programa);
        for item in &programa.itens { self.item(item); }
    }

    fn predeclarar(&mut self, programa: &Programa) {
        for item in &programa.itens {
            if let NoAst::Incluir { caminho, apelido } = item {
                let alternativo = caminho.replace("::", ".");
                if !self.modulos.contains_key(caminho) && !self.modulos.contains_key(&alternativo) {
                    self.erro("N3004", format!("Módulo '{caminho}' não foi disponibilizado ao compilador."));
                } else {
                    let nome = apelido.clone().unwrap_or_else(|| caminho.split("::").last().unwrap_or(caminho).to_owned());
                    if self.modulos_importados.insert(nome.clone(), caminho.clone()).is_some() {
                        self.erro("N3022", format!("O alias de módulo '{nome}' foi declarado mais de uma vez."));
                    }
                }
            }
            if let NoAst::Tipo { nome, destino } = item {
                if self.aliases.insert(nome.clone(), destino.clone()).is_some() {
                    self.erro("N3002", format!("O tipo '{nome}' foi declarado mais de uma vez."));
                }
            }
            if let NoAst::Estrutura { nome, .. } = item {
                if !self.estruturas.insert(nome.clone()) {
                    self.erro("N3038", format!("A estrutura '{nome}' foi declarada mais de uma vez."));
                }
            }
            if let NoAst::Enumeracao { nome, .. } = item {
                if !self.enumeracoes.insert(nome.clone()) {
                    self.erro("N3039", format!("A enumeração '{nome}' foi declarada mais de uma vez."));
                }
            }
        }
        let aliases: Vec<String> = self.aliases.keys().cloned().collect();
        for nome in aliases {
            if self.resolver_tipo(&nome) == Tipo::Desconhecido {
                self.erro("N3003", format!("O alias de tipo '{nome}' contém referência inválida ou ciclo."));
            }
        }
        for item in &programa.itens {
            if let NoAst::Estrutura { nome, campos } = item {
                let descritores = campos.iter().map(|(campo, tipo)| (campo.clone(), self.resolver_tipo(tipo))).collect();
                self.campos_estruturas.insert(nome.clone(), descritores);
            }
        }
        for item in &programa.itens {
            if let NoAst::Enumeracao { nome, variantes } = item {
                let descritores = variantes.iter().map(|(variante, payload)| {
                    (variante.clone(), payload.iter().map(|tipo| self.resolver_tipo(tipo)).collect())
                }).collect();
                self.variantes_enumeracoes.insert(nome.clone(), descritores);
            }
        }
        for item in &programa.itens {
            if let NoAst::Funcao { nome, genericos, parametros, retorno, .. } = item {
                let mut vistos = HashSet::new();
                for parametro in genericos {
                    if !vistos.insert(parametro.clone()) {
                        self.erro("N3031", format!("Parâmetro genérico '{parametro}' duplicado na função '{nome}'."));
                    }
                }
                let anteriores = self.parametros_genericos.clone();
                self.parametros_genericos.extend(genericos.iter().cloned());
                let tipos: Vec<Tipo> = parametros.iter().map(|(parametro, nome_tipo)| {
                    let tipo = self.resolver_tipo(nome_tipo);
                    if tipo == Tipo::Desconhecido {
                        self.erro("N3021", format!("Tipo '{nome_tipo}' do parâmetro '{parametro}' não foi declarado."));
                    }
                    tipo
                }).collect();
                let retorno = retorno.as_deref().map_or(Tipo::Vazio, |nome_tipo| {
                    let tipo = self.resolver_tipo(nome_tipo);
                    if tipo == Tipo::Desconhecido {
                        self.erro("N3021", format!("Tipo de retorno '{nome_tipo}' da função '{nome}' não foi declarado."));
                    }
                    tipo
                });
                self.parametros_genericos = anteriores;
                if self.funcoes.contains_key(nome) {
                    self.erro("N3001", format!("A função '{nome}' foi declarada mais de uma vez."));
                } else {
                    self.funcoes.insert(nome.clone(), Funcao {
                        genericos: genericos.clone(),
                        parametros: tipos,
                        retorno,
                    });
                }
            }
        }
        let aliases_modulos: Vec<String> = self.modulos_importados.keys().cloned().collect();
        for alias in aliases_modulos {
            if self.funcoes.contains_key(&alias) {
                self.erro("N3019", format!("O nome '{alias}' já identifica uma função e um módulo."));
            }
        }
    }

    fn item(&mut self, item: &NoAst) {
        match item {
            NoAst::Variavel { nome, tipo, valor, constante } => {
                let inferido = self.expressao(valor);
                self.mover_valor(valor);
                let final_tipo = if let Some(tipo) = tipo {
                    let esperado = self.resolver_tipo(tipo);
                    if esperado == Tipo::Desconhecido {
                        self.erro("N3021", format!("Tipo '{tipo}' não foi declarado."));
                    }
                    self.exigir_expressao_tipo(valor, &inferido, &esperado, "inicialização da variável");
                    esperado
                } else {
                    if matches!(&inferido, Tipo::Lista(elemento) if **elemento == Tipo::Desconhecido) {
                        self.erro("N3027", format!("Não foi possível inferir o tipo dos elementos da lista '{nome}'; anote seu tipo."));
                    }
                    inferido
                };
                self.declarar(nome, Simbolo { tipo: final_tipo, mutavel: !constante, movido: false });
            }
            NoAst::Funcao { nome, genericos, parametros, corpo, .. } => {
                if !self.funcoes_vistas.insert(nome.clone()) { return; }
                let Some(funcao) = self.funcoes.get(nome).cloned() else {
                    self.erro("N3023", format!("Função aninhada '{nome}' não é permitida no escopo inicial."));
                    return;
                };
                let genericos_anteriores = self.parametros_genericos.clone();
                self.parametros_genericos.extend(genericos.iter().cloned());
                self.abrir_escopo();
                for ((nome_parametro, _), tipo) in parametros.iter().zip(&funcao.parametros) {
                    self.declarar(nome_parametro, Simbolo { tipo: tipo.clone(), mutavel: true, movido: false });
                }
                let retorno_anterior = self.retorno_atual.replace(funcao.retorno.clone());
                let laco_anterior = self.profundidade_laco;
                self.profundidade_laco = 0;
                for instrucao in corpo { self.item(instrucao); }
                if funcao.retorno != Tipo::Vazio && !Self::garante_retorno(corpo) {
                    self.erro("N3024", format!("A função '{nome}' pode terminar sem devolver um valor."));
                }
                self.profundidade_laco = laco_anterior;
                self.retorno_atual = retorno_anterior;
                self.fechar_escopo();
                self.parametros_genericos = genericos_anteriores;
            }
            NoAst::Tipo { .. } => {
                if self.escopos.len() > 1 {
                    self.erro("N3025", "Aliases de tipo só podem ser declarados no escopo do arquivo.");
                }
            }
            NoAst::Estrutura { nome, campos } => {
                if self.escopos.len() > 1 {
                    self.erro("N3032", "Estruturas só podem ser declaradas no escopo do arquivo.");
                }
                let mut vistos = HashSet::new();
                for (campo, tipo) in campos {
                    if !vistos.insert(campo) {
                        self.erro("N3033", format!("Campo '{campo}' duplicado na estrutura '{nome}'."));
                    }
                    if self.resolver_tipo(tipo) == Tipo::Desconhecido {
                        self.erro("N3034", format!("Tipo '{tipo}' do campo '{campo}' não foi declarado."));
                    }
                }
            }
            NoAst::Enumeracao { nome, variantes } => {
                if self.escopos.len() > 1 {
                    self.erro("N3035", "Enumerações só podem ser declaradas no escopo do arquivo.");
                }
                let mut vistos = HashSet::new();
                for (variante, payload) in variantes {
                    if !vistos.insert(variante) {
                        self.erro("N3036", format!("Variante '{variante}' duplicada na enumeração '{nome}'."));
                    }
                    for tipo in payload {
                        if self.resolver_tipo(tipo) == Tipo::Desconhecido {
                            self.erro("N3037", format!("Tipo '{tipo}' do payload da variante '{variante}' não foi declarado."));
                        }
                    }
                }
            }
            NoAst::Incluir { .. } => {
                if self.escopos.len() > 1 {
                    self.erro("N3026", "Módulos só podem ser incluídos no escopo do arquivo.");
                }
            }
            NoAst::Expressao(expressao) => {
                let tipo = self.expressao(expressao);
                if tipo == Tipo::Lista(Box::new(Tipo::Desconhecido)) {
                    self.erro("N3027", "Lista vazia sem tipo esperado não permite inferência.");
                }
            }
            NoAst::Quando { condicao, entao, senao } => {
                let tipo = self.expressao(condicao);
                self.exigir_tipo(&tipo, &Tipo::Logico, "condição de 'quando'");

                // Cada ramo parte do mesmo estado. Após a junção, um valor é
                // considerado movido se qualquer caminho possível o moveu.
                let antes = self.snapshot_movidos();
                self.bloco(entao);
                let depois_entao = self.snapshot_movidos();
                self.restaurar_movidos(&antes);
                self.bloco(senao);
                let depois_senao = self.snapshot_movidos();
                self.mesclar_movidos(&depois_entao, &depois_senao);
            }
            NoAst::Escolha { valor, casos } => {
                let tipo_valor = self.expressao(valor);
                let Tipo::Enumeracao(nome_enum) = tipo_valor else {
                    self.erro("N3046", "A expressão de 'escolha' precisa ter tipo de enumeração.");
                    for caso in casos { self.bloco(&caso.corpo); }
                    return;
                };
                let Some(variantes) = self.variantes_enumeracoes.get(&nome_enum).cloned() else {
                    self.erro("N3047", format!("Enumeração '{nome_enum}' não foi encontrada."));
                    return;
                };
                let mut cobertas = HashSet::new();
                let mut possui_curinga = false;
                let antes = self.snapshot_movidos();
                let mut estados = Vec::new();
                for (indice_caso, caso) in casos.iter().enumerate() {
                    self.abrir_escopo();
                    match (&caso.enumeracao, &caso.variante) {
                        (Some(enum_caso), Some(variante)) => {
                            if enum_caso != &nome_enum {
                                self.erro("N3048", format!("Caso pertence à enumeração '{enum_caso}', mas a expressão é '{nome_enum}'."));
                            }
                            if !cobertas.insert(variante.clone()) {
                                self.erro("N3049", format!("Variante '{variante}' repetida em 'escolha'."));
                            }
                            if let Some(payload) = variantes.get(variante) {
                                if payload.len() != caso.vinculacoes.len() {
                                    self.erro("N3050", format!("Variante '{variante}' exige {} vínculos de payload, mas recebeu {}.", payload.len(), caso.vinculacoes.len()));
                                }
                                for (vinculo, tipo) in caso.vinculacoes.iter().zip(payload) {
                                    self.declarar(vinculo, Simbolo { tipo: tipo.clone(), mutavel: false, movido: false });
                                }
                            } else {
                                self.erro("N3051", format!("Variante '{variante}' não existe na enumeração '{nome_enum}'."));
                            }
                        }
                        _ => {
                            if indice_caso + 1 != casos.len() {
                                self.erro("N3054", "O caso curinga deve ser o último caso de 'escolha'.");
                            }
                            if possui_curinga {
                                self.erro("N3052", "A instrução 'escolha' possui mais de um caso curinga.");
                            }
                            possui_curinga = true;
                        }
                    }
                    self.restaurar_movidos(&antes);
                    for instrucao in &caso.corpo { self.item(instrucao); }
                    estados.push(self.snapshot_movidos());
                    self.fechar_escopo();
                }
                if !possui_curinga {
                    for variante in variantes.keys() {
                        if !cobertas.contains(variante) {
                            self.erro("N3053", format!("'escolha' não cobre a variante '{variante}' da enumeração '{nome_enum}'."));
                        }
                    }
                }
                if let Some(primeiro) = estados.first() {
                    let mut combinado = primeiro.clone();
                    for estado in estados.iter().skip(1) {
                        combinado.extend(estado.iter().cloned());
                    }
                    self.mesclar_movidos(&antes, &combinado);
                }
            }
            NoAst::Enquanto { condicao, corpo } => {
                let tipo = self.expressao(condicao);
                self.exigir_tipo(&tipo, &Tipo::Logico, "condição de 'enquanto'");
                let antes_corpo = self.snapshot_movidos();
                self.profundidade_laco += 1;
                self.bloco(corpo);
                self.profundidade_laco -= 1;
                let depois_corpo = self.snapshot_movidos();
                // O laço pode executar zero vezes; preserva qualquer move
                // que possa ocorrer em uma iteração.
                self.mesclar_movidos(&antes_corpo, &depois_corpo);
            }
            NoAst::Para { inicial, condicao, atualizacao, corpo } => {
                self.abrir_escopo();
                if let Some(inicial) = inicial { self.item(inicial); }
                if let Some(condicao) = condicao {
                    let tipo = self.expressao(condicao);
                    self.exigir_tipo(&tipo, &Tipo::Logico, "condição de 'para'");
                }
                let antes_corpo = self.snapshot_movidos();
                self.profundidade_laco += 1;
                self.bloco(corpo);
                self.profundidade_laco -= 1;
                if let Some(atualizacao) = atualizacao { self.expressao(atualizacao); }
                let depois_corpo = self.snapshot_movidos();
                self.mesclar_movidos(&antes_corpo, &depois_corpo);
                self.fechar_escopo();
            }
            NoAst::Devolver(valor) => {
                let esperado = self.retorno_atual.clone();
                if esperado.is_none() {
                    self.erro("N3005", "'devolver' só pode ser usado dentro de uma função.");
                }
                let obtido = valor.as_ref().map_or(Tipo::Vazio, |e| self.expressao(e));
                if let Some(esperado) = esperado {
                    if let Some(valor) = valor {
                        self.mover_valor(valor);
                        self.exigir_expressao_tipo(valor, &obtido, &esperado, "retorno da função");
                    } else {
                        self.exigir_tipo(&obtido, &esperado, "retorno da função");
                    }
                }
            }
            NoAst::Parar | NoAst::Seguir => {
                if self.profundidade_laco == 0 {
                    self.erro("N3006", "'parar' e 'seguir' só podem ser usados dentro de laços.");
                }
            }
        }
    }

    fn garante_retorno(itens: &[NoAst]) -> bool {
        for item in itens {
            match item {
                NoAst::Devolver(_) => return true,
                NoAst::Quando { entao, senao, .. }
                    if Self::garante_retorno(entao) && Self::garante_retorno(senao) => return true,
                _ => {}
            }
        }
        false
    }

    fn bloco(&mut self, itens: &[NoAst]) {
        self.abrir_escopo();
        for item in itens { self.item(item); }
        self.fechar_escopo();
    }

    fn expressao(&mut self, expressao: &Expressao) -> Tipo {
        match expressao {
            Expressao::Nome(nome) => {
                if let Some(simbolo) = self.buscar(nome).cloned() {
                    if simbolo.movido {
                        self.erro("N3030", format!("O valor '{nome}' foi movido e não pode ser utilizado novamente."));
                    }
                    simbolo.tipo
                } else if let Some(f) = self.funcoes.get(nome) {
                    Tipo::Funcao(f.parametros.clone(), Box::new(f.retorno.clone()))
                } else {
                    self.erro("N3007", format!("Nome '{nome}' não foi declarado."));
                    Tipo::Desconhecido
                }
            },
            Expressao::Literal(valor) => {
                let tipo = Self::tipo_literal(valor);
                match Self::literal_numerico(expressao) {
                    Some(LiteralNumerico::Decimal(v)) if !v.is_finite() => {
                        self.erro("N3028", "Literal decimal está fora do intervalo representável.");
                    }
                    None if tipo == Tipo::Inteiro => {
                        self.erro("N3028", "Literal inteiro está fora do intervalo representável.");
                    }
                    _ => {}
                }
                tipo
            },
            Expressao::Lista(valores) => {
                if valores.is_empty() { return Tipo::Lista(Box::new(Tipo::Desconhecido)); }
                let primeiro = self.expressao(&valores[0]);
                for valor in valores.iter().skip(1) {
                    let atual = self.expressao(valor);
                    self.exigir_tipo(&atual, &primeiro, "elementos da lista");
                }
                Tipo::Lista(Box::new(primeiro))
            }
            Expressao::Tupla(valores) => {
                Tipo::Tupla(valores.iter().map(|valor| self.expressao(valor)).collect())
            }
            Expressao::ConstrucaoEstrutura { nome, campos } => {
                let Some(descritores) = self.campos_estruturas.get(nome).cloned() else {
                    self.erro("N3042", format!("'{nome}' não é uma estrutura declarada."));
                    for (_, valor) in campos { self.expressao(valor); }
                    return Tipo::Desconhecido;
                };
                for (campo, valor) in campos {
                    let obtido = self.expressao(valor);
                    if let Some(esperado) = descritores.get(campo) {
                        self.exigir_expressao_tipo(valor, &obtido, esperado, &format!("campo '{campo}' da estrutura '{nome}'"));
                    } else {
                        self.erro("N3040", format!("A estrutura '{nome}' não possui o campo '{campo}'."));
                    }
                }
                for campo in descritores.keys() {
                    if !campos.iter().any(|(informado, _)| informado == campo) {
                        self.erro("N3043", format!("Campo obrigatório '{campo}' não foi informado na estrutura '{nome}'."));
                    }
                }
                Tipo::Estrutura(nome.clone())
            }
            Expressao::Unaria { operador, valor } => {
                let tipo = self.expressao(valor);
                match operador {
                    TipoToken::Negacao | TipoToken::Nao => {
                        self.exigir_tipo(&tipo, &Tipo::Logico, "negação lógica");
                        Tipo::Logico
                    }
                    TipoToken::Mais | TipoToken::Menos => {
                        if !Self::numerico(&tipo) && tipo != Tipo::Desconhecido {
                            self.erro("N3008", "Operador unário exige um valor numérico.");
                        }
                        tipo
                    }
                    _ => Tipo::Desconhecido,
                }
            }
            Expressao::Binaria { esquerda, operador, direita } => {
                let a = self.expressao(esquerda);
                let b = self.expressao(direita);
                self.binaria(&a, *operador, &b)
            }
            Expressao::Atribuicao { destino, valor } => {
                let tipo_valor = self.expressao(valor);
                match destino.as_ref() {
                    Expressao::Nome(nome) => {
                        if let Some(simbolo) = self.buscar(nome).cloned() {
                            if !simbolo.mutavel {
                                self.erro("N3009", format!("Não é permitido alterar a constante '{nome}'."));
                            }
                            self.exigir_expressao_tipo(valor, &tipo_valor, &simbolo.tipo, "atribuição");
                            if let Some(simbolo) = self.buscar_mutavel(nome) { simbolo.movido = false; }
                            self.mover_valor(valor);
                            simbolo.tipo
                        } else {
                            self.erro("N3007", format!("Nome '{nome}' não foi declarado."));
                            Tipo::Desconhecido
                        }
                    }
                    Expressao::Membro { .. } => {
                        let tipo_destino = self.expressao(destino);
                        let mut raiz = destino.as_ref();
                        while let Expressao::Membro { alvo, operador: TipoToken::Ponto, .. } = raiz {
                            raiz = alvo;
                        }
                        if let Expressao::Nome(nome) = raiz {
                            if let Some(simbolo) = self.buscar(nome).cloned() {
                                if !simbolo.mutavel {
                                    self.erro("N3009", format!("Não é permitido alterar campos da constante '{nome}'."));
                                }
                            }
                        }
                        self.exigir_expressao_tipo(valor, &tipo_valor, &tipo_destino, "atribuição de campo");
                        tipo_destino
                    }
                    _ => {
                        self.erro("N3010", "O destino da atribuição precisa ser uma variável ou campo modificável.");
                        Tipo::Desconhecido
                    }
                }
            }
            Expressao::Chamada { alvo, argumentos } => {
                if let Expressao::Membro { alvo: tipo_expr, operador: TipoToken::Escopo, nome: variante } = alvo.as_ref() {
                    if let Expressao::Nome(tipo_enum) = tipo_expr.as_ref() {
                        if let Some(variantes) = self.variantes_enumeracoes.get(tipo_enum).cloned() {
                            if let Some(esperados) = variantes.get(variante) {
                                if argumentos.len() != esperados.len() {
                                    self.erro("N3044", format!("A variante '{tipo_enum}::{variante}' espera {} valor(es), mas recebeu {}.", esperados.len(), argumentos.len()));
                                }
                                for (indice, argumento) in argumentos.iter().enumerate() {
                                    let obtido = self.expressao(argumento);
                                    if let Some(esperado) = esperados.get(indice) {
                                        self.exigir_expressao_tipo(argumento, &obtido, esperado, "payload de enumeração");
                                    }
                                }
                                return Tipo::Enumeracao(tipo_enum.clone());
                            }
                            self.erro("N3045", format!("A enumeração '{tipo_enum}' não possui a variante '{variante}'."));
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                    }
                }
                if let Expressao::Nome(nome) = alvo.as_ref() {
                    if nome == "exibir" {
                        if argumentos.len() != 1 {
                            self.erro("N3015", "A função 'exibir' exige exatamente um argumento.");
                        }
                        for argumento in argumentos { self.expressao(argumento); }
                        return Tipo::Vazio;
                    }
                    if nome == "ler_texto" {
                        if !argumentos.is_empty() {
                            self.erro("N3015", "A função 'ler_texto' não aceita argumentos.");
                            for argumento in argumentos { self.expressao(argumento); }
                        }
                        return Tipo::Texto;
                    }
                    if nome == "comprimento" {
                        if argumentos.len() != 1 {
                            self.erro("N3015", "A função 'comprimento' exige um argumento.");
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        let tipo = self.expressao(&argumentos[0]);
                        if matches!(tipo, Tipo::Texto | Tipo::Lista(_)) {
                            return Tipo::Inteiro;
                        }
                        self.erro("N3016", "'comprimento' exige texto ou lista.");
                        return Tipo::Desconhecido;
                    }
                    if nome == "subtexto" {
                        if argumentos.len() != 3 {
                            self.erro("N3015", "'subtexto' exige texto, início e fim.");
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        let texto = self.expressao(&argumentos[0]);
                        let inicio = self.expressao(&argumentos[1]);
                        let fim = self.expressao(&argumentos[2]);
                        self.exigir_tipo(&texto, &Tipo::Texto, "primeiro argumento de 'subtexto'");
                        self.exigir_tipo(&inicio, &Tipo::Inteiro, "início de 'subtexto'");
                        self.exigir_tipo(&fim, &Tipo::Inteiro, "fim de 'subtexto'");
                        return Tipo::Texto;
                    }
                    if nome == "contem" {
                        if argumentos.len() != 2 {
                            self.erro("N3015", "'contem' exige dois argumentos.");
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        let texto = self.expressao(&argumentos[0]);
                        let trecho = self.expressao(&argumentos[1]);
                        self.exigir_tipo(&texto, &Tipo::Texto, "primeiro argumento de 'contem'");
                        self.exigir_tipo(&trecho, &Tipo::Texto, "segundo argumento de 'contem'");
                        return Tipo::Logico;
                    }
                    if nome == "aparar" {
                        if argumentos.len() != 1 {
                            self.erro("N3015", "'aparar' exige um texto.");
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        let texto = self.expressao(&argumentos[0]);
                        self.exigir_tipo(&texto, &Tipo::Texto, "argumento de 'aparar'");
                        return Tipo::Texto;
                    }
                    if nome == "comeca_com" || nome == "termina_com" {
                        if argumentos.len() != 2 {
                            self.erro("N3015", format!("'{nome}' exige dois textos."));
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        let texto = self.expressao(&argumentos[0]);
                        let trecho = self.expressao(&argumentos[1]);
                        self.exigir_tipo(&texto, &Tipo::Texto, "primeiro argumento de comparação de texto");
                        self.exigir_tipo(&trecho, &Tipo::Texto, "segundo argumento de comparação de texto");
                        return Tipo::Logico;
                    }
                    if nome == "substituir" {
                        if argumentos.len() != 3 {
                            self.erro("N3015", "'substituir' exige texto, trecho e substituição.");
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        for (indice, argumento) in argumentos.iter().enumerate() {
                            let tipo = self.expressao(argumento);
                            self.exigir_tipo(&tipo, &Tipo::Texto, match indice {
                                0 => "texto de 'substituir'",
                                1 => "trecho de 'substituir'",
                                _ => "substituição de 'substituir'",
                            });
                        }
                        return Tipo::Texto;
                    }
                    if nome == "maiusculas" || nome == "minusculas" {
                        if argumentos.len() != 1 {
                            self.erro("N3015", format!("'{nome}' exige um argumento de texto."));
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        let texto = self.expressao(&argumentos[0]);
                        self.exigir_tipo(&texto, &Tipo::Texto, "conversão de caixa");
                        return Tipo::Texto;
                    }
                    if nome == "caractere" {
                        if argumentos.len() != 2 {
                            self.erro("N3015", "'caractere' exige texto e índice.");
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        let texto = self.expressao(&argumentos[0]);
                        let indice = self.expressao(&argumentos[1]);
                        self.exigir_tipo(&texto, &Tipo::Texto, "primeiro argumento de 'caractere'");
                        self.exigir_tipo(&indice, &Tipo::Inteiro, "índice de 'caractere'");
                        return Tipo::Caractere;
                    }
                    if nome == "texto" {
                        if argumentos.len() != 1 {
                            self.erro("N3015", "'texto' exige exatamente um argumento.");
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        self.expressao(&argumentos[0]);
                        return Tipo::Texto;
                    }
                    if nome == "ler_arquivo"
                        || nome == "existe_arquivo"
                        || nome == "criar_diretorio"
                        || nome == "diretorio_existe"
                        || nome == "listar_diretorio"
                    {
                        if argumentos.len() != 1 {
                            self.erro("N3015", format!("'{nome}' exige um caminho de texto."));
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        let caminho = self.expressao(&argumentos[0]);
                        self.exigir_tipo(&caminho, &Tipo::Texto, "caminho de arquivo");
                        return if nome == "ler_arquivo" {
                            Tipo::Texto
                        } else if nome == "listar_diretorio" {
                            Tipo::Lista(Box::new(Tipo::Texto))
                        } else {
                            Tipo::Logico
                        };
                    }
                    if nome == "remover_arquivo" {
                        if argumentos.len() != 1 {
                            self.erro("N3015", "'remover_arquivo' exige um caminho de texto.");
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        let caminho = self.expressao(&argumentos[0]);
                        self.exigir_tipo(&caminho, &Tipo::Texto, "caminho de arquivo");
                        return Tipo::Logico;
                    }
                    if nome == "copiar_arquivo" || nome == "mover_arquivo" {
                        if argumentos.len() != 2 {
                            self.erro("N3015", format!("'{nome}' exige origem e destino."));
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        let origem = self.expressao(&argumentos[0]);
                        let destino = self.expressao(&argumentos[1]);
                        self.exigir_tipo(&origem, &Tipo::Texto, "origem de arquivo");
                        self.exigir_tipo(&destino, &Tipo::Texto, "destino de arquivo");
                        return if nome == "copiar_arquivo" { Tipo::Inteiro } else { Tipo::Logico };
                    }
                    if nome == "escrever_arquivo" {
                        if argumentos.len() != 2 {
                            self.erro("N3015", "'escrever_arquivo' exige caminho e conteúdo.");
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        let caminho = self.expressao(&argumentos[0]);
                        let conteudo = self.expressao(&argumentos[1]);
                        self.exigir_tipo(&caminho, &Tipo::Texto, "caminho de arquivo");
                        self.exigir_tipo(&conteudo, &Tipo::Texto, "conteúdo de arquivo");
                        return Tipo::Logico;
                    }
                    if nome == "diretorio_atual" || nome == "plataforma" {
                        if !argumentos.is_empty() {
                            self.erro("N3015", format!("'{nome}' não aceita argumentos."));
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        return Tipo::Texto;
                    }
                    if nome == "argumentos_processo" {
                        if !argumentos.is_empty() {
                            self.erro("N3015", "'argumentos_processo' não aceita argumentos.");
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        return Tipo::Lista(Box::new(Tipo::Texto));
                    }
                    if nome == "variavel_ambiente" {
                        if argumentos.len() != 1 {
                            self.erro("N3015", "'variavel_ambiente' exige um nome de variável.");
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        let nome_variavel = self.expressao(&argumentos[0]);
                        self.exigir_tipo(&nome_variavel, &Tipo::Texto, "nome da variável de ambiente");
                        return Tipo::Texto;
                    }
                    if nome == "tempo" {
                        if !argumentos.is_empty() {
                            self.erro("N3015", "'tempo' não aceita argumentos.");
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        return Tipo::Inteiro;
                    }
                    if nome == "dormir" {
                        if argumentos.len() != 1 {
                            self.erro("N3015", "'dormir' exige milissegundos.");
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        let duracao = self.expressao(&argumentos[0]);
                        self.exigir_tipo(&duracao, &Tipo::Inteiro, "duração de 'dormir'");
                        return Tipo::Vazio;
                    }
                    if matches!(
                        nome.as_str(),
                        "raiz" | "seno" | "cosseno" | "tangente" | "logaritmo"
                            | "exponencial" | "piso" | "teto" | "arredondar" | "absoluto"
                    ) {
                        if argumentos.len() != 1 {
                            self.erro("N3015", format!("'{nome}' exige um argumento numérico."));
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        let tipo = self.expressao(&argumentos[0]);
                        if !Self::numerico(&tipo) && tipo != Tipo::Desconhecido {
                            self.erro("N3016", format!("'{nome}' exige um argumento numérico."));
                        }
                        return Tipo::Decimal64;
                    }
                    if nome == "inteiro" || nome == "decimal" {
                        if argumentos.len() != 1 {
                            self.erro("N3015", format!("'{nome}' exige um argumento de texto."));
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        let obtido = self.expressao(&argumentos[0]);
                        self.exigir_tipo(&obtido, &Tipo::Texto, "conversão numérica");
                        return if nome == "inteiro" { Tipo::Inteiro } else { Tipo::Decimal64 };
                    }
                    if nome == "adicionar" || nome == "remover" {
                        if argumentos.len() != 2 {
                            self.erro("N3015", format!("'{nome}' exige dois argumentos."));
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        let tipo_lista = self.expressao(&argumentos[0]);
                        let Tipo::Lista(elemento) = tipo_lista else {
                            self.erro("N3016", format!("O primeiro argumento de '{nome}' deve ser uma lista."));
                            self.expressao(&argumentos[1]);
                            return Tipo::Desconhecido;
                        };
                        if let Expressao::Nome(variavel) = &argumentos[0] {
                            if self.buscar(variavel).is_some_and(|simbolo| !simbolo.mutavel) {
                                self.erro("N3009", format!("Não é permitido alterar a constante '{variavel}'."));
                            }
                        } else {
                            self.erro("N3010", format!("'{nome}' exige uma variável de lista modificável."));
                        }
                        if nome == "adicionar" {
                            let valor = self.expressao(&argumentos[1]);
                            self.exigir_tipo(&valor, &elemento, "elemento adicionado à lista");
                            return Tipo::Vazio;
                        }
                        let indice = self.expressao(&argumentos[1]);
                        self.exigir_tipo(&indice, &Tipo::Inteiro, "índice de 'remover'");
                        return *elemento;
                    }
                    if let Some(funcao) = self.funcoes.get(nome).cloned() {
                        if funcao.genericos.is_empty() {
                            self.validar_argumentos(argumentos, &funcao.parametros, nome);
                            return funcao.retorno;
                        }
                        if argumentos.len() != funcao.parametros.len() {
                            self.erro("N3013", format!("A função genérica '{nome}' espera {} argumentos, mas recebeu {}.", funcao.parametros.len(), argumentos.len()));
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        let tipos_obtidos: Vec<Tipo> = argumentos.iter().map(|argumento| self.expressao(argumento)).collect();
                        let mut inferidos = HashMap::new();
                        for (esperado, obtido) in funcao.parametros.iter().zip(&tipos_obtidos) {
                            if !Self::inferir_genericos(esperado, obtido, &funcao.genericos, &mut inferidos) {
                                self.erro("N3032", format!("Não foi possível inferir tipos genéricos consistentes na chamada de '{nome}'."));
                            }
                        }
                        for parametro in &funcao.genericos {
                            if !inferidos.contains_key(parametro) {
                                self.erro("N3033", format!("O parâmetro genérico '{parametro}' da função '{nome}' não pôde ser inferido pelos argumentos."));
                            }
                        }
                        for (esperado, obtido) in funcao.parametros.iter().zip(&tipos_obtidos) {
                            let esperado = Self::substituir_genericos(esperado, &inferidos);
                            self.exigir_tipo(obtido, &esperado, "argumento de função genérica");
                        }
                        return Self::substituir_genericos(&funcao.retorno, &inferidos);
                    }
                }
                if let Expressao::Membro { alvo: modulo_expr, operador: TipoToken::Escopo, nome } = alvo.as_ref() {
                    if let Expressao::Nome(alias) = modulo_expr.as_ref() {
                        if let Some(caminho) = self.modulos_importados.get(alias) {
                            let chave = if self.modulos.contains_key(caminho) { caminho.clone() } else { caminho.replace("::", ".") };
                            if let Some(funcao) = self.modulos.get(&chave).and_then(|m| m.get(nome)).cloned() {
                                self.validar_argumentos(argumentos, &funcao.parametros, nome);
                                return funcao.retorno;
                            }
                        }
                    }
                    self.erro("N3011", "Função qualificada não foi encontrada em um módulo importado.");
                    return Tipo::Desconhecido;
                }
                self.erro("N3012", "O alvo da chamada não é uma função conhecida.");
                for argumento in argumentos { self.expressao(argumento); }
                Tipo::Desconhecido
            }
            Expressao::Indexacao { alvo, indice } => {
                let tipo_alvo = self.expressao(alvo);
                let tipo_indice = self.expressao(indice);
                if !Self::inteiro(&tipo_indice) && tipo_indice != Tipo::Desconhecido {
                    self.erro("N3020", format!("Índice deve ser inteiro, mas recebeu {}.", tipo_indice.nome()));
                }
                match tipo_alvo {
                    Tipo::Lista(elemento) => *elemento,
                    Tipo::Tupla(elementos) => {
                        let posicao = Self::literal_numerico(indice).and_then(|valor| match valor {
                            LiteralNumerico::Inteiro(valor) => usize::try_from(valor).ok(),
                            LiteralNumerico::InteiroSemSinal(valor) => usize::try_from(valor).ok(),
                            LiteralNumerico::Decimal(_) => None,
                        });
                        match posicao.and_then(|posicao| elementos.get(posicao).cloned()) {
                            Some(tipo) => tipo,
                            None => {
                                self.erro("N3034", "O índice de uma tupla precisa ser um literal inteiro válido e dentro dos limites.");
                                Tipo::Desconhecido
                            }
                        }
                    }
                    _ => {
                        self.erro("N3013", "Indexação exige uma lista ou tupla.");
                        Tipo::Desconhecido
                    }
                }
            }
            Expressao::Membro { alvo, operador, nome } => {
                if *operador == TipoToken::Escopo {
                    if let Expressao::Nome(tipo_enum) = alvo.as_ref() {
                        if let Some(variantes) = self.variantes_enumeracoes.get(tipo_enum).cloned() {
                            if let Some(payload) = variantes.get(nome) {
                                if payload.is_empty() {
                                    return Tipo::Enumeracao(tipo_enum.clone());
                                }
                                self.erro("N3055", format!("A variante '{tipo_enum}::{nome}' exige {} valor(es) de payload.", payload.len()));
                                return Tipo::Desconhecido;
                            }
                            self.erro("N3045", format!("A variante '{nome}' não existe na enumeração '{tipo_enum}'."));
                            return Tipo::Desconhecido;
                        }
                    }
                }
                if *operador == TipoToken::Ponto {
                    let tipo_alvo = self.expressao(alvo);
                    match tipo_alvo {
                        Tipo::Estrutura(nome_estrutura) => {
                            if let Some(tipo_campo) = self.campos_estruturas.get(&nome_estrutura).and_then(|campos| campos.get(nome)).cloned() {
                                return tipo_campo;
                            }
                            self.erro("N3040", format!("A estrutura '{nome_estrutura}' não possui o campo '{nome}'."));
                        }
                        Tipo::Desconhecido => {}
                        outro => self.erro("N3041", format!("O tipo {} não possui campos acessíveis por '.'.", outro.nome())),
                    }
                }
                Tipo::Desconhecido
            }
        }
    }

    fn validar_argumentos(&mut self, argumentos: &[Expressao], esperados: &[Tipo], nome: &str) {
        if argumentos.len() != esperados.len() {
            self.erro("N3015", format!("A função '{nome}' espera {} argumento(s), mas recebeu {}.", esperados.len(), argumentos.len()));
        }
        for (indice, argumento) in argumentos.iter().enumerate() {
            let obtido = self.expressao(argumento);
            if let Some(esperado) = esperados.get(indice) {
                self.exigir_expressao_tipo(argumento, &obtido, esperado, "argumento de função");
                if !Self::tipo_copiavel(&obtido) { self.mover_valor(argumento); }
            }
        }
    }

    fn binaria(&mut self, a: &Tipo, op: TipoToken, b: &Tipo) -> Tipo {
        use TipoToken::{Diferente, Divide, E, Igual, Intervalo, Maior, MaiorIgual, Menor, MenorIgual, Menos, Mais, Ou, Potencia, Resto, Vezes};
        match op {
            Mais if a == &Tipo::Texto && b == &Tipo::Texto => Tipo::Texto,
            Mais | Menos | Vezes | Divide | Resto | Potencia => {
                if Self::numerico(a) && Self::numerico(b) {
                    if Self::inteiro(a) && Self::inteiro(b)
                        && Self::inteiro_sem_sinal(a) != Self::inteiro_sem_sinal(b) {
                        self.erro("N3038", "Não misture inteiros com e sem sinal sem conversão explícita.");
                        Tipo::Desconhecido
                    } else {
                        Self::promover_numerico(a, b)
                    }
                } else {
                    self.erro("N3016", "Operação aritmética exige operandos numéricos compatíveis.");
                    Tipo::Desconhecido
                }
            }
            Menor | MenorIgual | Maior | MaiorIgual => {
                if Self::numerico(a) && Self::numerico(b) {
                    if Self::inteiro(a) && Self::inteiro(b)
                        && Self::inteiro_sem_sinal(a) != Self::inteiro_sem_sinal(b) {
                        self.erro("N3038", "Não misture inteiros com e sem sinal sem conversão explícita.");
                    }
                    Tipo::Logico
                } else {
                    self.erro("N3017", "Comparação relacional exige operandos numéricos.");
                    Tipo::Desconhecido
                }
            }
            Igual | Diferente => {
                self.exigir_tipo(b, a, "comparação de igualdade");
                Tipo::Logico
            }
            E | Ou => {
                self.exigir_tipo(a, &Tipo::Logico, "operação lógica");
                self.exigir_tipo(b, &Tipo::Logico, "operação lógica");
                Tipo::Logico
            }
            Intervalo => {
                self.exigir_tipo(b, a, "limites do intervalo");
                if Self::numerico(a) { Tipo::Intervalo(Box::new(a.clone())) } else {
                    self.erro("N3018", "Intervalo exige limites numéricos.");
                    Tipo::Desconhecido
                }
            }
            _ => Tipo::Desconhecido,
        }
    }

    fn tipo_literal(literal: &str) -> Tipo {
        match literal {
            "verdadeiro" | "falso" => Tipo::Logico,
            "nulo" => Tipo::Desconhecido,
            _ if literal.starts_with('"') => Tipo::Texto,
            _ if literal.starts_with('\'') => Tipo::Caractere,
            _ if literal.contains('.') || literal.contains('e') || literal.contains('E') => Tipo::Decimal64,
            _ if literal.replace('_', "").parse::<u128>().is_ok()
                && literal.replace('_', "").parse::<i128>().is_err() => Tipo::InteiroU128,
            _ => Tipo::Inteiro,
        }
    }

    fn argumentos_genericos(nome: &str) -> Option<(&str, Vec<&str>)> {
        let abertura = nome.find('<')?;
        if !nome.ends_with('>') { return None; }
        let base = &nome[..abertura];
        if base.is_empty() { return None; }
        let conteudo = &nome[abertura + 1..nome.len() - 1];
        if conteudo.is_empty() { return None; }
        let mut argumentos = Vec::new();
        let mut profundidade = 0usize;
        let mut inicio = 0usize;
        for (indice, caractere) in conteudo.char_indices() {
            match caractere {
                '<' => profundidade += 1,
                '>' => {
                    profundidade = profundidade.checked_sub(1)?;
                }
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

    fn resolver_tipo(&self, nome: &str) -> Tipo {
        if self.parametros_genericos.contains(nome) {
            return Tipo::Parametro(nome.to_owned());
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
            "decimal" => Tipo::Decimal64,
            "decimal32" => Tipo::Decimal32,
            "decimal64" => Tipo::Decimal64,
            "texto" => Tipo::Texto,
            "caractere" => Tipo::Caractere,
            "byte" => Tipo::Byte,
            _ if nome.starts_with('[') && nome.ends_with(']') => {
                Tipo::Lista(Box::new(self.resolver_tipo(&nome[1..nome.len()-1])))
            }
            _ if Self::argumentos_genericos(nome).is_some() => {
                let (base, argumentos) = Self::argumentos_genericos(nome).expect("tipo genérico reconhecido");
                let tipos: Vec<Tipo> = argumentos.iter().map(|argumento| self.resolver_tipo(argumento)).collect();
                if tipos.iter().any(|tipo| *tipo == Tipo::Desconhecido) {
                    Tipo::Desconhecido
                } else {
                    match (base, tipos.as_slice()) {
                        ("Opcional" | "opcional", [interno]) => Tipo::Opcional(Box::new(interno.clone())),
                        ("Resultado" | "resultado", [sucesso, falha]) => Tipo::Resultado(Box::new(sucesso.clone()), Box::new(falha.clone())),
                        ("Tupla" | "tupla", elementos) if !elementos.is_empty() => Tipo::Tupla(elementos.to_vec()),
                        _ => Tipo::Desconhecido,
                    }
                }
            }
            _ if self.estruturas.contains(nome) => Tipo::Estrutura(nome.to_owned()),
            _ if self.enumeracoes.contains(nome) => Tipo::Enumeracao(nome.to_owned()),
            _ => {
                let mut atual = nome;
                let mut vistos = HashSet::new();
                while let Some(destino) = self.aliases.get(atual) {
                    if !vistos.insert(atual.to_owned()) { return Tipo::Desconhecido; }
                    atual = destino;
                }
                if atual != nome { self.resolver_tipo(atual) } else { Tipo::Desconhecido }
            }
        }
    }

    fn inferir_genericos(
        esperado: &Tipo,
        obtido: &Tipo,
        genericos: &[String],
        inferidos: &mut HashMap<String, Tipo>,
    ) -> bool {
        match esperado {
            Tipo::Parametro(nome) if genericos.contains(nome) => {
                if let Some(anterior) = inferidos.get(nome) {
                    Self::tipos_compativeis(obtido, anterior)
                } else {
                    inferidos.insert(nome.clone(), obtido.clone());
                    true
                }
            }
            Tipo::Lista(a) => matches!(obtido, Tipo::Lista(b) if Self::inferir_genericos(a, b, genericos, inferidos)),
            Tipo::Intervalo(a) => matches!(obtido, Tipo::Intervalo(b) if Self::inferir_genericos(a, b, genericos, inferidos)),
            Tipo::Opcional(a) => matches!(obtido, Tipo::Opcional(b) if Self::inferir_genericos(a, b, genericos, inferidos)),
            Tipo::Resultado(a, b) => matches!(obtido, Tipo::Resultado(c, d)
                if Self::inferir_genericos(a, c, genericos, inferidos)
                    && Self::inferir_genericos(b, d, genericos, inferidos)),
            Tipo::Tupla(a) => matches!(obtido, Tipo::Tupla(b) if a.len() == b.len()
                && a.iter().zip(b).all(|(a, b)| Self::inferir_genericos(a, b, genericos, inferidos))),
            _ => Self::tipos_compativeis(obtido, esperado),
        }
    }

    fn substituir_genericos(tipo: &Tipo, inferidos: &HashMap<String, Tipo>) -> Tipo {
        match tipo {
            Tipo::Parametro(nome) => inferidos.get(nome).cloned().unwrap_or(Tipo::Desconhecido),
            Tipo::Lista(t) => Tipo::Lista(Box::new(Self::substituir_genericos(t, inferidos))),
            Tipo::Intervalo(t) => Tipo::Intervalo(Box::new(Self::substituir_genericos(t, inferidos))),
            Tipo::Opcional(t) => Tipo::Opcional(Box::new(Self::substituir_genericos(t, inferidos))),
            Tipo::Resultado(a, b) => Tipo::Resultado(
                Box::new(Self::substituir_genericos(a, inferidos)),
                Box::new(Self::substituir_genericos(b, inferidos)),
            ),
            Tipo::Tupla(itens) => Tipo::Tupla(itens.iter().map(|t| Self::substituir_genericos(t, inferidos)).collect()),
            Tipo::Funcao(parametros, retorno) => Tipo::Funcao(
                parametros.iter().map(|t| Self::substituir_genericos(t, inferidos)).collect(),
                Box::new(Self::substituir_genericos(retorno, inferidos)),
            ),
            outro => outro.clone(),
        }
    }

    fn buscar_mutavel(&mut self, nome: &str) -> Option<&mut Simbolo> {
        self.escopos.iter_mut().rev().find_map(|escopo| escopo.get_mut(nome))
    }

    fn tipo_copiavel(tipo: &Tipo) -> bool {
        matches!(tipo,
            Tipo::Vazio | Tipo::Logico | Tipo::Inteiro | Tipo::Inteiro8 |
            Tipo::Inteiro16 | Tipo::Inteiro32 | Tipo::Inteiro64 | Tipo::Inteiro128 |
            Tipo::InteiroU8 | Tipo::InteiroU16 | Tipo::InteiroU32 | Tipo::InteiroU64 | Tipo::InteiroU128 |
            Tipo::Decimal | Tipo::Decimal32 | Tipo::Decimal64 | Tipo::Caractere | Tipo::Byte
        )
    }

    fn mover_valor(&mut self, expressao: &Expressao) {
        match expressao {
            Expressao::Nome(nome) => {
                if let Some(simbolo) = self.buscar_mutavel(nome) {
                    if !Self::tipo_copiavel(&simbolo.tipo) { simbolo.movido = true; }
                }
            }
            Expressao::Lista(itens) | Expressao::Tupla(itens) => {
                for item in itens { self.mover_valor(item); }
            }
            Expressao::ConstrucaoEstrutura { campos, .. } => {
                for (_, valor) in campos { self.mover_valor(valor); }
            }
            _ => {}
        }
    }

    fn declarar(&mut self, nome: &str, simbolo: Simbolo) {
        if self.funcoes.contains_key(nome) || self.modulos_importados.contains_key(nome) {
            self.erro("N3019", format!("O nome '{nome}' já identifica uma função ou módulo neste programa."));
            return;
        }
        let escopo = self.escopos.last_mut().expect("sempre há escopo global");
        if escopo.contains_key(nome) {
            self.diagnosticos.push(Diagnostico::novo(
                "N3019", Severidade::Erro,
                format!("O nome '{nome}' já foi declarado neste escopo."), None,
            ));
        } else { escopo.insert(nome.to_owned(), simbolo); }
    }

    fn buscar(&self, nome: &str) -> Option<&Simbolo> {
        self.escopos.iter().rev().find_map(|escopo| escopo.get(nome))
    }

    fn exigir_expressao_tipo(&mut self, expressao: &Expressao, obtido: &Tipo, esperado: &Tipo, contexto: &str) {
        if *esperado == Tipo::Desconhecido { return; }
        if let Some(valor) = Self::literal_numerico(expressao) {
            let compativel = match (valor, esperado) {
                (LiteralNumerico::Inteiro(v), Tipo::Inteiro8) => i8::try_from(v).is_ok(),
                (LiteralNumerico::Inteiro(v), Tipo::Inteiro16) => i16::try_from(v).is_ok(),
                (LiteralNumerico::Inteiro(v), Tipo::Inteiro32) => i32::try_from(v).is_ok(),
                (LiteralNumerico::Inteiro(v), Tipo::Inteiro64) => i64::try_from(v).is_ok(),
                (LiteralNumerico::Inteiro(_), Tipo::Inteiro128 | Tipo::Inteiro) => true,
                (LiteralNumerico::Inteiro(v), Tipo::InteiroU8) => u8::try_from(v).is_ok(),
                (LiteralNumerico::Inteiro(v), Tipo::InteiroU16) => u16::try_from(v).is_ok(),
                (LiteralNumerico::Inteiro(v), Tipo::InteiroU32) => u32::try_from(v).is_ok(),
                (LiteralNumerico::Inteiro(v), Tipo::InteiroU64) => u64::try_from(v).is_ok(),
                (LiteralNumerico::Inteiro(v), Tipo::InteiroU128) => u128::try_from(v).is_ok(),
                (LiteralNumerico::InteiroSemSinal(v), Tipo::InteiroU8) => u8::try_from(v).is_ok(),
                (LiteralNumerico::InteiroSemSinal(v), Tipo::InteiroU16) => u16::try_from(v).is_ok(),
                (LiteralNumerico::InteiroSemSinal(v), Tipo::InteiroU32) => u32::try_from(v).is_ok(),
                (LiteralNumerico::InteiroSemSinal(v), Tipo::InteiroU64) => u64::try_from(v).is_ok(),
                (LiteralNumerico::InteiroSemSinal(_), Tipo::InteiroU128) => true,
                (LiteralNumerico::Inteiro(v), Tipo::Byte) => u8::try_from(v).is_ok(),
                (LiteralNumerico::InteiroSemSinal(v), Tipo::Byte) => u8::try_from(v).is_ok(),
                (LiteralNumerico::Decimal(v), Tipo::Decimal32) => (v as f32).is_finite(),
                (LiteralNumerico::Decimal(v), Tipo::Decimal64 | Tipo::Decimal) => v.is_finite(),
                _ => false,
            };
            if compativel { return; }
            self.erro("N3020", format!("Literal numérico não cabe no tipo {} em {contexto}; tipo inferido: {}.", esperado.nome(), obtido.nome()));
            return;
        }
        if let (Expressao::Lista(itens), Tipo::Lista(tipo_esperado)) = (expressao, esperado) {
            if itens.iter().all(|item| Self::literal_numerico(item).is_some()) {
                for item in itens {
                    let tipo_item = self.expressao(item);
                    self.exigir_expressao_tipo(item, &tipo_item, tipo_esperado, contexto);
                }
                return;
            }
        }
        self.exigir_tipo(obtido, esperado, contexto);
    }

    fn literal_numerico(expressao: &Expressao) -> Option<LiteralNumerico> {
        match expressao {
            Expressao::Literal(bruto) => {
                let limpo = bruto.replace('_', "");
                if limpo.starts_with("0x") || limpo.starts_with("0X") {
                    i128::from_str_radix(&limpo[2..], 16).ok().map(LiteralNumerico::Inteiro).or_else(|| u128::from_str_radix(&limpo[2..], 16).ok().map(LiteralNumerico::InteiroSemSinal))
                } else if limpo.starts_with("0b") || limpo.starts_with("0B") {
                    i128::from_str_radix(&limpo[2..], 2).ok().map(LiteralNumerico::Inteiro).or_else(|| u128::from_str_radix(&limpo[2..], 2).ok().map(LiteralNumerico::InteiroSemSinal))
                } else if limpo.starts_with("0o") || limpo.starts_with("0O") {
                    i128::from_str_radix(&limpo[2..], 8).ok().map(LiteralNumerico::Inteiro).or_else(|| u128::from_str_radix(&limpo[2..], 8).ok().map(LiteralNumerico::InteiroSemSinal))
                } else if limpo.contains('.') || limpo.contains('e') || limpo.contains('E') {
                    limpo.parse().ok().map(LiteralNumerico::Decimal)
                } else {
                    limpo.parse().ok().map(LiteralNumerico::Inteiro).or_else(|| limpo.parse().ok().map(LiteralNumerico::InteiroSemSinal))
                }
            }
            Expressao::Unaria { operador: TipoToken::Menos, valor } => match Self::literal_numerico(valor)? {
                LiteralNumerico::Inteiro(v) => v.checked_neg().map(LiteralNumerico::Inteiro),
                LiteralNumerico::InteiroSemSinal(v) => i128::try_from(v).ok()?.checked_neg().map(LiteralNumerico::Inteiro),
                LiteralNumerico::Decimal(v) => Some(LiteralNumerico::Decimal(-v)),
            },
            Expressao::Unaria { operador: TipoToken::Mais, valor } => Self::literal_numerico(valor),
            _ => None,
        }
    }

    fn exigir_tipo(&mut self, obtido: &Tipo, esperado: &Tipo, contexto: &str) {
        if !Self::tipos_compativeis(obtido, esperado) {
            self.erro("N3020", format!("Tipo incompatível em {contexto}: esperado {}, recebido {}.", esperado.nome(), obtido.nome()));
        }
    }

    fn tipos_compativeis(obtido: &Tipo, esperado: &Tipo) -> bool {
        match (obtido, esperado) {
            (Tipo::Desconhecido, _) | (_, Tipo::Desconhecido) => true,
            (Tipo::Parametro(a), Tipo::Parametro(b)) => a == b,
            (Tipo::Lista(a), Tipo::Lista(b)) | (Tipo::Intervalo(a), Tipo::Intervalo(b))
            | (Tipo::Opcional(a), Tipo::Opcional(b)) => Self::tipos_compativeis(a, b),
            (Tipo::Resultado(a1, a2), Tipo::Resultado(b1, b2)) =>
                Self::tipos_compativeis(a1, b1) && Self::tipos_compativeis(a2, b2),
            (Tipo::Tupla(a), Tipo::Tupla(b)) => a.len() == b.len()
                && a.iter().zip(b).all(|(a, b)| Self::tipos_compativeis(a, b)),
            _ => obtido == esperado,
        }
    }

    fn inteiro(tipo: &Tipo) -> bool {
        matches!(tipo, Tipo::Inteiro | Tipo::Inteiro8 | Tipo::Inteiro16 | Tipo::Inteiro32 | Tipo::Inteiro64 | Tipo::Inteiro128 | Tipo::InteiroU8 | Tipo::InteiroU16 | Tipo::InteiroU32 | Tipo::InteiroU64 | Tipo::InteiroU128 | Tipo::Byte)
    }

    fn numerico(tipo: &Tipo) -> bool {
        matches!(tipo, Tipo::Inteiro | Tipo::Inteiro8 | Tipo::Inteiro16 | Tipo::Inteiro32 | Tipo::Inteiro64 | Tipo::Inteiro128 | Tipo::InteiroU8 | Tipo::InteiroU16 | Tipo::InteiroU32 | Tipo::InteiroU64 | Tipo::InteiroU128 | Tipo::Decimal | Tipo::Decimal32 | Tipo::Decimal64 | Tipo::Byte)
    }

    fn inteiro_sem_sinal(tipo: &Tipo) -> bool {
        matches!(tipo, Tipo::InteiroU8 | Tipo::InteiroU16 | Tipo::InteiroU32 | Tipo::InteiroU64 | Tipo::InteiroU128 | Tipo::Byte)
    }

    fn promover_numerico(a: &Tipo, b: &Tipo) -> Tipo {
        if matches!(a, Tipo::Decimal64 | Tipo::Decimal) || matches!(b, Tipo::Decimal64 | Tipo::Decimal) {
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
        if a == &Tipo::Decimal32 || b == &Tipo::Decimal32 {
            let outro = if a == &Tipo::Decimal32 { b } else { a };
            return if largura(outro) > 16 { Tipo::Decimal64 } else { Tipo::Decimal32 };
        }
        let sem_sinal = |tipo: &Tipo| matches!(tipo,
            Tipo::InteiroU8 | Tipo::InteiroU16 | Tipo::InteiroU32 | Tipo::InteiroU64 | Tipo::InteiroU128
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

    fn erro(&mut self, codigo: &'static str, mensagem: impl Into<String>) {
        self.diagnosticos.push(Diagnostico::novo(codigo, Severidade::Erro, mensagem, None));
    }

    fn snapshot_movidos(&self) -> HashSet<(usize, String)> {
        self.escopos.iter().enumerate().flat_map(|(indice, escopo)| {
            escopo.iter()
                .filter(|(_, simbolo)| simbolo.movido)
                .map(move |(nome, _)| (indice, nome.clone()))
        }).collect()
    }

    fn restaurar_movidos(&mut self, estado: &HashSet<(usize, String)>) {
        for (indice, escopo) in self.escopos.iter_mut().enumerate() {
            for (nome, simbolo) in escopo {
                simbolo.movido = estado.contains(&(indice, nome.clone()));
            }
        }
    }

    fn mesclar_movidos(
        &mut self,
        primeiro: &HashSet<(usize, String)>,
        segundo: &HashSet<(usize, String)>,
    ) {
        let combinado = primeiro.union(segundo).cloned().collect();
        self.restaurar_movidos(&combinado);
    }

    fn abrir_escopo(&mut self) { self.escopos.push(HashMap::new()); }
    fn fechar_escopo(&mut self) { if self.escopos.len() > 1 { self.escopos.pop(); } }
}

impl Tipo {
    /// Nome canônico do tipo para mensagens e ferramentas.
    #[must_use]
    pub fn nome(&self) -> String {
        match self {
            Self::Vazio => "vazio".into(), Self::Logico => "lógico".into(),
            Self::Inteiro => "inteiro".into(),
            Self::Inteiro8 => "inteiro8".into(), Self::Inteiro16 => "inteiro16".into(),
            Self::Inteiro32 => "inteiro32".into(), Self::Inteiro64 => "inteiro64".into(),
            Self::Inteiro128 => "inteiro128".into(),
            Self::InteiroU8 => "inteiro_u8".into(), Self::InteiroU16 => "inteiro_u16".into(),
            Self::InteiroU32 => "inteiro_u32".into(), Self::InteiroU64 => "inteiro_u64".into(),
            Self::InteiroU128 => "inteiro_u128".into(),
            Self::Decimal => "decimal".into(), Self::Decimal32 => "decimal32".into(),
            Self::Decimal64 => "decimal64".into(),
            Self::Texto => "texto".into(), Self::Caractere => "caractere".into(),
            Self::Byte => "byte".into(), Self::Lista(t) => format!("[{}]", t.nome()),
            Self::Intervalo(t) => format!("intervalo<{}>", t.nome()),
            Self::Opcional(t) => format!("Opcional<{}>", t.nome()),
            Self::Resultado(sucesso, falha) => format!("Resultado<{},{}>", sucesso.nome(), falha.nome()),
            Self::Tupla(itens) => format!("Tupla<{}>", itens.iter().map(Tipo::nome).collect::<Vec<_>>().join(",")),
            Self::Estrutura(nome) => nome.clone(), Self::Enumeracao(nome) => nome.clone(),
            Self::Funcao(_, _) => "função".into(),
            Self::Parametro(nome) => nome.clone(),
            Self::Desconhecido => "desconhecido".into(),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use nebula_fonte::ArquivoFonte;
    use nebula_sintaxe::analisar;

    fn analisar_codigo(codigo: &str) -> ResultadoSemantico {
        let fonte = ArquivoFonte::novo("teste.nbx", codigo);
        let sintaxe = analisar(&fonte);
        assert!(sintaxe.diagnosticos.is_empty(), "{:?}", sintaxe.diagnosticos);
        analisar_semantica(&sintaxe.programa)
    }

    #[test]
    fn valida_inteiros_sem_sinal_e_seus_limites() {
        let valido = analisar_codigo(
            "variavel maximo: inteiro_u64 = 18446744073709551615\nvariavel total: inteiro_u128 = 340282366920938463463374607431768211455",
        );
        assert!(valido.diagnosticos.is_empty(), "{:?}", valido.diagnosticos);

        let invalido = analisar_codigo("variavel limite: inteiro_u8 = 256");
        assert!(invalido.diagnosticos.iter().any(|d| d.codigo == "N3020"));
    }

    #[test]
    fn rejeita_operacao_implicita_entre_inteiros_com_sinais_diferentes() {
        let r = analisar_codigo(
            "variavel a: inteiro = 1\nvariavel b: inteiro_u32 = 2\nvariavel c = a + b",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3038"));
    }

    #[test]
    fn valida_campos_de_estrutura_e_payloads_de_enumeracao() {
        let r = analisar_codigo(
            "estrutura Pessoa { nome: texto, idade: inteiro }\nenumeracao Estado { Ativo, Falha(texto, inteiro) }",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn valida_construcao_de_variante_com_payload() {
        let r = analisar_codigo(
            "enumeracao Resultado { Sucesso(inteiro), Falha(texto) }\nvariavel resultado = Resultado::Sucesso(42)",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.tipos_globais.get("resultado"), Some(&Tipo::Enumeracao("Resultado".into())));
    }

    #[test]
    fn rejeita_payload_de_variante_com_tipo_incorreto() {
        let r = analisar_codigo(
            "enumeracao Resultado { Sucesso(inteiro), Falha(texto) }\nvariavel resultado = Resultado::Sucesso(\"texto\")",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3020"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_quantidade_incorreta_de_payload() {
        let r = analisar_codigo(
            "enumeracao Resultado { Sucesso(inteiro), Falha(texto) }\nvariavel resultado = Resultado::Sucesso()",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3044"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn valida_escolha_exaustiva_e_vinculos_de_payload() {
        let r = analisar_codigo(
            "enumeracao Resultado { Sucesso(inteiro), Falha(texto) }\nvariavel resultado = Resultado::Sucesso(42)\nescolha resultado { caso Resultado::Sucesso(valor) { exibir(valor) } caso Resultado::Falha(mensagem) { exibir(mensagem) } }",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn exige_caso_curinga_ao_final() {
        let r = analisar_codigo(
            "enumeracao Estado { Ativo, Inativo }\nvariavel estado = Estado::Ativo\nescolha estado { caso _ { exibir(0) } caso Estado::Ativo { exibir(1) } }",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3054"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_escolha_nao_exaustiva_e_payload_incorreto() {
        let r = analisar_codigo(
            "enumeracao Resultado { Sucesso(inteiro), Falha(texto) }\nvariavel resultado = Resultado::Sucesso(42)\nescolha resultado { caso Resultado::Sucesso() { exibir(1) } }",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3050"), "{:?}", r.diagnosticos);
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3053"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_tipo_desconhecido_em_estrutura_e_enumeracao() {
        let r = analisar_codigo(
            "estrutura Pessoa { nome: TipoAusente }\nenumeracao Estado { Falha(TipoAusente) }",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3034"));
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3037"));
    }

    #[test]
    fn resolve_tipos_nominais_em_assinaturas_de_funcoes() {
        let r = analisar_codigo(
            "estrutura Pessoa { nome: texto }\nenumeracao Estado { Ativo, Inativo }\nfuncao receber(pessoa: Pessoa, estado: Estado): Pessoa { devolver pessoa }",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn infere_tuplas_heterogeneas_e_acesso_estatico() {
        let r = analisar_codigo(
            "variavel ponto = (10, \"norte\", verdadeiro)\nvariavel x = ponto[0]\nvariavel direcao = ponto[1]\nvariavel ativo = ponto[2]",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.tipos_globais.get("ponto"), Some(&Tipo::Tupla(vec![Tipo::Inteiro, Tipo::Texto, Tipo::Logico])));
        assert_eq!(r.tipos_globais.get("x"), Some(&Tipo::Inteiro));
        assert_eq!(r.tipos_globais.get("direcao"), Some(&Tipo::Texto));
        assert_eq!(r.tipos_globais.get("ativo"), Some(&Tipo::Logico));
    }

    #[test]
    fn rejeita_indice_de_tupla_dinamico_ou_fora_dos_limites() {
        let r = analisar_codigo(
            "variavel ponto = (10, \"norte\")\nvariavel indice = 1\nvariavel valor = ponto[indice]\nvariavel invalido = ponto[3]",
        );
        assert_eq!(r.diagnosticos.iter().filter(|d| d.codigo == "N3034").count(), 2, "{:?}", r.diagnosticos);
    }

    #[test]
    fn infere_tipos_primitivos_e_variaveis() {
        let r = analisar_codigo("variavel total = 10\nvariavel nome: texto = \"Nebula\"");
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.tipos_globais.get("total"), Some(&Tipo::Inteiro));
        assert_eq!(r.tipos_globais.get("nome"), Some(&Tipo::Texto));
    }
    #[test]
    fn rejeita_nome_nao_declarado() {
        let r = analisar_codigo("variavel x = ausente");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3007"));
    }
    #[test]
    fn rejeita_tipo_incompativel() {
        let r = analisar_codigo("variavel x: inteiro = \"texto\"");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3020"));
    }
    #[test]
    fn rejeita_reatribuicao_de_constante() {
        let r = analisar_codigo("constante x = 1\nx = 2");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3009"));
    }
    #[test]
    fn valida_assinatura_e_chamada_de_funcao() {
        let r = analisar_codigo("funcao soma(a: inteiro, b: inteiro): inteiro { devolver a + b }\nvariavel x = soma(1, 2)");
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn rejeita_argumento_de_tipo_incorreto() {
        let r = analisar_codigo("funcao soma(a: inteiro): inteiro { devolver a }\nsoma(\"x\")");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3020"));
    }
    #[test]
    fn verifica_condicao_logica() {
        let r = analisar_codigo("quando 1 { exibir(\"x\") }");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3020"));
    }
    #[test]
    fn aceita_lista_homogenea_e_indexacao() {
        let r = analisar_codigo("variavel xs = [1, 2, 3]\nvariavel x = xs[0]");
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn rejeita_controle_de_laco_fora_de_laco() {
        let r = analisar_codigo("parar");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3006"));
    }
    #[test]
    fn resolve_alias_de_tipo() {
        let r = analisar_codigo("tipo Numero = inteiro\nvariavel x: Numero = 1");
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn resolve_modulo_e_funcao_exportada() {
        let mut modulos = ModulosDisponiveis::new();
        modulos.insert("Matematica".into(), HashMap::from([(
            "somar".into(),
            AssinaturaFuncao { parametros: vec![Tipo::Inteiro, Tipo::Inteiro], retorno: Tipo::Inteiro },
        )]));
        let fonte = ArquivoFonte::novo("teste.nbx", "variavel total = Mat::somar(1, 2)\nincluir Matematica como Mat");
        let sintaxe = analisar(&fonte);
        assert!(sintaxe.diagnosticos.is_empty(), "{:?}", sintaxe.diagnosticos);
        let r = analisar_com_modulos(&sintaxe.programa, &modulos);
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_modulo_ausente() {
        let r = analisar_codigo("incluir ModuloInexistente");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3004"));
    }

    #[test]
    fn rejeita_mutacao_de_lista_constante() {
        let r = analisar_codigo("constante itens = [1]\nadicionar(itens, 2)");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3009"));
        let r = analisar_codigo("constante itens = [1]\nremover(itens, 0)");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3009"));
    }

    #[test]
    fn valida_tipos_dos_builtins_de_texto_unicode() {
        let r = analisar_codigo(
            "variavel limpo = aparar(\" texto \")\nvariavel inicio = comeca_com(limpo, \"te\")\nvariavel sufixo_ok = termina_com(limpo, \"to\")\nvariavel trocado = substituir(limpo, \"e\", \"E\")",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.tipos_globais.get("limpo"), Some(&Tipo::Texto));
        assert_eq!(r.tipos_globais.get("inicio"), Some(&Tipo::Logico));
        assert_eq!(r.tipos_globais.get("sufixo_ok"), Some(&Tipo::Logico));
        assert_eq!(r.tipos_globais.get("trocado"), Some(&Tipo::Texto));
    }

    #[test]
    fn rejeita_tipos_incorretos_nos_builtins_de_texto() {
        let r = analisar_codigo("aparar(10)\ncomeca_com(\"texto\", 1)\nsubstituir(\"a\", \"b\", falso)");
        assert!(r.diagnosticos.iter().filter(|d| d.codigo == "N3020").count() >= 3);
    }

    #[test]
    fn rejeita_funcao_que_pode_terminar_sem_retorno() {
        let r = analisar_codigo("funcao calcular(condicao: logico): inteiro { quando condicao { devolver 1 } }");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3024"));
    }

    #[test]
    fn aceita_retorno_em_todos_os_caminhos_condicionais() {
        let r = analisar_codigo("funcao calcular(condicao: logico): inteiro { quando condicao { devolver 1 } senao { devolver 2 } }");
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn distingue_larguras_de_inteiros() {
        let r = analisar_codigo("variavel a: inteiro8 = 1\nvariavel b: inteiro64 = a");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3020"));
    }

    #[test]
    fn detecta_alias_de_tipo_ciclico() {
        let r = analisar_codigo("tipo A = B\ntipo B = A");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3003"));
    }


    #[test]
    fn rejeita_variavel_com_nome_de_funcao() {
        let r = analisar_codigo("funcao calcular(): inteiro { devolver 1 }\nvariavel calcular = 2");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3019"));
    }


    #[test]
    fn permite_sombreamento_em_escopo_interno() {
        let r = analisar_codigo("variavel valor = 1\nquando verdadeiro { variavel valor = 2\nexibir(valor) }");
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn impede_acesso_a_variavel_apos_fim_do_escopo() {
        let r = analisar_codigo("quando verdadeiro { variavel interna = 1 }\nexibir(interna)");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3007"));
    }

    #[test]
    fn rejeita_alias_de_modulo_duplicado() {
        let mut modulos = ModulosDisponiveis::new();
        modulos.insert("A".into(), HashMap::new());
        modulos.insert("B".into(), HashMap::new());
        let fonte = ArquivoFonte::novo("teste.nbx", "incluir A como M\nincluir B como M");
        let sintaxe = analisar(&fonte);
        assert!(sintaxe.diagnosticos.is_empty(), "{:?}", sintaxe.diagnosticos);
        let r = analisar_com_modulos(&sintaxe.programa, &modulos);
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3022"));
    }


    #[test]
    fn aceita_lista_vazia_com_tipo_explicito() {
        let r = analisar_codigo("variavel itens: [inteiro] = []");
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn exige_tipo_para_lista_vazia_sem_contexto() {
        let r = analisar_codigo("variavel itens = []");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3027"));
    }


    #[test]
    fn promove_decimal32_conforme_largura_do_inteiro() {
        let r = analisar_codigo("variavel d: decimal32 = 1.0\nvariavel pequeno: inteiro8 = 1\nvariavel grande: inteiro32 = 100\nvariavel x: decimal32 = d + pequeno\nvariavel y: decimal64 = d + grande");
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }


    #[test]
    fn aceita_literais_numericos_que_cabem_no_tipo_anotado() {
        let r = analisar_codigo("variavel pequeno: inteiro8 = -12\nvariavel byte: byte = 255\nvariavel precisao: decimal32 = 1.25");
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_literal_numerico_fora_da_largura() {
        let r = analisar_codigo("variavel pequeno: inteiro8 = 300\nvariavel byte: byte = -1");
        assert!(r.diagnosticos.iter().filter(|d| d.codigo == "N3020").count() >= 2);
    }


    #[test]
    fn rejeita_literal_decimal_infinito() {
        let r = analisar_codigo("variavel valor = 1e400");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3028"));
    }


    #[test]
    fn rejeita_colisao_entre_funcao_e_alias_de_modulo() {
        let mut modulos = ModulosDisponiveis::new();
        modulos.insert("Matematica".into(), HashMap::new());
        let fonte = ArquivoFonte::novo(
            "teste.nbx",
            "incluir Matematica como calcular\nfuncao calcular(): inteiro { devolver 1 }",
        );
        let sintaxe = analisar(&fonte);
        assert!(sintaxe.diagnosticos.is_empty(), "{:?}", sintaxe.diagnosticos);
        let r = analisar_com_modulos(&sintaxe.programa, &modulos);
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3019"));
    }

    #[test]
    fn aceita_indices_com_larguras_inteiras_explicitas() {
        let r = analisar_codigo("variavel itens = [10, 20]\nvariavel indice: inteiro32 = 1\nvariavel valor = itens[indice]\nvariavel byte: byte = 0\nvariavel outro = itens[byte]");
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_indice_decimal() {
        let r = analisar_codigo("variavel itens = [10, 20]\nvariavel indice: decimal32 = 1.0\nvariavel valor = itens[indice]");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3020"));
    }


    #[test]
    fn rejeita_uso_de_texto_apos_movimentacao() {
        let r = analisar_codigo("variavel origem: texto = \"Nebula\"\nvariavel destino: texto = origem\nexibir(origem)");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_uso_de_lista_apos_movimentacao() {
        let r = analisar_codigo("variavel origem = [1, 2]\nvariavel destino = origem\nexibir(comprimento(origem))");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn valores_primitivos_sao_copiados_e_nao_movidos() {
        let r = analisar_codigo("variavel origem = 42\nvariavel destino = origem\nexibir(origem)");
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn reatribuicao_reinicializa_valor_movido() {
        let r = analisar_codigo("variavel origem: texto = \"antigo\"\nvariavel destino: texto = origem\norigem = \"novo\"\nexibir(origem)");
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn resolve_tipos_genericos_aninhados_em_assinaturas() {
        let r = analisar_codigo(
            "funcao recebe(valor: Opcional<[inteiro]>) { exibir(\"ok\") }\nfuncao tenta(valor: Resultado<texto,inteiro>) { exibir(\"ok\") }",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn resolve_tupla_generica_com_mais_de_dois_elementos() {
        let r = analisar_codigo(
            "funcao recebe(valor: Tupla<inteiro,texto,logico>) { exibir(\"ok\") }",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_aridade_incorreta_de_tipo_generico() {
        let r = analisar_codigo("funcao recebe(valor: Resultado<inteiro>) { exibir(\"ok\") }");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3021"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn infere_generico_em_funcao_de_identidade() {
        let r = analisar_codigo(
            "funcao identidade<T>(valor: T): T { devolver valor }\nvariavel numero = identidade(42)\nvariavel texto = identidade(\"Nebula\")",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.tipos_globais.get("numero"), Some(&Tipo::Inteiro));
        assert_eq!(r.tipos_globais.get("texto"), Some(&Tipo::Texto));
    }

    #[test]
    fn infere_generico_aninhado_em_lista() {
        let r = analisar_codigo(
            "funcao primeiro<T>(valores: [T]): T { devolver valores[0] }\nvariavel numero = primeiro([1, 2, 3])",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.tipos_globais.get("numero"), Some(&Tipo::Inteiro));
    }

    #[test]
    fn rejeita_inferencia_generica_conflitante() {
        let r = analisar_codigo(
            "funcao escolher<T>(a: T, b: T): T { devolver a }\nvariavel valor = escolher(1, \"texto\")",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3032"), "{:?}", r.diagnosticos);
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3020"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_parametro_generico_sem_inferencia() {
        let r = analisar_codigo(
            "funcao produzir<T>(valor: inteiro): T { devolver valor }\nvariavel resultado = produzir(1)",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3033"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn tipos_genericos_aninhados_inferem_recursivamente() {
        let r = analisar_codigo(
            "funcao recebe<T>(valor: Opcional<[T]>) { exibir(\"ok\") }",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn nao_coage_literal_inteiro_para_tipo_decimal() {
        let r = analisar_codigo("variavel valor: decimal32 = 1");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3020"));
    }

    #[test]
    fn movimento_em_um_ramo_e_propagado_apos_a_juncao() {
        let r = analisar_codigo(
            "variavel texto = \"Nebula\"\nquando verdadeiro { variavel destino = texto }\nexibir(texto)",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn movimento_em_laco_e_considerado_possivel_apos_o_laco() {
        let r = analisar_codigo(
            "variavel texto = \"Nebula\"\nenquanto verdadeiro { variavel destino = texto }\nexibir(texto)",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn resolve_tipo_de_campo_de_estrutura() {
        let r = analisar_codigo(
            "estrutura Pessoa { nome: texto, idade: inteiro }\nfuncao obter_nome(pessoa: Pessoa): texto { devolver pessoa.nome }",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_campo_inexistente_em_estrutura() {
        let r = analisar_codigo(
            "estrutura Pessoa { nome: texto }\nfuncao obter_id(pessoa: Pessoa): inteiro { devolver pessoa.id }",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3040"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_acesso_a_campo_em_tipo_primitivo() {
        let r = analisar_codigo(
            "funcao obter_campo(valor: inteiro): texto { devolver valor.nome }",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3041"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn construcao_de_estrutura_valida_todos_os_campos() {
        let r = analisar_codigo(
            "estrutura Pessoa { nome: texto, idade: inteiro }\nvariavel pessoa = Pessoa { nome: \"Lia\", idade: 28 }",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn construcao_de_estrutura_rejeita_campo_ausente() {
        let r = analisar_codigo(
            "estrutura Pessoa { nome: texto, idade: inteiro }\nvariavel pessoa = Pessoa { nome: \"Lia\" }",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3043"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn construcao_de_estrutura_rejeita_tipo_incompativel() {
        let r = analisar_codigo(
            "estrutura Pessoa { idade: inteiro }\nvariavel pessoa = Pessoa { idade: \"vinte\" }",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3020"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn reinicializacao_em_todos_os_ramos_remove_estado_de_movido() {
        let r = analisar_codigo(
            "variavel texto = \"Nebula\"\nquando verdadeiro { variavel a = texto\ntexto = \"A\" } senao { variavel b = texto\ntexto = \"B\" }\nexibir(texto)",
        );
        assert!(!r.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", r.diagnosticos);
    }

}

}

pub use implementacao::*;
