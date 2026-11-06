//! Análise semântica, resolução de nomes e sistema de tipos da Nebula.

#![forbid(unsafe_code)]
#![allow(missing_docs)]

#[rustfmt::skip]
mod implementacao {
use nebula_diagnosticos::{Diagnostico, Severidade};
use nebula_sintaxe::{Expressao, NoAst, Programa, TipoAst, TipoToken};
use std::collections::{HashMap, HashSet};
use nebula_fonte::IntervaloFonte;

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
    pub genericos: Vec<String>,
    pub parametros: Vec<Tipo>,
    pub nomes_parametros: Vec<String>,
    pub parametros_padrao: Vec<bool>,
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
    let mut analisador = Analisador::novo(modulos, programa);
    analisador.analisar(programa);
    ResultadoSemantico {
        diagnosticos: analisador.diagnosticos,
        tipos_globais: analisador.escopos[0].iter()
            .map(|(nome, simbolo)| (nome.clone(), simbolo.tipo.clone()))
            .collect(),
    }
}

#[derive(Debug, Clone)]
struct Simbolo { tipo: Tipo, mutavel: bool, movido: bool, campos_movidos: HashSet<String> }

#[derive(Debug, Clone, Copy)]
enum LiteralNumerico { Inteiro(i128), InteiroSemSinal(u128), Decimal(f64) }

#[derive(Debug, Clone)]
struct Funcao { genericos: Vec<String>, parametros: Vec<Tipo>, nomes_parametros: Vec<String>, parametros_padrao: Vec<Option<Expressao>>, retorno: Tipo }

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
    intervalos_nos: Vec<(NoAst, IntervaloFonte)>,
    intervalos_expressoes: Vec<(Expressao, IntervaloFonte)>,
    intervalo_atual: Option<IntervaloFonte>,
    classes: HashMap<String, Option<String>>,
    classe_atual: Option<String>,
}

impl<'a> Analisador<'a> {
    fn novo(modulos: &'a ModulosDisponiveis, programa: &Programa) -> Self {
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
            intervalos_nos: programa.nos_localizados.iter().map(|localizado| (localizado.no.clone(), localizado.intervalo)).collect(),
            intervalos_expressoes: programa.expressoes_localizadas.iter().map(|localizado| (localizado.expressao.clone(), localizado.intervalo)).collect(),
            intervalo_atual: None,
            classes: HashMap::new(),
            classe_atual: None,
        }
    }

    fn analisar(&mut self, programa: &Programa) {
        self.predeclarar(programa);
        for item in &programa.itens { self.item(item); }
    }

    fn predeclarar(&mut self, programa: &Programa) {
        for item in &programa.itens {
            if let Some(intervalo) = self.intervalos_nos.iter().find(|(no, _)| no == item).map(|(_, intervalo)| *intervalo) {
                self.intervalo_atual = Some(intervalo);
            }
            let item = Self::alvo_atributo(item);
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
            if let NoAst::Classe { nome, herda, .. } = item {
                if self.classes.insert(nome.clone(), herda.clone()).is_some() {
                    self.erro("N3065", format!("A classe '{nome}' foi declarada mais de uma vez."));
                }
                if !self.estruturas.insert(nome.clone()) {
                    self.erro("N3086", format!("O nome '{nome}' já foi usado por uma estrutura ou classe."));
                }
            }
        }
        let aliases: Vec<String> = self.aliases.keys().cloned().collect();
        for nome in aliases {
            if Self::tipo_contem_desconhecido(&self.resolver_tipo(&nome)) {
                self.erro("N3003", format!("O alias de tipo '{nome}' contém referência inválida ou ciclo."));
            }
        }
        for item in &programa.itens {
            if let Some(intervalo) = self.intervalos_nos.iter().find(|(no, _)| no == item).map(|(_, intervalo)| *intervalo) {
                self.intervalo_atual = Some(intervalo);
            }
            let item = Self::alvo_atributo(item);
            if let NoAst::Estrutura { nome, campos } = item {
                let descritores = campos.iter().map(|(campo, tipo)| (campo.clone(), self.resolver_tipo(tipo))).collect();
                self.campos_estruturas.insert(nome.clone(), descritores);
            }
            if let NoAst::Classe { nome, corpo, .. } = item {
                let mut descritores = HashMap::new();
                for membro in corpo {
                    if let Some(intervalo) = self.intervalos_nos.iter().find(|(no, _)| no == membro).map(|(_, intervalo)| *intervalo) {
                        self.intervalo_atual = Some(intervalo);
                    }
                    let membro = Self::alvo_atributo(membro);
                    if let NoAst::Variavel { nome: campo, tipo: Some(tipo), .. } = membro {
                        descritores.insert(campo.clone(), self.resolver_tipo(tipo));
                    }
                }
                self.campos_estruturas.insert(nome.clone(), descritores);
            }
        }
        for item in &programa.itens {
            if let Some(intervalo) = self.intervalos_nos.iter().find(|(no, _)| no == item).map(|(_, intervalo)| *intervalo) {
                self.intervalo_atual = Some(intervalo);
            }
            let item = Self::alvo_atributo(item);
            if let NoAst::Classe { nome, corpo, .. } = item {
                for membro in corpo {
                    if let Some(intervalo) = self.intervalos_nos.iter().find(|(no, _)| no == membro).map(|(_, intervalo)| *intervalo) {
                        self.intervalo_atual = Some(intervalo);
                    }
                    let membro = Self::alvo_atributo(membro);
                    if let NoAst::Funcao { nome: metodo, genericos, parametros, parametros_padrao, retorno, .. } = membro {
                        let chave = format!("{nome}::{metodo}");
                        let anteriores = self.parametros_genericos.clone();
                        self.parametros_genericos.extend(genericos.iter().cloned());
                        let tipos: Vec<Tipo> = parametros.iter().map(|(_, tipo)| self.resolver_tipo(tipo)).collect();
                        let retorno = retorno.as_deref().map_or(Tipo::Vazio, |tipo| self.resolver_tipo(tipo));
                        self.parametros_genericos = anteriores;
                        let duplicado = match self.funcoes.entry(chave) {
                            std::collections::hash_map::Entry::Vacant(entrada) => {
                                entrada.insert(Funcao {
                                    genericos: genericos.clone(),
                                    parametros: tipos,
                                    nomes_parametros: parametros.iter().map(|(nome, _)| nome.clone()).collect(),
                                    parametros_padrao: parametros_padrao.clone(),
                                    retorno,
                                });
                                false
                            }
                            std::collections::hash_map::Entry::Occupied(_) => true,
                        };
                        if duplicado { self.erro("N3079", format!("Método '{metodo}' duplicado na classe '{nome}'.")); }
                    }
                }
            }
        }
        self.validar_hierarquia_classes();
        for item in &programa.itens {
            if let Some(intervalo) = self.intervalos_nos.iter().find(|(no, _)| no == item).map(|(_, intervalo)| *intervalo) {
                self.intervalo_atual = Some(intervalo);
            }
            let item = Self::alvo_atributo(item);
            if let NoAst::Enumeracao { nome, variantes } = item {
                let descritores = variantes.iter().map(|(variante, payload)| {
                    (variante.clone(), payload.iter().map(|tipo| self.resolver_tipo(tipo)).collect())
                }).collect();
                self.variantes_enumeracoes.insert(nome.clone(), descritores);
            }
        }
        for item in &programa.itens {
            if let Some(intervalo) = self.intervalos_nos.iter().find(|(no, _)| no == item).map(|(_, intervalo)| *intervalo) {
                self.intervalo_atual = Some(intervalo);
            }
            let item = Self::alvo_atributo(item);
            if let NoAst::Funcao { nome, genericos, parametros, parametros_padrao, retorno, .. } = item {
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
                    if Self::tipo_contem_desconhecido(&tipo) {
                        self.erro("N3021", format!("Tipo '{nome_tipo}' do parâmetro '{parametro}' não foi declarado."));
                    }
                    tipo
                }).collect();
                let retorno = retorno.as_deref().map_or(Tipo::Vazio, |nome_tipo| {
                    let tipo = self.resolver_tipo(nome_tipo);
                    if Self::tipo_contem_desconhecido(&tipo) {
                        self.erro("N3021", format!("Tipo de retorno '{nome_tipo}' da função '{nome}' não foi declarado."));
                    }
                    tipo
                });
                self.parametros_genericos = anteriores;
                if self.funcoes.contains_key(nome) {
                    self.erro("N3001", format!("A função '{nome}' foi declarada mais de uma vez."));
                } else {
                    let nomes_parametros = parametros.iter().map(|(nome, _)| nome.clone()).collect();
                    let padroes = parametros_padrao.clone();
                    let mut encontrou_padrao = false;
                    for padrao in &padroes {
                        if padrao.is_some() { encontrou_padrao = true; }
                        else if encontrou_padrao {
                            self.erro("N3066", format!("Parâmetros sem valor padrão não podem aparecer depois de parâmetros opcionais em '{nome}'."));
                        }
                    }
                    self.funcoes.insert(nome.clone(), Funcao {
                        genericos: genericos.clone(),
                        parametros: tipos,
                        nomes_parametros,
                        parametros_padrao: padroes,
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

    fn validar_hierarquia_classes(&mut self) {
        let classes = self.classes.clone();
        for (nome, base) in &classes {
            if let Some(base) = base {
                if !classes.contains_key(base) {
                    self.erro("N3069", format!("A classe-base '{base}' da classe '{nome}' não foi declarada."));
                } else if base == nome {
                    self.erro("N3070", format!("A classe '{nome}' não pode herdar de si mesma."));
                }
            }
        }
        for inicio in classes.keys() {
            let mut visitados = HashSet::new();
            let mut atual = inicio.as_str();
            while let Some(Some(base)) = classes.get(atual) {
                if !visitados.insert(atual.to_owned()) || base == inicio {
                    self.erro("N3070", format!("A hierarquia da classe '{inicio}' contém um ciclo de herança."));
                    break;
                }
                atual = base;
            }
        }
        for _ in 0..classes.len() {
            let snapshot = self.campos_estruturas.clone();
            for (nome, base) in &classes {
                if let Some(base) = base {
                    if let Some(campos_base) = snapshot.get(base) {
                        let campos = self.campos_estruturas.entry(nome.clone()).or_default();
                        for (campo, tipo) in campos_base {
                            campos.entry(campo.clone()).or_insert_with(|| tipo.clone());
                        }
                    }
                }
            }
        }
        let metodos: Vec<(String, Funcao)> = self.funcoes.iter().map(|(nome, funcao)| (nome.clone(), funcao.clone())).collect();
        for (nome, base) in &classes {
            let Some(base) = base else { continue; };
            for (chave, metodo) in &metodos {
                let Some(nome_metodo) = chave.strip_prefix(&format!("{nome}::")) else { continue; };
                let mut ancestral = Some(base.as_str());
                while let Some(nome_ancestral) = ancestral {
                    let chave_base = format!("{nome_ancestral}::{nome_metodo}");
                    if let Some(metodo_base) = self.funcoes.get(&chave_base).cloned() {
                        if metodo.parametros.len() != metodo_base.parametros.len()
                            || !metodo.parametros.iter().zip(&metodo_base.parametros).all(|(a, b)| Self::tipos_compativeis(a, b))
                            || !Self::tipos_compativeis(&metodo.retorno, &metodo_base.retorno)
                        {
                            self.erro("N3087", format!("O método '{nome_metodo}' da classe '{nome}' não respeita a assinatura herdada."));
                        }
                        break;
                    }
                    ancestral = classes.get(nome_ancestral).and_then(Option::as_deref);
                }
            }
        }
    }

    fn alvo_atributo(declaracao: &NoAst) -> &NoAst {
        match declaracao {
            NoAst::Atributada { declaracao, .. } => Self::alvo_atributo(declaracao),
            outro => outro,
        }
    }

    fn item(&mut self, item: &NoAst) {
        if let Some((_, intervalo)) = self.intervalos_nos.iter().find(|(no, _)| no == item) { self.intervalo_atual = Some(*intervalo); }
        match item {
            NoAst::Variavel { nome, tipo, valor, constante } => {
                let esperado_declarado = tipo.as_deref().map(|tipo| self.resolver_tipo(tipo));
                let inferido = if let Some(esperado) = esperado_declarado.as_ref() {
                    self.expressao_com_tipo_esperado(valor, esperado)
                } else {
                    self.expressao(valor)
                };
                self.mover_valor(valor);
                let final_tipo = if let Some(esperado) = esperado_declarado {
                    if Self::tipo_contem_desconhecido(&esperado) {
                        self.erro("N3021", format!("Tipo '{}' não foi declarado.", tipo.as_deref().unwrap_or("")));
                    }
                    self.exigir_expressao_tipo(valor, &inferido, &esperado, "inicialização da variável");
                    esperado
                } else {
                    if matches!(&inferido, Tipo::Lista(elemento) if Self::tipo_contem_desconhecido(elemento)) {
                        self.erro("N3027", format!("Não foi possível inferir o tipo dos elementos da lista '{nome}'; anote seu tipo."));
                    }
                    inferido
                };
                self.declarar(nome, Simbolo { tipo: final_tipo, mutavel: !constante, movido: false, campos_movidos: HashSet::new() });
            }
            NoAst::Funcao { nome, genericos, parametros, parametros_padrao, corpo, .. } => {
                let chave = self.classe_atual.as_ref().map_or_else(|| nome.clone(), |classe| format!("{classe}::{nome}"));
                if !self.funcoes_vistas.insert(chave.clone()) { return; }
                let Some(funcao) = self.funcoes.get(&chave).cloned() else {
                    self.erro("N3023", format!("Função aninhada '{nome}' não é permitida no escopo inicial."));
                    return;
                };
                let genericos_anteriores = self.parametros_genericos.clone();
                self.parametros_genericos.extend(genericos.iter().cloned());
                self.abrir_escopo();
                for (indice, ((nome_parametro, _), tipo)) in parametros.iter().zip(&funcao.parametros).enumerate() {
                    if let Some(Some(padrao)) = parametros_padrao.get(indice) {
                        let tipo_obtido = self.expressao_com_tipo_esperado(padrao, tipo);
                        self.exigir_expressao_tipo(padrao, &tipo_obtido, tipo, "valor padrão do parâmetro");
                    }
                    self.declarar(nome_parametro, Simbolo { tipo: tipo.clone(), mutavel: true, movido: false, campos_movidos: HashSet::new() });
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
            NoAst::Atributada { atributos, declaracao } => {
                let alvo = Self::alvo_atributo(declaracao);
                let mut vistos = HashSet::new();
                for atributo in atributos {
                    if !vistos.insert(&atributo.nome) {
                        self.erro("N3074", format!("Atributo '{}' foi declarado mais de uma vez.", atributo.nome));
                    }
                    match atributo.nome.as_str() {
                        "alinhamento" => {
                            if !matches!(alvo, NoAst::Estrutura { .. } | NoAst::Classe { .. } | NoAst::Enumeracao { .. }) {
                                self.erro("N3075", "O atributo '@alinhamento' só pode ser aplicado a tipos compostos.");
                            }
                            if atributo.argumentos.len() != 1 { self.erro("N3075", "O atributo '@alinhamento' exige exatamente um argumento inteiro."); }
                            for argumento in &atributo.argumentos {
                                let tipo = self.expressao(argumento);
                                if !Self::inteiro(&tipo) { self.erro("N3075", "O argumento de '@alinhamento' precisa ser inteiro."); }
                                match Self::literal_numerico(argumento) {
                                    Some(LiteralNumerico::Inteiro(valor)) if valor > 0 && (valor as u128).is_power_of_two() => {}
                                    Some(LiteralNumerico::InteiroSemSinal(valor)) if valor > 0 && valor.is_power_of_two() => {}
                                    _ => self.erro("N3075", "O alinhamento precisa ser um literal inteiro constante e potência de dois positiva."),
                                }
                            }
                        }
                        "representacao" | "convencao" => {
                            if atributo.nome == "representacao" && !matches!(alvo, NoAst::Estrutura { .. } | NoAst::Classe { .. } | NoAst::Enumeracao { .. }) {
                                self.erro("N3076", "O atributo '@representacao' só pode ser aplicado a estrutura, classe ou enumeração.");
                            }
                            if atributo.nome == "convencao" && !matches!(alvo, NoAst::Funcao { .. }) {
                                self.erro("N3076", "O atributo '@convencao' só pode ser aplicado a uma função.");
                            }
                            if atributo.argumentos.len() != 1 { self.erro("N3076", format!("O atributo '@{}' exige exatamente um argumento de texto.", atributo.nome)); }
                            for argumento in &atributo.argumentos {
                                let tipo = self.expressao(argumento);
                                self.exigir_tipo(&tipo, &Tipo::Texto, "argumento de atributo");
                            }
                        }
                        "externa" => {
                            let funcao_externa_sem_corpo = matches!(alvo, NoAst::Funcao { corpo, .. } if corpo.is_empty());
                            if !atributo.argumentos.is_empty() || !funcao_externa_sem_corpo {
                                self.erro("N3077", "O atributo '@externa' só pode ser aplicado sem argumentos a uma função sem corpo.");
                            }
                        }
                        outro => {
                            for argumento in &atributo.argumentos { self.expressao(argumento); }
                            self.erro("N3078", format!("Atributo '@{outro}' ainda não possui contrato semântico definido."));
                        }
                    }
                }
                if atributos.iter().any(|atributo| atributo.nome == "externa") && matches!(alvo, NoAst::Funcao { .. }) {
                    return;
                }
                self.item(declaracao);
            },
            NoAst::Tente { corpo, capturas, sempre } => {
                self.abrir_escopo();
                for instrucao in corpo { self.item(instrucao); }
                for captura in capturas { self.abrir_escopo(); for instrucao in &captura.corpo { self.item(instrucao); } self.fechar_escopo(); }
                for instrucao in sempre { self.item(instrucao); }
                self.fechar_escopo();
            }
            NoAst::Com { recurso, corpo, .. } => { self.expressao(recurso); self.abrir_escopo(); for instrucao in corpo { self.item(instrucao); } self.fechar_escopo(); }
            NoAst::Lancar(expressao) => { self.expressao(expressao); }
            NoAst::Verifique { condicao, mensagem } => {
                let tipo = self.expressao(condicao);
                self.exigir_tipo(&tipo, &Tipo::Logico, "condição de 'verifique'");
                if let Some(mensagem) = mensagem { self.expressao(mensagem); }
            }
            NoAst::Escopo { .. } => {}
            NoAst::BlocoContextual { contexto, argumento, tipos_genericos, argumentos, corpo } => {
                match contexto.as_str() {
                    "critico" if argumento.is_some() || !tipos_genericos.is_empty() || !argumentos.is_empty() => self.erro("N3072", "O contexto 'critico' não aceita argumentos."),
                    "seguro" if !tipos_genericos.is_empty() || !argumentos.is_empty() => self.erro("N3072", "O contexto 'seguro' não aceita tipos genéricos nem argumentos de chamada."),
                    "privilegio" if argumento.as_deref() != Some("sistema") => self.erro("N3072", "O contexto 'privilegio' exige o perfil 'sistema'."),
                    "montagem" if argumento.as_deref() != Some("x86_64") => self.erro("N3072", "O bloco 'montagem' só possui validação semântica para 'x86_64'."),
                    "critico" | "seguro" | "privilegio" | "montagem" => {},
                    _ => self.erro("N3072", format!("Contexto semântico desconhecido: '{contexto}'.")),
                }
                for nome_tipo in tipos_genericos {
                    if Self::tipo_contem_desconhecido(&self.resolver_tipo(nome_tipo)) {
                        self.erro("N3021", format!("Tipo '{nome_tipo}' do bloco contextual não foi declarado."));
                    }
                }
                let mut nomes_argumentos = HashSet::new();
                for argumento in argumentos {
                    if let Expressao::ArgumentoNomeado { nome, valor } = argumento {
                        if !nomes_argumentos.insert(nome) { self.erro("N3073", format!("Argumento contextual '{nome}' foi informado mais de uma vez.")); }
                        if contexto == "montagem" && !matches!(nome.as_str(), "entradas" | "saidas" | "efeitos") {
                            self.erro("N3073", format!("Argumento '{nome}' não é reconhecido pelo contrato de 'montagem'."));
                        }
                        self.expressao(valor);
                    } else {
                        if contexto == "montagem" { self.erro("N3073", "Argumentos de 'montagem' precisam ser nomeados: entradas, saidas ou efeitos."); }
                        self.expressao(argumento);
                    }
                }
                self.abrir_escopo();
                for instrucao in corpo { self.item(instrucao); }
                self.fechar_escopo();
            }
            NoAst::Classe { nome, herda: _, corpo } => {
                if self.escopos.len() > 1 { self.erro("N3080", "Classes só podem ser declaradas no escopo do arquivo."); }
                let anterior = self.classe_atual.replace(nome.clone());
                self.abrir_escopo();
                self.declarar("isto", Simbolo { tipo: Tipo::Estrutura(nome.clone()), mutavel: true, movido: false, campos_movidos: HashSet::new() });
                let mut membros = HashSet::new();
                for membro in corpo {
                    let nome_membro = match membro {
                        NoAst::Variavel { nome, .. } | NoAst::Funcao { nome, .. } => Some(nome.as_str()),
                        NoAst::Atributada { declaracao, .. } => match declaracao.as_ref() {
                            NoAst::Variavel { nome, .. } | NoAst::Funcao { nome, .. } => Some(nome.as_str()),
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(nome_membro) = nome_membro {
                        if !membros.insert(nome_membro.to_owned()) { self.erro("N3079", format!("Membro '{nome_membro}' duplicado na classe '{nome}'.")); }
                    }
                    self.item(membro);
                }
                self.fechar_escopo();
                self.classe_atual = anterior;
            }
            NoAst::Passar | NoAst::Ceder(_) => {}
            NoAst::Estrutura { nome, campos } => {
                if self.escopos.len() > 1 {
                    self.erro("N3062", "Estruturas só podem ser declaradas no escopo do arquivo.");
                }
                let mut vistos = HashSet::new();
                for (campo, tipo) in campos {
                    if !vistos.insert(campo) {
                        self.erro("N3033", format!("Campo '{campo}' duplicado na estrutura '{nome}'."));
                    }
                    if Self::tipo_contem_desconhecido(&self.resolver_tipo(tipo)) {
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
                        if Self::tipo_contem_desconhecido(&self.resolver_tipo(tipo)) {
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
                // Pattern matching consome valores algébricos não copiáveis.
                // Os vínculos de payload passam a representar propriedade independente
                // dentro de cada braço; o enum original não pode ser reutilizado.
                if !self.tipo_copiavel(&tipo_valor) {
                    self.mover_valor(valor);
                }
                let (nome_enum, variantes) = match tipo_valor {
                    Tipo::Enumeracao(nome) => {
                        let Some(variantes) = self.variantes_enumeracoes.get(&nome).cloned() else {
                            self.erro("N3047", format!("Enumeração '{nome}' não foi encontrada."));
                            return;
                        };
                        (nome, variantes)
                    }
                    Tipo::Opcional(tipo) => ("Opcional".to_owned(), HashMap::from([
                        ("Algum".to_owned(), vec![*tipo]),
                        ("Nenhum".to_owned(), Vec::new()),
                    ])),
                    Tipo::Resultado(sucesso, falha) => ("Resultado".to_owned(), HashMap::from([
                        ("Sucesso".to_owned(), vec![*sucesso]),
                        ("Falha".to_owned(), vec![*falha]),
                    ])),
                    _ => {
                        self.erro("N3046", "A expressão de 'escolha' precisa ter tipo de enumeração, Opcional ou Resultado.");
                        for caso in casos { self.bloco(&caso.corpo); }
                        return;
                    }
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
                                    self.declarar(vinculo, Simbolo { tipo: tipo.clone(), mutavel: false, movido: false, campos_movidos: HashSet::new() });
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
            NoAst::Cada { nome, iteravel, corpo } => {
                let tipo_iteravel = self.expressao(iteravel);
                let tipo_elemento = match tipo_iteravel {
                    Tipo::Lista(tipo) | Tipo::Intervalo(tipo) => *tipo,
                    Tipo::Texto => Tipo::Caractere,
                    Tipo::Desconhecido => Tipo::Desconhecido,
                    outro => {
                        self.erro("N3058", format!("'cada' exige lista, intervalo ou texto; recebeu '{}'.", outro.nome()));
                        Tipo::Desconhecido
                    }
                };
                let antes_corpo = self.snapshot_movidos();
                self.abrir_escopo();
                self.declarar(nome, Simbolo {
                    tipo: tipo_elemento,
                    mutavel: false,
                    movido: false,
                    campos_movidos: HashSet::new(),
                });
                self.profundidade_laco += 1;
                self.bloco(corpo);
                self.profundidade_laco -= 1;
                self.fechar_escopo();
                let depois_corpo = self.snapshot_movidos();
                self.mesclar_movidos(&antes_corpo, &depois_corpo);
            }
            NoAst::Pulso { intervalo, corpo } => {
                let tipo = self.expressao(intervalo);
                if !matches!(
                    tipo,
                    Tipo::Inteiro | Tipo::Inteiro8 | Tipo::Inteiro16 | Tipo::Inteiro32
                        | Tipo::Inteiro64 | Tipo::Inteiro128 | Tipo::InteiroU8
                        | Tipo::InteiroU16 | Tipo::InteiroU32 | Tipo::InteiroU64
                        | Tipo::InteiroU128 | Tipo::Byte
                ) {
                    self.erro("N3059", "'pulso' exige intervalo inteiro em milissegundos.");
                }
                let antes_corpo = self.snapshot_movidos();
                self.profundidade_laco += 1;
                self.bloco(corpo);
                self.profundidade_laco -= 1;
                let depois_corpo = self.snapshot_movidos();
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
                let obtido = if let Some(expressao) = valor {
                    if let Some(tipo) = esperado.as_ref() {
                        self.expressao_com_tipo_esperado(expressao, tipo)
                    } else {
                        self.expressao(expressao)
                    }
                } else {
                    Tipo::Vazio
                };
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
                NoAst::Escolha { casos, .. }
                    if !casos.is_empty()
                        && casos.iter().all(|caso| Self::garante_retorno(&caso.corpo)) =>
                {
                    return true;
                }
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

    fn expressao_com_tipo_esperado(&mut self, expressao: &Expressao, esperado: &Tipo) -> Tipo {
        match expressao {
            Expressao::Lambda { parametros, corpo } => return self.analisar_lambda_expressao(parametros, corpo, Some(esperado)),
            Expressao::LambdaBloco { parametros, corpo } => return self.analisar_lambda_bloco(parametros, corpo, Some(esperado)),
            _ => {}
        }
        let Expressao::Chamada { alvo, argumentos, .. } = expressao else {
            return self.expressao(expressao);
        };
        if let Expressao::Membro { alvo: base, operador: TipoToken::Escopo, nome: variante } = alvo.as_ref() {
            if let Expressao::Nome(tipo_nome) = base.as_ref() {
                match (tipo_nome.as_str(), esperado) {
                    ("Opcional" | "opcional", Tipo::Opcional(interno)) if variante == "Algum" && argumentos.len() == 1 => {
                        let obtido = self.expressao(&argumentos[0]);
                        self.exigir_expressao_tipo(&argumentos[0], &obtido, interno, "payload de Opcional");
                        return esperado.clone();
                    }
                    ("Opcional" | "opcional", Tipo::Opcional(_)) if variante == "Nenhum" && argumentos.is_empty() => {
                        return esperado.clone();
                    }
                    ("Resultado" | "resultado", Tipo::Resultado(sucesso, _)) if variante == "Sucesso" && argumentos.len() == 1 => {
                        let obtido = self.expressao(&argumentos[0]);
                        self.exigir_expressao_tipo(&argumentos[0], &obtido, sucesso, "payload de Resultado");
                        return esperado.clone();
                    }
                    ("Resultado" | "resultado", Tipo::Resultado(_, falha)) if variante == "Falha" && argumentos.len() == 1 => {
                        let obtido = self.expressao(&argumentos[0]);
                        self.exigir_expressao_tipo(&argumentos[0], &obtido, falha, "payload de Resultado");
                        return esperado.clone();
                    }
                    _ => {}
                }
            }
        }
        let Expressao::Nome(nome) = alvo.as_ref() else {
            return self.expressao(expressao);
        };
        let chave_metodo = self.classe_atual.as_ref().map(|classe| format!("{classe}::{nome}"));
        let Some(funcao) = self.funcoes.get(nome).or_else(|| chave_metodo.as_ref().and_then(|chave| self.funcoes.get(chave))).cloned() else {
            return self.expressao(expressao);
        };
        if funcao.genericos.is_empty() {
            return self.expressao(expressao);
        }

        let mapeados = self.mapear_argumentos_funcao(argumentos, &funcao, nome);
        let tipos_obtidos: Vec<Tipo> = mapeados.iter()
            .map(|argumento| argumento.map_or(Tipo::Desconhecido, |valor| self.expressao(valor)))
            .collect();
        let (inferencia_valida, inferidos) =
            Self::inferir_genericos_chamada(&funcao, &tipos_obtidos, Some(esperado));
        if !inferencia_valida {
            self.erro("N3032", format!("Não foi possível inferir tipos genéricos consistentes na chamada de '{nome}'."));
        }
        for parametro in &funcao.genericos {
            if !inferidos.contains_key(parametro) {
                self.erro("N3064", format!("O parâmetro genérico '{parametro}' da função '{nome}' não pôde ser inferido pelos argumentos nem pelo tipo esperado."));
            }
        }
        for (indice, (parametro, obtido)) in funcao.parametros.iter().zip(&tipos_obtidos).enumerate() {
            let parametro = Self::substituir_genericos(parametro, &inferidos);
            if let Some(Some(argumento)) = mapeados.get(indice) {
                self.exigir_expressao_tipo(argumento, obtido, &parametro, "argumento de função genérica");
                if !self.tipo_copiavel(obtido) { self.mover_valor(argumento); }
            }
        }
        Self::substituir_genericos(&funcao.retorno, &inferidos)
    }

    fn analisar_lambda_expressao(&mut self, parametros: &[String], corpo: &Expressao, esperado: Option<&Tipo>) -> Tipo {
        if esperado.is_none() && !parametros.is_empty() {
            self.erro("N3088", "Lambda com parâmetros exige um tipo de função esperado para inferir os tipos dos parâmetros.");
        }
        let (tipos, retorno_esperado) = match esperado {
            Some(Tipo::Funcao(tipos, retorno)) => {
                if tipos.len() != parametros.len() { self.erro("N3068", format!("Lambda declara {} parâmetro(s), mas o tipo esperado exige {}.", parametros.len(), tipos.len())); }
                (tipos.clone(), Some((**retorno).clone()))
            }
            Some(outro) => { self.erro("N3068", format!("Lambda exige um tipo de função esperado; recebeu '{}'.", outro.nome())); (vec![Tipo::Desconhecido; parametros.len()], None) }
            None => (vec![Tipo::Desconhecido; parametros.len()], None),
        };
        self.abrir_escopo();
        for (indice, nome) in parametros.iter().enumerate() {
            let tipo = tipos.get(indice).cloned().unwrap_or(Tipo::Desconhecido);
            self.declarar(nome, Simbolo { tipo, mutavel: false, movido: false, campos_movidos: HashSet::new() });
        }
        let obtido = self.expressao(corpo);
        if let Some(retorno) = retorno_esperado.as_ref() { self.exigir_tipo(&obtido, retorno, "retorno de lambda"); }
        self.fechar_escopo();
        Tipo::Funcao(tipos, Box::new(retorno_esperado.unwrap_or(obtido)))
    }

    fn analisar_lambda_bloco(&mut self, parametros: &[String], corpo: &[NoAst], esperado: Option<&Tipo>) -> Tipo {
        if esperado.is_none() && !parametros.is_empty() {
            self.erro("N3088", "Lambda com parâmetros exige um tipo de função esperado para inferir os tipos dos parâmetros.");
        }
        let (tipos, retorno) = match esperado {
            Some(Tipo::Funcao(tipos, retorno)) => {
                if tipos.len() != parametros.len() { self.erro("N3068", format!("Lambda declara {} parâmetro(s), mas o tipo esperado exige {}.", parametros.len(), tipos.len())); }
                (tipos.clone(), (**retorno).clone())
            }
            Some(outro) => { self.erro("N3068", format!("Lambda exige um tipo de função esperado; recebeu '{}'.", outro.nome())); (vec![Tipo::Desconhecido; parametros.len()], Tipo::Desconhecido) }
            None => (vec![Tipo::Desconhecido; parametros.len()], Tipo::Desconhecido),
        };
        self.abrir_escopo();
        for (indice, nome) in parametros.iter().enumerate() {
            let tipo = tipos.get(indice).cloned().unwrap_or(Tipo::Desconhecido);
            self.declarar(nome, Simbolo { tipo, mutavel: false, movido: false, campos_movidos: HashSet::new() });
        }
        let retorno_anterior = self.retorno_atual.replace(retorno.clone());
        let laco_anterior = self.profundidade_laco;
        self.profundidade_laco = 0;
        for instrucao in corpo { self.item(instrucao); }
        if retorno != Tipo::Desconhecido && retorno != Tipo::Vazio && !Self::garante_retorno(corpo) {
            self.erro("N3024", "O bloco lambda pode terminar sem devolver um valor.");
        }
        self.profundidade_laco = laco_anterior;
        self.retorno_atual = retorno_anterior;
        self.fechar_escopo();
        Tipo::Funcao(tipos, Box::new(retorno))
    }

    fn expressao(&mut self, expressao: &Expressao) -> Tipo {
        if let Some((_, intervalo)) = self.intervalos_expressoes.iter().find(|(origem, _)| origem == expressao) { self.intervalo_atual = Some(*intervalo); }
        match expressao {
            Expressao::Nome(nome) => {
                if let Some(simbolo) = self.buscar(nome).cloned() {
                    if simbolo.movido || !simbolo.campos_movidos.is_empty() {
                        self.erro("N3030", format!("O valor '{nome}' ou parte de seus campos foi movido e não pode ser utilizado integralmente."));
                    }
                    simbolo.tipo
                } else if let Some(f) = self.funcoes.get(nome).or_else(|| self.classe_atual.as_ref().and_then(|classe| self.funcoes.get(&format!("{classe}::{nome}")))) {
                    if !f.genericos.is_empty() {
                        self.erro(
                            "N3057",
                            format!("A função genérica '{nome}' precisa de uma instância concreta antes de ser usada como valor."),
                        );
                        Tipo::Desconhecido
                    } else {
                        Tipo::Funcao(f.parametros.clone(), Box::new(f.retorno.clone()))
                    }
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
            Expressao::ArgumentoNomeado { valor, .. } => self.expressao(valor),
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
            Expressao::Lambda { parametros, corpo } => self.analisar_lambda_expressao(parametros, corpo, None),
            Expressao::LambdaBloco { parametros, corpo } => self.analisar_lambda_bloco(parametros, corpo, None),
            Expressao::Condicional { condicao, entao, senao } => {
                let tipo_condicao = self.expressao(condicao);
                self.exigir_tipo(&tipo_condicao, &Tipo::Logico, "condição ternária");
                let tipo_entao = self.expressao(entao);
                let tipo_senao = self.expressao(senao);
                if Self::tipos_compativeis(&tipo_entao, &tipo_senao) {
                    if tipo_entao == Tipo::Desconhecido { tipo_senao } else { tipo_entao }
                } else {
                    self.erro("N3067", format!("Os ramos da expressão condicional possuem tipos incompatíveis: '{}' e '{}'.", tipo_entao.nome(), tipo_senao.nome()));
                    Tipo::Desconhecido
                }
            },
            Expressao::Binaria { esquerda, operador, direita } => {
                let a = self.expressao(esquerda);
                let b = self.expressao(direita);
                self.binaria(&a, *operador, &b, direita)
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
                            if let Some(simbolo) = self.buscar_mutavel(nome) { simbolo.movido = false; simbolo.campos_movidos.clear(); }
                            self.mover_valor(valor);
                            simbolo.tipo
                        } else {
                            self.erro("N3007", format!("Nome '{nome}' não foi declarado."));
                            Tipo::Desconhecido
                        }
                    }
                    Expressao::Membro { .. } | Expressao::Indexacao { .. } => {
                        let tipo_destino = if let Some((raiz, caminho)) = Self::decompor_caminho_campo(destino) {
                            let tipo = self.resolver_tipo_caminho(&raiz, &caminho, false);
                            if let Some(simbolo) = self.buscar(&raiz).cloned() {
                                if !simbolo.mutavel {
                                    self.erro("N3009", format!("Não é permitido alterar campos da constante '{raiz}'."));
                                }
                            }
                            if let Some(simbolo) = self.buscar_mutavel(&raiz) {
                                let prefixo = caminho.join(".");
                                simbolo.campos_movidos.retain(|movido|
                                    movido != &prefixo && !movido.starts_with(&format!("{prefixo}."))
                                );
                            }
                            tipo
                        } else {
                            self.expressao(destino)
                        };
                        self.exigir_expressao_tipo(valor, &tipo_valor, &tipo_destino, "atribuição de campo");
                        self.mover_valor(valor);
                        tipo_destino
                    }
                    _ => {
                        self.erro("N3010", "O destino da atribuição precisa ser uma variável ou campo modificável.");
                        Tipo::Desconhecido
                    }
                }
            }
            Expressao::Chamada { alvo, argumentos, .. } => {
                if let Expressao::Membro { alvo: objeto, operador: TipoToken::Ponto, nome: metodo } = alvo.as_ref() {
                    let tipo_objeto = self.expressao(objeto);
                    if let Tipo::Estrutura(classe) = tipo_objeto {
                        if let Some(funcao) = self.buscar_metodo_classe(&classe, metodo) {
                            return self.analisar_chamada_funcao(argumentos, &funcao, metodo);
                        }
                    }
                }
                if matches!(alvo.as_ref(), Expressao::Nome(nome) if nome == "copiar") {
                    if argumentos.len() != 1 {
                        self.erro("N3058", "'copiar' exige exatamente um argumento.");
                        for argumento in argumentos { self.expressao(argumento); }
                        return Tipo::Desconhecido;
                    }
                    let tipo = self.expressao(&argumentos[0]);
                    if !self.tipo_copiavel(&tipo) {
                        self.erro("N3058", format!("O tipo '{}' não implementa cópia.", tipo.nome()));
                    }
                    return tipo;
                }
                if let Expressao::Membro { alvo: tipo_expr, operador: TipoToken::Escopo, nome: variante } = alvo.as_ref() {
                    if let Expressao::Nome(tipo_base) = tipo_expr.as_ref() {
                        if !self.variantes_enumeracoes.contains_key(tipo_base) {
                            match (tipo_base.as_str(), variante.as_str(), argumentos.len()) {
                            ("Opcional" | "opcional", "Algum", 1) => {
                                let interno = self.expressao(&argumentos[0]);
                                return Tipo::Opcional(Box::new(interno));
                            }
                            ("Opcional" | "opcional", "Nenhum", 0) => {
                                return Tipo::Opcional(Box::new(Tipo::Desconhecido));
                            }
                            ("Resultado" | "resultado", "Sucesso", 1) => {
                                let sucesso = self.expressao(&argumentos[0]);
                                return Tipo::Resultado(Box::new(sucesso), Box::new(Tipo::Desconhecido));
                            }
                            ("Resultado" | "resultado", "Falha", 1) => {
                                let falha = self.expressao(&argumentos[0]);
                                return Tipo::Resultado(Box::new(Tipo::Desconhecido), Box::new(falha));
                            }
                            _ => {}
                        }
                            if matches!(tipo_base.as_str(), "Opcional" | "opcional" | "Resultado" | "resultado") {
                                self.erro("N3054", format!("Construtor inválido: '{tipo_base}::{variante}' com {} argumento(s).", argumentos.len()));
                                for argumento in argumentos { self.expressao(argumento); }
                                return Tipo::Desconhecido;
                            }
                        }
                    }
                }
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

                    if let Some(destino) = Self::tipo_conversao_explicita(nome) {
                        if argumentos.len() != 1 {
                            self.erro("N3015", format!("'{nome}' exige exatamente um argumento."));
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        let obtido = self.expressao(&argumentos[0]);
                        if !Self::numerico(&obtido) && obtido != Tipo::Desconhecido {
                            self.erro("N3016", format!("'{nome}' exige um argumento numérico."));
                        }
                        if !Self::literal_cabe_em_tipo(&argumentos[0], &destino) {
                            self.erro(
                                "N3020",
                                format!("O literal não cabe no tipo de destino '{}'.", destino.nome()),
                            );
                        }
                        return destino;
                    }
                    if nome == "em_paralelo" {
                        if argumentos.is_empty() {
                            self.erro("N3060", "'em_paralelo' exige pelo menos uma função sem parâmetros.");
                            return Tipo::Desconhecido;
                        }
                        let mut retorno_comum: Option<Tipo> = None;
                        for argumento in argumentos {
                            let tipo = self.expressao(argumento);
                            match tipo {
                                Tipo::Funcao(parametros, retorno) if parametros.is_empty() => {
                                    if let Some(anterior) = retorno_comum.as_ref() {
                                        if *retorno != *anterior {
                                            self.erro("N3061", "'em_paralelo' exige funções com o mesmo tipo de retorno.");
                                        }
                                    } else {
                                        retorno_comum = Some(*retorno);
                                    }
                                }
                                _ => self.erro("N3060", "'em_paralelo' aceita apenas referências a funções sem parâmetros."),
                            }
                        }
                        return Tipo::Lista(Box::new(retorno_comum.unwrap_or(Tipo::Desconhecido)));
                    }
                    if nome == "gerar" {
                        if argumentos.len() != 2 {
                            self.erro("N3015", "'gerar' exige um valor e uma quantidade inteira.");
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        let tipo_valor = self.expressao(&argumentos[0]);
                        let tipo_quantidade = self.expressao(&argumentos[1]);
                        if Self::tipo_contem_desconhecido(&tipo_valor) {
                            self.erro("N3016", "Não foi possível inferir o tipo do valor passado a 'gerar'.");
                        }
                        if !matches!(tipo_quantidade,
                            Tipo::Inteiro | Tipo::Inteiro8 | Tipo::Inteiro16 | Tipo::Inteiro32
                            | Tipo::Inteiro64 | Tipo::Inteiro128 | Tipo::InteiroU8 | Tipo::InteiroU16
                            | Tipo::InteiroU32 | Tipo::InteiroU64 | Tipo::InteiroU128 | Tipo::Byte
                        ) {
                            self.erro("N3016", "'gerar' exige quantidade inteira e não negativa.");
                        }
                        return Tipo::Lista(Box::new(tipo_valor));
                    }
                    if nome == "exibir" {
                        for argumento in argumentos { self.expressao(argumento); }
                        return Tipo::Vazio;
                    }
                    if !self.funcoes.contains_key(nome)
                        && matches!(nome.as_str(), "ler_texto" | "receber" | "perguntar")
                    {
                        if argumentos.len() > 1 {
                            self.erro("N3015", format!("'{nome}' aceita no máximo um texto de prompt."));
                        }
                        for argumento in argumentos {
                            let tipo = self.expressao(argumento);
                            self.exigir_tipo(&tipo, &Tipo::Texto, "prompt de entrada");
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
                    if nome == "json_valido" || nome == "json_canonico" {
                        if argumentos.len() != 1 {
                            self.erro("N3015", format!("'{nome}' exige exatamente um texto JSON."));
                            for argumento in argumentos { self.expressao(argumento); }
                            return Tipo::Desconhecido;
                        }
                        let entrada = self.expressao(&argumentos[0]);
                        self.exigir_tipo(&entrada, &Tipo::Texto, "entrada JSON");
                        return if nome == "json_valido" {
                            Tipo::Logico
                        } else {
                            Tipo::Texto
                        };
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
                    if nome == "dormir" || nome == "aguardar" {
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
                    if let Some(simbolo) = self.buscar(nome).cloned() {
                        if let Tipo::Funcao(parametros, retorno) = simbolo.tipo {
                            self.validar_argumentos(argumentos, &parametros, nome);
                            return *retorno;
                        }
                        self.erro("N3058", format!("O valor '{nome}' não é chamável."));
                        for argumento in argumentos { self.expressao(argumento); }
                        return Tipo::Desconhecido;
                    }
                    let chave_metodo = self.classe_atual.as_ref().map(|classe| format!("{classe}::{nome}"));
                    if let Some(funcao) = self.funcoes.get(nome).or_else(|| chave_metodo.as_ref().and_then(|chave| self.funcoes.get(chave))).cloned() {
                        return self.analisar_chamada_funcao(argumentos, &funcao, nome);
                    }
                }
                if let Expressao::Membro { alvo: modulo_expr, operador: TipoToken::Escopo, nome } = alvo.as_ref() {
                    if let Expressao::Nome(alias) = modulo_expr.as_ref() {
                        if let Some(caminho) = self.modulos_importados.get(alias) {
                            let chave = if self.modulos.contains_key(caminho) { caminho.clone() } else { caminho.replace("::", ".") };
                            if let Some(assinatura) = self.modulos.get(&chave).and_then(|m| m.get(nome)).cloned() {
                                let funcao = Funcao {
                                    genericos: assinatura.genericos,
                                    parametros: assinatura.parametros,
                                    nomes_parametros: assinatura.nomes_parametros,
                                    parametros_padrao: assinatura.parametros_padrao.into_iter()
                                        .map(|tem_padrao| tem_padrao.then_some(Expressao::Nome(String::new())))
                                        .collect(),
                                    retorno: assinatura.retorno,
                                };
                                return self.analisar_chamada_funcao(argumentos, &funcao, nome);
                            }
                        }
                    }
                    self.erro("N3011", "Função qualificada não foi encontrada em um módulo importado.");
                    return Tipo::Desconhecido;
                }
                let tipo_alvo = self.expressao(alvo);
                if let Tipo::Funcao(parametros, retorno) = tipo_alvo {
                    self.validar_argumentos(argumentos, &parametros, "função indireta");
                    return *retorno;
                }
                self.erro("N3012", "O alvo da chamada não é uma função conhecida nem um valor de função.");
                for argumento in argumentos { self.expressao(argumento); }
                Tipo::Desconhecido
            }
            Expressao::Indexacao { alvo, indice } => {
                if let Some((raiz, caminho)) = Self::decompor_caminho_campo(expressao) {
                    return self.resolver_tipo_caminho(&raiz, &caminho, true);
                }
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
                                self.erro("N3063", "O índice de uma tupla precisa ser um literal inteiro válido e dentro dos limites.");
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
                    if let Some((raiz, caminho)) = Self::decompor_caminho_campo(alvo) {
                        if let Some(Tipo::Estrutura(nome_estrutura)) = self.resolver_tipo_caminho_puro(&raiz, &caminho) {
                            if self.campos_estruturas.get(&nome_estrutura).is_some_and(|campos| campos.contains_key(nome)) {
                                let mut caminho_completo = caminho.clone();
                                caminho_completo.push(nome.clone());
                                return self.resolver_tipo_caminho(&raiz, &caminho_completo, true);
                            }
                            if let Some(funcao) = self.buscar_metodo_classe(&nome_estrutura, nome) {
                                if !funcao.genericos.is_empty() {
                                    self.erro("N3057", format!("O método genérico '{nome}' precisa ser chamado com uma instância de tipo."));
                                    return Tipo::Desconhecido;
                                }
                                return Tipo::Funcao(funcao.parametros, Box::new(funcao.retorno));
                            }
                        }
                        let mut caminho_completo = caminho;
                        caminho_completo.push(nome.clone());
                        return self.resolver_tipo_caminho(&raiz, &caminho_completo, true);
                    }
                    let tipo_alvo = self.expressao(alvo);
                    match tipo_alvo {
                        Tipo::Estrutura(nome_estrutura) => {
                            if let Some(tipo_campo) = self.campos_estruturas.get(&nome_estrutura).and_then(|campos| campos.get(nome)).cloned() {
                                return tipo_campo;
                            }
                            if let Some(funcao) = self.buscar_metodo_classe(&nome_estrutura, nome) {
                                if !funcao.genericos.is_empty() {
                                    self.erro("N3057", format!("O método genérico '{nome}' precisa ser chamado com uma instância de tipo."));
                                    return Tipo::Desconhecido;
                                }
                                return Tipo::Funcao(funcao.parametros, Box::new(funcao.retorno));
                            }
                            self.erro("N3040", format!("A estrutura '{nome_estrutura}' não possui o campo ou método '{nome}'."));
                        }
                        Tipo::Desconhecido => {}
                        outro => self.erro("N3041", format!("O tipo {} não possui campos acessíveis por '.'.", outro.nome())),
                    }
                }
                Tipo::Desconhecido
            }
        }
    }

    fn buscar_metodo_classe(&self, classe: &str, nome: &str) -> Option<Funcao> {
        let mut atual = Some(classe);
        let mut visitadas = HashSet::new();
        while let Some(nome_classe) = atual {
            if !visitadas.insert(nome_classe.to_owned()) { return None; }
            if let Some(funcao) = self.funcoes.get(&format!("{nome_classe}::{nome}")) {
                return Some(funcao.clone());
            }
            atual = self.classes.get(nome_classe).and_then(Option::as_deref);
        }
        None
    }

    fn analisar_chamada_funcao(&mut self, argumentos: &[Expressao], funcao: &Funcao, nome: &str) -> Tipo {
        let mapeados = self.mapear_argumentos_funcao(argumentos, funcao, nome);
        let tipos_obtidos: Vec<Tipo> = mapeados.iter()
            .map(|argumento| argumento.map_or(Tipo::Desconhecido, |valor| self.expressao(valor)))
            .collect();
        let inferidos = if funcao.genericos.is_empty() {
            HashMap::new()
        } else {
            let (valida, inferidos) = Self::inferir_genericos_chamada(funcao, &tipos_obtidos, None);
            if !valida {
                self.erro("N3032", format!("Não foi possível inferir tipos genéricos consistentes na chamada de '{nome}'."));
            }
            for parametro in &funcao.genericos {
                if !inferidos.contains_key(parametro) {
                    self.erro("N3064", format!("O parâmetro genérico '{parametro}' da função '{nome}' não pôde ser inferido pelos argumentos."));
                }
            }
            inferidos
        };
        for (indice, argumento) in mapeados.iter().enumerate() {
            if let Some(argumento) = argumento {
                if let Some(obtido) = tipos_obtidos.get(indice) {
                    let esperado = funcao.parametros.get(indice)
                        .map(|tipo| Self::substituir_genericos(tipo, &inferidos))
                        .unwrap_or(Tipo::Desconhecido);
                    self.exigir_expressao_tipo(argumento, obtido, &esperado, "argumento de função");
                    if !self.tipo_copiavel(obtido) { self.mover_valor(argumento); }
                }
            }
        }
        Self::substituir_genericos(&funcao.retorno, &inferidos)
    }

    fn mapear_argumentos_funcao<'b>(&mut self, argumentos: &'b [Expressao], funcao: &Funcao, nome: &str) -> Vec<Option<&'b Expressao>> {
        let mut mapeados = vec![None; funcao.parametros.len()];
        let mut proximo_posicional = 0usize;
        let mut encontrou_nomeado = false;
        for argumento in argumentos {
            let (indice, valor) = match argumento {
                Expressao::ArgumentoNomeado { nome: nome_argumento, valor } => {
                    encontrou_nomeado = true;
                    let Some(indice) = funcao.nomes_parametros.iter().position(|parametro| parametro == nome_argumento) else {
                        self.erro("N3083", format!("A função '{nome}' não possui parâmetro chamado '{nome_argumento}'."));
                        self.expressao(valor);
                        continue;
                    };
                    (indice, valor.as_ref())
                }
                outro => {
                    if encontrou_nomeado { self.erro("N3082", format!("Argumento posicional não pode aparecer depois de argumento nomeado na chamada de '{nome}'.")); }
                    while proximo_posicional < mapeados.len() && mapeados[proximo_posicional].is_some() { proximo_posicional += 1; }
                    if proximo_posicional >= mapeados.len() {
                        self.erro("N3015", format!("A função '{nome}' recebeu argumentos demais."));
                        self.expressao(outro);
                        continue;
                    }
                    let indice = proximo_posicional;
                    proximo_posicional += 1;
                    (indice, outro)
                }
            };
            if mapeados[indice].is_some() {
                self.erro("N3084", format!("O parâmetro '{}' foi informado mais de uma vez na chamada de '{nome}'.", funcao.nomes_parametros.get(indice).map_or("?", String::as_str)));
                self.expressao(valor);
            } else {
                mapeados[indice] = Some(valor);
            }
        }
        for (indice, argumento) in mapeados.iter().enumerate() {
            if argumento.is_none() && funcao.parametros_padrao.get(indice).is_none_or(Option::is_none) {
                self.erro("N3015", format!("O parâmetro obrigatório '{}' não foi informado na chamada de '{nome}'.", funcao.nomes_parametros.get(indice).map_or("?", String::as_str)));
            }
        }
        mapeados
    }

    fn validar_argumentos(&mut self, argumentos: &[Expressao], esperados: &[Tipo], nome: &str) {
        if argumentos.len() != esperados.len() {
            self.erro("N3015", format!("A função '{nome}' espera {} argumento(s), mas recebeu {}.", esperados.len(), argumentos.len()));
        }
        for (indice, argumento) in argumentos.iter().enumerate() {
            let valor = if let Expressao::ArgumentoNomeado { valor, .. } = argumento {
                self.erro("N3085", format!("A chamada indireta ou importada de '{nome}' não possui nomes de parâmetros para validar argumentos nomeados."));
                valor.as_ref()
            } else { argumento };
            let obtido = self.expressao(valor);
            if let Some(esperado) = esperados.get(indice) {
                self.exigir_expressao_tipo(valor, &obtido, esperado, "argumento de função");
                if !self.tipo_copiavel(&obtido) { self.mover_valor(valor); }
            }
        }
    }

    fn binaria(&mut self, a: &Tipo, op: TipoToken, b: &Tipo, expressao_direita: &Expressao) -> Tipo {
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
                self.exigir_expressao_tipo(expressao_direita, b, a, "comparação de igualdade");
                if *a != Tipo::Desconhecido
                    && !self.tipo_implementa_igualdade(a, 0, &mut HashSet::new())
                {
                    self.erro(
                        "N3056",
                        format!("O tipo '{}' não implementa igualdade.", a.nome()),
                    );
                }
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
        let restante = &conteudo[fechamento + 1..];
        let retorno = restante.strip_prefix(':')?;
        if retorno.trim().is_empty() {
            return None;
        }
        let parametros_brutos = &conteudo[..fechamento];
        if parametros_brutos.trim().is_empty() {
            return Some((Vec::new(), retorno.trim()));
        }
        let mut parametros = Vec::new();
        let mut inicio = 0usize;
        let mut parenteses = 0usize;
        let mut angulos = 0usize;
        let mut colchetes = 0usize;
        for (indice, caractere) in parametros_brutos.char_indices() {
            match caractere {
                '(' => parenteses += 1,
                ')' => parenteses = parenteses.checked_sub(1)?,
                '<' => angulos += 1,
                '>' => angulos = angulos.checked_sub(1)?,
                '[' => colchetes += 1,
                ']' => colchetes = colchetes.checked_sub(1)?,
                ',' if parenteses == 0 && angulos == 0 && colchetes == 0 => {
                    let parametro = parametros_brutos[inicio..indice].trim();
                    if parametro.is_empty() {
                        return None;
                    }
                    parametros.push(parametro);
                    inicio = indice + 1;
                }
                _ => {}
            }
        }
        let ultimo = parametros_brutos[inicio..].trim();
        if ultimo.is_empty() {
            return None;
        }
        parametros.push(ultimo);
        Some((parametros, retorno.trim()))
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
        self.resolver_tipo_interno(nome, &mut HashSet::new())
    }

    fn resolver_tipo_interno(
        &self,
        nome: &str,
        aliases_visitados: &mut HashSet<String>,
    ) -> Tipo {
        if self.parametros_genericos.contains(nome) {
            return Tipo::Parametro(nome.to_owned());
        }
        match nome {
            "vazio" => Tipo::Vazio,
            "logico" | "lógico" => Tipo::Logico,
            "inteiro" => Tipo::Inteiro,
            "inteiro8" | "i8" => Tipo::Inteiro8,
            "inteiro16" | "i16" => Tipo::Inteiro16,
            "inteiro32" | "i32" => Tipo::Inteiro32,
            "inteiro64" | "i64" => Tipo::Inteiro64,
            "inteiro128" | "i128" => Tipo::Inteiro128,
            "inteiro_u8" | "u8" => Tipo::InteiroU8,
            "inteiro_u16" | "u16" => Tipo::InteiroU16,
            "inteiro_u32" | "u32" => Tipo::InteiroU32,
            "inteiro_u64" | "u64" => Tipo::InteiroU64,
            "inteiro_u128" | "u128" => Tipo::InteiroU128,
            "decimal" => Tipo::Decimal64,
            "decimal32" | "f32" => Tipo::Decimal32,
            "decimal64" | "f64" => Tipo::Decimal64,
            "texto" => Tipo::Texto,
            "caractere" => Tipo::Caractere,
            "byte" => Tipo::Byte,
            _ if nome.starts_with('(') && nome.ends_with(')') => {
                match TipoAst::analisar(nome) {
                    TipoAst::Tupla(elementos) => {
                        let tipos: Vec<_> = elementos
                            .iter()
                            .map(|tipo| {
                                self.resolver_tipo_interno(&tipo.para_texto(), aliases_visitados)
                            })
                            .collect();
                        if tipos.iter().any(Self::tipo_contem_desconhecido) {
                            Tipo::Desconhecido
                        } else {
                            Tipo::Tupla(tipos)
                        }
                    }
                    _ => Tipo::Desconhecido,
                }
            }
            _ if Self::dividir_tipo_funcao(nome).is_some() => {
                let (parametros, retorno) =
                    Self::dividir_tipo_funcao(nome).expect("tipo de função reconhecido");
                let parametros: Vec<_> = parametros
                    .iter()
                    .map(|tipo| self.resolver_tipo_interno(tipo, aliases_visitados))
                    .collect();
                let retorno = self.resolver_tipo_interno(retorno, aliases_visitados);
                if parametros.iter().any(|tipo| {
                    *tipo == Tipo::Vazio || Self::tipo_contem_desconhecido(tipo)
                }) || Self::tipo_contem_desconhecido(&retorno)
                {
                    Tipo::Desconhecido
                } else {
                    Tipo::Funcao(parametros, Box::new(retorno))
                }
            }
            _ if nome.starts_with('[') && nome.ends_with(']') => Tipo::Lista(Box::new(
                self.resolver_tipo_interno(&nome[1..nome.len() - 1], aliases_visitados),
            )),
            _ if Self::argumentos_genericos(nome).is_some() => {
                let (base, argumentos) =
                    Self::argumentos_genericos(nome).expect("tipo genérico reconhecido");
                let tipos: Vec<Tipo> = argumentos
                    .iter()
                    .map(|argumento| {
                        self.resolver_tipo_interno(argumento, aliases_visitados)
                    })
                    .collect();
                if tipos.iter().any(Self::tipo_contem_desconhecido) {
                    Tipo::Desconhecido
                } else {
                    match (base, tipos.as_slice()) {
                        ("Intervalo" | "intervalo", [interno]) => {
                            Tipo::Intervalo(Box::new(interno.clone()))
                        }
                        ("Opcional" | "opcional", [interno]) => {
                            Tipo::Opcional(Box::new(interno.clone()))
                        }
                        ("Resultado" | "resultado", [sucesso, falha]) => Tipo::Resultado(
                            Box::new(sucesso.clone()),
                            Box::new(falha.clone()),
                        ),
                        ("Tupla" | "tupla", elementos) if !elementos.is_empty() => {
                            Tipo::Tupla(elementos.to_vec())
                        }
                        ("Vetor" | "vetor", [elemento]) => {
                            Tipo::Lista(Box::new(elemento.clone()))
                        }
                        ("Matriz" | "matriz", [elemento]) => Tipo::Lista(Box::new(
                            Tipo::Lista(Box::new(elemento.clone())),
                        )),
                        _ => Tipo::Desconhecido,
                    }
                }
            }
            _ if self.estruturas.contains(nome) => Tipo::Estrutura(nome.to_owned()),
            _ if self.enumeracoes.contains(nome) => Tipo::Enumeracao(nome.to_owned()),
            _ => {
                let Some(destino) = self.aliases.get(nome) else {
                    return Tipo::Desconhecido;
                };
                if !aliases_visitados.insert(nome.to_owned()) {
                    return Tipo::Desconhecido;
                }
                let resolvido = self.resolver_tipo_interno(destino, aliases_visitados);
                aliases_visitados.remove(nome);
                resolvido
            }
        }
    }

    fn tipo_contem_desconhecido(tipo: &Tipo) -> bool {
        match tipo {
            Tipo::Desconhecido => true,
            Tipo::Lista(interno) | Tipo::Intervalo(interno) | Tipo::Opcional(interno) =>
                Self::tipo_contem_desconhecido(interno),
            Tipo::Resultado(sucesso, falha) =>
                Self::tipo_contem_desconhecido(sucesso) || Self::tipo_contem_desconhecido(falha),
            Tipo::Tupla(tipos) => tipos.iter().any(Self::tipo_contem_desconhecido),
            Tipo::Funcao(parametros, retorno) =>
                parametros.iter().any(Self::tipo_contem_desconhecido)
                    || Self::tipo_contem_desconhecido(retorno),
            _ => false,
        }
    }

    fn inferir_genericos_chamada(
        funcao: &Funcao,
        tipos_obtidos: &[Tipo],
        esperado: Option<&Tipo>,
    ) -> (bool, HashMap<String, Tipo>) {
        if tipos_obtidos.len() != funcao.parametros.len() {
            return (false, HashMap::new());
        }
        // O mapa parcial só serve para produzir diagnósticos específicos dos
        // argumentos. A chamada só é válida se todos os vínculos concordarem.
        let mut tentativa = HashMap::new();
        let mut consistente = true;
        for (parametro, obtido) in funcao.parametros.iter().zip(tipos_obtidos) {
            if !Self::inferir_genericos(
                parametro,
                obtido,
                &funcao.genericos,
                &mut tentativa,
            ) {
                consistente = false;
            }
        }
        if let Some(esperado) = esperado {
            if !Self::inferir_genericos(
                &funcao.retorno,
                esperado,
                &funcao.genericos,
                &mut tentativa,
            ) {
                consistente = false;
            }
        }
        if funcao.genericos.iter().any(|nome| !tentativa.contains_key(nome)) {
            consistente = false;
        }
        (consistente, tentativa)
    }

    fn inferir_genericos(
        esperado: &Tipo,
        obtido: &Tipo,
        genericos: &[String],
        inferidos: &mut HashMap<String, Tipo>,
    ) -> bool {
        // A inferência é transacional: uma unificação composta que falha
        // não pode deixar parâmetros parcialmente vinculados.
        let mut tentativa = inferidos.clone();
        if Self::inferir_genericos_interno(esperado, obtido, genericos, &mut tentativa) {
            *inferidos = tentativa;
            true
        } else {
            false
        }
    }

    fn inferir_genericos_interno(
        esperado: &Tipo,
        obtido: &Tipo,
        genericos: &[String],
        inferidos: &mut HashMap<String, Tipo>,
    ) -> bool {
        // Não transforme lacunas de inferência em instâncias concretas.
        // Ex.: Opcional::Nenhum() não fornece informação suficiente para T.
        if Self::tipo_contem_desconhecido(obtido) {
            return false;
        }
        match esperado {
            Tipo::Parametro(nome) if genericos.contains(nome) => {
                if let Some(anterior) = inferidos.get(nome) {
                    Self::tipos_compativeis(obtido, anterior)
                } else {
                    inferidos.insert(nome.clone(), obtido.clone());
                    true
                }
            }
            Tipo::Lista(a) => matches!(obtido, Tipo::Lista(b) if Self::inferir_genericos_interno(a, b, genericos, inferidos)),
            Tipo::Intervalo(a) => matches!(obtido, Tipo::Intervalo(b) if Self::inferir_genericos_interno(a, b, genericos, inferidos)),
            Tipo::Opcional(a) => matches!(obtido, Tipo::Opcional(b) if Self::inferir_genericos_interno(a, b, genericos, inferidos)),
            Tipo::Resultado(a, b) => matches!(obtido, Tipo::Resultado(c, d)
                if Self::inferir_genericos_interno(a, c, genericos, inferidos)
                    && Self::inferir_genericos_interno(b, d, genericos, inferidos)),
            Tipo::Tupla(a) => matches!(obtido, Tipo::Tupla(b) if a.len() == b.len()
                && a.iter().zip(b).all(|(a, b)| Self::inferir_genericos_interno(a, b, genericos, inferidos))),
            Tipo::Funcao(parametros, retorno) => matches!(obtido, Tipo::Funcao(outros, outro_retorno)
                if parametros.len() == outros.len()
                    && parametros.iter().zip(outros).all(|(a, b)| Self::inferir_genericos_interno(a, b, genericos, inferidos))
                    && Self::inferir_genericos_interno(retorno, outro_retorno, genericos, inferidos)),
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

    fn decompor_caminho_campo(expressao: &Expressao) -> Option<(String, Vec<String>)> {
        match expressao {
            Expressao::Nome(nome) => Some((nome.clone(), Vec::new())),
            Expressao::Membro { alvo, operador: TipoToken::Ponto, nome } => {
                let (raiz, mut caminho) = Self::decompor_caminho_campo(alvo)?;
                caminho.push(nome.clone());
                Some((raiz, caminho))
            }
            Expressao::Indexacao { alvo, indice } => {
                let posicao = match Self::literal_numerico(indice)? {
                    LiteralNumerico::Inteiro(posicao) => usize::try_from(posicao).ok()?,
                    LiteralNumerico::InteiroSemSinal(posicao) => usize::try_from(posicao).ok()?,
                    LiteralNumerico::Decimal(_) => return None,
                };
                let (raiz, mut caminho) = Self::decompor_caminho_campo(alvo)?;
                caminho.push(format!("[{posicao}]"));
                Some((raiz, caminho))
            }
            _ => None,
        }
    }

    fn resolver_tipo_caminho_puro(&self, raiz: &str, caminho: &[String]) -> Option<Tipo> {
        let mut tipo = self.buscar(raiz)?.tipo.clone();
        for campo in caminho {
            if let Some(indice) = campo.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
                let Ok(indice) = indice.parse::<usize>() else { return None; };
                tipo = match tipo {
                    Tipo::Tupla(elementos) => elementos.get(indice)?.clone(),
                    Tipo::Lista(elemento) => *elemento,
                    _ => return None,
                };
            } else {
                let Tipo::Estrutura(nome) = tipo else { return None; };
                tipo = self.campos_estruturas.get(&nome)?.get(campo)?.clone();
            }
        }
        Some(tipo)
    }

    fn resolver_tipo_caminho(&mut self, raiz: &str, caminho: &[String], verificar_movidos: bool) -> Tipo {
        let Some(simbolo) = self.buscar(raiz).cloned() else {
            self.erro("N3007", format!("Nome '{raiz}' não foi declarado."));
            return Tipo::Desconhecido;
        };
        if simbolo.movido {
            self.erro("N3030", format!("O valor '{raiz}' foi movido e não pode ser utilizado novamente."));
            return Tipo::Desconhecido;
        }
        let mut tipo = simbolo.tipo;
        let mut prefixo = Vec::new();
        for campo in caminho {
            prefixo.push(campo.clone());
            if verificar_movidos && simbolo.campos_movidos.contains(&prefixo.join(".")) {
                self.erro("N3030", format!("O campo '{}.{}' foi movido e não pode ser utilizado novamente.", raiz, prefixo.join(".")));
                return Tipo::Desconhecido;
            }
            if let Some(indice) = campo.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
                let Ok(indice) = indice.parse::<usize>() else {
                    self.erro("N3063", "O índice de uma tupla precisa ser um literal inteiro válido.");
                    return Tipo::Desconhecido;
                };
                tipo = match tipo {
                    Tipo::Tupla(elementos) => {
                        let Some(tipo_indice) = elementos.get(indice).cloned() else {
                            self.erro("N3063", "O índice da tupla está fora dos limites.");
                            return Tipo::Desconhecido;
                        };
                        tipo_indice
                    }
                    Tipo::Lista(elemento) => *elemento,
                    outro => {
                        self.erro("N3013", format!("O tipo '{}' não pode ser indexado.", outro.nome()));
                        return Tipo::Desconhecido;
                    }
                };
            } else {
                let Tipo::Estrutura(nome) = tipo else {
                    self.erro("N3041", format!("O tipo '{}' não possui campos acessíveis por '.'.", tipo.nome()));
                    return Tipo::Desconhecido;
                };
                let Some(tipo_campo) = self.campos_estruturas.get(&nome).and_then(|campos| campos.get(campo)).cloned() else {
                    self.erro("N3040", format!("A estrutura '{nome}' não possui o campo '{campo}'."));
                    return Tipo::Desconhecido;
                };
                tipo = tipo_campo;
            }
        }
        if verificar_movidos && !prefixo.is_empty() {
            let caminho = prefixo.join(".");
            let descendente_movido = simbolo.campos_movidos.iter().any(|movido| {
                movido.starts_with(&format!("{caminho}."))
            });
            if descendente_movido {
                self.erro(
                    "N3030",
                    format!("O agregado '{raiz}.{caminho}' contém um campo movido e não pode ser utilizado integralmente."),
                );
                return Tipo::Desconhecido;
            }
        }
        tipo
    }

    fn buscar_mutavel(&mut self, nome: &str) -> Option<&mut Simbolo> {
        self.escopos.iter_mut().rev().find_map(|escopo| escopo.get_mut(nome))
    }

    fn tipo_implementa_igualdade(
        &self,
        tipo: &Tipo,
        profundidade: usize,
        ativos: &mut HashSet<String>,
    ) -> bool {
        if profundidade >= 256 {
            return false;
        }
        let proxima = profundidade + 1;
        match tipo {
            Tipo::Logico | Tipo::Inteiro | Tipo::Inteiro8 | Tipo::Inteiro16
            | Tipo::Inteiro32 | Tipo::Inteiro64 | Tipo::Inteiro128
            | Tipo::InteiroU8 | Tipo::InteiroU16 | Tipo::InteiroU32
            | Tipo::InteiroU64 | Tipo::InteiroU128 | Tipo::Decimal
            | Tipo::Decimal32 | Tipo::Decimal64 | Tipo::Texto
            | Tipo::Caractere | Tipo::Byte => true,
            Tipo::Lista(elemento) | Tipo::Intervalo(elemento)
            | Tipo::Opcional(elemento) => {
                self.tipo_implementa_igualdade(elemento, proxima, ativos)
            }
            Tipo::Resultado(sucesso, falha) => {
                self.tipo_implementa_igualdade(sucesso, proxima, ativos)
                    && self.tipo_implementa_igualdade(falha, proxima, ativos)
            }
            Tipo::Tupla(tipos) => tipos.iter().all(|tipo| {
                self.tipo_implementa_igualdade(tipo, proxima, ativos)
            }),
            Tipo::Estrutura(nome) => {
                let chave = format!("estrutura:{nome}");
                if !ativos.insert(chave.clone()) {
                    return false;
                }
                let valido = self.campos_estruturas.get(nome).is_some_and(|campos| {
                    campos.values().all(|tipo| {
                        self.tipo_implementa_igualdade(tipo, proxima, ativos)
                    })
                });
                ativos.remove(&chave);
                valido
            }
            Tipo::Enumeracao(nome) => {
                let chave = format!("enumeracao:{nome}");
                if !ativos.insert(chave.clone()) {
                    return false;
                }
                let valido = self.variantes_enumeracoes.get(nome).is_some_and(|variantes| {
                    variantes.values().flatten().all(|tipo| {
                        self.tipo_implementa_igualdade(tipo, proxima, ativos)
                    })
                });
                ativos.remove(&chave);
                valido
            }
            Tipo::Vazio | Tipo::Funcao(_, _) | Tipo::Parametro(_) | Tipo::Desconhecido => false,
        }
    }

    fn tipo_conversao_explicita(nome: &str) -> Option<Tipo> {
        Some(match nome {
            "inteiro8" | "i8" => Tipo::Inteiro8,
            "inteiro16" | "i16" => Tipo::Inteiro16,
            "inteiro32" | "i32" => Tipo::Inteiro32,
            "inteiro64" | "i64" => Tipo::Inteiro64,
            "inteiro128" | "i128" => Tipo::Inteiro128,
            "inteiro_u8" | "u8" => Tipo::InteiroU8,
            "inteiro_u16" | "u16" => Tipo::InteiroU16,
            "inteiro_u32" | "u32" => Tipo::InteiroU32,
            "inteiro_u64" | "u64" => Tipo::InteiroU64,
            "inteiro_u128" | "u128" => Tipo::InteiroU128,
            "byte" => Tipo::Byte,
            "decimal32" | "f32" => Tipo::Decimal32,
            "decimal64" | "f64" => Tipo::Decimal64,
            _ => return None,
        })
    }

    fn literal_cabe_em_tipo(expressao: &Expressao, destino: &Tipo) -> bool {
        let Some(valor) = Self::literal_numerico(expressao) else {
            return true;
        };
        match (valor, destino) {
            (LiteralNumerico::Inteiro(v), Tipo::Inteiro8) => i8::try_from(v).is_ok(),
            (LiteralNumerico::Inteiro(v), Tipo::Inteiro16) => i16::try_from(v).is_ok(),
            (LiteralNumerico::Inteiro(v), Tipo::Inteiro32) => i32::try_from(v).is_ok(),
            (LiteralNumerico::Inteiro(v), Tipo::Inteiro64) => i64::try_from(v).is_ok(),
            (LiteralNumerico::Inteiro(_), Tipo::Inteiro128) => true,
            (LiteralNumerico::Inteiro(v), Tipo::InteiroU8 | Tipo::Byte) => u8::try_from(v).is_ok(),
            (LiteralNumerico::Inteiro(v), Tipo::InteiroU16) => u16::try_from(v).is_ok(),
            (LiteralNumerico::Inteiro(v), Tipo::InteiroU32) => u32::try_from(v).is_ok(),
            (LiteralNumerico::Inteiro(v), Tipo::InteiroU64) => u64::try_from(v).is_ok(),
            (LiteralNumerico::Inteiro(v), Tipo::InteiroU128) => u128::try_from(v).is_ok(),
            (LiteralNumerico::InteiroSemSinal(v), Tipo::Inteiro8) => i8::try_from(v).is_ok(),
            (LiteralNumerico::InteiroSemSinal(v), Tipo::Inteiro16) => i16::try_from(v).is_ok(),
            (LiteralNumerico::InteiroSemSinal(v), Tipo::Inteiro32) => i32::try_from(v).is_ok(),
            (LiteralNumerico::InteiroSemSinal(v), Tipo::Inteiro64) => i64::try_from(v).is_ok(),
            (LiteralNumerico::InteiroSemSinal(v), Tipo::Inteiro128) => i128::try_from(v).is_ok(),
            (LiteralNumerico::InteiroSemSinal(v), Tipo::InteiroU8 | Tipo::Byte) => u8::try_from(v).is_ok(),
            (LiteralNumerico::InteiroSemSinal(v), Tipo::InteiroU16) => u16::try_from(v).is_ok(),
            (LiteralNumerico::InteiroSemSinal(v), Tipo::InteiroU32) => u32::try_from(v).is_ok(),
            (LiteralNumerico::InteiroSemSinal(v), Tipo::InteiroU64) => u64::try_from(v).is_ok(),
            (LiteralNumerico::InteiroSemSinal(_), Tipo::InteiroU128) => true,
            (LiteralNumerico::Decimal(v), Tipo::Inteiro8) =>
                v.is_finite() && v.trunc() >= i8::MIN as f64 && v.trunc() < i8::MAX as f64 + 1.0,
            (LiteralNumerico::Decimal(v), Tipo::Inteiro16) =>
                v.is_finite() && v.trunc() >= i16::MIN as f64 && v.trunc() < i16::MAX as f64 + 1.0,
            (LiteralNumerico::Decimal(v), Tipo::Inteiro32) =>
                v.is_finite() && v.trunc() >= i32::MIN as f64 && v.trunc() < i32::MAX as f64 + 1.0,
            (LiteralNumerico::Decimal(v), Tipo::Inteiro64) =>
                v.is_finite() && v.trunc() >= i64::MIN as f64 && v.trunc() < -(i64::MIN as f64),
            (LiteralNumerico::Decimal(v), Tipo::Inteiro128) =>
                v.is_finite() && v.trunc() >= i128::MIN as f64 && v.trunc() < -(i128::MIN as f64),
            (LiteralNumerico::Decimal(v), Tipo::InteiroU8 | Tipo::Byte) =>
                v.is_finite() && v.trunc() >= 0.0 && v.trunc() < u8::MAX as f64 + 1.0,
            (LiteralNumerico::Decimal(v), Tipo::InteiroU16) =>
                v.is_finite() && v.trunc() >= 0.0 && v.trunc() < u16::MAX as f64 + 1.0,
            (LiteralNumerico::Decimal(v), Tipo::InteiroU32) =>
                v.is_finite() && v.trunc() >= 0.0 && v.trunc() < u32::MAX as f64 + 1.0,
            (LiteralNumerico::Decimal(v), Tipo::InteiroU64) =>
                v.is_finite() && v.trunc() >= 0.0 && v.trunc() < u64::MAX as f64 + 1.0,
            (LiteralNumerico::Decimal(v), Tipo::InteiroU128) =>
                v.is_finite() && v.trunc() >= 0.0 && v.trunc() < u128::MAX as f64,
            (LiteralNumerico::Decimal(v), Tipo::Decimal32) => (v as f32).is_finite(),
            (LiteralNumerico::Decimal(v), Tipo::Decimal64 | Tipo::Decimal) => v.is_finite(),
            (LiteralNumerico::Inteiro(_) | LiteralNumerico::InteiroSemSinal(_), Tipo::Decimal32) => true,
            (LiteralNumerico::Inteiro(_) | LiteralNumerico::InteiroSemSinal(_), Tipo::Decimal64 | Tipo::Decimal) => true,
            _ => false,
        }
    }

    fn tipo_copiavel(&mut self, tipo: &Tipo) -> bool {
        self.tipo_copiavel_interno(tipo, &mut std::collections::HashSet::new())
    }

    fn tipo_copiavel_interno(
        &mut self,
        tipo: &Tipo,
        ativos: &mut std::collections::HashSet<String>,
    ) -> bool {
        match tipo {
            Tipo::Vazio | Tipo::Funcao(_, _) | Tipo::Logico | Tipo::Inteiro
            | Tipo::Inteiro8 | Tipo::Inteiro16 | Tipo::Inteiro32 | Tipo::Inteiro64
            | Tipo::Inteiro128 | Tipo::InteiroU8 | Tipo::InteiroU16 | Tipo::InteiroU32
            | Tipo::InteiroU64 | Tipo::InteiroU128 | Tipo::Decimal | Tipo::Decimal32
            | Tipo::Decimal64 | Tipo::Caractere | Tipo::Byte => true,
            Tipo::Tupla(elementos) => elementos
                .iter()
                .all(|elemento| self.tipo_copiavel_interno(elemento, ativos)),
            Tipo::Opcional(elemento) | Tipo::Intervalo(elemento) => {
                self.tipo_copiavel_interno(elemento, ativos)
            }
            Tipo::Resultado(sucesso, falha) => {
                self.tipo_copiavel_interno(sucesso, ativos)
                    && self.tipo_copiavel_interno(falha, ativos)
            }
            Tipo::Estrutura(nome) => {
                let chave = format!("estrutura:{nome}");
                if !ativos.insert(chave.clone()) {
                    return false;
                }
                let campos = self.campos_estruturas.get(nome).cloned();
                let copiavel = campos.is_some_and(|campos| {
                    campos.values().all(|tipo_campo| {
                        self.tipo_copiavel_interno(tipo_campo, ativos)
                    })
                });
                ativos.remove(&chave);
                copiavel
            }
            Tipo::Enumeracao(nome) => {
                let chave = format!("enumeracao:{nome}");
                if !ativos.insert(chave.clone()) {
                    return false;
                }
                let variantes = self.variantes_enumeracoes.get(nome).cloned();
                let copiavel = variantes.is_some_and(|variantes| {
                    variantes.values().flatten().all(|tipo_payload| {
                        self.tipo_copiavel_interno(tipo_payload, ativos)
                    })
                });
                ativos.remove(&chave);
                copiavel
            }
            // Texto e listas têm armazenamento próprio e continuam move-only.
            _ => false,
        }
    }

    fn mover_valor(&mut self, expressao: &Expressao) {
        match expressao {
            Expressao::Nome(nome) => {
                let tipo = self.buscar_mutavel(nome).map(|simbolo| simbolo.tipo.clone());
                if tipo.is_some_and(|tipo| !self.tipo_copiavel(&tipo)) {
                    if let Some(simbolo) = self.buscar_mutavel(nome) {
                        simbolo.movido = true;
                    }
                }
            }
            Expressao::Lista(itens) | Expressao::Tupla(itens) => {
                for item in itens { self.mover_valor(item); }
            }
            Expressao::ConstrucaoEstrutura { campos, .. } => {
                for (_, valor) in campos { self.mover_valor(valor); }
            }
            Expressao::Membro { alvo, operador: TipoToken::Ponto, nome } => {
                if let Some((raiz, mut caminho)) = Self::decompor_caminho_campo(alvo) {
                    caminho.push(nome.clone());
                    if self.resolver_tipo_caminho_puro(&raiz, &caminho)
                        .is_some_and(|tipo| !self.tipo_copiavel(&tipo))
                    {
                        if let Some(simbolo) = self.buscar_mutavel(&raiz) {
                            simbolo.campos_movidos.insert(caminho.join("."));
                        }
                    }
                }
            }
            Expressao::Indexacao { alvo, indice } => {
                if let Some((raiz, caminho)) = Self::decompor_caminho_campo(expressao) {
                    let tipo_elemento = self.resolver_tipo_caminho_puro(&raiz, &caminho);
                    if tipo_elemento.is_some_and(|tipo| !self.tipo_copiavel(&tipo)) {
                        if let Some(simbolo) = self.buscar_mutavel(&raiz) {
                            // Um índice literal identifica um elemento independente.
                            // Índices dinâmicos não chegam aqui: continuam movendo a lista inteira.
                            simbolo.campos_movidos.insert(caminho.join("."));
                        }
                    }
                } else if let Some((raiz, caminho_lista)) = Self::decompor_caminho_campo(alvo) {
                    if self.resolver_tipo_caminho_puro(&raiz, &caminho_lista)
                        .is_some_and(|tipo| matches!(tipo, Tipo::Lista(ref elemento) if !self.tipo_copiavel(elemento)))
                    {
                        if let Some(simbolo) = self.buscar_mutavel(&raiz) {
                            if caminho_lista.is_empty() {
                                simbolo.movido = true;
                                simbolo.campos_movidos.clear();
                            } else {
                                simbolo.campos_movidos.insert(caminho_lista.join("."));
                            }
                        }
                    }
                }
                let _ = indice;
            }
            Expressao::Chamada { alvo, argumentos, .. } => {
                let construtor_algebrico = matches!(
                    alvo.as_ref(),
                    Expressao::Membro {
                        alvo: tipo,
                        operador: TipoToken::Escopo,
                        ..
                    } if matches!(tipo.as_ref(), Expressao::Nome(nome)
                        if self.variantes_enumeracoes.contains_key(nome)
                            || matches!(nome.as_str(), "Opcional" | "opcional" | "Resultado" | "resultado"))
                );
                if construtor_algebrico {
                    for argumento in argumentos {
                        self.mover_valor(argumento);
                    }
                }
            }
            _ => {}
        }
    }

    fn declarar(&mut self, nome: &str, simbolo: Simbolo) {
        if self.funcoes.contains_key(nome) || self.modulos_importados.contains_key(nome) {
            self.erro("N3019", format!("O nome '{nome}' já identifica uma função ou módulo neste programa."));
            return;
        }
        let ja_declarado = self.escopos.last().is_some_and(|escopo| escopo.contains_key(nome));
        if ja_declarado {
            self.erro("N3019", format!("O nome '{nome}' já foi declarado neste escopo."));
        } else {
            self.escopos.last_mut().expect("sempre há escopo global").insert(nome.to_owned(), simbolo);
        }
    }

    fn buscar(&self, nome: &str) -> Option<&Simbolo> {
        self.escopos.iter().rev().find_map(|escopo| escopo.get(nome))
    }

    fn exigir_expressao_tipo(&mut self, expressao: &Expressao, obtido: &Tipo, esperado: &Tipo, contexto: &str) {
        if Self::tipo_contem_desconhecido(esperado) { return; }
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
            (Tipo::Inteiro, Tipo::Inteiro64) | (Tipo::Inteiro64, Tipo::Inteiro) => true,
            (Tipo::Parametro(a), Tipo::Parametro(b)) => a == b,
            (Tipo::Lista(a), Tipo::Lista(b)) | (Tipo::Intervalo(a), Tipo::Intervalo(b))
            | (Tipo::Opcional(a), Tipo::Opcional(b)) => Self::tipos_compativeis(a, b),
            (Tipo::Resultado(a1, a2), Tipo::Resultado(b1, b2)) =>
                Self::tipos_compativeis(a1, b1) && Self::tipos_compativeis(a2, b2),
            (Tipo::Tupla(a), Tipo::Tupla(b)) => a.len() == b.len()
                && a.iter().zip(b).all(|(a, b)| Self::tipos_compativeis(a, b)),
            (Tipo::Funcao(a, retorno_a), Tipo::Funcao(b, retorno_b)) => {
                a.len() == b.len()
                    && a.iter().zip(b).all(|(a, b)| Self::tipos_compativeis(a, b))
                    && Self::tipos_compativeis(retorno_a, retorno_b)
            }
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
        if a == &Tipo::Decimal32 && b == &Tipo::Decimal32 {
            return Tipo::Decimal32;
        }
        if a == &Tipo::Decimal32 || b == &Tipo::Decimal32 {
            let outro = if a == &Tipo::Decimal32 { b } else { a };
            return if largura(outro) > 16 { Tipo::Decimal64 } else { Tipo::Decimal32 };
        }
        let sem_sinal = |tipo: &Tipo| matches!(tipo,
            Tipo::InteiroU8 | Tipo::InteiroU16 | Tipo::InteiroU32 | Tipo::InteiroU64 | Tipo::InteiroU128 | Tipo::Byte
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
        self.diagnosticos.push(Diagnostico::novo(codigo, Severidade::Erro, mensagem, self.intervalo_atual));
    }

    fn snapshot_movidos(&self) -> HashSet<(usize, String, String)> {
        let mut estado = HashSet::new();
        for (indice, escopo) in self.escopos.iter().enumerate() {
            for (nome, simbolo) in escopo {
                if simbolo.movido {
                    estado.insert((indice, nome.clone(), String::new()));
                }
                for campo in &simbolo.campos_movidos {
                    estado.insert((indice, nome.clone(), campo.clone()));
                }
            }
        }
        estado
    }

    fn restaurar_movidos(&mut self, estado: &HashSet<(usize, String, String)>) {
        for (indice, escopo) in self.escopos.iter_mut().enumerate() {
            for (nome, simbolo) in escopo {
                simbolo.movido = estado.contains(&(indice, nome.clone(), String::new()));
                simbolo.campos_movidos = estado.iter()
                    .filter(|(escopo_indice, nome_simbolo, campo)|
                        *escopo_indice == indice && nome_simbolo == nome && !campo.is_empty()
                    )
                    .map(|(_, _, campo)| campo.clone())
                    .collect();
            }
        }
    }

    fn mesclar_movidos(
        &mut self,
        primeiro: &HashSet<(usize, String, String)>,
        segundo: &HashSet<(usize, String, String)>,
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

fn normalizar_blocos_legados(codigo: &str) -> String {
    let codigo = codigo.replace("\\n", "\n").replace("\\t", "\t").replace("): ", ") -> ");
    if codigo.contains(": {") {
        return codigo;
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
            if blocos
                .last()
                .is_some_and(|bloco| bloco.tipo == TipoBloco::Caso && bloco.indentacao == nivel)
            {
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
            while blocos
                .last()
                .is_some_and(|bloco| bloco.tipo == TipoBloco::Caso && bloco.indentacao > nivel)
            {
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

        let (tipo, cabecalho) =
            if conteudo.starts_with("publico funcao ") || conteudo.starts_with("funcao ") {
                let assinatura = conteudo.replace("): ", ") -> ");
                let assinatura = if assinatura.ends_with(':') {
                    format!("{assinatura} {{")
                } else {
                    format!("{assinatura}: {{")
                };
                (Some(TipoBloco::Completo), Some(assinatura))
            } else if conteudo.starts_with("estrutura ") || conteudo.starts_with("enumeracao ") || conteudo.starts_with("classe ") {
                let assinatura = if conteudo.ends_with(':') {
                    format!("{conteudo} {{")
                } else {
                    format!("{conteudo}: {{")
                };
                (Some(TipoBloco::Completo), Some(assinatura))
            } else if matches!(conteudo, "critico" | "seguro")
                || conteudo.starts_with("privilegio ")
                || conteudo.starts_with("montagem ")
                || conteudo.starts_with("tente")
                || conteudo.starts_with("com ")
                || conteudo.starts_with("async ")
                || conteudo.starts_with("exportar ")
            {
                (Some(TipoBloco::Completo), Some(format!("{conteudo}: {{")))
            } else if conteudo.starts_with("escolha ") || conteudo.starts_with("avaliar ") {
                (Some(TipoBloco::Escolha), Some(format!("{conteudo}: {{")))
            } else if conteudo.starts_with("quando ") || conteudo.starts_with("se ") {
                (
                    Some(TipoBloco::Condicional),
                    Some(format!("{conteudo}: {{")),
                )
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

    fn analisar_codigo(codigo: &str) -> ResultadoSemantico {
        let codigo = normalizar_blocos_legados(codigo);
        let fonte = ArquivoFonte::novo("teste.nbx", &codigo);
        let sintaxe = analisar(&fonte);
        assert!(sintaxe.diagnosticos.is_empty(), "{:?}", sintaxe.diagnosticos);
        analisar_semantica(&sintaxe.programa)
    }

    #[test]
    fn permite_igualdade_estrutural_de_tipos_compostos() {
        let r = analisar_codigo(
            r#"estrutura Pessoa:
    nome: texto
    idade: inteiro
fim
enumeracao Estado:
    Ativo
    Falha(texto)
fim
variavel a = Pessoa { nome: "Lia", idade: 20, fim }
variavel b = Pessoa { nome: "Lia", idade: 20, fim }
variavel equivalentes = a == b
variavel estado = Estado::Falha("erro")
variavel estados_iguais = estado == Estado::Falha("erro")
variavel tupla_igual = (1, "x") == (1, "x")
variavel lista_igual = [1, 2] == [1, 2]
variavel opcional_igual = Opcional::Algum(1) == Opcional::Algum(1)"#,
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_igualdade_de_tipos_sem_implementacao() {
        let r = analisar_codigo(
            "funcao nada(): vazio\n    passar\nfim\n\nvariavel comparacao = nada() == nada()",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3056"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn move_payload_de_enum_opcional_e_resultado() {
        let r = analisar_codigo(
            "variavel itens = [1]\nvariavel resultado: Resultado<[inteiro], texto> = Resultado::Sucesso(itens)\nexibir(itens)",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", r.diagnosticos);
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
    fn rejeita_parametro_vazio_em_tipo_de_funcao() {
        let r = analisar_codigo(
            "estrutura Registro:\n    callback: funcao(vazio): logico\nfim",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3034"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn valida_valores_de_funcao_e_chamada_indireta() {
        let r = analisar_codigo(
            "funcao somar(valor: inteiro): inteiro\n    devolver valor + 1\nfim\n\nvariavel callback: funcao(inteiro) -> inteiro = somar\nvariavel resultado = callback(41)",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.tipos_globais.get("callback"), Some(&Tipo::Funcao(
            vec![Tipo::Inteiro],
            Box::new(Tipo::Inteiro),
        )));
        assert_eq!(r.tipos_globais.get("resultado"), Some(&Tipo::Inteiro));
    }

    #[test]
    fn rejeita_referencia_generica_sem_instanciacao() {
        let r = analisar_codigo_fase5(
            "funcao identidade<T>(valor: T): T\n    devolver valor\nfim\n\nvariavel callback = identidade",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3057"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn valida_conversoes_numericas_explicitas() {
        let valido = analisar_codigo(
            "variavel pequeno = inteiro8(127)\nvariavel sem_sinal = inteiro_u8(255)\nvariavel fracao = decimal32(1.25)",
        );
        assert!(valido.diagnosticos.is_empty(), "{:?}", valido.diagnosticos);
        let invalido = analisar_codigo("variavel pequeno = inteiro8(128)");
        assert!(invalido.diagnosticos.iter().any(|d| d.codigo == "N3020"), "{:?}", invalido.diagnosticos);
    }

    #[test]
    fn infere_generico_a_partir_do_tipo_esperado_do_retorno() {
        let r = analisar_codigo(
            "funcao opcionalVazio<T>(): Opcional<T>\n    devolver Opcional::Nenhum()\nfim\n\nvariavel valor: Opcional<inteiro> = opcionalVazio()",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(
            r.tipos_globais.get("valor"),
            Some(&Tipo::Opcional(Box::new(Tipo::Inteiro))),
        );
    }

    #[test]
    fn compara_e_infere_assinaturas_de_funcao_recursivamente() {
        let esperado = Tipo::Funcao(
            vec![Tipo::Lista(Box::new(Tipo::Parametro("T".into())))],
            Box::new(Tipo::Opcional(Box::new(Tipo::Parametro("R".into())))),
        );
        let obtido = Tipo::Funcao(
            vec![Tipo::Lista(Box::new(Tipo::Texto))],
            Box::new(Tipo::Opcional(Box::new(Tipo::Inteiro))),
        );
        let equivalente = Tipo::Funcao(
            vec![Tipo::Lista(Box::new(Tipo::Texto))],
            Box::new(Tipo::Opcional(Box::new(Tipo::Inteiro))),
        );
        assert!(Analisador::tipos_compativeis(&equivalente, &obtido));
        let mut inferidos = HashMap::new();
        assert!(Analisador::inferir_genericos(
            &esperado,
            &obtido,
            &["T".into(), "R".into()],
            &mut inferidos,
        ));
        assert_eq!(inferidos.get("T"), Some(&Tipo::Texto));
        assert_eq!(inferidos.get("R"), Some(&Tipo::Inteiro));
        let incompatível = Tipo::Funcao(vec![Tipo::Texto], Box::new(Tipo::Logico));
        assert!(!Analisador::tipos_compativeis(&obtido, &incompatível));
    }

    #[test]
    fn resolve_tipos_de_funcao_em_campos_nominais() {
        let r = analisar_codigo(
            "estrutura Registro:\n    callback: funcao(inteiro, texto): logico\n    inicializar: funcao(funcao(inteiro): logico): vazio\nfim",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn valida_campos_de_estrutura_e_payloads_de_enumeracao() {
        let r = analisar_codigo(
            "estrutura Pessoa:\n    nome: texto\n    idade: inteiro\nfim\nenumeracao Estado:\n    Ativo\n    Falha(texto, inteiro)\nfim",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn valida_construcao_de_variante_com_payload() {
        let r = analisar_codigo(
            "enumeracao Resultado:\n    Sucesso(inteiro)\n    Falha(texto)\nfim\nvariavel resultado = Resultado::Sucesso(42)",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.tipos_globais.get("resultado"), Some(&Tipo::Enumeracao("Resultado".into())));
    }

    #[test]
    fn rejeita_payload_de_variante_com_tipo_incorreto() {
        let r = analisar_codigo(
            "enumeracao Resultado:\n    Sucesso(inteiro)\n    Falha(texto)\nfim\nvariavel resultado = Resultado::Sucesso(\"texto\")",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3020"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_quantidade_incorreta_de_payload() {
        let r = analisar_codigo(
            "enumeracao Resultado:\n    Sucesso(inteiro)\n    Falha(texto)\nfim\nvariavel resultado = Resultado::Sucesso()",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3044"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn valida_escolha_exaustiva_e_vinculos_de_payload() {
        let r = analisar_codigo(
            "enumeracao Resultado:\n    Sucesso(inteiro)\n    Falha(texto)\nfim\nvariavel resultado = Resultado::Sucesso(42)\nescolha resultado\n    caso Resultado::Sucesso(valor)\n        exibir(valor)\n    caso Resultado::Falha(mensagem)\n        exibir(mensagem)\nfim",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn valida_escolha_exaustiva_de_opcional_e_resultado() {
        let opcional = analisar_codigo(
            "variavel valor: Opcional<inteiro> = Opcional::Algum(42)\nescolha valor\n    caso Opcional::Algum(numero)\n        exibir(numero)\n    caso Opcional::Nenhum\n        exibir(0)\nfim",
        );
        assert!(opcional.diagnosticos.is_empty(), "{:?}", opcional.diagnosticos);
        let resultado = analisar_codigo(
            "variavel valor: Resultado<inteiro,texto> = Resultado::Falha(\"erro\")\nescolha valor\n    caso Resultado::Sucesso(numero)\n        exibir(numero)\n    caso Resultado::Falha(mensagem)\n        exibir(mensagem)\nfim",
        );
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
    }

    #[test]
    fn exige_caso_curinga_ao_final() {
        let r = analisar_codigo(
            "enumeracao Estado:\n    Ativo\n    Inativo\nfim\nvariavel estado = Estado::Ativo\nescolha estado\n    caso _\n        exibir(0)\n    caso Estado::Ativo\n        exibir(1)\nfim",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3054"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_escolha_nao_exaustiva_e_payload_incorreto() {
        let r = analisar_codigo(
            "enumeracao Resultado:\n    Sucesso(inteiro)\n    Falha(texto)\nfim\nvariavel resultado = Resultado::Sucesso(42)\nescolha resultado\n    caso Resultado::Sucesso()\n        exibir(1)\nfim",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3050"), "{:?}", r.diagnosticos);
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3053"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_tipo_desconhecido_em_estrutura_e_enumeracao() {
        let r = analisar_codigo(
            "estrutura Pessoa:\n    nome: TipoAusente\nfim\nenumeracao Estado:\n    Falha(TipoAusente)\nfim",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3034"));
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3037"));
    }

    #[test]
    fn resolve_tipos_nominais_em_assinaturas_de_funcoes() {
        let r = analisar_codigo(
            "estrutura Pessoa:\n    nome: texto\nfim\nenumeracao Estado:\n    Ativo\n    Inativo\nfim\nfuncao receber(pessoa: Pessoa, estado: Estado): Pessoa\n    devolver pessoa\nfim",
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
        assert_eq!(r.diagnosticos.iter().filter(|d| d.codigo == "N3063").count(), 2, "{:?}", r.diagnosticos);
    }

    #[test]
    fn valida_intervalos_numericos_e_rejeita_limites_incompativeis() {
        let valido = analisar_codigo("variavel faixa = 1..10");
        assert!(valido.diagnosticos.is_empty(), "{:?}", valido.diagnosticos);
        assert_eq!(
            valido.tipos_globais.get("faixa"),
            Some(&Tipo::Intervalo(Box::new(Tipo::Inteiro))),
        );

        let anotado = analisar_codigo(
            "variavel faixa: Intervalo<inteiro> = 1..10",
        );
        assert!(anotado.diagnosticos.is_empty(), "{:?}", anotado.diagnosticos);
        assert_eq!(
            anotado.tipos_globais.get("faixa"),
            Some(&Tipo::Intervalo(Box::new(Tipo::Inteiro))),
        );

        let texto = analisar_codigo("variavel faixa = 1..\"10\"");
        assert!(texto.diagnosticos.iter().any(|d| d.codigo == "N3020"), "{:?}", texto.diagnosticos);

        let decimal_misto = analisar_codigo("variavel faixa = 1..2.0");
        assert!(decimal_misto.diagnosticos.iter().any(|d| d.codigo == "N3020"), "{:?}", decimal_misto.diagnosticos);
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
        let r = analisar_codigo("funcao soma(a: inteiro, b: inteiro): inteiro\n    devolver a + b\nfim\n\nvariavel x = soma(1, 2)");
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn rejeita_argumento_de_tipo_incorreto() {
        let r = analisar_codigo("funcao soma(a: inteiro): inteiro\n    devolver a\nfim\n\nsoma(\"x\")");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3020"));
    }
    #[test]
    fn verifica_condicao_logica() {
        let r = analisar_codigo("quando 1\n    exibir(\"x\")\nfim");
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
            AssinaturaFuncao {
                genericos: Vec::new(),
                parametros: vec![Tipo::Inteiro, Tipo::Inteiro],
                nomes_parametros: vec!["a".into(), "b".into()],
                parametros_padrao: vec![false, false],
                retorno: Tipo::Inteiro,
            },
        )]));
        let fonte = ArquivoFonte::novo("teste.nbx", "variavel total = Mat::somar(1, 2)\nincluir(biblioteca: Matematica) como Mat");
        let sintaxe = analisar(&fonte);
        assert!(sintaxe.diagnosticos.is_empty(), "{:?}", sintaxe.diagnosticos);
        let r = analisar_com_modulos(&sintaxe.programa, &modulos);
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_modulo_ausente() {
        let r = analisar_codigo("incluir(biblioteca: ModuloInexistente)");
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
    fn valida_tipos_dos_builtins_json() {
        let r = analisar_codigo(
            "variavel valido = json_valido(\"{\\\"chave\\\": 1}\")\nvariavel canonico = json_canonico(\"{\\\"b\\\":2,\\\"a\\\":1}\")",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.tipos_globais.get("valido"), Some(&Tipo::Logico));
        assert_eq!(r.tipos_globais.get("canonico"), Some(&Tipo::Texto));
    }

    #[test]
    fn rejeita_argumento_nao_textual_nos_builtins_json() {
        let r = analisar_codigo("json_valido(42)\njson_canonico(falso)");
        assert!(r.diagnosticos.iter().filter(|d| d.codigo == "N3020").count() >= 2);
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
        let r = analisar_codigo("funcao calcular(condicao: logico): inteiro\n    quando condicao\n        devolver 1\n    fim\nfim");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3024"));
    }

    #[test]
    fn aceita_retorno_em_todos_os_caminhos_condicionais() {
        let r = analisar_codigo("funcao calcular(condicao: logico): inteiro\n    quando condicao\n        devolver 1\n    \n    senao\n        devolver 2\n    fim\nfim");
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn distingue_larguras_de_inteiros() {
        let r = analisar_codigo("variavel a: inteiro8 = 1\nvariavel b: inteiro64 = a");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3020"));
    }

    #[test]
    fn inteiro_padrao_e_alias_semantico_de_inteiro64() {
        assert!(Analisador::tipos_compativeis(&Tipo::Inteiro, &Tipo::Inteiro64));
        assert!(Analisador::tipos_compativeis(&Tipo::Inteiro64, &Tipo::Inteiro));
        let r = analisar_codigo("funcao consultar(): inteiro64\n    devolver 42\nfim\n\nvariavel valor: inteiro = consultar()");
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn literal_de_comparacao_recebe_tipo_numerico_contextual() {
        let r = analisar_codigo("variavel valor: inteiro_u64 = 42\nvariavel igual = valor == 42");
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn detecta_ciclos_de_aliases_aninhados_em_tipos_compostos() {
        for codigo in [
            "tipo A = Vetor<A>",
            "tipo A = (A,)",
            "tipo A = Opcional<B>\ntipo B = Resultado<inteiro, A>",
        ] {
            let r = analisar_codigo(codigo);
            assert!(
                r.diagnosticos.iter().any(|d| d.codigo == "N3003"),
                "{codigo}: {:?}",
                r.diagnosticos,
            );
        }
    }

    #[test]
    fn detecta_alias_de_tipo_ciclico() {
        let r = analisar_codigo("tipo A = B\ntipo B = A");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3003"));
    }


    #[test]
    fn rejeita_variavel_com_nome_de_funcao() {
        let r = analisar_codigo("funcao calcular() -> inteiro: {\n    devolver 1\n    fim\n} fim\n\nvariavel calcular = 2");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3019"));
    }


    #[test]
    fn permite_sombreamento_em_escopo_interno() {
        let r = analisar_codigo("variavel valor = 1\nquando verdadeiro\n    variavel valor = 2\n    exibir(valor)\nfim");
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn impede_acesso_a_variavel_apos_fim_do_escopo() {
        let r = analisar_codigo("quando verdadeiro\n    variavel interna = 1\nfim\n\nexibir(interna)");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3007"));
    }

    #[test]
    fn rejeita_alias_de_modulo_duplicado() {
        let mut modulos = ModulosDisponiveis::new();
        modulos.insert("A".into(), HashMap::new());
        modulos.insert("B".into(), HashMap::new());
        let fonte = ArquivoFonte::novo("teste.nbx", "incluir(biblioteca: A) como M\nincluir(biblioteca: B) como M");
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
        let codigo = normalizar_blocos_legados("incluir(biblioteca: Matematica) como calcular\nfuncao calcular(): inteiro\n    devolver 1\nfim");
        let fonte = ArquivoFonte::novo("teste.nbx", &codigo);
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
    fn inteiros8_sao_copiados_e_nao_movidos() {
        let r = analisar_codigo(
            "variavel origem: inteiro8 = 42\nvariavel destino: inteiro8 = origem\nexibir(origem)",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.tipos_globais.get("origem"), Some(&Tipo::Inteiro8));
        assert_eq!(r.tipos_globais.get("destino"), Some(&Tipo::Inteiro8));
    }

    #[test]
    fn deriva_copia_de_estruturas_com_todos_os_campos_copiaveis() {
        let r = analisar_codigo(
            "estrutura Ponto:\n    x: inteiro\n    y: inteiro\nfim\nvariavel original = Ponto { x: 10, y: 20, fim }\nvariavel copia = original\nexibir(original.x)",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn estrutura_com_campo_move_only_nao_e_copiavel() {
        let r = analisar_codigo(
            "estrutura Caixa:\n    conteudo: texto\n    codigo: inteiro\nfim\nvariavel original = Caixa { conteudo: \"Nebula\", codigo: 42, fim }\nvariavel copia = original\nexibir(original.codigo)",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn deriva_copia_de_enumeracoes_e_tuplas_recursivamente() {
        let r = analisar_codigo(
            "enumeracao Estado:\n    Codigo(inteiro)\n    Vazio\nfim\nvariavel estado = Estado::Codigo(42)\nvariavel estado_copia = estado\nvariavel tupla = (42, verdadeiro)\nvariavel tupla_copia = tupla\nexibir(estado)\nexibir(tupla)",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn enum_e_tupla_com_payload_move_only_nao_sao_copiaveis() {
        let r = analisar_codigo(
            "enumeracao Resposta:\n    Texto(texto)\n    Vazia\nfim\nvariavel resposta = Resposta::Texto(\"ok\")\nvariavel resposta_copia = resposta\nvariavel tupla = (\"Nebula\", 42)\nvariavel tupla_copia = tupla\nexibir(resposta)\nexibir(tupla)",
        );
        assert!(r.diagnosticos.iter().filter(|d| d.codigo == "N3030").count() >= 2, "{:?}", r.diagnosticos);
    }

    #[test]
    fn deriva_copia_de_opcional_e_resultado_apenas_quando_payloads_sao_copiaveis() {
        let copiaveis = analisar_codigo(
            "variavel opcional = Opcional::Algum(42)\nvariavel opcional_copia = opcional\nvariavel resultado: Resultado<inteiro, inteiro> = Resultado::Sucesso(42)\nvariavel resultado_copia = resultado\nexibir(opcional)\nexibir(resultado)",
        );
        assert!(copiaveis.diagnosticos.is_empty(), "{:?}", copiaveis.diagnosticos);

        let move_only = analisar_codigo(
            "variavel opcional = Opcional::Algum(\"texto\")\nvariavel opcional_copia = opcional\nexibir(opcional)",
        );
        assert!(move_only.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", move_only.diagnosticos);
    }

    #[test]
    fn deriva_copia_para_tuplas_estruturas_e_enumeracoes() {
        let r = analisar_codigo(
            "estrutura Ponto:\n    x: inteiro8\n    ativo: logico\nfim\nenumeracao Estado:\n    Inativo\n    Codigo(inteiro8)\nfim\nvariavel ponto = Ponto { x: 7, ativo: verdadeiro, fim }\nvariavel outro_ponto = ponto\nvariavel estado = Estado::Codigo(1)\nvariavel outro_estado = estado\nvariavel par = (inteiro8(1), verdadeiro)\nvariavel outro_par = par\nvariavel opcional: Opcional<inteiro8> = Opcional::Algum(7)\nvariavel outro_opcional = opcional\nvariavel resultado: Resultado<inteiro8,inteiro8> = Resultado::Sucesso(7)\nvariavel outro_resultado = resultado\nvariavel faixa: Intervalo<inteiro8> = inteiro8(1)..inteiro8(3)\nvariavel outra_faixa = faixa\nexibir(ponto.x)\nexibir(estado)\nexibir(par)",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.tipos_globais.get("ponto"), Some(&Tipo::Estrutura("Ponto".into())));
        assert_eq!(r.tipos_globais.get("estado"), Some(&Tipo::Enumeracao("Estado".into())));
        assert_eq!(
            r.tipos_globais.get("par"),
            Some(&Tipo::Tupla(vec![Tipo::Inteiro8, Tipo::Logico])),
        );
    }

    #[test]
    fn copiar_explicita_valores_com_derivacao_de_copia() {
        let r = analisar_codigo(
            "estrutura Ponto:\n    x: inteiro8\nfim\nvariavel origem = Ponto { x: 7, fim }\nvariavel destino = copiar(origem)\nexibir(origem.x)\nexibir(destino.x)",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.tipos_globais.get("origem"), Some(&Tipo::Estrutura("Ponto".into())));
        assert_eq!(r.tipos_globais.get("destino"), Some(&Tipo::Estrutura("Ponto".into())));
    }

    #[test]
    fn copiar_rejeita_tipos_move_only_e_aridade_invalida() {
        let texto = analisar_codigo(
            "variavel original: texto = \"conteudo\"\nvariavel copia = copiar(original)",
        );
        assert!(texto.diagnosticos.iter().any(|d| d.codigo == "N3058"), "{:?}", texto.diagnosticos);

        let aridade = analisar_codigo("variavel valor = copiar()");
        assert!(aridade.diagnosticos.iter().any(|d| d.codigo == "N3058"), "{:?}", aridade.diagnosticos);
    }

    #[test]
    fn estruturas_com_texto_continuam_move_only() {
        let r = analisar_codigo(
            "estrutura Documento:\n    conteudo: texto\nfim\nvariavel original = Documento { conteudo: \"texto\", fim }\nvariavel destino = original\nexibir(original.conteudo)",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn reatribuicao_reinicializa_valor_movido() {
        let r = analisar_codigo("variavel origem: texto = \"antigo\"\nvariavel destino: texto = origem\norigem = \"novo\"\nexibir(origem)");
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn resolve_tipos_genericos_aninhados_em_assinaturas() {
        let r = analisar_codigo(
            "funcao recebe(valor: Opcional<[inteiro]>)\n    exibir(\"ok\")\nfim\nfuncao tenta(valor: Resultado<texto,inteiro>)\n    exibir(\"ok\")\nfim",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn resolve_tupla_generica_com_mais_de_dois_elementos() {
        let r = analisar_codigo(
            "funcao recebe(valor: Tupla<inteiro,texto,logico>)\n    exibir(\"ok\")\nfim",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_aridade_incorreta_de_tipo_generico() {
        let r = analisar_codigo("funcao recebe(valor: Resultado<inteiro>)\n    exibir(\"ok\")\nfim");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3021"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_tipos_desconhecidos_aninhados_em_declaracoes() {
        let alias = analisar_codigo("tipo Dados = [TipoAusente]");
        assert!(alias.diagnosticos.iter().any(|d| d.codigo == "N3003"), "{:?}", alias.diagnosticos);

        let parametro = analisar_codigo("funcao receber(valor: [TipoAusente])\n    exibir(1)\nfim");
        assert!(parametro.diagnosticos.iter().any(|d| d.codigo == "N3021"), "{:?}", parametro.diagnosticos);

        let retorno = analisar_codigo("funcao produzir(): [TipoAusente]\n    devolver []\nfim");
        assert!(retorno.diagnosticos.iter().any(|d| d.codigo == "N3021"), "{:?}", retorno.diagnosticos);

        let variavel = analisar_codigo("variavel dados: [TipoAusente] = []");
        assert!(variavel.diagnosticos.iter().any(|d| d.codigo == "N3021"), "{:?}", variavel.diagnosticos);

        let estrutura = analisar_codigo("estrutura Caixa:\n    dados: [TipoAusente]\nfim");
        assert!(estrutura.diagnosticos.iter().any(|d| d.codigo == "N3034"), "{:?}", estrutura.diagnosticos);

        let enumeracao = analisar_codigo("enumeracao Resposta:\n    Dados([TipoAusente])\nfim");
        assert!(enumeracao.diagnosticos.iter().any(|d| d.codigo == "N3037"), "{:?}", enumeracao.diagnosticos);
    }

    #[test]
    fn infere_generico_em_funcao_de_identidade() {
        let r = analisar_codigo_fase5(
            "funcao identidade<T>(valor: T): T\n    devolver valor\nfim\n\nvariavel numero = identidade(42)\nvariavel texto = identidade(\"Nebula\")",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.tipos_globais.get("numero"), Some(&Tipo::Inteiro));
        assert_eq!(r.tipos_globais.get("texto"), Some(&Tipo::Texto));
    }

    #[test]
    fn infere_generico_aninhado_em_lista() {
        let r = analisar_codigo(
            "funcao primeiro<T>(valores: [T]): T\n    devolver valores[0]\nfim\n\nvariavel numero = primeiro([1, 2, 3])",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.tipos_globais.get("numero"), Some(&Tipo::Inteiro));
    }

    #[test]
    fn infere_genericos_em_tuplas_resultados_opcionais_e_funcoes() {
        let r = analisar_codigo_fase5(
            "funcao empacotar<T>(valor: T): Tupla<T,texto>\n    devolver (valor, \"ok\")\nfim\nfuncao receber<T>(valor: Resultado<Opcional<T>,texto>)\n    exibir(\"ok\")\nfim\n\nvariavel pacote = empacotar(42)\nvariavel valor: Resultado<Opcional<inteiro>,texto> = Resultado::Sucesso(Opcional::Algum(42))\nreceber(valor)",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(
            r.tipos_globais.get("pacote"),
            Some(&Tipo::Tupla(vec![Tipo::Inteiro, Tipo::Texto]))
        );
    }

    #[test]
    fn rejeita_inferencia_generica_conflitante() {
        let r = analisar_codigo(
            "funcao escolher<T>(a: T, b: T): T\n    devolver a\nfim\n\nvariavel valor = escolher(1, \"texto\")",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3032"), "{:?}", r.diagnosticos);
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3020"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_parametro_generico_sem_inferencia() {
        let r = analisar_codigo(
            "funcao produzir<T>(valor: inteiro): T\n    devolver valor\nfim\n\nvariavel resultado = produzir(1)",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3064"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn tipos_genericos_aninhados_inferem_recursivamente() {
        let r = analisar_codigo(
            "funcao recebe<T>(valor: Opcional<[T]>)\n    exibir(\"ok\")\nfim",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn valida_construtores_opcional_e_resultado() {
        let r = analisar_codigo(
            "variavel numero: Opcional<inteiro> = Opcional::Algum(42)\nvariavel ausente: Opcional<texto> = Opcional::Nenhum()\nvariavel sucesso: Resultado<inteiro,texto> = Resultado::Sucesso(7)\nvariavel falha: Resultado<inteiro,texto> = Resultado::Falha(\"erro\")",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.tipos_globais.get("numero"), Some(&Tipo::Opcional(Box::new(Tipo::Inteiro))));
        assert_eq!(r.tipos_globais.get("ausente"), Some(&Tipo::Opcional(Box::new(Tipo::Texto))));
        assert_eq!(r.tipos_globais.get("sucesso"), Some(&Tipo::Resultado(Box::new(Tipo::Inteiro), Box::new(Tipo::Texto))));
        assert_eq!(r.tipos_globais.get("falha"), Some(&Tipo::Resultado(Box::new(Tipo::Inteiro), Box::new(Tipo::Texto))));
    }

    #[test]
    fn rejeita_construtor_opcional_com_aridade_incorreta() {
        let r = analisar_codigo("variavel valor = Opcional::Algum()");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3054"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn nao_coage_literal_inteiro_para_tipo_decimal() {
        let r = analisar_codigo("variavel valor: decimal32 = 1");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3020"));
    }

    #[test]
    fn move_campo_sem_invalidar_campos_independentes() {
        let r = analisar_codigo(
            "estrutura Conta:\n    nome: texto\n    idade: inteiro\nfim\nvariavel conta = Conta { nome: \"Lia\", idade: 20, fim }\nvariavel nome = conta.nome\nexibir(conta.idade)",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_reuso_de_campo_movido_e_uso_integral_parcialmente_movido() {
        let campo = analisar_codigo(
            "estrutura Conta:\n    nome: texto\n    idade: inteiro\nfim\nvariavel conta = Conta { nome: \"Lia\", idade: 20, fim }\nvariavel nome = conta.nome\nexibir(conta.nome)",
        );
        assert!(campo.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", campo.diagnosticos);

        let inteiro = analisar_codigo(
            "estrutura Conta:\n    nome: texto\n    idade: inteiro\nfim\nvariavel conta = Conta { nome: \"Lia\", idade: 20, fim }\nvariavel nome = conta.nome\nexibir(conta)",
        );
        assert!(inteiro.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", inteiro.diagnosticos);
    }

    #[test]
    fn atribuir_campo_move_o_valor_de_origem() {
        let r = analisar_codigo(
            "estrutura Conta:\n    nome: texto\nfim\nvariavel nome = \"Lia\"\nvariavel conta = Conta { nome: \"Ana\", fim }\nconta.nome = nome\nexibir(nome)",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn reatribuir_campo_reinicializa_movimento_parcial() {
        let r = analisar_codigo(
            "estrutura Conta:\n    nome: texto\n    idade: inteiro\nfim\nvariavel conta = Conta { nome: \"Lia\", idade: 20, fim }\nvariavel nome = conta.nome\nconta.nome = \"Bia\"\nexibir(conta.nome)\nexibir(conta.idade)",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn move_campo_aninhado_preserva_campos_irmãos() {
        let r = analisar_codigo(
            "estrutura Endereco:\n    rua: texto\n    numero: inteiro\nfim\nestrutura Pessoa:\n    endereco: Endereco\n    idade: inteiro\nfim\nfuncao obter_numero(pessoa: Pessoa): inteiro\n    variavel rua = pessoa.endereco.rua\n    devolver pessoa.endereco.numero\nfim",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_acesso_ao_agregado_que_contem_campo_movido() {
        let r = analisar_codigo(
            "estrutura Endereco:\n    rua: texto\n    numero: inteiro\nfim\nestrutura Pessoa:\n    endereco: Endereco\n    idade: inteiro\nfim\nfuncao obter_endereco(pessoa: Pessoa): Endereco\n    variavel rua = pessoa.endereco.rua\n    devolver pessoa.endereco\nfim",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn permite_irmao_apos_movimento_de_campo_aninhado() {
        let r = analisar_codigo(
            "estrutura Endereco:\n    rua: texto\n    numero: inteiro\nfim\nestrutura Pessoa:\n    endereco: Endereco\n    idade: inteiro\nfim\nfuncao obter_numero(pessoa: Pessoa): inteiro\n    variavel rua = pessoa.endereco.rua\n    devolver pessoa.endereco.numero\nfim",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_reuso_de_campo_aninhado_movido() {
        let r = analisar_codigo(
            "estrutura Endereco:\n    rua: texto\n    numero: inteiro\nfim\nestrutura Pessoa:\n    endereco: Endereco\n    idade: inteiro\nfim\nfuncao obter_rua(pessoa: Pessoa): texto\n    variavel rua = pessoa.endereco.rua\n    devolver pessoa.endereco.rua\nfim",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn move_posicao_de_tupla_sem_invalidar_outras_posicoes() {
        let r = analisar_codigo(
            "variavel par = (\"Lia\", 20)\nvariavel nome = par[0]\nexibir(par[1])",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_reuso_de_posicao_movida_de_tupla() {
        let r = analisar_codigo(
            "variavel par = (\"Lia\", 20)\nvariavel nome = par[0]\nexibir(par[0])",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn reatribuir_posicao_reinicializa_movimento_de_tupla() {
        let r = analisar_codigo(
            "variavel par = (\"Lia\", 20)\nvariavel nome = par[0]\npar[0] = \"Bia\"\nexibir(par[0])",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn mover_elemento_literal_de_lista_preserva_irmaos() {
        let r = analisar_codigo(
            "variavel nomes = [\"Lia\", \"Bia\"]\nvariavel nome = nomes[0]\nexibir(nomes[1])",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_reuso_de_elemento_literal_movido_de_lista() {
        let r = analisar_codigo(
            "variavel nomes = [\"Lia\", \"Bia\"]\nvariavel nome = nomes[0]\nexibir(nomes[0])",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn reatribuir_elemento_literal_reinicializa_movimento_de_lista() {
        let r = analisar_codigo(
            "variavel nomes = [\"Lia\", \"Bia\"]\nvariavel nome = nomes[0]\nnomes[0] = \"Ana\"\nexibir(nomes[0])\nexibir(nomes[1])",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn mover_elemento_de_lista_move_a_lista_inteira() {
        let r = analisar_codigo(
            "variavel indice = 0\nvariavel nomes = [\"Lia\", \"Bia\"]\nvariavel nome = nomes[indice]\nexibir(nomes[1])",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn escolha_consume_enum_nao_copiavel() {
        let r = analisar_codigo(
            "enumeracao Resposta:\n    Sucesso(texto)\n    Falha(texto)\nfim\nvariavel resposta = Resposta::Sucesso(\"ok\")\nescolha resposta\n    caso Resposta::Sucesso(valor)\n        exibir(valor)\n    caso Resposta::Falha(falha)\n        exibir(falha)\nfim\n\nexibir(resposta)",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn escolha_consume_opcional_e_resultado_nao_copiaveis() {
        let opcional = analisar_codigo(
            "variavel valor = Opcional::Algum(\"texto\")\nescolha valor\n    caso Opcional::Algum(conteudo)\n        exibir(conteudo)\n    caso Opcional::Nenhum\n        passar\nfim\n\nexibir(valor)",
        );
        assert!(opcional.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", opcional.diagnosticos);
        let resultado = analisar_codigo(
            "variavel valor = Resultado::Sucesso(\"texto\")\nescolha valor\n    caso Resultado::Sucesso(conteudo)\n        exibir(conteudo)\n    caso Resultado::Falha(falha)\n        exibir(falha)\nfim\n\nexibir(valor)",
        );
        assert!(resultado.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", resultado.diagnosticos);
    }

    #[test]
    fn movimento_parcial_e_mesclado_entre_ramos() {
        let r = analisar_codigo(
            "estrutura Conta:\n    nome: texto\n    idade: inteiro\nfim\nvariavel conta = Conta { nome: \"Lia\", idade: 20, fim }\nquando verdadeiro\n    variavel nome = conta.nome\n\nsenao\n    passar\nfim\n\nexibir(conta.nome)",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn movimento_em_um_ramo_e_propagado_apos_a_juncao() {
        let r = analisar_codigo(
            "variavel texto = \"Nebula\"\nquando verdadeiro\n    variavel destino = texto\nfim\n\nexibir(texto)",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn movimento_em_laco_e_considerado_possivel_apos_o_laco() {
        let r = analisar_codigo(
            "variavel texto = \"Nebula\"\nenquanto verdadeiro\n    variavel destino = texto\nfim\n\nexibir(texto)",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn resolve_tipo_de_campo_de_estrutura() {
        let r = analisar_codigo(
            "estrutura Pessoa:\n    nome: texto\n    idade: inteiro\nfim\nfuncao obter_nome(pessoa: Pessoa): texto\n    devolver pessoa.nome\nfim",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_campo_inexistente_em_estrutura() {
        let r = analisar_codigo(
            "estrutura Pessoa:\n    nome: texto\nfim\nfuncao obter_id(pessoa: Pessoa): inteiro\n    devolver pessoa.id\nfim",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3040"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_acesso_a_campo_em_tipo_primitivo() {
        let r = analisar_codigo(
            "funcao obter_campo(valor: inteiro): texto\n    devolver valor.nome\nfim",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3041"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn construcao_de_estrutura_valida_todos_os_campos() {
        let r = analisar_codigo(
            "estrutura Pessoa:\n    nome: texto\n    idade: inteiro\nfim\nvariavel pessoa = Pessoa { nome: \"Lia\", idade: 28, fim }",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn construcao_de_estrutura_rejeita_campo_ausente() {
        let r = analisar_codigo(
            "estrutura Pessoa:\n    nome: texto\n    idade: inteiro\nfim\nvariavel pessoa = Pessoa { nome: \"Lia\", fim }",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3043"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn construcao_de_estrutura_rejeita_tipo_incompativel() {
        let r = analisar_codigo(
            "estrutura Pessoa:\n    idade: inteiro\nfim\nvariavel pessoa = Pessoa { idade: \"vinte\", fim }",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3020"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn reinicializacao_em_todos_os_ramos_remove_estado_de_movido() {
        let r = analisar_codigo(
            "variavel texto = \"Nebula\"\nquando verdadeiro\n    variavel a = texto\n    texto = \"A\"\n\nsenao\n    variavel b = texto\n    texto = \"B\"\nfim\n\nexibir(texto)",
        );
        assert!(!r.diagnosticos.iter().any(|d| d.codigo == "N3030"), "{:?}", r.diagnosticos);
    }

fn analisar_codigo_fase5(codigo: &str) -> ResultadoSemantico {
    let codigo = normalizar_blocos_legados(codigo);
    let fonte = nebula_fonte::ArquivoFonte::novo("teste-fase5.nbx", &codigo);
    let sintaxe = nebula_sintaxe::analisar(&fonte);
    assert!(
        sintaxe.diagnosticos.is_empty(),
        "{:?}",
        sintaxe.diagnosticos
    );
    analisar_semantica(&sintaxe.programa)
}

    #[test]
    fn rejeita_instancia_generica_com_tipo_aninhado_nao_inferido() {
        let r = analisar_codigo_fase5(
            "funcao identidade<T>(valor: T): T\n    devolver valor\nfim\n\nvariavel ausente = identidade(Opcional::Nenhum())",
        );
        assert!(
            r.diagnosticos.iter().any(|d| d.codigo == "N3032"),
            "Uma lacuna de tipo aninhada não pode virar uma instância genérica concreta: {:?}",
            r.diagnosticos
        );
    }

    #[test]
    fn valida_aliases_de_entrada_e_espera() {
        let r = analisar_codigo("variavel nome = receber(\"Nome: \")\naguardar(1)");
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.tipos_globais.get("nome"), Some(&Tipo::Texto));
    }


    #[test]
    fn valida_cada_sobre_lista_intervalo_e_texto() {
        let valido = analisar_codigo(
            "variavel total = 0\ncada numero em [1, 2, 3]\n    total = total + numero\nfim\ncada caractere em \"abc\"\n    exibir(caractere)\nfim\ncada indice em 1..3\n    exibir(indice)\nfim",
        );
        assert!(valido.diagnosticos.is_empty(), "{:?}", valido.diagnosticos);
    }

    #[test]
    fn rejeita_cada_sobre_valor_nao_iteravel() {
        let invalido = analisar_codigo("cada item em 42\n    exibir(item)\nfim");
        assert!(invalido.diagnosticos.iter().any(|d| d.codigo == "N3058"), "{:?}", invalido.diagnosticos);
    }


    #[test]
    fn reconhece_tipos_vetor_matriz_e_gerar() {
        let r = analisar_codigo(
            "variavel numeros: Vetor<inteiro> = [1, 2]\nvariavel grade: Matriz<inteiro> = [[1, 2], [3, 4]]\nvariavel repetidos = gerar(7, 3)",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.tipos_globais.get("numeros"), Some(&Tipo::Lista(Box::new(Tipo::Inteiro))));
        assert_eq!(r.tipos_globais.get("grade"), Some(&Tipo::Lista(Box::new(Tipo::Lista(Box::new(Tipo::Inteiro))))));
        assert_eq!(r.tipos_globais.get("repetidos"), Some(&Tipo::Lista(Box::new(Tipo::Inteiro))));
    }


    #[test]
    fn valida_pulso_com_intervalo_inteiro() {
        let r = analisar_codigo("pulso(10)\n    encerrar\nfim");
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_pulso_com_intervalo_nao_inteiro() {
        let r = analisar_codigo("pulso(\"um segundo\")\n    encerrar\nfim");
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3059"), "{:?}", r.diagnosticos);
    }


    #[test]
    fn valida_em_paralelo_com_funcoes_sem_parametros_e_retorno_igual() {
        let r = analisar_codigo(
            "funcao um(): inteiro\n    devolver 1\nfim\nfuncao dois(): inteiro\n    devolver 2\nfim\n\nvariavel resultados = em_paralelo(um, dois)",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_em_paralelo_com_retorno_incompativel() {
        let r = analisar_codigo(
            "funcao um(): inteiro\n    devolver 1\nfim\nfuncao dois(): texto\n    devolver \"dois\"\nfim\n\nvariavel resultados = em_paralelo(um, dois)",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3061"), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_inferencia_generica_inconsistente_em_tipos_compostos() {
        let r = analisar_codigo_fase5(
            "funcao par<T>(primeiro: T, segundo: T): T\n    devolver primeiro\nfim\n\nvariavel resultado = par([1], [verdadeiro])",
        );
        assert!(
            r.diagnosticos.iter().any(|d| d.codigo == "N3032"),
            "O mesmo parâmetro genérico deve unificar com um único tipo concreto: {:?}",
            r.diagnosticos
        );
    }

    #[test]
    fn inferencia_generica_resolve_tipos_aninhados_em_resultados_e_funcoes() {
        let esperado = Tipo::Resultado(
            Box::new(Tipo::Lista(Box::new(Tipo::Parametro("T".into())))),
            Box::new(Tipo::Funcao(
                vec![Tipo::Parametro("T".into())],
                Box::new(Tipo::Opcional(Box::new(Tipo::Parametro("T".into())))),
            )),
        );
        let obtido = Tipo::Resultado(
            Box::new(Tipo::Lista(Box::new(Tipo::Inteiro128))),
            Box::new(Tipo::Funcao(
                vec![Tipo::Inteiro128],
                Box::new(Tipo::Opcional(Box::new(Tipo::Inteiro128))),
            )),
        );
        let mut inferidos = HashMap::new();

        assert!(Analisador::inferir_genericos(
            &esperado,
            &obtido,
            &["T".into()],
            &mut inferidos,
        ));
        assert_eq!(inferidos.get("T"), Some(&Tipo::Inteiro128));
    }

    #[test]
    fn inferencia_generica_rejeita_conflito_aninhado_sem_vazar_vinculos() {
        let esperado = Tipo::Resultado(
            Box::new(Tipo::Lista(Box::new(Tipo::Parametro("T".into())))),
            Box::new(Tipo::Funcao(
                vec![Tipo::Parametro("T".into())],
                Box::new(Tipo::Vazio),
            )),
        );
        let obtido = Tipo::Resultado(
            Box::new(Tipo::Lista(Box::new(Tipo::Inteiro))),
            Box::new(Tipo::Funcao(
                vec![Tipo::Texto],
                Box::new(Tipo::Vazio),
            )),
        );
        let mut inferidos = HashMap::new();

        assert!(!Analisador::inferir_genericos(
            &esperado,
            &obtido,
            &["T".into()],
            &mut inferidos,
        ));
        assert!(inferidos.is_empty());
    }

    #[test]
    fn inferencia_generica_composta_nao_deixa_vinculos_parciais_ao_falhar() {
        let esperado = Tipo::Tupla(vec![
            Tipo::Parametro("T".into()),
            Tipo::Parametro("T".into()),
        ]);
        let obtido = Tipo::Tupla(vec![Tipo::Inteiro, Tipo::Texto]);
        let mut inferidos = HashMap::new();

        assert!(!Analisador::inferir_genericos(
            &esperado,
            &obtido,
            &["T".into()],
            &mut inferidos,
        ));
        assert!(inferidos.is_empty(), "inferência rejeitada não pode vazar vínculos parciais");
    }

    #[test]
    fn inferencia_de_chamada_generica_e_atomica_entre_argumentos_e_retorno() {
        let funcao = Funcao {
            genericos: vec!["T".into(), "R".into()],
            nomes_parametros: vec!["primeiro".into(), "segundo".into()],
            parametros_padrao: vec![None, None],
            parametros: vec![
                Tipo::Lista(Box::new(Tipo::Parametro("T".into()))),
                Tipo::Funcao(
                    vec![Tipo::Parametro("T".into())],
                    Box::new(Tipo::Opcional(Box::new(Tipo::Parametro("R".into())))),
                ),
            ],
            retorno: Tipo::Resultado(
                Box::new(Tipo::Parametro("R".into())),
                Box::new(Tipo::Parametro("T".into())),
            ),
        };
        let argumentos_validos = vec![
            Tipo::Lista(Box::new(Tipo::Inteiro128)),
            Tipo::Funcao(
                vec![Tipo::Inteiro128],
                Box::new(Tipo::Opcional(Box::new(Tipo::Texto))),
            ),
        ];
        let esperado = Tipo::Resultado(
            Box::new(Tipo::Texto),
            Box::new(Tipo::Inteiro128),
        );
        let (valida, inferidos) = Analisador::inferir_genericos_chamada(
            &funcao,
            &argumentos_validos,
            Some(&esperado),
        );
        assert!(valida, "todos os parâmetros e o retorno devem unificar");
        assert_eq!(inferidos.get("T"), Some(&Tipo::Inteiro128));
        assert_eq!(inferidos.get("R"), Some(&Tipo::Texto));

        let argumentos_conflitantes = vec![
            Tipo::Lista(Box::new(Tipo::Inteiro128)),
            Tipo::Funcao(
                vec![Tipo::Texto],
                Box::new(Tipo::Opcional(Box::new(Tipo::Texto))),
            ),
        ];
        let (inferencia_argumentos_valida, inferidos_parciais) =
            Analisador::inferir_genericos_chamada(
                &funcao,
                &argumentos_conflitantes,
                Some(&esperado),
            );
        assert!(!inferencia_argumentos_valida, "conflito em argumento deve invalidar a chamada");
        assert_eq!(inferidos_parciais.get("T"), Some(&Tipo::Inteiro128));

        let retorno_conflitante = Tipo::Resultado(
            Box::new(Tipo::Logico),
            Box::new(Tipo::Inteiro128),
        );
        let (retorno_valido, _) = Analisador::inferir_genericos_chamada(
            &funcao,
            &argumentos_validos,
            Some(&retorno_conflitante),
        );
        assert!(!retorno_valido, "conflito com tipo esperado deve invalidar a chamada");
    }

    #[test]
    fn matriz_de_tipos_primitivos_da_fase5_permanece_tipavel() {
        let codigo = r#"
variavel logico_valor: logico = verdadeiro
variavel i8: inteiro8 = -128
variavel i16: inteiro16 = 32767
variavel i32: inteiro32 = 2147483647
variavel i64: inteiro64 = 42
variavel i128: inteiro128 = 42
variavel u8: inteiro_u8 = 255
variavel u16: inteiro_u16 = 65535
variavel u32: inteiro_u32 = 42
variavel u64: inteiro_u64 = 42
variavel u128: inteiro_u128 = 42
variavel d32: decimal32 = 1.25
variavel d64: decimal64 = 2.5
variavel byte_valor: byte = 255
variavel caractere_valor: caractere = 'Ω'
variavel texto_valor: texto = "Nebula"
funcao sem_retorno(): vazio
    passar
fim
"#;
        let r = analisar_codigo(codigo);
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.tipos_globais.get("i128"), Some(&Tipo::Inteiro128));
        assert_eq!(r.tipos_globais.get("u128"), Some(&Tipo::InteiroU128));
        assert_eq!(r.tipos_globais.get("caractere_valor"), Some(&Tipo::Caractere));
    }

    #[test]
    fn rejeita_coercao_implicita_entre_familias_de_tipos() {
        let r = analisar_codigo(
            "variavel texto_invalido: texto = 'a'\n\
             variavel caractere_invalido: caractere = \"a\"\n\
             variavel logico_invalido: logico = 1\n\
             variavel decimal_invalido: decimal64 = 1\n\
             variavel sinal_invalido: inteiro_u64 = -1",
        );
        assert!(
            r.diagnosticos.iter().filter(|d| d.codigo == "N3020").count() >= 5,
            "{:?}",
            r.diagnosticos
        );
    }

    #[test]
    fn preserva_limites_superiores_de_inteiros_de_128_bits() {
        let r = analisar_codigo(
            "variavel maximo: inteiro128 = 170141183460469231731687303715884105727\n\
             variavel maximo_sem_sinal: inteiro_u128 = 340282366920938463463374607431768211455",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }

    #[test]
    fn rejeita_inteiros_de_128_bits_acima_da_faixa() {
        let r = analisar_codigo(
            "variavel excedido: inteiro128 = 170141183460469231731687303715884105728\n\
             variavel excedido_sem_sinal: inteiro_u128 = 340282366920938463463374607431768211456",
        );
        assert!(
            r.diagnosticos.iter().filter(|d| d.codigo == "N3020").count() >= 2,
            "{:?}",
            r.diagnosticos
        );
    }

    #[test]
    fn infere_retorno_generico_a_partir_do_contexto() {
        let r = analisar_codigo_fase5(
            "funcao ausente<T>(): Opcional<T>\\n    devolver Opcional::Nenhum()\\nfim\\nvariavel numero: Opcional<inteiro> = ausente()",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(
            r.tipos_globais.get("numero"),
            Some(&Tipo::Opcional(Box::new(Tipo::Inteiro)))
        );
    }

    #[test]
    fn unifica_parametros_genericos_em_assinaturas_de_funcao() {
        let r = analisar_codigo_fase5(
            "funcao aplicar<T>(acao: funcao(T): T, valor: T): T\\n    devolver acao(valor)\\nfim\\nfuncao identidade(valor: inteiro): inteiro\\n    devolver valor\\nfim\\nvariavel resultado = aplicar(identidade, 42)",
        );
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.tipos_globais.get("resultado"), Some(&Tipo::Inteiro));
    }

    #[test]
    fn rejeita_funcao_generica_sem_instancia_como_valor() {
        let r = analisar_codigo_fase5(
            "funcao identidade<T>(valor: T): T\\n    devolver valor\\nfim\\nvariavel referencia = identidade",
        );
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N3057"), "{:?}", r.diagnosticos);
    }


    #[test]
    fn valida_lambda_com_tipo_contextual() {
        let valido = analisar_codigo(
            "variavel dobrar: funcao(inteiro) -> inteiro = lambda valor: valor + 1",
        );
        assert!(valido.diagnosticos.is_empty(), "{:?}", valido.diagnosticos);
        assert_eq!(
            valido.tipos_globais.get("dobrar"),
            Some(&Tipo::Funcao(vec![Tipo::Inteiro], Box::new(Tipo::Inteiro)))
        );

        let invalido = analisar_codigo(
            "variavel dobrar: funcao(inteiro) -> texto = lambda valor: valor + 1",
        );
        assert!(invalido.diagnosticos.iter().any(|d| d.codigo == "N3020"));
    }

    #[test]
    fn rejeita_lambda_sem_tipo_contextual_para_parametros() {
        let resultado = analisar_codigo("variavel dobrar = lambda valor: valor + 1");
        assert!(resultado.diagnosticos.iter().any(|d| d.codigo == "N3088"));
    }

    #[test]
    fn rejeita_ramos_incompativeis_em_expressao_condicional() {
        let resultado = analisar_codigo(
            "variavel resultado = verdadeiro ? 42 : \"Nebula\"",
        );
        assert!(resultado.diagnosticos.iter().any(|d| d.codigo == "N3067"), "{:?}", resultado.diagnosticos);
    }

    #[test]
    fn valida_argumentos_nomeados_e_parametros_padrao() {
        let resultado = analisar_codigo(
            "funcao saudar(nome: texto = \"Visitante\", vezes: inteiro = 1): vazio\n    passar\nfim\n\
             variavel com_padrao = saudar()\n\
             variavel nomeado = saudar(vezes: 2, nome: \"Lia\")",
        );
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
        assert_eq!(resultado.tipos_globais.get("com_padrao"), Some(&Tipo::Vazio));
    }

    #[test]
    fn rejeita_argumentos_nomeados_invalidos_e_parametros_sem_padrao() {
        let desconhecido = analisar_codigo(
            "funcao saudar(nome: texto): vazio\n    passar\nfim\nsaudar(apelido: \"Lia\")",
        );
        assert!(desconhecido.diagnosticos.iter().any(|d| d.codigo == "N3083"), "{:?}", desconhecido.diagnosticos);

        let duplicado = analisar_codigo(
            "funcao saudar(nome: texto): vazio\n    passar\nfim\nsaudar(nome: \"A\", nome: \"B\")",
        );
        assert!(duplicado.diagnosticos.iter().any(|d| d.codigo == "N3084"), "{:?}", duplicado.diagnosticos);

        let obrigatorio = analisar_codigo(
            "funcao somar(a: inteiro, b: inteiro): inteiro\n    devolver a + b\nfim\nvariavel resultado = somar(a: 1)",
        );
        assert!(obrigatorio.diagnosticos.iter().any(|d| d.codigo == "N3015"), "{:?}", obrigatorio.diagnosticos);
    }

    #[test]
    fn valida_hierarquia_e_campos_herdados_de_classes() {
        let valido = analisar_codigo(
            "classe Animal\n    variavel idade: inteiro = 0\nfim\n\
             classe Cachorro herda Animal\n    funcao atualizar(): vazio\n        isto.idade = 2\n    fim\nfim",
        );
        assert!(valido.diagnosticos.is_empty(), "{:?}", valido.diagnosticos);

        let base_ausente = analisar_codigo("classe Cachorro herda Animal\nfim");
        assert!(base_ausente.diagnosticos.iter().any(|d| d.codigo == "N3069"), "{:?}", base_ausente.diagnosticos);

        let ciclo = analisar_codigo(
            "classe Animal herda Cachorro\nfim\nclasse Cachorro herda Animal\nfim",
        );
        assert!(ciclo.diagnosticos.iter().any(|d| d.codigo == "N3070"), "{:?}", ciclo.diagnosticos);
    }

    #[test]
    fn valida_contextos_e_atributos_de_codigo_nativo() {
        let valido = analisar_codigo(
            "@alinhamento(8)\n@representacao(\"C\")\nestrutura Cabecalho\n    codigo: u32\nfim\n\
             critico\n    passar\nfim\n\
             privilegio sistema\n    passar\nfim\n\
             montagem x86_64<u32>\n    passar\nfim",
        );
        assert!(valido.diagnosticos.is_empty(), "{:?}", valido.diagnosticos);

        let alinhamento = analisar_codigo(
            "@alinhamento(3)\nestrutura Cabecalho\n    codigo: u32\nfim",
        );
        assert!(alinhamento.diagnosticos.iter().any(|d| d.codigo == "N3075"), "{:?}", alinhamento.diagnosticos);

        let contexto = analisar_codigo("privilegio usuario\n    passar\nfim");
        assert!(contexto.diagnosticos.iter().any(|d| d.codigo == "N3072"), "{:?}", contexto.diagnosticos);

        let atributo = analisar_codigo("@nao_reconhecido\nestrutura Dados\n    valor: inteiro\nfim");
        assert!(atributo.diagnosticos.iter().any(|d| d.codigo == "N3078"), "{:?}", atributo.diagnosticos);
    }


    #[test]
    fn resolve_metodos_de_classes_e_metodos_herdados() {
        let resultado = analisar_codigo(
            "classe Animal\n    funcao falar(mensagem: texto = \"Oi\"): texto\n        devolver mensagem\n    fim\nfim\n\
             classe Cachorro herda Animal\n    funcao testar(): texto\n        devolver isto.falar(mensagem: \"Olá\")\n    fim\nfim",
        );
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
    }

    #[test]
    fn rejeita_sobrescrita_com_assinatura_incompativel() {
        let resultado = analisar_codigo(
            "classe Animal\n    funcao falar(mensagem: texto): texto\n        devolver mensagem\n    fim\nfim\n\
             classe Cachorro herda Animal\n    funcao falar(mensagem: inteiro): texto\n        devolver \"Olá\"\n    fim\nfim",
        );
        assert!(resultado.diagnosticos.iter().any(|d| d.codigo == "N3087"), "{:?}", resultado.diagnosticos);
    }

    #[test]
    fn valida_funcao_externa_com_convencao() {
        let resultado = analisar_codigo(
            "@convencao(\"C\")\nexterna funcao escreverByte(valor: u8): vazio\nescreverByte(255)",
        );
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
    }


    #[test]
    fn associa_diagnosticos_semanticos_a_intervalos_da_fonte() {
        let resultado = analisar_codigo("variavel valor: inteiro = verdadeiro");
        let diagnostico = resultado.diagnosticos.iter().find(|d| d.codigo == "N3020").expect("diagnóstico de tipo");
        assert!(diagnostico.intervalo.is_some(), "diagnóstico precisa apontar para a origem: {diagnostico:?}");
    }


}

}

pub use implementacao::*;
