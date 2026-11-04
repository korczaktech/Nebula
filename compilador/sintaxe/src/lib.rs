//! Lexer, AST e Parser do subconjunto inicial da linguagem Nebula.

#![forbid(unsafe_code)]
#![allow(missing_docs)]

pub mod lexer;

#[rustfmt::skip]
mod implementacao {
//! Lexer, AST e Parser do subconjunto inicial da linguagem Nebula.

use nebula_diagnosticos::{Diagnostico, Severidade};
use nebula_fonte::ArquivoFonte;
use crate::lexer::{analisar_lexico, TipoToken, Token};
use unicode_normalization::UnicodeNormalization;

/// Tipos de nós da AST, independentes de detalhes de formatação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expressao {
    Nome(String), Literal(String), Lista(Vec<Expressao>), Tupla(Vec<Expressao>),
    Unaria { operador: TipoToken, valor: Box<Expressao> },
    Binaria { esquerda: Box<Expressao>, operador: TipoToken, direita: Box<Expressao> },
    Atribuicao { destino: Box<Expressao>, valor: Box<Expressao> },
    Chamada { alvo: Box<Expressao>, argumentos: Vec<Expressao> },
    Indexacao { alvo: Box<Expressao>, indice: Box<Expressao> },
    Membro { alvo: Box<Expressao>, operador: TipoToken, nome: String },
    ConstrucaoEstrutura { nome: String, campos: Vec<(String, Expressao)> },
}
/// Visibilidade declarada para uma função.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibilidade { Publico, Privado }
/// Caso de correspondência em uma instrução escolha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CasoEscolha { pub enumeracao: Option<String>, pub variante: Option<String>, pub vinculacoes: Vec<String>, pub corpo: Vec<NoAst> }
/// Declarações e instruções representadas na AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoAst {
    Variavel { nome: String, tipo: Option<String>, valor: Expressao, constante: bool },
    Funcao { visibilidade: Option<Visibilidade>, nome: String, genericos: Vec<String>, parametros: Vec<(String, String)>, retorno: Option<String>, corpo: Vec<NoAst> },
    Tipo { nome: String, destino: String },
    Estrutura { nome: String, campos: Vec<(String, String)> },
    Enumeracao { nome: String, variantes: Vec<(String, Vec<String>)> },
    Incluir { caminho: String, apelido: Option<String> },
    Expressao(Expressao), Quando { condicao: Expressao, entao: Vec<NoAst>, senao: Vec<NoAst> },
    Escolha { valor: Expressao, casos: Vec<CasoEscolha> },
    Enquanto { condicao: Expressao, corpo: Vec<NoAst> },
    Cada { nome: String, iteravel: Expressao, corpo: Vec<NoAst> },
    Pulso { intervalo: Expressao, corpo: Vec<NoAst> },
    Para { inicial: Option<Box<NoAst>>, condicao: Option<Expressao>, atualizacao: Option<Expressao>, corpo: Vec<NoAst> },
    Devolver(Option<Expressao>), Parar, Seguir,
}
/// Programa analisado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Programa { pub itens: Vec<NoAst> }
/// Resultado sintático com AST parcial e todos os diagnósticos coletados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultadoSintatico { pub programa: Programa, pub diagnosticos: Vec<Diagnostico> }

/// Constrói a AST a partir de uma fonte Nebula. Erros léxicos são preservados.
#[must_use]
pub fn analisar(fonte: &ArquivoFonte) -> ResultadoSintatico {
    let lexico = analisar_lexico(fonte);
    let mut parser = Parser::novo(lexico.tokens);
    let programa = parser.programa();
    let mut diagnosticos = lexico.diagnosticos;
    diagnosticos.extend(parser.diagnosticos);
    ResultadoSintatico { programa, diagnosticos }
}

const LIMITE_PROFUNDIDADE_PARSER: usize = 256;

struct Parser { tokens: Vec<Token>, atual: usize, profundidade: usize, diagnosticos: Vec<Diagnostico> }
impl Parser {
    fn novo(tokens: Vec<Token>) -> Self { Self { tokens, atual: 0, profundidade: 0, diagnosticos: Vec::new() } }
    fn com_profundidade<T>(&mut self, analisar: impl FnOnce(&mut Self) -> Option<T>) -> Option<T> {
        if self.profundidade >= LIMITE_PROFUNDIDADE_PARSER {
            self.erro_atual("N2049", "Limite de aninhamento sintático excedido (256 níveis). Reduza o aninhamento.");
            return None;
        }
        self.profundidade += 1;
        let resultado = analisar(self);
        self.profundidade -= 1;
        resultado
    }
    fn programa(&mut self) -> Programa {
        let mut itens = Vec::new(); self.separadores();
        while !self.verifica(TipoToken::Fim) {
            let antes = self.atual;
            if let Some(no) = self.item() { itens.push(no); }
            if self.atual == antes { self.avancar(); }
            if !self.verifica(TipoToken::Fim) && !self.verifica(TipoToken::FechaChave) && !self.verifica(TipoToken::FimBloco) && !self.verifica(TipoToken::Senao) && !self.separador_atual() {
                self.erro_atual("N2001", "Esperado quebra de linha ou ';' entre instruções.");
                self.sincronizar();
            }
            self.separadores();
        }
        Programa { itens }
    }
    fn item(&mut self) -> Option<NoAst> {
        match self.tipo() {
            TipoToken::Variavel | TipoToken::Definir | TipoToken::Mutavel => self.declaracao_variavel(false),
            TipoToken::Constante => self.declaracao_variavel(true),
            TipoToken::Alterar => {
                self.avancar();
                let expressao = self.expressao(0)?;
                if !matches!(expressao, Expressao::Atribuicao { .. }) {
                    self.erro_atual("N2047", "'alterar' exige uma atribuição, por exemplo: alterar total <- 10.");
                }
                Some(NoAst::Expressao(expressao))
            }
            TipoToken::Funcao => self.funcao(),
            TipoToken::Publico | TipoToken::Privado => {
                let visibilidade = if self.avancar().tipo == TipoToken::Publico {
                    Visibilidade::Publico
                } else {
                    Visibilidade::Privado
                };
                if self.corresponde(TipoToken::Funcao) {
                    self.funcao_corpo(Some(visibilidade))
                } else {
                    self.erro_atual("N2002", "Esperado 'funcao' após modificador de visibilidade.");
                    None
                }
            }
            TipoToken::Tipo => self.tipo_alias(),
            TipoToken::Estrutura => self.estrutura(),
            TipoToken::Enumeracao => self.enumeracao(),
            TipoToken::ReservadaFutura => {
                let token = self.avancar();
                self.diagnosticos.push(Diagnostico::novo(
                    "N2016",
                    Severidade::Erro,
                    format!("A construção '{}' está reservada para uma versão futura e ainda não possui implementação.", token.lexema),
                    Some(token.intervalo),
                ));
                None
            }
            TipoToken::Incluir => self.incluir(),
            TipoToken::Quando => self.quando(),
            TipoToken::Escolha | TipoToken::Avaliar => self.escolha(),
            TipoToken::Enquanto | TipoToken::Durante | TipoToken::Repetir => self.enquanto(),
            TipoToken::Pulso => self.pulso(),
            TipoToken::Para => self.para(),
            TipoToken::Cada => self.cada(),
            TipoToken::Devolver => { self.avancar(); let e = if self.fim_item() { None } else { self.expressao(0) }; Some(NoAst::Devolver(e)) }
            TipoToken::Parar | TipoToken::Encerrar => { self.avancar(); Some(NoAst::Parar) }
            TipoToken::Seguir => { self.avancar(); Some(NoAst::Seguir) }
            _ => self.expressao(0).map(NoAst::Expressao),
        }
    }
    fn declaracao_variavel(&mut self, constante: bool) -> Option<NoAst> {
        self.avancar(); let nome = self.consumir_nome("Esperado nome após declaração.")?;
        let tipo = if self.corresponde(TipoToken::DoisPontos) { Some(self.caminho_tipo()?) } else { None };
        if !self.corresponde(TipoToken::Atribui) { self.erro_atual("N2003", "Esperado '=' na declaração."); return None; }
        self.pular_novas_linhas();
        let valor = self.expressao(0)?;
        Some(NoAst::Variavel { nome, tipo, valor, constante })
    }
    fn funcao(&mut self) -> Option<NoAst> { self.avancar(); self.funcao_corpo(None) }
    fn funcao_corpo(&mut self, visibilidade: Option<Visibilidade>) -> Option<NoAst> {
        let nome = self.consumir_nome("Esperado nome da função.")?;
        let mut genericos = Vec::new();
        if self.corresponde(TipoToken::Menor) {
            loop {
                let parametro = self.consumir_nome("Esperado nome de parâmetro genérico.")?;
                if genericos.contains(&parametro) {
                    self.erro_atual("N2014", "Parâmetro genérico duplicado.");
                } else {
                    genericos.push(parametro);
                }
                if !self.corresponde(TipoToken::Virgula) { break; }
            }
            self.esperar(TipoToken::Maior, "Esperado '>' após parâmetros genéricos.");
            if genericos.is_empty() {
                self.erro_atual("N2015", "Uma função genérica precisa declarar ao menos um parâmetro de tipo.");
            }
        }
        if !self.corresponde(TipoToken::AbrePar) { self.erro_atual("N2004", "Esperado '(' após nome da função."); return None; }
        let mut parametros = Vec::new();
        if !self.verifica(TipoToken::FechaPar) {
            loop {
                let n = self.consumir_nome("Esperado nome do parâmetro.")?;
                if !self.corresponde(TipoToken::DoisPontos) { self.erro_atual("N2005", "Esperado ':' no parâmetro."); return None; }
                let t = self.caminho_tipo()?; parametros.push((n,t));
                if !self.corresponde(TipoToken::Virgula) { break; }
            }
        }
        self.esperar(TipoToken::FechaPar, "Esperado ')' após parâmetros.");
        let retorno = if self.corresponde(TipoToken::DoisPontos) { Some(self.caminho_tipo()?) } else { None };
        let corpo = self.bloco()?;
        Some(NoAst::Funcao { visibilidade, nome, genericos, parametros, retorno, corpo })
    }
    fn tipo_alias(&mut self) -> Option<NoAst> {
        self.avancar(); let nome = self.consumir_nome("Esperado nome após 'tipo'.")?;
        if !self.corresponde(TipoToken::Atribui) { self.erro_atual("N2006", "Esperado '=' na declaração de tipo."); return None; }
        self.pular_novas_linhas();
        Some(NoAst::Tipo { nome, destino: self.caminho_tipo()? })
    }
    fn estrutura(&mut self) -> Option<NoAst> {
        self.avancar();
        let nome = self.consumir_nome("Esperado nome após 'estrutura'.")?;
        if !self.corresponde(TipoToken::AbreChave) {
            self.erro_atual("N2030", "Esperado '{' após o nome da estrutura.");
            return None;
        }
        self.separadores();
        let mut campos = Vec::new();
        while !self.verifica(TipoToken::FechaChave) && !self.verifica(TipoToken::Fim) {
            let campo = self.consumir_nome("Esperado nome do campo da estrutura.")?;
            self.esperar(TipoToken::DoisPontos, "Esperado ':' após nome do campo.");
            let tipo = self.caminho_tipo()?;
            if campos.iter().any(|(existente, _)| existente == &campo) {
                self.erro_atual("N2031", "Campo declarado mais de uma vez na estrutura.");
            } else {
                campos.push((campo, tipo));
            }
            if !self.corresponde(TipoToken::Virgula) && !self.separador_atual()
                && !self.verifica(TipoToken::FechaChave) {
                self.erro_atual("N2032", "Esperado ',' ou quebra de linha entre campos.");
                self.sincronizar();
            }
            self.separadores();
        }
        self.esperar(TipoToken::FechaChave, "Estrutura não foi fechada com '}'.");
        Some(NoAst::Estrutura { nome, campos })
    }

    fn enumeracao(&mut self) -> Option<NoAst> {
        self.avancar();
        let nome = self.consumir_nome("Esperado nome após 'enumeracao'.")?;
        if !self.corresponde(TipoToken::AbreChave) {
            self.erro_atual("N2033", "Esperado '{' após o nome da enumeração.");
            return None;
        }
        self.separadores();
        let mut variantes: Vec<(String, Vec<String>)> = Vec::new();
        while !self.verifica(TipoToken::FechaChave) && !self.verifica(TipoToken::Fim) {
            let variante = self.consumir_nome("Esperado nome da variante.")?;
            let mut payload = Vec::new();
            if self.corresponde(TipoToken::AbrePar) {
                if !self.verifica(TipoToken::FechaPar) {
                    loop {
                        payload.push(self.caminho_tipo()?);
                        if !self.corresponde(TipoToken::Virgula) { break; }
                    }
                }
                self.esperar(TipoToken::FechaPar, "Esperado ')' após tipos da variante.");
            }
            if variantes.iter().any(|(existente, _)| existente == &variante) {
                self.erro_atual("N2034", "Variante declarada mais de uma vez na enumeração.");
            } else {
                variantes.push((variante, payload));
            }
            if !self.corresponde(TipoToken::Virgula) && !self.separador_atual()
                && !self.verifica(TipoToken::FechaChave) {
                self.erro_atual("N2035", "Esperado ',' ou quebra de linha entre variantes.");
                self.sincronizar();
            }
            self.separadores();
        }
        self.esperar(TipoToken::FechaChave, "Enumeração não foi fechada com '}'.");
        if variantes.is_empty() {
            self.erro_atual("N2036", "Uma enumeração precisa declarar ao menos uma variante.");
        }
        Some(NoAst::Enumeracao { nome, variantes })
    }

    fn incluir(&mut self) -> Option<NoAst> {
        self.avancar(); let caminho = self.caminho_modulo()?;
        let apelido = if self.corresponde(TipoToken::Como) { self.consumir_nome("Esperado apelido após 'como'.") } else { None };
        Some(NoAst::Incluir { caminho, apelido })
    }
    fn quando(&mut self) -> Option<NoAst> {
        self.com_profundidade(|parser| parser.quando_interno())
    }
    fn quando_interno(&mut self) -> Option<NoAst> {
        self.avancar(); let condicao = self.expressao(0)?; let entao = self.bloco()?;
        let mut senao = Vec::new();
        let proximo_significativo = self.tokens.iter().skip(self.atual)
            .find(|token| token.tipo != TipoToken::NovaLinha)
            .map(|token| token.tipo);
        if proximo_significativo == Some(TipoToken::Senao) {
            self.pular_novas_linhas();
        }
        if self.corresponde(TipoToken::Senao) {
            if self.verifica(TipoToken::Quando) { senao.push(self.quando()?); }
            else { senao = self.bloco()?; }
        }
        Some(NoAst::Quando { condicao, entao, senao })
    }
    fn escolha(&mut self) -> Option<NoAst> {
        self.avancar();
        let valor = self.expressao(0)?;
        if !self.corresponde(TipoToken::AbreChave) {
            self.erro_atual("N2040", "Esperado '{' após expressão de escolha.");
            return None;
        }
        self.separadores();
        let mut casos = Vec::new();
        let mut possui_curinga = false;
        let mut padroes_vistos: Vec<(String, String)> = Vec::new();
        while !self.verifica(TipoToken::FechaChave) && !self.verifica(TipoToken::Fim) {
            if !self.corresponde(TipoToken::Caso) {
                self.erro_atual("N2041", "Esperado 'caso' dentro de escolha.");
                self.sincronizar();
                self.separadores();
                continue;
            }
            let mut padrao_valido = true;
            let (enumeracao, variante, vinculacoes, curinga) =
                if self.verifica(TipoToken::Identificador)
                    && self.tokens.get(self.atual).is_some_and(|token| token.lexema == "_")
                {
                    self.avancar();
                    (None, None, Vec::new(), true)
                } else if self.corresponde(TipoToken::Identificador) {
                    let primeiro = self.tokens[self.atual - 1].lexema.nfc().collect::<String>();
                    if self.corresponde(TipoToken::Escopo) {
                        if let Some(variante) = self.consumir_nome("Esperado variante após '::'.") {
                            let mut vinculacoes = Vec::new();
                            if self.corresponde(TipoToken::AbrePar) {
                                if !self.verifica(TipoToken::FechaPar) {
                                    loop {
                                        let vinculacao = if self.verifica(TipoToken::Identificador) {
                                            Some(self.avancar().lexema.nfc().collect::<String>())
                                        } else if matches!(self.tipo(), TipoToken::ReservadaFutura | TipoToken::Estrutura | TipoToken::Enumeracao) {
                                            self.consumir_nome("Esperado nome para payload.");
                                            None
                                        } else {
                                            self.erro_atual("N2045", "Esperado identificador no payload do padrão.");
                                            None
                                        };
                                        if let Some(nome) = vinculacao {
                                            if vinculacoes.contains(&nome) {
                                                self.erro_atual("N2046", "Vínculo repetido no payload do padrão.");
                                                padrao_valido = false;
                                            } else {
                                                vinculacoes.push(nome);
                                            }
                                        } else {
                                            padrao_valido = false;
                                            while !self.verifica(TipoToken::Fim)
                                                && !self.verifica(TipoToken::Virgula)
                                                && !self.verifica(TipoToken::FechaPar)
                                                && !self.verifica(TipoToken::AbreChave)
                                            {
                                                self.avancar();
                                            }
                                        }
                                        if !self.corresponde(TipoToken::Virgula) {
                                            break;
                                        }
                                        if self.verifica(TipoToken::FechaPar) {
                                            self.erro_atual("N2045", "Esperado identificador após vírgula no payload.");
                                            padrao_valido = false;
                                            break;
                                        }
                                    }
                                }
                                if !self.corresponde(TipoToken::FechaPar) {
                                    self.erro_atual("N2009", "Esperado ')' após vínculos do caso.");
                                    padrao_valido = false;
                                    while !self.verifica(TipoToken::Fim)
                                        && !self.verifica(TipoToken::FechaPar)
                                        && !self.verifica(TipoToken::AbreChave)
                                        && !self.verifica(TipoToken::FechaChave)
                                    {
                                        self.avancar();
                                    }
                                    self.corresponde(TipoToken::FechaPar);
                                }
                            }
                            (Some(primeiro), Some(variante), vinculacoes, false)
                        } else {
                            padrao_valido = false;
                            (Some(primeiro), None, Vec::new(), false)
                        }
                    } else {
                        self.erro_atual("N2042", "Padrão deve ser uma variante Enum::Variante ou '_'.");
                        padrao_valido = false;
                        (None, None, Vec::new(), false)
                    }
                } else {
                    self.erro_atual("N2042", "Esperado padrão de enumeração ou '_'.");
                    padrao_valido = false;
                    (None, None, Vec::new(), false)
                };

            if curinga {
                if possui_curinga {
                    self.erro_atual("N2043", "Caso curinga duplicado.");
                    padrao_valido = false;
                }
                possui_curinga = true;
                if self.verifica(TipoToken::AbrePar) {
                    self.erro_atual("N2042", "O padrão curinga '_' não aceita payload.");
                    padrao_valido = false;
                }
            }

            if self.verifica(TipoToken::Escopo) {
                self.erro_atual("N2042", "Um padrão deve conter exatamente Enum::Variante.");
                padrao_valido = false;
            }
            if let (Some(enumeracao), Some(variante)) = (&enumeracao, &variante) {
                let identidade = (enumeracao.clone(), variante.clone());
                if padrao_valido && padroes_vistos.contains(&identidade) {
                    self.erro_atual("N2048", "Padrão de enumeração duplicado em escolha.");
                    padrao_valido = false;
                } else if padrao_valido {
                    padroes_vistos.push(identidade);
                }
            }
            if !self.verifica(TipoToken::AbreChave) && !self.verifica(TipoToken::NovaLinha) {
                while !self.verifica(TipoToken::Fim)
                    && !self.verifica(TipoToken::AbreChave)
                    && !self.verifica(TipoToken::NovaLinha)
                    && !self.verifica(TipoToken::FechaChave)
                    && !self.verifica(TipoToken::Caso)
                {
                    self.avancar();
                }
            }
            let corpo = match self.bloco() {
                Some(corpo) => corpo,
                None => {
                    if !self.separador_atual()
                        && !self.verifica(TipoToken::FechaChave)
                        && !self.verifica(TipoToken::Fim)
                        && !self.verifica(TipoToken::Caso)
                    {
                        self.sincronizar();
                    }
                    self.separadores();
                    continue;
                }
            };
            if padrao_valido {
                casos.push(CasoEscolha { enumeracao, variante, vinculacoes, corpo });
            }
            self.separadores();
        }
        self.esperar(TipoToken::FechaChave, "Escolha não foi fechada com '}'.");
        if casos.is_empty() { self.erro_atual("N2044", "Escolha precisa conter ao menos um caso."); }
        Some(NoAst::Escolha { valor, casos })
    }
    fn enquanto(&mut self) -> Option<NoAst> {
        self.avancar(); let condicao = self.expressao(0)?; let corpo = self.bloco()?;
        Some(NoAst::Enquanto { condicao, corpo })
    }
    fn cada(&mut self) -> Option<NoAst> {
        self.avancar();
        let nome = self.consumir_nome("Esperado nome da variável após 'cada'.")?;
        self.esperar(TipoToken::Em, "Esperado 'em' após o nome da variável de 'cada'.");
        let iteravel = self.expressao(0)?;
        let corpo = self.bloco()?;
        Some(NoAst::Cada { nome, iteravel, corpo })
    }

    fn pulso(&mut self) -> Option<NoAst> {
        self.avancar();
        self.esperar(TipoToken::AbrePar, "Esperado '(' após 'pulso'.");
        let intervalo = self.expressao(0)?;
        self.esperar(TipoToken::FechaPar, "Esperado ')' após o intervalo de 'pulso'.");
        let corpo = self.bloco()?;
        Some(NoAst::Pulso { intervalo, corpo })
    }

    fn para(&mut self) -> Option<NoAst> {
        self.avancar(); self.esperar(TipoToken::AbrePar, "Esperado '(' após 'para'.");
        let inicial = if self.verifica(TipoToken::PontoVirgula) { None } else if self.verifica(TipoToken::Variavel) { self.declaracao_variavel(false).map(Box::new) } else { self.expressao(0).map(|e| Box::new(NoAst::Expressao(e))) };
        self.esperar(TipoToken::PontoVirgula, "Esperado ';' após inicialização do laço.");
        let condicao = if self.verifica(TipoToken::PontoVirgula) { None } else { self.expressao(0) };
        self.esperar(TipoToken::PontoVirgula, "Esperado ';' após condição do laço.");
        let atualizacao = if self.verifica(TipoToken::FechaPar) { None } else { self.expressao(0) };
        self.esperar(TipoToken::FechaPar, "Esperado ')' após cabeçalho do laço.");
        let corpo = self.bloco()?;
        Some(NoAst::Para { inicial, condicao, atualizacao, corpo })
    }
    fn bloco(&mut self) -> Option<Vec<NoAst>> {
        self.com_profundidade(|parser| parser.bloco_interno())
    }
    fn bloco_interno(&mut self) -> Option<Vec<NoAst>> {
        if self.verifica(TipoToken::NovaLinha) {
            let proximo = self.tokens.iter().skip(self.atual)
                .find(|token| token.tipo != TipoToken::NovaLinha)
                .map(|token| token.tipo);
            if proximo == Some(TipoToken::AbreChave) {
                self.pular_novas_linhas();
            }
        }
        if self.corresponde(TipoToken::AbreChave) {
            self.separadores(); let mut itens = Vec::new();
            while !self.verifica(TipoToken::FechaChave) && !self.verifica(TipoToken::Fim) {
                let antes = self.atual; if let Some(n) = self.item() { itens.push(n); }
                if self.atual == antes { self.avancar(); }
                if !self.verifica(TipoToken::FechaChave) && !self.separador_atual() {
                    self.erro_atual("N2001", "Esperado separador dentro do bloco."); self.sincronizar();
                }
                self.separadores();
            }
            self.esperar(TipoToken::FechaChave, "Bloco aberto com '{' não foi fechado com '}'.");
            Some(itens)
        } else if self.corresponde(TipoToken::NovaLinha) {
            self.separadores(); let mut itens = Vec::new();
            while !self.verifica(TipoToken::FimBloco) && !self.verifica(TipoToken::Fim) && !self.verifica(TipoToken::Senao) {
                let antes = self.atual; if let Some(n) = self.item() { itens.push(n); }
                if self.atual == antes { self.avancar(); }
                if !self.separador_atual() && !self.verifica(TipoToken::FimBloco) { self.erro_atual("N2001", "Esperado separador dentro do bloco."); self.sincronizar(); }
                self.separadores();
            }
            self.esperar(TipoToken::FimBloco, "Bloco iniciado por quebra de linha precisa terminar com 'fim'.");
            Some(itens)
        } else { self.erro_atual("N2007", "Esperado bloco com '{' ou quebra de linha seguida de 'fim'."); None }
    }
    fn caminho_tipo(&mut self) -> Option<String> {
        self.com_profundidade(|parser| parser.caminho_tipo_interno())
    }
    fn caminho_tipo_interno(&mut self) -> Option<String> {
        if self.corresponde(TipoToken::Funcao) {
            self.esperar(TipoToken::AbrePar, "Esperado '(' após 'funcao' em tipo de função.");
            let mut parametros = Vec::new();
            if !self.verifica(TipoToken::FechaPar) {
                loop {
                    parametros.push(self.caminho_tipo()?);
                    if !self.corresponde(TipoToken::Virgula) {
                        break;
                    }
                }
            }
            self.esperar(TipoToken::FechaPar, "Esperado ')' após parâmetros do tipo de função.");
            let retorno = if self.corresponde(TipoToken::DoisPontos) {
                self.caminho_tipo()?
            } else {
                "vazio".into()
            };
            return Some(format!("funcao({}):{retorno}", parametros.join(",")));
        }
        if self.corresponde(TipoToken::AbreColchete) {
            let interno = self.caminho_tipo()?;
            self.esperar(TipoToken::FechaColchete, "Esperado ']' após tipo de lista.");
            return Some(format!("[{interno}]"));
        }
        let base = self.caminho_qualificado("Esperado nome de tipo.")?;
        if !self.corresponde(TipoToken::Menor) {
            return Some(base);
        }
        let mut argumentos = Vec::new();
        loop {
            argumentos.push(self.caminho_tipo()?);
            if !self.corresponde(TipoToken::Virgula) { break; }
        }
        self.esperar(TipoToken::Maior, "Esperado '>' após argumentos genéricos do tipo.");
        Some(format!("{base}<{}>", argumentos.join(",")))
    }
    fn caminho_modulo(&mut self) -> Option<String> {
        self.caminho_qualificado("Esperado nome de módulo.")
    }
    fn caminho_qualificado(&mut self, mensagem: &str) -> Option<String> {
        let mut nome = self.consumir_nome(mensagem)?;
        loop {
            if self.verifica(TipoToken::NovaLinha) {
                let proximo = self.tokens.iter().skip(self.atual)
                    .find(|token| token.tipo != TipoToken::NovaLinha)
                    .map(|token| token.tipo);
                if proximo == Some(TipoToken::Escopo) {
                    self.pular_novas_linhas();
                }
            }
            if !self.corresponde(TipoToken::Escopo) { break; }
            self.pular_novas_linhas();
            nome.push_str("::");
            nome.push_str(&self.consumir_nome("Esperado nome após '::'.")?);
        }
        Some(nome)
    }
    fn expressao(&mut self, minimo: u8) -> Option<Expressao> {
        self.com_profundidade(|parser| parser.expressao_interna(minimo))
    }
    fn expressao_interna(&mut self, minimo: u8) -> Option<Expressao> {
        let mut comparacao_relacional = false;
        let mut intervalo_visto = false;
        let mut esquerda = match self.tipo() {
            TipoToken::Negacao | TipoToken::Nao | TipoToken::Menos | TipoToken::Mais => {
                let op = self.avancar().tipo; self.pular_novas_linhas(); let valor = self.expressao(9)?;
                Expressao::Unaria { operador: op, valor: Box::new(valor) }
            }
            TipoToken::Identificador => {
                let nome = self.avancar().lexema.nfc().collect::<String>();
                let indice_abertura = self.atual;
                let indice_campo = (indice_abertura + 1..self.tokens.len())
                    .find(|indice| self.tokens[*indice].tipo != TipoToken::NovaLinha);
                let indice_dois_pontos = indice_campo.and_then(|indice| {
                    (indice + 1..self.tokens.len())
                        .find(|proximo| self.tokens[*proximo].tipo != TipoToken::NovaLinha)
                });
                let parece_construcao = self.tokens.get(indice_abertura).is_some_and(|t| t.tipo == TipoToken::AbreChave)
                    && indice_campo.is_some_and(|indice| self.tokens[indice].tipo == TipoToken::FechaChave
                        || self.tokens[indice].tipo == TipoToken::Identificador
                            && indice_dois_pontos.is_some_and(|proximo| self.tokens[proximo].tipo == TipoToken::DoisPontos));
                if parece_construcao && self.corresponde(TipoToken::AbreChave) {
                    let mut campos = Vec::new();
                    self.separadores();
                    while !self.verifica(TipoToken::FechaChave) && !self.verifica(TipoToken::Fim) {
                        let campo = self.consumir_nome("Esperado nome do campo na construção da estrutura.")?;
                        self.esperar(TipoToken::DoisPontos, "Esperado ':' após nome do campo.");
                        let valor = match self.expressao(0) {
                            Some(valor) => valor,
                            None => {
                                self.sincronizar_ate(&[
                                    TipoToken::Virgula,
                                    TipoToken::NovaLinha,
                                    TipoToken::PontoVirgula,
                                    TipoToken::FechaChave,
                                ]);
                                if self.corresponde(TipoToken::Virgula) || self.separador_atual() {
                                    self.separadores();
                                    continue;
                                }
                                break;
                            }
                        };
                        if campos.iter().any(|(existente, _)| existente == &campo) {
                            self.erro_atual("N2037", "Campo informado mais de uma vez na construção.");
                        } else {
                            campos.push((campo, valor));
                        }
                        if !self.corresponde(TipoToken::Virgula) && !self.separador_atual()
                            && !self.verifica(TipoToken::FechaChave) {
                            self.erro_atual("N2038", "Esperado ',' ou quebra de linha entre campos.");
                            self.sincronizar();
                        }
                        self.separadores();
                    }
                    self.esperar(TipoToken::FechaChave, "Construção de estrutura não foi fechada com '}'.");
                    Expressao::ConstrucaoEstrutura { nome, campos }
                } else {
                    Expressao::Nome(nome)
                }
            },
            TipoToken::Inteiro | TipoToken::Decimal | TipoToken::Texto | TipoToken::Caractere | TipoToken::Verdadeiro | TipoToken::Falso | TipoToken::Nulo => Expressao::Literal(self.avancar().lexema),
            TipoToken::AbrePar => {
                self.avancar();
                if self.corresponde(TipoToken::FechaPar) {
                    Expressao::Tupla(Vec::new())
                } else {
                    let primeiro = match self.expressao(0) {
                        Some(primeiro) => primeiro,
                        None => {
                            self.sincronizar_ate(&[TipoToken::FechaPar]);
                            self.corresponde(TipoToken::FechaPar);
                            return None;
                        }
                    };
                    if self.corresponde(TipoToken::Virgula) {
                        let mut elementos = vec![primeiro];
                        while !self.verifica(TipoToken::FechaPar) && !self.verifica(TipoToken::Fim) {
                            match self.expressao(0) {
                                Some(elemento) => elementos.push(elemento),
                                None => {
                                    self.sincronizar_ate(&[TipoToken::Virgula, TipoToken::FechaPar]);
                                    if self.corresponde(TipoToken::Virgula) { continue; }
                                    break;
                                }
                            }
                            if !self.corresponde(TipoToken::Virgula) { break; }
                        }
                        self.esperar(TipoToken::FechaPar, "Esperado ')' após elementos da tupla.");
                        Expressao::Tupla(elementos)
                    } else {
                        self.esperar(TipoToken::FechaPar, "Esperado ')' após expressão.");
                        primeiro
                    }
                }
            }
            TipoToken::AbreColchete => {
                self.avancar();
                let mut valores = Vec::new();
                if !self.verifica(TipoToken::FechaColchete) {
                    loop {
                        match self.expressao(0) {
                            Some(valor) => valores.push(valor),
                            None => {
                                self.sincronizar_ate(&[TipoToken::Virgula, TipoToken::FechaColchete]);
                                if self.corresponde(TipoToken::Virgula) { continue; }
                                break;
                            }
                        }
                        if !self.corresponde(TipoToken::Virgula) { break; }
                    }
                }
                self.esperar(TipoToken::FechaColchete, "Esperado ']' após lista.");
                Expressao::Lista(valores)
            }
            TipoToken::ReservadaFutura | TipoToken::Estrutura | TipoToken::Enumeracao => {
                let token = self.avancar();
                self.diagnosticos.push(Diagnostico::novo(
                    "N2011",
                    Severidade::Erro,
                    format!("A palavra '{}' está reservada para uma versão futura e ainda não é uma construção válida.", token.lexema),
                    Some(token.intervalo),
                ));
                return None;
            }
            _ => { self.erro_atual("N2008", "Esperada uma expressão."); return None; }
        };
        loop {
            if self.verifica(TipoToken::NovaLinha) {
                let proximo_significativo = self.tokens.iter().skip(self.atual)
                    .find(|token| token.tipo != TipoToken::NovaLinha)
                    .map(|token| token.tipo);
                if matches!(proximo_significativo,
                    Some(TipoToken::Atribui | TipoToken::Ou | TipoToken::E | TipoToken::Igual
                    | TipoToken::Diferente | TipoToken::Menor | TipoToken::MenorIgual
                    | TipoToken::Maior | TipoToken::MaiorIgual | TipoToken::Intervalo
                    | TipoToken::Mais | TipoToken::Menos | TipoToken::Vezes
                    | TipoToken::Divide | TipoToken::Resto | TipoToken::Potencia
                    | TipoToken::Ponto | TipoToken::Escopo | TipoToken::AbrePar | TipoToken::AbreColchete)) {
                    self.pular_novas_linhas();
                }
            }
            let tipo = self.tipo();
            if tipo == TipoToken::AbrePar && 14 >= minimo {
                self.avancar();
                let mut args = Vec::new();
                if !self.verifica(TipoToken::FechaPar) {
                    loop {
                        match self.expressao(0) {
                            Some(argumento) => args.push(argumento),
                            None => {
                                self.sincronizar_ate(&[TipoToken::Virgula, TipoToken::FechaPar]);
                                if self.corresponde(TipoToken::Virgula) { continue; }
                                break;
                            }
                        }
                        if !self.corresponde(TipoToken::Virgula) { break; }
                    }
                }
                self.esperar(TipoToken::FechaPar, "Esperado ')' após argumentos.");
                esquerda = Expressao::Chamada { alvo: Box::new(esquerda), argumentos: args }; continue;
            }
            if tipo == TipoToken::AbreColchete && 14 >= minimo {
                self.avancar();
                let indice = match self.expressao(0) {
                    Some(indice) => indice,
                    None => {
                        self.sincronizar_ate(&[TipoToken::FechaColchete]);
                        self.corresponde(TipoToken::FechaColchete);
                        continue;
                    }
                };
                self.esperar(TipoToken::FechaColchete, "Esperado ']' após índice.");
                esquerda = Expressao::Indexacao { alvo: Box::new(esquerda), indice: Box::new(indice) };
                continue;
            }
            if matches!(tipo, TipoToken::Ponto | TipoToken::Escopo) && 14 >= minimo {
                let operador = self.avancar().tipo;
                self.pular_novas_linhas();
                let nome = self.consumir_nome("Esperado nome após operador de acesso.")?;
                esquerda = Expressao::Membro { alvo: Box::new(esquerda), operador, nome };
                continue;
            }
            let (precedencia, direita) = match tipo {
                TipoToken::Atribui => (1,true), TipoToken::Ou => (2,false), TipoToken::E => (3,false),
                TipoToken::Igual | TipoToken::Diferente => (4,false),
                TipoToken::Menor | TipoToken::MenorIgual | TipoToken::Maior | TipoToken::MaiorIgual => (5,false),
                TipoToken::Intervalo => (6,false), TipoToken::Mais | TipoToken::Menos => (7,false),
                TipoToken::Vezes | TipoToken::Divide | TipoToken::Resto => (8,false),
                TipoToken::Potencia => (10,true), _ => break,
            };
            let relacional = matches!(tipo, TipoToken::Menor | TipoToken::MenorIgual | TipoToken::Maior | TipoToken::MaiorIgual);
            let intervalo = tipo == TipoToken::Intervalo;
            if relacional && comparacao_relacional {
                self.erro_atual("N2012", "Comparações relacionais não podem ser encadeadas; combine comparações com 'e' ou use parênteses.");
            }
            if intervalo && intervalo_visto {
                self.erro_atual("N2013", "Operadores de intervalo não podem ser encadeados.");
            }
            if precedencia < minimo { break; }
            if relacional { comparacao_relacional = true; }
            if intervalo { intervalo_visto = true; }
            let op = self.avancar().tipo;
            let proximo = if direita { precedencia } else { precedencia + 1 };
            self.pular_novas_linhas();
            let valor = self.expressao(proximo)?;
            esquerda = if op == TipoToken::Atribui {
                Expressao::Atribuicao { destino: Box::new(esquerda), valor: Box::new(valor) }
            } else { Expressao::Binaria { esquerda: Box::new(esquerda), operador: op, direita: Box::new(valor) } };
        }
        Some(esquerda)
    }
    fn tipo(&self) -> TipoToken { self.tokens.get(self.atual).map_or(TipoToken::Fim, |t| t.tipo) }
    fn verifica(&self, t: TipoToken) -> bool { self.tipo() == t }
    fn avancar(&mut self) -> Token { let t = self.tokens[self.atual].clone(); if t.tipo != TipoToken::Fim { self.atual += 1; } t }
    fn corresponde(&mut self, t: TipoToken) -> bool { if self.verifica(t) { self.avancar(); true } else { false } }
    fn esperar(&mut self, t: TipoToken, mensagem: &str) { if !self.corresponde(t) { self.erro_atual("N2009", mensagem); } }
    fn erro_atual(&mut self, codigo: &'static str, mensagem: &str) {
        let intervalo = self.tokens.get(self.atual).map(|t| t.intervalo);
        self.diagnosticos.push(Diagnostico::novo(codigo, Severidade::Erro, mensagem, intervalo));
    }
    fn consumir_nome(&mut self, mensagem: &str) -> Option<String> {
        if self.verifica(TipoToken::Identificador) {
            Some(self.avancar().lexema.nfc().collect())
        } else if matches!(self.tipo(), TipoToken::ReservadaFutura | TipoToken::Estrutura | TipoToken::Enumeracao) {
            let token = self.avancar();
            self.diagnosticos.push(Diagnostico::novo(
                "N2011",
                Severidade::Erro,
                format!("A palavra '{}' está reservada para uma versão futura e ainda não pode ser usada aqui.", token.lexema),
                Some(token.intervalo),
            ));
            None
        } else {
            self.erro_atual("N2010", mensagem);
            None
        }
    }
    fn separador_atual(&self) -> bool { matches!(self.tipo(), TipoToken::NovaLinha | TipoToken::PontoVirgula) }
    fn separadores(&mut self) { while self.separador_atual() { self.avancar(); } }
    fn pular_novas_linhas(&mut self) { while self.corresponde(TipoToken::NovaLinha) {} }
    fn fim_item(&self) -> bool { matches!(self.tipo(), TipoToken::NovaLinha | TipoToken::PontoVirgula | TipoToken::FechaChave | TipoToken::Fim | TipoToken::FimBloco | TipoToken::Senao) }
    fn sincronizar(&mut self) {
        self.sincronizar_ate(&[
            TipoToken::NovaLinha,
            TipoToken::PontoVirgula,
            TipoToken::FechaChave,
            TipoToken::FimBloco,
            TipoToken::Senao,
        ]);
    }
    fn sincronizar_ate(&mut self, finais: &[TipoToken]) {
        let mut delimitadores = Vec::new();
        while !self.verifica(TipoToken::Fim) {
            let atual = self.tipo();
            if delimitadores.is_empty() && finais.contains(&atual) {
                break;
            }
            match atual {
                TipoToken::AbrePar => delimitadores.push(TipoToken::FechaPar),
                TipoToken::AbreColchete => delimitadores.push(TipoToken::FechaColchete),
                TipoToken::AbreChave => delimitadores.push(TipoToken::FechaChave),
                TipoToken::FechaPar | TipoToken::FechaColchete | TipoToken::FechaChave => {
                    if delimitadores.last() == Some(&atual) {
                        delimitadores.pop();
                    } else if delimitadores.is_empty() && finais.contains(&atual) {
                        break;
                    }
                }
                _ => {}
            }
            self.avancar();
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    fn fonte(codigo: &str) -> ArquivoFonte { ArquivoFonte::novo("teste.nbx", codigo) }

    #[test]
    fn nao_aceita_palavras_reservadas_como_identificadores() {
        for codigo in [
            "variavel estrutura = 1",
            "funcao testar(enumeracao: inteiro) { devolver enumeracao }",
        ] {
            let r = analisar(&fonte(codigo));
            assert!(
                r.diagnosticos.iter().any(|d| d.codigo == "N2011"),
                "{codigo}: {:?}",
                r.diagnosticos
            );
        }
    }

    #[test]
    fn analisa_tipos_de_funcao_aninhados() {
        let resultado = analisar(&fonte(
            "estrutura Registro { callback: funcao(inteiro, texto): logico, padrao: funcao(funcao(inteiro): logico): vazio }",
        ));
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
        let Some(NoAst::Estrutura { campos, .. }) = resultado.programa.itens.first() else {
            panic!("estrutura deveria ser analisada");
        };
        assert_eq!(campos[0].1, "funcao(inteiro,texto):logico");
        assert_eq!(campos[1].1, "funcao(funcao(inteiro):logico):vazio");
    }

    #[test]
    fn analisa_declaracoes_de_estrutura_e_enumeracao() {
        let r = analisar(&fonte("estrutura Pessoa { nome: texto, idade: inteiro }\nenumeracao Estado { Ativo, Inativo, Erro(texto) }"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert!(matches!(&r.programa.itens[0], NoAst::Estrutura { nome, campos } if nome == "Pessoa" && campos.len() == 2));
        assert!(matches!(&r.programa.itens[1], NoAst::Enumeracao { nome, variantes } if nome == "Estado" && variantes.len() == 3 && variantes[2].1 == ["texto"]));
    }

    #[test]
    fn rejeita_campos_e_variantes_duplicados() {
        let estrutura = analisar(&fonte("estrutura Pessoa { nome: texto, nome: inteiro }"));
        assert!(estrutura.diagnosticos.iter().any(|d| d.codigo == "N2031"));
        let enumeracao = analisar(&fonte("enumeracao Estado { Ativo, Ativo }"));
        assert!(enumeracao.diagnosticos.iter().any(|d| d.codigo == "N2034"));
    }

    #[test]
    fn analisa_escolha_de_enumeracao_com_payload_e_curinga() {
        let r = analisar(&fonte("escolha resultado { caso Resultado::Sucesso(valor) { exibir(valor) } caso Resultado::Falha(mensagem) { exibir(mensagem) } caso _ { exibir(0) } }"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        let NoAst::Escolha { casos, .. } = &r.programa.itens[0] else { panic!("AST não contém escolha"); };
        assert_eq!(casos.len(), 3);
        assert_eq!(casos[0].vinculacoes, ["valor"]);
        assert!(casos[2].enumeracao.is_none() && casos[2].variante.is_none());
    }

    #[test]
    fn reconhece_palavras_identificadores_e_operadores_maiores() {
        let r = analisar_lexico(&fonte("variavel ação = 2 >= 1 // comentário\n"));
        assert!(r.diagnosticos.is_empty());
        assert_eq!(r.tokens[0].tipo, TipoToken::Variavel);
        assert_eq!(r.tokens[1].lexema, "ação");
        assert!(r.tokens.iter().any(|t| t.tipo == TipoToken::MaiorIgual));
    }
    #[test]
    fn trata_crlf_como_uma_quebra_logica() {
        let r = analisar_lexico(&fonte("variavel a = 1\r\nvariavel b = 2"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.tokens.iter().filter(|t| t.tipo == TipoToken::NovaLinha).count(), 1);
    }
    #[test]
    fn rejeita_cr_isolado() {
        let r = analisar_lexico(&fonte("variavel a = 1\rvariavel b = 2"));
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N1001"));
    }
    #[test]
    fn rejeita_escape_unicode_fora_do_intervalo_escalar() {
        let barra = char::from(92);
        let codigo = format!("\"{barra}u{{110000}}\"");
        let r = analisar_lexico(&fonte(&codigo));
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N1007"));
    }
    #[test]
    fn rejeita_cr_isolado_em_comentario_de_linha() {
        let codigo = format!("// comentario{}variavel x = 1", char::from(13));
        let r = analisar_lexico(&fonte(&codigo));
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N1001"));
    }
    #[test]
    fn rejeita_separador_invalido_em_decimal() {
        let r = analisar_lexico(&fonte("1.2_"));
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N1005"));
    }
    #[test]
    fn rejeita_comentario_de_bloco_nao_terminado() {
        let r = analisar_lexico(&fonte("variavel a = 1 /* sem fim"));
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N1002"));
    }
    #[test]
    fn normaliza_identificadores_em_nfc() {
        let composto = analisar(&fonte("variavel café = 1"));
        let decomposto = analisar(&fonte("variavel cafe\u{301} = 1"));
        let nome_a = match &composto.programa.itens[0] { NoAst::Variavel { nome, .. } => nome, _ => unreachable!() };
        let nome_b = match &decomposto.programa.itens[0] { NoAst::Variavel { nome, .. } => nome, _ => unreachable!() };
        assert_eq!(nome_a, nome_b);
    }
    #[test]
    fn reconhece_inteiros_em_bases_diferentes() {
        let r = analisar_lexico(&fonte("0xFF 0b1010 0o77"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.tokens.iter().filter(|t| t.tipo == TipoToken::Inteiro).count(), 3);
    }
    #[test]
    fn rejeita_inteiro_de_base_malformado() {
        let r = analisar_lexico(&fonte("0b102"));
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N1005"));
    }
    #[test]
    fn preserva_quebras_dentro_de_comentario_de_bloco() {
        let r = analisar_lexico(&fonte("variavel a = 1 /* comentário\ncontinua */ variavel b = 2"));
        assert!(r.tokens.iter().any(|t| t.tipo == TipoToken::NovaLinha));
    }
    #[test]
    fn permite_quebra_de_linha_dentro_de_delimitadores() {
        let r = analisar(&fonte("variavel soma = calcular(1,\n 2)\nvariavel lista = [1,\n 2]"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn permite_quebra_apos_atribuicao_declarativa() {
        let r = analisar(&fonte("variavel soma =\n 1 + 2"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn permite_quebra_apos_operador_unario() {
        let r = analisar(&fonte("variavel negativo = -\n 2"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn permite_linhas_em_branco_antes_de_operador_de_continuacao() {
        let r = analisar(&fonte("variavel soma = 1\n\n + 2"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn permite_continuacao_de_expressao_apos_operador() {
        let r = analisar(&fonte("variavel soma = 1 +\n 2"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn analisa_tuplas_com_elementos_e_tupla_unitaria() {
        for codigo in [
            "variavel ponto = (10, 20)",
            "variavel unidade = (42,)",
            "variavel vazio = ()",
        ] {
            let r = analisar(&fonte(codigo));
            assert!(r.diagnosticos.is_empty(), "{codigo}: {:?}", r.diagnosticos);
        }
    }

    #[test]
    fn permite_tipos_de_lista() {
        let r = analisar(&fonte("variavel valores: [inteiro] = [1, 2]"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn comenta_bloco_e_preserva_nova_linha() {
        let r = analisar_lexico(&fonte("/* x */\nvariavel a = 1"));
        assert_eq!(r.tokens[0].tipo, TipoToken::NovaLinha);
    }
    #[test]
    fn texto_invalido_nao_consume_separador_da_proxima_instrucao() {
        let r = analisar(&fonte("variavel a = \"sem fim\nvariavel b = 2"));
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N1006"));
        assert!(!r.programa.itens.is_empty());
    }
    #[test]
    fn caractere_unicode_conta_um_escalar() {
        let r = analisar(&fonte("variavel inicial = 'á'"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn diagnostica_texto_nao_terminado() {
        let r = analisar_lexico(&fonte("\"abc"));
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N1008"));
    }
    #[test]
    fn analisa_funcao_condicional_e_precedencia() {
        let r = analisar(&fonte("funcao principal(): vazio {\n variavel x: inteiro = 1 + 2 * 3\n quando x > 2 { devolver x } senao { devolver 0 }\n}"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.programa.itens.len(), 1);
    }
    #[test]
    fn analisa_funcao_generica_com_um_e_varios_parametros() {
        let r = analisar(&fonte(
            "funcao identidade<T>(valor: T): T { devolver valor }\nfuncao combinar<T, U>(a: T, b: U): T { devolver a }",
        ));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert!(matches!(
            &r.programa.itens[0],
            NoAst::Funcao { genericos, .. } if genericos == &["T"]
        ));
        assert!(matches!(
            &r.programa.itens[1],
            NoAst::Funcao { genericos, .. } if genericos == &["T", "U"]
        ));
    }

    #[test]
    fn rejeita_parametro_generico_duplicado() {
        let r = analisar(&fonte("funcao combinar<T, T>(a: T) { devolver }"));
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N2014"));
    }

    #[test]
    fn preserva_visibilidade_de_funcao_na_ast() {
        let r = analisar(&fonte("publico funcao abrir() { devolver }"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert!(matches!(&r.programa.itens[0], NoAst::Funcao { visibilidade: Some(Visibilidade::Publico), .. }));
    }
    #[test]
    fn aceita_chave_de_abertura_na_linha_seguinte() {
        let r = analisar(&fonte("funcao principal()\n{\n variavel ativo = verdadeiro\n}"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn analisa_bloco_com_fim() {
        let r = analisar(&fonte("funcao executar()\n variavel ativo = verdadeiro\n enquanto ativo\n  ativo = falso\n fim\nfim"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn recupera_e_emite_diagnostico_de_sintaxe() {
        let r = analisar(&fonte("variavel =\nvariavel correto = 2"));
        assert!(!r.diagnosticos.is_empty());
        assert!(!r.programa.itens.is_empty());
    }
    #[test]
    fn incluir_aceita_caminho_qualificado() {
        let r = analisar(&fonte("incluir Biblioteca::Texto"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn incluir_rejeita_tipo_de_lista_como_caminho_de_modulo() {
        let r = analisar(&fonte("incluir [inteiro]"));
        assert!(!r.diagnosticos.is_empty());
    }
    #[test]
    fn analisa_laco_para_e_inclusao_de_modulo() {
        let r = analisar(&fonte("incluir Matemática como Mat\npara (variavel i = 0; i < 3; i = i + 1) { exibir(i) }"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.programa.itens.len(), 2);
    }


    #[test]
    fn analisa_pulso_com_intervalo() {
        let r = analisar(&fonte("pulso(100) { encerrar }"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert!(matches!(&r.programa.itens[0], NoAst::Pulso { .. }));
    }

    #[test]
    fn analisa_laco_cada_com_colecao() {
        let r = analisar(&fonte("cada item em [1, 2, 3] { exibir(item) }"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert!(matches!(&r.programa.itens[0], NoAst::Cada { nome, .. } if nome == "item"));
    }

    #[test]
    fn condicional_sem_senao_preserva_separador_da_proxima_instrucao() {
        let r = analisar(&fonte("quando verdadeiro { exibir(1) }\nvariavel depois = 2"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.programa.itens.len(), 2);
    }
    #[test]
    fn aceita_quando_senao_em_linhas_separadas() {
        let r = analisar(&fonte("quando verdadeiro {\n exibir(1)\n}\nsenao {\n exibir(0)\n}"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn rejeita_palavras_reservadas_de_recursos_futuros() {
        let r = analisar(&fonte("variavel tentar = 1"));
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N2011"));
    }
    #[test]
    fn rejeita_comparacoes_relacionais_encadeadas() {
        let r = analisar(&fonte("variavel resultado = 1 < 2 < 3"));
        assert!(!r.diagnosticos.is_empty());
    }
    #[test]
    fn rejeita_comparacao_relacional_encadeada_com_codigo_especifico() {
        let r = analisar(&fonte("variavel resultado = 1 < 2 < 3"));
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N2012"));
    }
    #[test]
    fn permite_comparacoes_relacionais_separadas_por_parenteses() {
        let r = analisar(&fonte("variavel resultado = (1 < 2) e (2 < 3)"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn rejeita_intervalos_encadeados() {
        let r = analisar(&fonte("variavel faixa = 1..2..3"));
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N2013"));
    }
    #[test]
    fn permite_acesso_a_membro_apos_quebra_de_linha() {
        let r = analisar(&fonte("variavel valor = objeto\n.nome"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn permite_quebra_apos_operador_de_membro() {
        let r = analisar(&fonte("variavel valor = objeto.\n nome"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn permite_quebra_antes_de_operador_de_escopo_em_tipo() {
        let r = analisar(&fonte("tipo Caminho = Modulo\n:: Tipo"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn permite_quebra_apos_operador_de_escopo_em_tipo() {
        let r = analisar(&fonte("tipo Caminho = Modulo::\n Tipo"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn cabecalho_para_rejeita_declaracao_constante() {
        let r = analisar(&fonte("para (constante i = 0; i < 3; i = i + 1) { parar }"));
        assert!(!r.diagnosticos.is_empty());
    }
    #[test]
    fn precedencia_da_potencia_e_superior_a_unaria() {
        let r = analisar(&fonte("variavel resultado = -2 ** 2"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        let NoAst::Variavel { valor: Expressao::Unaria { valor, .. }, .. } = &r.programa.itens[0] else {
            panic!("esperada operação unária envolvendo potência");
        };
        assert!(matches!(**valor, Expressao::Binaria { operador: TipoToken::Potencia, .. }));
    }
    #[test]
    fn potencia_associa_a_direita() {
        let r = analisar(&fonte("variavel resultado = 2 ** 3 ** 2"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        let NoAst::Variavel { valor: Expressao::Binaria { direita, .. }, .. } = &r.programa.itens[0] else {
            panic!("esperada potência");
        };
        assert!(matches!(**direita, Expressao::Binaria { operador: TipoToken::Potencia, .. }));
    }
    #[test]
    fn analisa_exemplo_e_fixture_publicados() {
        let exemplos = [
            include_str!("../../../exemplos/ola_mundo.nbx"),
            include_str!("../../../testes/conformidade/sintaxe_visual.nbx"),
        ];
        for codigo in exemplos {
            let resultado = analisar(&fonte(codigo));
            assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
        }
    }
    #[test]
    fn lexer_e_parser_resistem_a_entradas_malformadas_deterministicas() {
        let alfabeto = [
            "a", "z", "0", "9", "_", " ", "\n", "\t", "{", "}", "(", ")",
            "[", "]", ";", ":", ".", ",", "+", "-", "*", "/", "=", "<", ">",
            "\"", "'", "#", "🌌", "ç", "ã",
        ];
        let mut estado = 0x5eed_u64;
        for caso in 0..512 {
            let tamanho = (caso % 97) + 1;
            let mut codigo = String::new();
            for _ in 0..tamanho {
                estado = estado
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                codigo.push_str(alfabeto[(estado as usize) % alfabeto.len()]);
            }
            let _ = analisar(&fonte(&codigo));
        }
    }

    #[test]
    fn reconhece_lista_chamada_indexacao_e_membro() {
        let r = analisar(&fonte("variavel x = sistema::somar([1, 2][0], objeto.nome)"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn reconhece_aliases_de_declaracao_e_controle() {
        let codigo = "definir a = 1\nmutável b = 2\nmutavel c = 3\nalterar a <- 3\navaliar a { caso _ { exibir(b) } }\ndurante falso { encerrar }\nrepetir falso { seguir }\ncada item em [1, 2] { exibir(item) }";
        let r = analisar(&fonte(codigo));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.programa.itens.len(), 8);
    }

    #[test]
    fn aceita_grafias_portuguesas_com_e_sem_acentos() {
        let codigo = "função somar(a: inteiro, b: inteiro): inteiro { devolver a + b }\nquando não falso { exibir(1) } senão { exibir(0) }\nenumeração Estado { Ativo\nInativo }\npúblico função principal() { exibir(Estado::Ativo) }";
        let r = analisar(&fonte(codigo));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.programa.itens.len(), 4);
    }

    #[test]
    fn operador_seta_esquerda_e_aceito_como_atribuicao() {
        let r = analisar(&fonte("variavel total = 1\ntotal <- 2"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert!(matches!(&r.programa.itens[1], NoAst::Expressao(Expressao::Atribuicao { .. })));
    }


    #[test]
    fn cobre_producoes_normativas_do_parser() {
        let exemplos = [
            "variavel x = 1",
            "definir x: inteiro = 1",
            "mutável x = 1",
            "constante limite: inteiro = 10",
            "funcao somar(a: inteiro, b: inteiro): inteiro { devolver a + b }",
            "publico funcao identidade<T>(valor: T): T { devolver valor }",
            "privado funcao combinar<T, U>(a: [T], b: U): funcao(T): U { devolver b }",
            "tipo Caminho = Modulo::Submodulo::Tipo",
            "tipo Colecao = [Resultado<inteiro>]",
            "estrutura Pessoa { nome: texto; idade: inteiro, }",
            "enumeracao Estado { Ativo\nInativo(texto, inteiro) }",
            "incluir Biblioteca::Texto como Texto",
            "quando verdadeiro { exibir(1) } senao { exibir(0) }",
            "quando verdadeiro { exibir(1) } senao quando falso { exibir(2) }",
            "escolha resultado { caso Resultado::Sucesso(valor) { exibir(valor) } caso _ { exibir(0) } }",
            "enquanto verdadeiro { parar }",
            "durante verdadeiro { seguir }",
            "repetir falso { encerrar }",
            "cada item em [1, 2, 3] { exibir(item) }",
            "pulso(100) { exibir(1) }",
            "para (variavel i = 0; i < 3; i = i + 1) { exibir(i) }",
            "alterar total <- 3",
            "devolver",
            "variavel vazia = ()",
            "variavel tupla = (1, 2,)",
            "variavel lista = [1, 2, 3]",
            "variavel chamada = sistema::somar(1, 2)[0].valor",
            "variavel construcao = Pessoa { nome: \"Ana\", idade: 20 }",
            "variavel potencia = -2 ** 3 ** 2",
            "variavel logica = (1 < 2) e (3 >= 2) ou falso",
            "funcao bloco_fim()\n variavel ativo = verdadeiro\n enquanto ativo\n  ativo = falso\n fim\nfim",
        ];

        for codigo in exemplos {
            let resultado = analisar(&fonte(codigo));
            assert!(
                resultado.diagnosticos.is_empty(),
                "produção válida rejeitada: {codigo:?}; diagnósticos: {:?}",
                resultado.diagnosticos
            );
            assert!(
                !resultado.programa.itens.is_empty(),
                "nenhum item produzido para: {codigo:?}"
            );
        }
    }

    #[test]
    fn rejeita_formas_invalidas_das_producoes_normativas() {
        let exemplos = [
            "variavel = 1",
            "constante limite =",
            "funcao sem_parametros( { devolver }",
            "funcao incompleta(valor inteiro) { devolver }",
            "tipo SemDestino =",
            "estrutura Pessoa { nome texto }",
            "enumeracao Estado {}",
            "incluir [inteiro]",
            "quando verdadeiro { exibir(1)",
            "escolha valor { caso _ { exibir(1) }",
            "cada item [1, 2] { exibir(item) }",
            "pulso { exibir(1) }",
            "para (variavel i = 0; i < 3) { parar }",
            "alterar total",
            "variavel lista = [1, 2",
            "variavel chamada = funcao(1, 2",
            "variavel tupla = (1, 2",
            "variavel membro = objeto.",
        ];

        for codigo in exemplos {
            let resultado = analisar(&fonte(codigo));
            assert!(
                !resultado.diagnosticos.is_empty(),
                "forma inválida aceita sem diagnóstico: {codigo:?}"
            );
        }
    }


    #[test]
    fn cobre_todos_os_literais_e_familias_de_operadores() {
        let exemplos = [
            "variavel inteiro = 42",
            "variavel decimal = 3.14",
            "variavel texto = \"Nebula\"",
            "variavel caractere = 'á'",
            "variavel literal_verdadeiro = verdadeiro",
            "variavel literal_falso = falso",
            "variavel literal_nulo = nulo",
            "variavel lista_vazia = []",
            "variavel chamada_vazia = executar()",
            "variavel agrupada = (1)",
            "variavel unarios = !falso e nao falso e -1 < +2",
            "variavel aritmetica = 1 + 2 - 3 * 4 / 5 % 2 ** 3",
            "variavel logica = verdadeiro e falso ou nao nulo",
            "variavel igualdade = 1 == 1 != 2",
            "variavel relacional = 1 < 2 e 3 <= 4 e 5 > 4 e 6 >= 6",
            "variavel intervalo = 1..10",
            "variavel atribuicao = a = b <- 1",
            "variavel membros = objeto.campo::subcampo[0](1)",
        ];

        for codigo in exemplos {
            let resultado = analisar(&fonte(codigo));
            assert!(
                resultado.diagnosticos.is_empty(),
                "expressão válida rejeitada: {codigo:?}; diagnósticos: {:?}",
                resultado.diagnosticos
            );
        }
    }

    #[test]
    fn reconhece_blocos_vazios_e_construcoes_vazias() {
        for codigo in [
            "funcao vazia() {}",
            "estrutura Vazia {}",
            "variavel tupla_vazia = ()",
            "variavel lista_vazia = []",
            "variavel pessoa = Pessoa {}",
            "escolha valor { caso _ {} }",
        ] {
            let resultado = analisar(&fonte(codigo));
            assert!(
                resultado.diagnosticos.is_empty(),
                "construção vazia válida rejeitada: {codigo:?}; diagnósticos: {:?}",
                resultado.diagnosticos
            );
        }
    }


    #[test]
    fn aceita_todas_as_formas_validas_de_padroes_escolha() {
        let codigo = "escolha resultado {\n caso Resultado::Vazio {}\n caso Resultado::Unico() {}\n caso Resultado::Um(valor) {}\n caso Resultado::Dois(primeiro, segundo) {}\n caso _ {}\n}";
        let resultado = analisar(&fonte(codigo));
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
        let Some(NoAst::Escolha { casos, .. }) = resultado.programa.itens.first() else {
            panic!("esperada instrução escolha");
        };
        assert_eq!(casos.len(), 5);
        assert!(casos[0].vinculacoes.is_empty());
        assert!(casos[1].vinculacoes.is_empty());
        assert_eq!(casos[2].vinculacoes, ["valor"]);
        assert_eq!(casos[3].vinculacoes, ["primeiro", "segundo"]);
        assert!(casos[4].enumeracao.is_none() && casos[4].variante.is_none());
    }

    #[test]
    fn rejeita_padroes_escolha_malformados() {
        let casos = [
            ("escolha x { caso Variante {} }", "N2042"),
            ("escolha x { caso Enum:: {} }", "N2010"),
            ("escolha x { caso Enum::Variante(1) {} }", "N2045"),
            ("escolha x { caso Enum::Variante(valor, valor) {} }", "N2046"),
            ("escolha x { caso _() {} }", "N2042"),
            ("escolha x { caso Enum::Variante(valor,) {} }", "N2045"),
            ("escolha x { caso Enum::Variante::Aninhada {} }", "N2042"),
            ("escolha x { caso _ {} caso _ {} }", "N2043"),
        ];

        for (codigo, esperado) in casos {
            let resultado = analisar(&fonte(codigo));
            assert!(
                resultado.diagnosticos.iter().any(|d| d.codigo == esperado),
                "{codigo:?}: esperado {esperado}; recebido {:?}",
                resultado.diagnosticos
            );
        }
    }

    #[test]
    fn padrao_invalido_nao_e_confundido_com_curinga() {
        let resultado = analisar(&fonte(
            "escolha x { caso VarianteInvalida {} caso _ {} }"
        ));
        assert!(resultado.diagnosticos.iter().any(|d| d.codigo == "N2042"));
        assert!(
            !resultado.diagnosticos.iter().any(|d| d.codigo == "N2043"),
            "padrão inválido não pode contar como curinga: {:?}",
            resultado.diagnosticos
        );
        let Some(NoAst::Escolha { casos, .. }) = resultado.programa.itens.first() else {
            panic!("esperada instrução escolha parcial");
        };
        assert_eq!(casos.len(), 1);
        assert!(casos[0].enumeracao.is_none() && casos[0].variante.is_none());
    }


    #[test]
    fn rejeita_variante_repetida_em_escolha() {
        let resultado = analisar(&fonte(
            "escolha x { caso Resultado::Ok(valor) {} caso Resultado::Ok(outro) {} }"
        ));
        assert!(
            resultado.diagnosticos.iter().any(|d| d.codigo == "N2048"),
            "padrão repetido deveria ser diagnosticado: {:?}",
            resultado.diagnosticos
        );
    }


    #[test]
    fn respeita_associatividade_e_precedencia_em_todos_os_niveis() {
        let codigo = "variavel esquerda = 10 - 3 - 2\nvariavel direita = a = b = 1\nvariavel prioridade = 1 + 2 * 3\nvariavel comparacao = 1 < 2 == verdadeiro";
        let resultado = analisar(&fonte(codigo));
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);

        let NoAst::Variavel { valor: Expressao::Binaria { esquerda, operador: TipoToken::Menos, .. }, .. } =
            &resultado.programa.itens[0] else { panic!("esperada subtração"); };
        assert!(matches!(**esquerda, Expressao::Binaria { operador: TipoToken::Menos, .. }));

        let NoAst::Variavel { valor: Expressao::Atribuicao { valor, .. }, .. } =
            &resultado.programa.itens[1] else { panic!("esperada atribuição"); };
        assert!(matches!(**valor, Expressao::Atribuicao { .. }));

        let NoAst::Variavel { valor: Expressao::Binaria { operador: TipoToken::Mais, direita, .. }, .. } =
            &resultado.programa.itens[2] else { panic!("esperada adição"); };
        assert!(matches!(**direita, Expressao::Binaria { operador: TipoToken::Vezes, .. }));

        let NoAst::Variavel { valor: Expressao::Binaria { operador: TipoToken::Igual, esquerda, .. }, .. } =
            &resultado.programa.itens[3] else { panic!("esperada igualdade"); };
        assert!(matches!(**esquerda, Expressao::Binaria { operador: TipoToken::Menor, .. }));
    }

    #[test]
    fn diagnostica_operadores_sem_operando() {
        for codigo in [
            "variavel a = 1 +",
            "variavel b = 2 **",
            "variavel c = !",
            "variavel d = (1 + )",
            "variavel e = 1..",
        ] {
            let resultado = analisar(&fonte(codigo));
            assert!(
                !resultado.diagnosticos.is_empty(),
                "operador sem operando foi aceito: {codigo:?}"
            );
        }
    }

    #[test]
    fn limita_recursao_de_expressoes_tipos_e_blocos() {
        let expressoes = format!(
            "variavel profunda = {}1{}",
            "(".repeat(300),
            ")".repeat(300)
        );
        let resultado = analisar(&fonte(&expressoes));
        assert!(
            resultado.diagnosticos.iter().any(|d| d.codigo == "N2049"),
            "aninhamento de expressão deveria atingir o limite: {:?}",
            resultado.diagnosticos
        );

        let tipo = format!(
            "tipo Profundo = {}inteiro{}",
            "[".repeat(300),
            "]".repeat(300)
        );
        let resultado = analisar(&fonte(&tipo));
        assert!(
            resultado.diagnosticos.iter().any(|d| d.codigo == "N2049"),
            "aninhamento de tipo deveria atingir o limite: {:?}",
            resultado.diagnosticos
        );

        let mut blocos = "exibir(1)".to_string();
        for _ in 0..150 {
            blocos = format!("quando verdadeiro {{ {blocos} }}");
        }
        let resultado = analisar(&fonte(&blocos));
        assert!(
            resultado.diagnosticos.iter().any(|d| d.codigo == "N2049"),
            "aninhamento de blocos deveria atingir o limite: {:?}",
            resultado.diagnosticos
        );
    }

}

}

pub use lexer::{ResultadoLexico, TipoToken, Token, analisar_lexico};

pub use implementacao::*;
