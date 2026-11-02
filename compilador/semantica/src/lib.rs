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
    Vazio, Logico, Inteiro, Inteiro8, Inteiro16, Inteiro32, Inteiro64, Inteiro128,
    Decimal, Decimal32, Decimal64, Texto, Caractere, Byte,
    Lista(Box<Tipo>), Intervalo(Box<Tipo>), Funcao(Vec<Tipo>, Box<Tipo>), Desconhecido,
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
struct Simbolo { tipo: Tipo, mutavel: bool }

#[derive(Debug, Clone)]
struct Funcao { parametros: Vec<Tipo>, retorno: Tipo }

struct Analisador<'a> {
    escopos: Vec<HashMap<String, Simbolo>>,
    funcoes: HashMap<String, Funcao>,
    aliases: HashMap<String, String>,
    modulos: &'a ModulosDisponiveis,
    modulos_importados: HashMap<String, String>,
    diagnosticos: Vec<Diagnostico>,
    retorno_atual: Option<Tipo>,
    profundidade_laco: usize,
    funcoes_vistas: HashSet<String>,
}

impl<'a> Analisador<'a> {
    fn novo(modulos: &'a ModulosDisponiveis) -> Self {
        Self {
            escopos: vec![HashMap::new()],
            funcoes: HashMap::new(),
            aliases: HashMap::new(),
            modulos,
            modulos_importados: HashMap::new(),
            diagnosticos: Vec::new(),
            retorno_atual: None,
            profundidade_laco: 0,
            funcoes_vistas: HashSet::new(),
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
        }
        let aliases: Vec<String> = self.aliases.keys().cloned().collect();
        for nome in aliases {
            if self.resolver_tipo(&nome) == Tipo::Desconhecido {
                self.erro("N3003", format!("O alias de tipo '{nome}' contém referência inválida ou ciclo."));
            }
        }
        for item in &programa.itens {
            if let NoAst::Funcao { nome, parametros, retorno, .. } = item {
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
                if self.funcoes.contains_key(nome) {
                    self.erro("N3001", format!("A função '{nome}' foi declarada mais de uma vez."));
                } else {
                    self.funcoes.insert(nome.clone(), Funcao { parametros: tipos, retorno });
                }
            }
        }
    }

    fn item(&mut self, item: &NoAst) {
        match item {
            NoAst::Variavel { nome, tipo, valor, constante } => {
                let inferido = self.expressao(valor);
                let final_tipo = if let Some(tipo) = tipo {
                    let esperado = self.resolver_tipo(tipo);
                    if esperado == Tipo::Desconhecido {
                        self.erro("N3021", format!("Tipo '{tipo}' não foi declarado."));
                    }
                    self.exigir_tipo(&inferido, &esperado, "inicialização da variável");
                    esperado
                } else { inferido };
                self.declarar(nome, Simbolo { tipo: final_tipo, mutavel: !constante });
            }
            NoAst::Funcao { nome, parametros, corpo, .. } => {
                if !self.funcoes_vistas.insert(nome.clone()) { return; }
                let Some(funcao) = self.funcoes.get(nome).cloned() else {
                    self.erro("N3023", format!("Função aninhada '{nome}' não é permitida no escopo inicial."));
                    return;
                };
                self.abrir_escopo();
                for ((nome_parametro, _), tipo) in parametros.iter().zip(&funcao.parametros) {
                    self.declarar(nome_parametro, Simbolo { tipo: tipo.clone(), mutavel: true });
                }
                let retorno_anterior = self.retorno_atual.replace(funcao.retorno);
                let laco_anterior = self.profundidade_laco;
                self.profundidade_laco = 0;
                for instrucao in corpo { self.item(instrucao); }
                if funcao.retorno != Tipo::Vazio && !Self::garante_retorno(corpo) {
                    self.erro("N3024", format!("A função '{nome}' pode terminar sem devolver um valor."));
                }
                self.profundidade_laco = laco_anterior;
                self.retorno_atual = retorno_anterior;
                self.fechar_escopo();
            }
            NoAst::Tipo { .. } => {
                if self.escopos.len() > 1 {
                    self.erro("N3025", "Aliases de tipo só podem ser declarados no escopo do arquivo.");
                }
            }
            NoAst::Incluir { .. } => {
                if self.escopos.len() > 1 {
                    self.erro("N3026", "Módulos só podem ser incluídos no escopo do arquivo.");
                }
            }
            NoAst::Expressao(expressao) => { self.expressao(expressao); }
            NoAst::Quando { condicao, entao, senao } => {
                let tipo = self.expressao(condicao);
                self.exigir_tipo(&tipo, &Tipo::Logico, "condição de 'quando'");
                self.bloco(entao);
                self.bloco(senao);
            }
            NoAst::Enquanto { condicao, corpo } => {
                let tipo = self.expressao(condicao);
                self.exigir_tipo(&tipo, &Tipo::Logico, "condição de 'enquanto'");
                self.profundidade_laco += 1;
                self.bloco(corpo);
                self.profundidade_laco -= 1;
            }
            NoAst::Para { inicial, condicao, atualizacao, corpo } => {
                self.abrir_escopo();
                if let Some(inicial) = inicial { self.item(inicial); }
                if let Some(condicao) = condicao {
                    let tipo = self.expressao(condicao);
                    self.exigir_tipo(&tipo, &Tipo::Logico, "condição de 'para'");
                }
                self.profundidade_laco += 1;
                self.bloco(corpo);
                self.profundidade_laco -= 1;
                if let Some(atualizacao) = atualizacao { self.expressao(atualizacao); }
                self.fechar_escopo();
            }
            NoAst::Devolver(valor) => {
                let esperado = self.retorno_atual.clone();
                if esperado.is_none() {
                    self.erro("N3005", "'devolver' só pode ser usado dentro de uma função.");
                }
                let obtido = valor.as_ref().map_or(Tipo::Vazio, |e| self.expressao(e));
                if let Some(esperado) = esperado {
                    self.exigir_tipo(&obtido, &esperado, "retorno da função");
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
            Expressao::Nome(nome) => self.buscar(nome).map(|s| s.tipo.clone()).unwrap_or_else(|| {
                if let Some(f) = self.funcoes.get(nome) {
                    Tipo::Funcao(f.parametros.clone(), Box::new(f.retorno.clone()))
                } else {
                    self.erro("N3007", format!("Nome '{nome}' não foi declarado."));
                    Tipo::Desconhecido
                }
            }),
            Expressao::Literal(valor) => Self::tipo_literal(valor),
            Expressao::Lista(valores) => {
                if valores.is_empty() { return Tipo::Lista(Box::new(Tipo::Desconhecido)); }
                let primeiro = self.expressao(&valores[0]);
                for valor in valores.iter().skip(1) {
                    let atual = self.expressao(valor);
                    self.exigir_tipo(&atual, &primeiro, "elementos da lista");
                }
                Tipo::Lista(Box::new(primeiro))
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
                            self.exigir_tipo(&tipo_valor, &simbolo.tipo, "atribuição");
                            simbolo.tipo
                        } else {
                            self.erro("N3007", format!("Nome '{nome}' não foi declarado."));
                            Tipo::Desconhecido
                        }
                    }
                    _ => {
                        self.erro("N3010", "O destino da atribuição precisa ser uma variável modificável.");
                        Tipo::Desconhecido
                    }
                }
            }
            Expressao::Chamada { alvo, argumentos } => {
                if let Expressao::Nome(nome) = alvo.as_ref() {
                    if nome == "exibir" {
                        for argumento in argumentos { self.expressao(argumento); }
                        return Tipo::Vazio;
                    }
                    if let Some(funcao) = self.funcoes.get(nome).cloned() {
                        self.validar_argumentos(argumentos, &funcao.parametros, nome);
                        return funcao.retorno;
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
                self.exigir_tipo(&tipo_indice, &Tipo::Inteiro, "índice de lista");
                if let Tipo::Lista(elemento) = tipo_alvo { *elemento } else {
                    self.erro("N3013", "Indexação exige uma lista.");
                    Tipo::Desconhecido
                }
            }
            Expressao::Membro { operador, .. } => {
                if *operador == TipoToken::Ponto {
                    self.erro("N3014", "Acesso a membros de valores ainda não está definido no subconjunto semântico.");
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
                self.exigir_tipo(&obtido, esperado, "argumento de função");
            }
        }
    }

    fn binaria(&mut self, a: &Tipo, op: TipoToken, b: &Tipo) -> Tipo {
        use TipoToken::{Diferente, Divide, E, Igual, Intervalo, Maior, MaiorIgual, Menor, MenorIgual, Menos, Mais, Ou, Potencia, Resto, Vezes};
        match op {
            Mais if a == &Tipo::Texto && b == &Tipo::Texto => Tipo::Texto,
            Mais | Menos | Vezes | Divide | Resto | Potencia => {
                if Self::numerico(a) && Self::numerico(b) {
                    Self::promover_numerico(a, b)
                } else {
                    self.erro("N3016", "Operação aritmética exige operandos numéricos compatíveis.");
                    Tipo::Desconhecido
                }
            }
            Menor | MenorIgual | Maior | MaiorIgual => {
                if Self::numerico(a) && Self::numerico(b) { Tipo::Logico } else {
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
            _ => Tipo::Inteiro,
        }
    }

    fn resolver_tipo(&self, nome: &str) -> Tipo {
        match nome {
            "vazio" => Tipo::Vazio,
            "logico" | "lógico" => Tipo::Logico,
            "inteiro" => Tipo::Inteiro,
            "inteiro8" => Tipo::Inteiro8,
            "inteiro16" => Tipo::Inteiro16,
            "inteiro32" => Tipo::Inteiro32,
            "inteiro64" => Tipo::Inteiro64,
            "inteiro128" => Tipo::Inteiro128,
            "decimal" => Tipo::Decimal64,
            "decimal32" => Tipo::Decimal32,
            "decimal64" => Tipo::Decimal64,
            "texto" => Tipo::Texto,
            "caractere" => Tipo::Caractere,
            "byte" => Tipo::Byte,
            _ if nome.starts_with('[') && nome.ends_with(']') => {
                Tipo::Lista(Box::new(self.resolver_tipo(&nome[1..nome.len()-1])))
            }
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

    fn declarar(&mut self, nome: &str, simbolo: Simbolo) {
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

    fn exigir_tipo(&mut self, obtido: &Tipo, esperado: &Tipo, contexto: &str) {
        if obtido != esperado && *obtido != Tipo::Desconhecido && *esperado != Tipo::Desconhecido {
            self.erro("N3020", format!("Tipo incompatível em {contexto}: esperado {}, recebido {}.", esperado.nome(), obtido.nome()));
        }
    }

    fn numerico(tipo: &Tipo) -> bool {
        matches!(tipo, Tipo::Inteiro | Tipo::Inteiro8 | Tipo::Inteiro16 | Tipo::Inteiro32 | Tipo::Inteiro64 | Tipo::Inteiro128 | Tipo::Decimal | Tipo::Decimal32 | Tipo::Decimal64 | Tipo::Byte)
    }

    fn promover_numerico(a: &Tipo, b: &Tipo) -> Tipo {
        if matches!(a, Tipo::Decimal64 | Tipo::Decimal) || matches!(b, Tipo::Decimal64 | Tipo::Decimal) {
            return Tipo::Decimal64;
        }
        if a == &Tipo::Decimal32 || b == &Tipo::Decimal32 { return Tipo::Decimal32; }
        let largura = |tipo: &Tipo| match tipo {
            Tipo::Inteiro8 | Tipo::Byte => 8,
            Tipo::Inteiro16 => 16,
            Tipo::Inteiro32 => 32,
            Tipo::Inteiro64 | Tipo::Inteiro => 64,
            Tipo::Inteiro128 => 128,
            _ => 64,
        };
        match largura(a).max(largura(b)) {
            8 => Tipo::Inteiro8,
            16 => Tipo::Inteiro16,
            32 => Tipo::Inteiro32,
            128 => Tipo::Inteiro128,
            _ => Tipo::Inteiro,
        }
    }

    fn erro(&mut self, codigo: &'static str, mensagem: impl Into<String>) {
        self.diagnosticos.push(Diagnostico::novo(codigo, Severidade::Erro, mensagem, None));
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
            Self::Decimal => "decimal".into(), Self::Decimal32 => "decimal32".into(),
            Self::Decimal64 => "decimal64".into(),
            Self::Texto => "texto".into(), Self::Caractere => "caractere".into(),
            Self::Byte => "byte".into(), Self::Lista(t) => format!("[{}]", t.nome()),
            Self::Intervalo(t) => format!("intervalo<{}>", t.nome()),
            Self::Funcao(_, _) => "função".into(), Self::Desconhecido => "desconhecido".into(),
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

}
}

pub use implementacao::*;
