//! Lexer, AST e Parser do subconjunto inicial da linguagem Nebula.

#![forbid(unsafe_code)]
#![allow(missing_docs)]

pub mod ast;
pub mod lexer;

#[rustfmt::skip]
mod implementacao {
//! Lexer, AST e Parser do subconjunto inicial da linguagem Nebula.

use nebula_diagnosticos::{Diagnostico, Severidade};
use nebula_fonte::{ArquivoFonte, IntervaloFonte};
use crate::lexer::{analisar_lexico, TipoToken, Token};
use crate::ast::{AtributoAst, CapturaAst, CasoEscolha, CasoEscolhaLocalizado, Expressao, ExpressaoLocalizada, NoAst, NoLocalizado, Programa, ResultadoSintatico, TipoAst, TipoLocalizado, Visibilidade};
use unicode_normalization::UnicodeNormalization;

/// Constrói a AST a partir de uma fonte Nebula. Erros léxicos são preservados.
#[must_use]
pub fn analisar(fonte: &ArquivoFonte) -> ResultadoSintatico {
    let lexico = analisar_lexico(fonte);
    let mut parser = Parser::novo(lexico.tokens);
    let mut programa = parser.programa();
    programa.caminho = Some(fonte.caminho().to_path_buf());
    let mut diagnosticos = lexico.diagnosticos;
    diagnosticos.extend(parser.diagnosticos);
    // A ordem de apresentação acompanha a ordem da fonte, não a ordem em que
    // Lexer e Parser executaram suas respectivas etapas.
    diagnosticos.sort_by_key(|diagnostico| {
        diagnostico.intervalo.map_or(usize::MAX, |intervalo| intervalo.inicio)
    });
    ResultadoSintatico { programa, diagnosticos }
}

const LIMITE_PROFUNDIDADE_PARSER: usize = 256;

struct Parser { tokens: Vec<Token>, atual: usize, profundidade: usize, bloquear_construcao_vazia: bool, diagnosticos: Vec<Diagnostico>, nos_localizados: Vec<NoLocalizado>, expressoes_localizadas: Vec<ExpressaoLocalizada>, tipos_localizados: Vec<TipoLocalizado>, casos_localizados: Vec<CasoEscolhaLocalizado> }
impl Parser {
    fn novo(tokens: Vec<Token>) -> Self { Self { tokens, atual: 0, profundidade: 0, bloquear_construcao_vazia: false, diagnosticos: Vec::new(), nos_localizados: Vec::new(), expressoes_localizadas: Vec::new(), tipos_localizados: Vec::new(), casos_localizados: Vec::new() } }
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
            let diagnosticos_antes = self.diagnosticos.len();
            let no = self.item();
            let bloco_recuperado = no.as_ref().is_some_and(Self::eh_bloco)
                && self.diagnosticos.len() > diagnosticos_antes
                && self.inicia_item();
            if let Some(no) = no { itens.push(no); }
            if self.atual == antes { self.avancar(); }
            if self.fechamento_sem_contexto() {
                self.erro_atual("N2051", "Delimitador de fechamento sem abertura correspondente ou fora do bloco esperado.");
                self.avancar();
            } else if !self.verifica(TipoToken::Fim) && !self.verifica(TipoToken::FechaChave)
                && !self.verifica(TipoToken::FimBloco) && !self.verifica(TipoToken::Senao)
                && !self.separador_atual() && !bloco_recuperado {
                self.erro_atual("N2001", "Esperado quebra de linha ou ';' entre instruções.");
                self.sincronizar_item();
            }
            self.separadores();
        }
        let ordenar = |a: &IntervaloFonte, b: &IntervaloFonte| {
            a.inicio.cmp(&b.inicio).then_with(|| b.fim.cmp(&a.fim))
        };
        self.nos_localizados.sort_by(|a, b| ordenar(&a.intervalo, &b.intervalo));
        self.expressoes_localizadas.sort_by(|a, b| ordenar(&a.intervalo, &b.intervalo));
        self.tipos_localizados.sort_by(|a, b| ordenar(&a.intervalo, &b.intervalo));
        self.casos_localizados.sort_by(|a, b| ordenar(&a.intervalo, &b.intervalo));
        Programa {
            caminho: None,
            itens,
            nos_localizados: std::mem::take(&mut self.nos_localizados),
            expressoes_localizadas: std::mem::take(&mut self.expressoes_localizadas),
            tipos_localizados: std::mem::take(&mut self.tipos_localizados),
            casos_localizados: std::mem::take(&mut self.casos_localizados),
        }
    }
    fn item(&mut self) -> Option<NoAst> {
        let inicio = self.tokens.get(self.atual).map(|token| token.intervalo.inicio);
        let no = self.item_interno();
        if let (Some(no), Some(inicio)) = (no.as_ref(), inicio) {
            if self.atual > 0 {
                if let Some(token) = self.tokens.get(self.atual - 1) {
                    if let Some(intervalo) = IntervaloFonte::novo(inicio, token.intervalo.fim) {
                        self.nos_localizados.push(NoLocalizado { no: no.clone(), intervalo });
                    }
                }
            }
        }
        no
    }
    fn item_interno(&mut self) -> Option<NoAst> {
        match self.tipo() {
            TipoToken::FechaPar | TipoToken::FechaColchete | TipoToken::FechaChave | TipoToken::FimBloco => {
                self.erro_atual("N2051", "Delimitador de fechamento sem abertura correspondente ou fora do bloco esperado.");
                None
            }
            TipoToken::Indent | TipoToken::Dedent => {
                self.erro_atual("N2009", "Indentação inesperada fora de um bloco.");
                self.avancar();
                None
            }
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
            TipoToken::Funcao | TipoToken::Estatico | TipoToken::Publico | TipoToken::Privado | TipoToken::Protegido => self.funcao_modificada(false, None),
            TipoToken::Assinc => {
                self.avancar();
                self.funcao_modificada(true, None)
            }
            TipoToken::Classe => self.classe(),
            TipoToken::Tente => self.tente(),
            TipoToken::Com => self.com(),
            TipoToken::Lancar => {
                self.avancar();
                self.corresponde(TipoToken::Erro);
                let expressao = self.expressao(0)?;
                Some(NoAst::Lancar(expressao))
            }
            TipoToken::Verifique => {
                self.avancar();
                let condicao = self.expressao(0)?;
                let mensagem = if self.corresponde(TipoToken::Virgula) { self.expressao(0) } else { None };
                Some(NoAst::Verifique { condicao, mensagem })
            }
            TipoToken::Global => {
                self.avancar();
                let nome = self.consumir_nome("Esperado nome após declaração de escopo.")?;
                Some(NoAst::Escopo { externa: false, nome })
            }
            TipoToken::Externa => {
                self.avancar();
                if self.corresponde(TipoToken::Funcao) {
                    self.funcao_corpo(None, true)
                } else {
                    let nome = self.consumir_nome("Esperado nome ou 'funcao' após 'externa'.")?;
                    Some(NoAst::Escopo { externa: true, nome })
                }
            }
            TipoToken::Arroba => self.atributada(),
            TipoToken::Critico | TipoToken::Seguro | TipoToken::Privilegio | TipoToken::Montagem => self.bloco_contextual(),
            TipoToken::Exportar => {
                self.avancar();
                if self.corresponde(TipoToken::Funcao) { self.funcao_corpo(Some(Visibilidade::Publico), false) }
                else { self.erro_atual("N2002", "'exportar' deve anteceder uma função."); None }
            }
            TipoToken::Tipo => self.tipo_alias(),
            TipoToken::Estrutura => self.estrutura(),
            TipoToken::Enumeracao => self.enumeracao(),
            TipoToken::ReservadaFutura | TipoToken::Erro | TipoToken::Sempre | TipoToken::Herda => {
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
            TipoToken::Quando | TipoToken::Se => self.quando(),
            TipoToken::Escolha | TipoToken::Avaliar => self.escolha(),
            TipoToken::Enquanto | TipoToken::Durante | TipoToken::Repetir => self.enquanto(),
            TipoToken::Pulso => self.pulso(),
            TipoToken::Para => self.para(),
            TipoToken::Cada => self.cada(),
            TipoToken::Devolver | TipoToken::Retorne => { self.avancar(); let e = if self.fim_item() { None } else { self.expressao(0) }; Some(NoAst::Devolver(e)) }
            TipoToken::Parar | TipoToken::Encerrar | TipoToken::Pare => { self.avancar(); Some(NoAst::Parar) }
            TipoToken::Seguir | TipoToken::Prossiga => { self.avancar(); Some(NoAst::Seguir) }
            TipoToken::Passar => { self.avancar(); Some(NoAst::Passar) }
            TipoToken::Ceder => { self.avancar(); let valor = if self.fim_item() { None } else { self.expressao(0) }; Some(NoAst::Ceder(valor)) }
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
    fn funcao_modificada(&mut self, assincrona: bool, mut visibilidade: Option<Visibilidade>) -> Option<NoAst> {
        let mut estatica = false;
        loop {
            match self.tipo() {
                TipoToken::Estatico => { estatica = true; self.avancar(); }
                TipoToken::Publico => { visibilidade = Some(Visibilidade::Publico); self.avancar(); }
                TipoToken::Privado => { visibilidade = Some(Visibilidade::Privado); self.avancar(); }
                TipoToken::Protegido => { visibilidade = Some(Visibilidade::Protegido); self.avancar(); }
                _ => break,
            }
        }
        if !self.corresponde(TipoToken::Funcao) {
            self.erro_atual("N2002", "Modificadores devem anteceder uma declaração de função.");
            return None;
        }
        let mut funcao = self.funcao_corpo(visibilidade, false)?;
        if let NoAst::Funcao { assincrona: destino_assinc, estatica: destino_estatico, .. } = &mut funcao {
            *destino_assinc = assincrona;
            *destino_estatico = estatica;
        }
        Some(funcao)
    }

    fn funcao_corpo(&mut self, visibilidade: Option<Visibilidade>, externa: bool) -> Option<NoAst> {
        let nome = if self.verifica(TipoToken::Construtor) { self.avancar().lexema } else { self.consumir_nome("Esperado nome da função.")? };
        let mut genericos = Vec::new();
        if self.corresponde(TipoToken::Menor) {
            let mut generico_sem_parametro = false;
            if self.verifica(TipoToken::Maior) {
                self.erro_atual("N2015", "Uma função genérica precisa declarar ao menos um parâmetro de tipo.");
                generico_sem_parametro = true;
            } else {
                loop {
                    let intervalo_parametro = self.tokens.get(self.atual).map(|token| token.intervalo);
                    match self.consumir_nome("Esperado nome de parâmetro genérico.") {
                        Some(parametro) => {
                            if genericos.contains(&parametro) {
                                self.erro_intervalo("N2014", "Parâmetro genérico duplicado.", intervalo_parametro);
                            } else {
                                genericos.push(parametro);
                            }
                        }
                        None => {
                            self.sincronizar_ate(&[
                                TipoToken::Virgula,
                                TipoToken::Maior,
                                TipoToken::AbrePar,
                                TipoToken::AbreChave,
                            ]);
                        }
                    }
                    if !self.corresponde(TipoToken::Virgula) { break; }
                    self.pular_novas_linhas();
                    if self.verifica(TipoToken::Maior) {
                        self.erro_atual("N2015", "Esperado parâmetro genérico após vírgula.");
                        generico_sem_parametro = true;
                        break;
                    }
                }
            }
            self.esperar(TipoToken::Maior, "Esperado '>' após parâmetros genéricos.");
            if genericos.is_empty() && !generico_sem_parametro {
                self.erro_atual("N2015", "Uma função genérica precisa declarar ao menos um parâmetro de tipo.");
            }
        }
        if !self.corresponde(TipoToken::AbrePar) {
            self.erro_atual("N2004", "Esperado '(' após nome da função.");
            self.sincronizar_ate(&[TipoToken::AbrePar, TipoToken::AbreChave, TipoToken::NovaLinha]);
            self.corresponde(TipoToken::AbrePar);
        }
        let mut parametros = Vec::new();
        let mut parametros_padrao = Vec::new();
        if !self.verifica(TipoToken::FechaPar) && !self.verifica(TipoToken::AbreChave) {
            loop {
                let intervalo_parametro = self.tokens.get(self.atual).map(|token| token.intervalo);
                let n = match self.consumir_nome("Esperado nome do parâmetro.") {
                    Some(nome) => nome,
                    None => {
                        self.sincronizar_ate(&[
                            TipoToken::Virgula,
                            TipoToken::FechaPar,
                            TipoToken::AbreChave,
                            TipoToken::NovaLinha,
                        ]);
                        if self.corresponde(TipoToken::Virgula) { continue; }
                        break;
                    }
                };
                if !self.corresponde(TipoToken::DoisPontos) {
                    self.erro_atual("N2005", "Esperado ':' no parâmetro.");
                    self.sincronizar_ate(&[
                        TipoToken::Virgula,
                        TipoToken::FechaPar,
                        TipoToken::AbreChave,
                        TipoToken::NovaLinha,
                    ]);
                    if self.corresponde(TipoToken::Virgula) { continue; }
                    break;
                }
                let t = match self.caminho_tipo() {
                    Some(tipo) => tipo,
                    None => {
                        self.sincronizar_ate(&[
                            TipoToken::Virgula,
                            TipoToken::FechaPar,
                            TipoToken::AbreChave,
                            TipoToken::NovaLinha,
                        ]);
                        if self.corresponde(TipoToken::Virgula) { continue; }
                        break;
                    }
                };
                let padrao = if self.corresponde(TipoToken::Atribui) { self.expressao(0) } else { None };
                if parametros.iter().any(|(existente, _)| existente == &n) {
                    self.erro_intervalo("N2050", "Parâmetro declarado mais de uma vez na função.", intervalo_parametro);
                } else {
                    parametros.push((n, t));
                    parametros_padrao.push(padrao);
                }
                if !self.corresponde(TipoToken::Virgula) { break; }
                if self.verifica(TipoToken::FechaPar) {
                    self.erro_atual("N2005", "Esperado parâmetro após vírgula.");
                    break;
                }
            }
        }
        self.esperar(TipoToken::FechaPar, "Esperado ')' após parâmetros.");
        let retorno = if self.corresponde(TipoToken::Seta) {
            Some(self.caminho_tipo()?)
        } else if self.corresponde(TipoToken::DoisPontos) {
            if matches!(self.tipo(), TipoToken::NovaLinha | TipoToken::Indent | TipoToken::AbreChave | TipoToken::Fim) {
                None
            } else {
                Some(self.caminho_tipo()?)
            }
        } else {
            None
        };
        let corpo = if externa { Vec::new() } else { self.bloco()? };
        let funcao = NoAst::Funcao {
            visibilidade,
            assincrona: false,
            estatica: false,
            nome,
            genericos,
            parametros,
            parametros_padrao,
            retorno,
            corpo,
        };
        if externa {
            Some(NoAst::Atributada {
                atributos: vec![AtributoAst {
                    nome: "externa".into(),
                    argumentos: Vec::new(),
                }],
                declaracao: Box::new(funcao),
            })
        } else {
            Some(funcao)
        }
    }
    fn atributada(&mut self) -> Option<NoAst> {
        let mut atributos = Vec::new();
        while self.corresponde(TipoToken::Arroba) {
            let nome = self.consumir_nome("Esperado nome do atributo após '@'.")?;
            let mut argumentos = Vec::new();
            if self.corresponde(TipoToken::AbrePar) {
                if !self.verifica(TipoToken::FechaPar) {
                    loop {
                        argumentos.push(self.expressao(0)?);
                        if !self.corresponde(TipoToken::Virgula) { break; }
                    }
                }
                self.esperar(TipoToken::FechaPar, "Esperado ')' após argumentos do atributo.");
            }
            atributos.push(AtributoAst { nome, argumentos });
            self.pular_novas_linhas();
        }
        let declaracao = self.item()?;
        Some(NoAst::Atributada { atributos, declaracao: Box::new(declaracao) })
    }

    fn bloco_contextual(&mut self) -> Option<NoAst> {
        let token = self.avancar();
        let contexto = token.lexema;
        let argumento = if matches!(contexto.as_str(), "privilegio" | "montagem")
            || (contexto == "seguro" && self.verifica(TipoToken::Identificador)) {
            self.consumir_nome("Esperado identificador após o contexto.")
        } else { None };
        let mut tipos_genericos = Vec::new();
        let mut argumentos = Vec::new();
        if self.corresponde(TipoToken::Menor) {
            while !self.verifica(TipoToken::Maior) && !self.verifica(TipoToken::Fim) {
                tipos_genericos.push(self.caminho_tipo()?);
                if !self.corresponde(TipoToken::Virgula) { break; }
                self.pular_novas_linhas();
            }
            self.esperar(TipoToken::Maior, "Esperado '>' após parâmetros do contexto.");
        }
        if self.corresponde(TipoToken::AbrePar) {
            if !self.verifica(TipoToken::FechaPar) {
                loop {
                    argumentos.push(self.expressao(0)?);
                    if !self.corresponde(TipoToken::Virgula) { break; }
                }
            }
            self.esperar(TipoToken::FechaPar, "Esperado ')' após argumentos do contexto.");
        }
        if self.corresponde(TipoToken::DoisPontos)
            || self.verifica(TipoToken::NovaLinha)
            || self.verifica(TipoToken::Indent)
        {
            let corpo = self.bloco()?;
            Some(NoAst::BlocoContextual { contexto, argumento, tipos_genericos, argumentos, corpo })
        } else if self.corresponde(TipoToken::FimBloco) {
            Some(NoAst::BlocoContextual { contexto, argumento, tipos_genericos, argumentos, corpo: Vec::new() })
        } else {
            self.erro_atual("N2007", "Esperado bloco após declaração de contexto.");
            None
        }
    }

    fn tente(&mut self) -> Option<NoAst> {
        self.avancar();
        let corpo = self.bloco_sem_fim()?;
        let mut capturas = Vec::new();
        while self.corresponde(TipoToken::Erro) {
            let tipo = if self.verifica(TipoToken::Identificador) {
                Some(self.avancar().lexema)
            } else { None };
            let corpo_captura = self.bloco_sem_fim()?;
            capturas.push(CapturaAst { tipo, corpo: corpo_captura });
            self.pular_novas_linhas();
        }
        let sempre = if self.corresponde(TipoToken::Sempre) {
            self.bloco_sem_fim()?
        } else { Vec::new() };
        if self.verifica(TipoToken::FimBloco) {
            self.avancar();
        } else {
            self.erro_atual("N2009", "Bloco 'tente' precisa terminar com 'fim'.");
        }
        if capturas.is_empty() && sempre.is_empty() {
            self.erro_atual("N2007", "'tente' precisa de ao menos uma cláusula 'erro' ou 'sempre'.");
        }
        Some(NoAst::Tente { corpo, capturas, sempre })
    }

    fn com(&mut self) -> Option<NoAst> {
        self.avancar();
        let recurso = self.expressao_antes_de_bloco()?;
        let nome = if self.corresponde(TipoToken::Como) {
            self.consumir_nome("Esperado nome após 'como' em bloco 'com'.")
        } else { None };
        let corpo = self.bloco()?;
        Some(NoAst::Com { recurso, nome, corpo })
    }

    fn classe(&mut self) -> Option<NoAst> {
        self.avancar();
        let nome = self.consumir_nome("Esperado nome após 'classe'.")?;
        let herda = if self.corresponde(TipoToken::Herda) {
            self.consumir_nome("Esperado nome da classe-base após 'herda'.")
        } else { None };
        let corpo = self.bloco()?;
        Some(NoAst::Classe { nome, herda, corpo })
    }

    fn tipo_alias(&mut self) -> Option<NoAst> {
        self.avancar(); let nome = self.consumir_nome("Esperado nome após 'tipo'.")?;
        if !self.corresponde(TipoToken::Atribui) { self.erro_atual("N2006", "Esperado '=' na declaração de tipo."); return None; }
        self.pular_novas_linhas();
        Some(NoAst::Tipo { nome, destino: self.caminho_tipo()? })
    }
    fn abrir_declaracao_indentada(
        &mut self,
        codigo: &'static str,
        mensagem: &str,
    ) -> Option<bool> {
        if self.corresponde(TipoToken::AbreChave) {
            return Some(false);
        }
        if !self.corresponde(TipoToken::DoisPontos) {
            self.erro_atual(codigo, mensagem);
            return None;
        }
        let houve_quebra = self.corresponde(TipoToken::NovaLinha);
        if !houve_quebra && !self.verifica(TipoToken::Indent) && !self.verifica(TipoToken::FimBloco) {
            self.erro_atual(codigo, "Esperada quebra de linha e indentação após ':'.");
            return None;
        }
        if houve_quebra {
            self.separadores();
        }
        if self.corresponde(TipoToken::Indent) || self.verifica(TipoToken::FimBloco) {
            Some(true)
        } else {
            self.erro_atual(codigo, "Esperada indentação no corpo da declaração.");
            None
        }
    }

    fn fechar_declaracao_indentada(&mut self, indentado: bool, mensagem: &str) {
        if indentado {
            self.corresponde(TipoToken::Dedent);
            self.esperar(TipoToken::FimBloco, mensagem);
        } else {
            self.esperar(TipoToken::FechaChave, mensagem);
        }
    }

    fn estrutura(&mut self) -> Option<NoAst> {
        self.avancar();
        let nome = self.consumir_nome("Esperado nome após 'estrutura'.")?;
        let indentado = self.abrir_declaracao_indentada(
            "N2030",
            "Esperado '{' ou ':' após o nome da estrutura.",
        )?;
        let fechamento = if indentado { TipoToken::Dedent } else { TipoToken::FechaChave };
        self.separadores();
        let mut campos = Vec::new();
        while !(self.verifica(fechamento)
            || self.verifica(TipoToken::Fim)
            || (indentado && self.verifica(TipoToken::FimBloco)))
        {
            let intervalo_campo = self.tokens.get(self.atual).map(|token| token.intervalo);
            let campo = match self.consumir_nome("Esperado nome do campo da estrutura.") {
                Some(campo) => campo,
                None => {
                    self.sincronizar_ate(&[
                        TipoToken::Virgula,
                        TipoToken::NovaLinha,
                        TipoToken::PontoVirgula,
                        fechamento,
                        TipoToken::FimBloco,
                    ]);
                    if self.corresponde(TipoToken::Virgula) || self.separador_atual() {
                        self.separadores();
                        continue;
                    }
                    break;
                }
            };
            if !self.corresponde(TipoToken::DoisPontos) {
                self.erro_atual("N2032", "Esperado ':' após nome do campo da estrutura.");
                self.sincronizar_ate(&[
                    TipoToken::Virgula,
                    TipoToken::NovaLinha,
                    TipoToken::PontoVirgula,
                    fechamento,
                    TipoToken::FimBloco,
                ]);
                if self.corresponde(TipoToken::Virgula) || self.separador_atual() {
                    self.separadores();
                    continue;
                }
                break;
            }
            let tipo = match self.caminho_tipo() {
                Some(tipo) => tipo,
                None => {
                    self.sincronizar_ate(&[
                        TipoToken::Virgula,
                        TipoToken::NovaLinha,
                        TipoToken::PontoVirgula,
                        fechamento,
                        TipoToken::FimBloco,
                    ]);
                    if self.corresponde(TipoToken::Virgula) || self.separador_atual() {
                        self.separadores();
                        continue;
                    }
                    break;
                }
            };
            if campos.iter().any(|(existente, _)| existente == &campo) {
                self.erro_intervalo("N2031", "Campo declarado mais de uma vez na estrutura.", intervalo_campo);
            } else {
                campos.push((campo, tipo));
            }
            if !(self.corresponde(TipoToken::Virgula)
                || self.separador_atual()
                || self.verifica(fechamento)
                || (indentado && self.verifica(TipoToken::FimBloco)))
            {
                self.erro_atual("N2032", "Esperado ',' ou quebra de linha entre campos.");
                self.sincronizar();
            }
            self.separadores();
        }
        self.fechar_declaracao_indentada(
            indentado,
            if indentado {
                "Estrutura precisa terminar com 'fim'."
            } else {
                "Estrutura não foi fechada com '}'."
            },
        );
        Some(NoAst::Estrutura { nome, campos })
    }

    fn enumeracao(&mut self) -> Option<NoAst> {
        self.avancar();
        let nome = self.consumir_nome("Esperado nome após 'enumeracao'.")?;
        let indentado = self.abrir_declaracao_indentada(
            "N2033",
            "Esperado '{' ou ':' após o nome da enumeração.",
        )?;
        let fechamento = if indentado { TipoToken::Dedent } else { TipoToken::FechaChave };
        self.separadores();
        let mut variantes: Vec<(String, Vec<String>)> = Vec::new();
        while !(self.verifica(fechamento)
            || self.verifica(TipoToken::Fim)
            || (indentado && self.verifica(TipoToken::FimBloco)))
        {
            let intervalo_variante = self.tokens.get(self.atual).map(|token| token.intervalo);
            let variante = match self.consumir_nome("Esperado nome da variante.") {
                Some(variante) => variante,
                None => {
                    self.sincronizar_ate(&[
                        TipoToken::Virgula,
                        TipoToken::NovaLinha,
                        TipoToken::PontoVirgula,
                        fechamento,
                        TipoToken::FimBloco,
                    ]);
                    if self.corresponde(TipoToken::Virgula) || self.separador_atual() {
                        self.separadores();
                        continue;
                    }
                    break;
                }
            };
            let mut payload = Vec::new();
            if self.corresponde(TipoToken::AbrePar) {
                if !self.verifica(TipoToken::FechaPar) {
                    loop {
                        match self.caminho_tipo() {
                            Some(tipo) => payload.push(tipo),
                            None => {
                                self.sincronizar_ate(&[TipoToken::Virgula, TipoToken::FechaPar]);
                                if self.corresponde(TipoToken::Virgula) { continue; }
                                break;
                            }
                        }
                        if !self.corresponde(TipoToken::Virgula) { break; }
                        if self.verifica(TipoToken::FechaPar) {
                            self.erro_atual("N2035", "Esperado tipo após vírgula no payload da variante.");
                            break;
                        }
                    }
                }
                self.esperar(TipoToken::FechaPar, "Esperado ')' após tipos da variante.");
            }
            if variantes.iter().any(|(existente, _)| existente == &variante) {
                self.erro_intervalo("N2034", "Variante declarada mais de uma vez na enumeração.", intervalo_variante);
            } else {
                variantes.push((variante, payload));
            }
            if !(self.corresponde(TipoToken::Virgula)
                || self.separador_atual()
                || self.verifica(fechamento)
                || (indentado && self.verifica(TipoToken::FimBloco)))
            {
                self.erro_atual("N2035", "Esperado ',' ou quebra de linha entre variantes.");
                self.sincronizar();
            }
            self.separadores();
        }
        self.fechar_declaracao_indentada(
            indentado,
            if indentado {
                "Enumeração precisa terminar com 'fim'."
            } else {
                "Enumeração não foi fechada com '}'."
            },
        );
        if variantes.is_empty() {
            self.erro_atual("N2036", "Uma enumeração precisa declarar ao menos uma variante.");
        }
        Some(NoAst::Enumeracao { nome, variantes })
    }

    fn incluir(&mut self) -> Option<NoAst> {
        self.avancar();
        if matches!(self.tipo(), TipoToken::Biblioteca | TipoToken::Modulo) { self.avancar(); }
        let primeiro = self.caminho_modulo()?;
        let caminho = if self.corresponde(TipoToken::De) {
            let origem = self.caminho_modulo()?;
            format!("{origem}::{primeiro}")
        } else { primeiro };
        let apelido = if self.corresponde(TipoToken::Como) { self.consumir_nome("Esperado apelido após 'como'.") } else { None };
        Some(NoAst::Incluir { caminho, apelido })
    }
    fn quando(&mut self) -> Option<NoAst> {
        self.com_profundidade(|parser| parser.quando_interno())
    }
    fn quando_interno(&mut self) -> Option<NoAst> {
        self.avancar();
        let condicao = self.expressao_antes_de_bloco()?;
        let entao = self.bloco_sem_fim()?;
        let mut alternativas = Vec::new();
        while self.proximo_tipo_significativo() == TipoToken::Sendo {
            self.pular_novas_linhas();
            self.avancar();
            let condicao_alternativa = self.expressao_antes_de_bloco()?;
            let corpo_alternativo = self.bloco_sem_fim()?;
            alternativas.push((condicao_alternativa, corpo_alternativo));
        }
        let mut senao = if self.proximo_tipo_significativo() == TipoToken::Senao {
            self.pular_novas_linhas();
            self.avancar();
            if matches!(self.proximo_tipo_significativo(), TipoToken::Quando | TipoToken::Se) {
                self.pular_novas_linhas();
                vec![self.quando()?]
            } else {
                self.bloco_sem_fim()?
            }
        } else {
            Vec::new()
        };
        if self.verifica(TipoToken::FimBloco) {
            self.avancar();
        } else {
            self.erro_atual("N2009", "Condicional precisa terminar com 'fim'.");
        }
        for (condicao_alternativa, corpo_alternativo) in alternativas.into_iter().rev() {
            senao = vec![NoAst::Quando {
                condicao: condicao_alternativa,
                entao: corpo_alternativo,
                senao,
            }];
        }
        Some(NoAst::Quando { condicao, entao, senao })
    }
    fn escolha(&mut self) -> Option<NoAst> {
        self.avancar();
        let valor = self.expressao_antes_de_bloco()?;
        self.corresponde(TipoToken::DoisPontos);
        let delimitado_por_chaves = self.corresponde(TipoToken::AbreChave);
        if delimitado_por_chaves {
            self.erro_atual("N2040", "'escolha' exige indentação; chaves não substituem um bloco indentado.");
        }
        let indentado = if delimitado_por_chaves {
            false
        } else {
            let houve_quebra = self.corresponde(TipoToken::NovaLinha);
            if !houve_quebra && !self.verifica(TipoToken::Indent) {
                self.erro_atual("N2040", "Esperada quebra de linha antes dos casos de escolha.");
                return None;
            }
            if houve_quebra {
                self.separadores();
            }
            if !self.corresponde(TipoToken::Indent) {
                self.erro_atual("N2040", "Esperada indentação antes dos casos de escolha.");
                return None;
            }
            true
        };
        self.separadores();
        let mut casos = Vec::new();
        let mut possui_curinga = false;
        let mut padroes_vistos: Vec<(String, String)> = Vec::new();
        while !(if delimitado_por_chaves { self.verifica(TipoToken::FechaChave) } else { self.verifica(TipoToken::Dedent) })
            && !self.verifica(TipoToken::Fim) && !self.verifica(TipoToken::FimBloco) {
            let padrao_padrao = self.corresponde(TipoToken::Padrao);
            if !padrao_padrao && !self.corresponde(TipoToken::Caso) {
                self.erro_atual("N2041", "Esperado 'caso' ou 'padrao' dentro de escolha.");
                self.sincronizar();
                self.separadores();
                continue;
            }
            let inicio_caso = self.tokens.get(self.atual.saturating_sub(1)).map(|token| token.intervalo.inicio);
            let mut padrao_valido = true;
            let mut intervalo_padrao = self.tokens.get(self.atual.saturating_sub(1)).map(|token| token.intervalo);
            let (enumeracao, variante, vinculacoes, curinga, expressao_padrao) =
                if padrao_padrao {
                    (None, None, Vec::new(), true, None)
                } else if self.verifica(TipoToken::Identificador)
                    && self.tokens.get(self.atual).is_some_and(|token| token.lexema == "_")
                {
                    intervalo_padrao = self.tokens.get(self.atual).map(|token| token.intervalo);
                    self.avancar();
                    (None, None, Vec::new(), true, None)
                } else if self.verifica(TipoToken::Identificador) {
                    intervalo_padrao = self.tokens.get(self.atual).map(|token| token.intervalo);
                    let primeiro = self.avancar().lexema.nfc().collect::<String>();
                    if self.corresponde(TipoToken::Escopo) {
                        if let Some(variante) = self.consumir_nome("Esperado variante após '::'.") {
                            let mut vinculacoes = Vec::new();
                            if self.corresponde(TipoToken::AbrePar) {
                                if !self.verifica(TipoToken::FechaPar) {
                                    loop {
                                        let intervalo_vinculacao = self.tokens.get(self.atual).map(|token| token.intervalo);
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
                                                self.erro_intervalo("N2046", "Vínculo repetido no payload do padrão.", intervalo_vinculacao);
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
                            (Some(primeiro), Some(variante), vinculacoes, false, None)
                        } else {
                            padrao_valido = false;
                            (Some(primeiro), None, Vec::new(), false, None)
                        }
                    } else {
                        self.erro_atual("N2042", "Padrão deve ser uma variante Enum::Variante ou '_'.");
                        padrao_valido = false;
                        (None, None, Vec::new(), false, None)
                    }
                } else if matches!(self.tipo(), TipoToken::Inteiro | TipoToken::Decimal | TipoToken::Texto | TipoToken::TextoInterpolado | TipoToken::Caractere | TipoToken::Sim | TipoToken::Verdadeiro | TipoToken::Falso | TipoToken::Nulo) {
                    let expressao = self.expressao(0);
                    (None, None, Vec::new(), false, expressao)
                } else {
                    self.erro_atual("N2042", "Esperado padrão de enumeração, literal ou '_'.");
                    padrao_valido = false;
                    (None, None, Vec::new(), false, None)
                };

            if curinga {
                if self.verifica(TipoToken::AbrePar) {
                    self.erro_atual("N2042", "O padrão curinga '_' não aceita payload.");
                    padrao_valido = false;
                }
                if padrao_valido {
                    if possui_curinga {
                        self.erro_intervalo("N2043", "Caso curinga duplicado.", intervalo_padrao);
                        padrao_valido = false;
                    } else {
                        possui_curinga = true;
                    }
                }
            }

            if self.verifica(TipoToken::Escopo) {
                self.erro_atual("N2042", "Um padrão deve conter exatamente Enum::Variante.");
                padrao_valido = false;
            }
            if let (Some(enumeracao), Some(variante)) = (&enumeracao, &variante) {
                let identidade = (enumeracao.clone(), variante.clone());
                if padrao_valido && padroes_vistos.contains(&identidade) {
                    self.erro_intervalo("N2048", "Padrão de enumeração duplicado em escolha.", intervalo_padrao);
                    padrao_valido = false;
                } else if padrao_valido {
                    padroes_vistos.push(identidade);
                }
            }
            if !self.verifica(TipoToken::NovaLinha) {
                self.erro_atual(
                    "N2042",
                    "Esperado bloco imediatamente após o padrão do caso; tokens extras não são permitidos.",
                );
                padrao_valido = false;
                while !self.verifica(TipoToken::Fim)
                    && !self.verifica(TipoToken::AbreChave)
                    && !self.verifica(TipoToken::NovaLinha)
                    && !self.verifica(TipoToken::FechaChave)
                    && !self.verifica(TipoToken::Caso)
                {
                    self.avancar();
                }
            }
            let corpo = match if indentado { self.bloco_sem_fim() } else { self.bloco() } {
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
                let caso = CasoEscolha { enumeracao, variante, vinculacoes, expressao: expressao_padrao, corpo };
                if let (Some(inicio), Some(token)) = (inicio_caso, self.tokens.get(self.atual.saturating_sub(1))) {
                    if let Some(intervalo) = IntervaloFonte::novo(inicio, token.intervalo.fim) {
                        self.casos_localizados.push(CasoEscolhaLocalizado {
                            caso: caso.clone(),
                            intervalo,
                        });
                    }
                }
                casos.push(caso);
            }
            self.separadores();
        }
        if delimitado_por_chaves {
            self.esperar(TipoToken::FechaChave, "Escolha não foi fechada com '}'.");
            self.erro_atual("N2009", "'escolha' deve terminar com 'fim' e usar indentação.");
        } else {
            self.esperar(TipoToken::Dedent, "Bloco de escolha não foi dedentado corretamente.");
            if self.verifica(TipoToken::FimBloco) { self.avancar(); }
            else { self.erro_atual("N2009", "Escolha precisa terminar com 'fim'."); }
        }
        if casos.is_empty() { self.erro_atual("N2044", "Escolha precisa conter ao menos um caso."); }
        Some(NoAst::Escolha { valor, casos })
    }
    fn enquanto(&mut self) -> Option<NoAst> {
        self.avancar(); let condicao = self.expressao_antes_de_bloco()?; let corpo = self.bloco()?;
        Some(NoAst::Enquanto { condicao, corpo })
    }
    fn cada(&mut self) -> Option<NoAst> {
        self.avancar();
        let nome = self.consumir_nome("Esperado nome da variável após 'cada'.")?;
        self.esperar(TipoToken::Em, "Esperado 'em' após o nome da variável de 'cada'.");
        let iteravel = self.expressao_antes_de_bloco()?;
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
        self.avancar();
        if !self.corresponde(TipoToken::AbrePar) {
            let nome = self.consumir_nome("Esperado variável após 'para'.")?;
            self.esperar(TipoToken::Dentro, "Esperado 'dentro' após variável do laço.");
            let iteravel = self.expressao_antes_de_bloco()?;
            let corpo = self.bloco()?;
            return Some(NoAst::Cada { nome, iteravel, corpo });
        }

        let inicial = if self.verifica(TipoToken::PontoVirgula) {
            None
        } else if self.verifica(TipoToken::Variavel) {
            self.declaracao_variavel(false).map(Box::new)
        } else {
            self.expressao(0).map(|expressao| Box::new(NoAst::Expressao(expressao)))
        };
        self.esperar(TipoToken::PontoVirgula, "Esperado ';' após inicialização do laço.");
        let condicao = if self.verifica(TipoToken::PontoVirgula) { None } else { self.expressao(0) };
        self.esperar(TipoToken::PontoVirgula, "Esperado ';' após condição do laço.");
        let atualizacao = if self.verifica(TipoToken::FechaPar) { None } else { self.expressao(0) };
        self.esperar(TipoToken::FechaPar, "Esperado ')' após cabeçalho do laço.");
        let corpo = self.bloco()?;
        Some(NoAst::Para { inicial, condicao, atualizacao, corpo })
    }
    fn bloco(&mut self) -> Option<Vec<NoAst>> {
        self.com_profundidade(|parser| parser.bloco_interno(true))
    }
    fn bloco_sem_fim(&mut self) -> Option<Vec<NoAst>> {
        self.com_profundidade(|parser| parser.bloco_interno(false))
    }
    fn bloco_interno(&mut self, consumir_fim: bool) -> Option<Vec<NoAst>> {
        self.corresponde(TipoToken::DoisPontos);
        if self.verifica(TipoToken::NovaLinha) {
            let proximo = self.tokens.iter().skip(self.atual)
                .find(|token| token.tipo != TipoToken::NovaLinha)
                .map(|token| token.tipo);
            if proximo == Some(TipoToken::AbreChave) {
                self.pular_novas_linhas();
            }
        }
        if self.corresponde(TipoToken::AbreChave) {
            self.erro_atual("N2009", "Blocos de instruções exigem indentação; chaves não substituem indentação.");
            self.separadores();
            let mut itens = Vec::new();
            while !self.verifica(TipoToken::FechaChave) && !self.verifica(TipoToken::Fim) {
                let antes = self.atual;
                let diagnosticos_antes = self.diagnosticos.len();
                if self.corresponde(TipoToken::Indent) {
                    continue;
                }
                if self.corresponde(TipoToken::Dedent) {
                    continue;
                }
                let no = self.item();
                let bloco_recuperado = no.as_ref().is_some_and(Self::eh_bloco)
                    && self.diagnosticos.len() > diagnosticos_antes
                    && self.inicia_item();
                if let Some(no) = no { itens.push(no); }
                if self.atual == antes { self.avancar(); }
                if self.fechamento_incompativel_com_chaves() {
                    self.erro_atual("N2051", "Delimitador de fechamento incompatível com o bloco aberto.");
                    self.avancar();
                } else if !self.verifica(TipoToken::FechaChave) && !self.separador_atual()
                    && !bloco_recuperado {
                    self.erro_atual("N2001", "Esperado separador dentro do bloco.");
                    self.sincronizar_item();
                }
                self.separadores();
            }
            self.esperar(TipoToken::FechaChave, "Bloco aberto com '{' não foi fechado com '}'.");
            if consumir_fim {
                self.erro_atual("N2009", "Blocos de instruções devem usar indentação e terminar com 'fim'.");
            }
            return Some(itens);
        }

        let houve_quebra = self.corresponde(TipoToken::NovaLinha);
        if !houve_quebra && !self.verifica(TipoToken::Indent) {
            self.erro_atual("N2007", "Esperada quebra de linha antes do bloco indentado.");
            return None;
        }
        if houve_quebra {
            self.separadores();
        }

        let indentado = self.corresponde(TipoToken::Indent);
        if !indentado {
            self.erro_atual("N2009", "Todo bloco precisa iniciar com indentação.");
            if self.verifica(TipoToken::FimBloco) {
                self.avancar();
            }
            return Some(Vec::new());
        }
        let mut profundidade_indentacao = 1usize;
        let mut itens = Vec::new();

        loop {
            if self.verifica(TipoToken::Fim) || self.verifica(TipoToken::Senao)
                || self.verifica(TipoToken::Sendo) || self.verifica(TipoToken::Erro)
                || self.verifica(TipoToken::Sempre) {
                break;
            }
            if indentado && profundidade_indentacao == 0 {
                break;
            }
            if self.corresponde(TipoToken::Indent) {
                self.erro_atual("N2009", "Indentação inesperada: cada nível deve pertencer a um bloco explícito.");
                let mut profundidade_inesperada = 1usize;
                while profundidade_inesperada > 0 && !self.verifica(TipoToken::Fim) {
                    match self.tipo() {
                        TipoToken::Indent => profundidade_inesperada += 1,
                        TipoToken::Dedent => profundidade_inesperada -= 1,
                        _ => {}
                    }
                    self.avancar();
                }
                continue;
            }
            if self.corresponde(TipoToken::Dedent) {
                profundidade_indentacao = profundidade_indentacao.saturating_sub(1);
                if indentado && profundidade_indentacao == 0 {
                    break;
                }
                continue;
            }

            let antes = self.atual;
            let diagnosticos_antes = self.diagnosticos.len();
            let no = self.item();
            let bloco_recuperado = no.as_ref().is_some_and(Self::eh_bloco)
                && self.diagnosticos.len() > diagnosticos_antes
                && self.inicia_item();
            if let Some(no) = no { itens.push(no); }
            if self.atual == antes { self.avancar(); }

            if self.fechamento_incompativel_com_fim() {
                self.erro_atual("N2051", "Delimitador incompatível com o bloco iniciado em nova linha.");
                self.avancar();
            } else if !(self.separador_atual()
                || self.verifica(TipoToken::FimBloco)
                || self.verifica(TipoToken::Fim)
                || bloco_recuperado
                || (indentado && self.verifica(TipoToken::Dedent))) {
                self.erro_atual("N2001", "Esperado separador dentro do bloco.");
                self.sincronizar_item();
            }
            self.separadores();
        }

        if profundidade_indentacao > 0 {
            self.erro_atual("N2009", "Bloco indentado não foi fechado corretamente.");
        }
        if consumir_fim {
            if self.verifica(TipoToken::FimBloco) {
                self.avancar();
            } else {
                self.erro_atual("N2009", "Bloco indentado precisa terminar com 'fim'.");
            }
        }
        Some(itens)
    }
    fn caminho_tipo(&mut self) -> Option<String> {
        let inicio = self.tokens.get(self.atual).map(|token| token.intervalo.inicio);
        let tipo = self.com_profundidade(|parser| parser.caminho_tipo_interno());
        if let (Some(tipo), Some(inicio)) = (tipo.as_ref(), inicio) {
            if self.atual > 0 {
                if let Some(token) = self.tokens.get(self.atual - 1) {
                    if let Some(intervalo) = IntervaloFonte::novo(inicio, token.intervalo.fim) {
                        self.tipos_localizados.push(TipoLocalizado {
                            tipo: TipoAst::analisar(tipo),
                            intervalo,
                        });
                    }
                }
            }
        }
        tipo
    }
    fn caminho_tipo_interno(&mut self) -> Option<String> {
        if self.corresponde(TipoToken::Vazio) {
            return Some("vazio".into());
        }
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
            if self.verifica(TipoToken::Inteiro) {
                argumentos.push(self.avancar().lexema);
            } else {
                argumentos.push(self.caminho_tipo()?);
            }
            if !self.corresponde(TipoToken::Virgula) { break; }
            self.pular_novas_linhas();
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
        let inicio = self.tokens.get(self.atual).map(|token| token.intervalo.inicio);
        let expressao = self.com_profundidade(|parser| parser.expressao_interna(minimo));
        if let (Some(expressao), Some(inicio)) = (expressao.as_ref(), inicio) {
            if self.atual > 0 {
                if let Some(token) = self.tokens.get(self.atual - 1) {
                    if let Some(intervalo) = IntervaloFonte::novo(inicio, token.intervalo.fim) {
                        self.expressoes_localizadas.push(ExpressaoLocalizada {
                            expressao: expressao.clone(),
                            intervalo,
                        });
                    }
                }
            }
        }
        expressao
    }
    fn expressao_antes_de_bloco(&mut self) -> Option<Expressao> {
        let anterior = self.bloquear_construcao_vazia;
        self.bloquear_construcao_vazia = true;
        let resultado = self.expressao(0);
        self.bloquear_construcao_vazia = anterior;
        resultado
    }
    fn expressao_interna(&mut self, minimo: u8) -> Option<Expressao> {
        let mut comparacao_relacional = false;
        let mut intervalo_visto = false;
        let mut esquerda = match self.tipo() {
            TipoToken::Incrementa | TipoToken::Decrementa => {
                let operador = self.avancar().tipo;
                let destino = self.expressao(14)?;
                let calculo = Expressao::Binaria {
                    esquerda: Box::new(destino.clone()),
                    operador: if operador == TipoToken::Incrementa { TipoToken::Mais } else { TipoToken::Menos },
                    direita: Box::new(Expressao::Literal("1".into())),
                };
                Expressao::Atribuicao { destino: Box::new(destino), valor: Box::new(calculo) }
            }
            TipoToken::Negacao | TipoToken::NaoBit | TipoToken::EBit | TipoToken::Reticencias | TipoToken::Menos | TipoToken::Mais | TipoToken::Aguarda | TipoToken::Novo | TipoToken::Apagar | TipoToken::Nao if self.tipo() != TipoToken::Nao || self.tokens.get(self.atual + 1).is_some_and(|token| !matches!(token.tipo, TipoToken::NovaLinha | TipoToken::PontoVirgula | TipoToken::FechaPar | TipoToken::FechaColchete | TipoToken::FechaChave | TipoToken::FimBloco | TipoToken::Dedent | TipoToken::Fim | TipoToken::Variavel | TipoToken::Constante | TipoToken::Retorne | TipoToken::Devolver | TipoToken::Pare | TipoToken::Parar | TipoToken::Prossiga | TipoToken::Seguir | TipoToken::Passar | TipoToken::Se | TipoToken::Quando | TipoToken::Sendo | TipoToken::Senao | TipoToken::Para | TipoToken::Enquanto | TipoToken::Escolha | TipoToken::Avaliar | TipoToken::Tente | TipoToken::Com | TipoToken::Lancar | TipoToken::Verifique | TipoToken::Incluir | TipoToken::Classe | TipoToken::Funcao | TipoToken::Assinc | TipoToken::Tipo | TipoToken::Estrutura | TipoToken::Enumeracao | TipoToken::Global | TipoToken::Externa | TipoToken::Estatico | TipoToken::Publico | TipoToken::Privado | TipoToken::Protegido | TipoToken::Arroba | TipoToken::Critico | TipoToken::Seguro | TipoToken::Privilegio | TipoToken::Montagem | TipoToken::Exportar)) => {
                let op = self.avancar().tipo; self.pular_novas_linhas(); let valor = self.expressao(14)?;
                Expressao::Unaria { operador: op, valor: Box::new(valor) }
            }
            TipoToken::Lambda => {
                self.avancar();
                let mut parametros = Vec::new();
                if !self.verifica(TipoToken::DoisPontos) {
                    loop {
                        parametros.push(self.consumir_nome("Esperado parâmetro após 'lambda'.")?);
                        if !self.corresponde(TipoToken::Virgula) { break; }
                    }
                }
                self.esperar(TipoToken::DoisPontos, "Esperado ':' após parâmetros de lambda.");
                let corpo = self.expressao(0)?;
                Expressao::Lambda { parametros, corpo: Box::new(corpo) }
            }
            TipoToken::Montagem => {
                self.avancar();
                let arquitetura = self.consumir_nome("Esperado arquitetura após 'montagem'.")?;
                let mut tipos_genericos = Vec::new();
                if self.corresponde(TipoToken::Menor) {
                    loop {
                        tipos_genericos.push(self.caminho_tipo()?);
                        if !self.corresponde(TipoToken::Virgula) {
                            break;
                        }
                        self.pular_novas_linhas();
                    }
                    self.esperar(TipoToken::Maior, "Esperado '>' após tipo da montagem.");
                }
                self.esperar(TipoToken::AbrePar, "Esperado '(' após arquitetura da montagem.");
                let argumentos = self.argumentos_chamada(TipoToken::FechaPar)?;
                Expressao::Chamada {
                    alvo: Box::new(Expressao::Nome(format!("montagem::{arquitetura}"))),
                    argumentos,
                    tipos_genericos,
                }
            }
            TipoToken::Isto | TipoToken::TipoDe | TipoToken::InstanciaDe | TipoToken::Obter
            | TipoToken::Modulo | TipoToken::Biblioteca => Expressao::Nome(self.avancar().lexema),
            TipoToken::Identificador => {
                let nome = self.avancar().lexema.nfc().collect::<String>();
                let indice_abertura = self.atual;
                let indice_campo = (indice_abertura + 1..self.tokens.len())
                    .find(|indice| !matches!(
                        self.tokens[*indice].tipo,
                        TipoToken::NovaLinha | TipoToken::PontoVirgula
                    ));
                let indice_dois_pontos = indice_campo.and_then(|indice| {
                    (indice + 1..self.tokens.len())
                        .find(|proximo| !matches!(
                            self.tokens[*proximo].tipo,
                            TipoToken::NovaLinha | TipoToken::PontoVirgula
                        ))
                });
                let parece_construcao = self.tokens.get(indice_abertura).is_some_and(|t| t.tipo == TipoToken::AbreChave)
                    && indice_campo.is_some_and(|indice| {
                        self.tokens[indice].tipo == TipoToken::Identificador
                            && (!self.bloquear_construcao_vazia
                                || indice_dois_pontos.is_some_and(|proximo| self.tokens[proximo].tipo == TipoToken::DoisPontos))
                            || self.tokens[indice].tipo == TipoToken::FechaChave
                                && !self.bloquear_construcao_vazia
                    });
                if parece_construcao && self.corresponde(TipoToken::AbreChave) {
                    let mut campos = Vec::new();
                    self.separadores();
                    while !self.verifica(TipoToken::FechaChave) && !self.verifica(TipoToken::Fim) {
                        let intervalo_campo = self.tokens.get(self.atual).map(|token| token.intervalo);
                        let campo = match self.consumir_nome("Esperado nome do campo na construção da estrutura.") {
                            Some(campo) => campo,
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
                        if !self.corresponde(TipoToken::DoisPontos) {
                            self.erro_atual("N2038", "Esperado ':' após nome do campo na construção.");
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
                            self.erro_intervalo("N2037", "Campo informado mais de uma vez na construção.", intervalo_campo);
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
            TipoToken::Inteiro | TipoToken::Decimal | TipoToken::Texto | TipoToken::Caractere | TipoToken::Sim | TipoToken::Verdadeiro | TipoToken::Falso | TipoToken::Nulo => Expressao::Literal(self.avancar().lexema),
            TipoToken::Nao if !self.tokens.get(self.atual + 1).is_some_and(|token| !matches!(token.tipo, TipoToken::NovaLinha | TipoToken::PontoVirgula | TipoToken::FechaPar | TipoToken::FechaColchete | TipoToken::FechaChave | TipoToken::FimBloco | TipoToken::Dedent | TipoToken::Fim)) => Expressao::Literal(self.avancar().lexema),
            TipoToken::AbrePar => {
                self.avancar();
                if self.corresponde(TipoToken::FechaPar) {
                    Expressao::Tupla(Vec::new())
                } else {
                    let bloqueio_anterior = self.bloquear_construcao_vazia;
                    self.bloquear_construcao_vazia = false;
                    let primeiro_resultado = self.expressao(0);
                    self.bloquear_construcao_vazia = bloqueio_anterior;
                    let primeiro = match primeiro_resultado {
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
            TipoToken::FechaPar | TipoToken::FechaColchete => {
                self.erro_atual(
                    "N2051",
                    "Delimitador de fechamento sem abertura correspondente na expressão.",
                );
                return None;
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
            if tipo == TipoToken::Incrementa || tipo == TipoToken::Decrementa {
                let operador = self.avancar().tipo;
                let destino = esquerda.clone();
                let calculo = Expressao::Binaria {
                    esquerda: Box::new(esquerda),
                    operador: if operador == TipoToken::Incrementa { TipoToken::Mais } else { TipoToken::Menos },
                    direita: Box::new(Expressao::Literal("1".into())),
                };
                esquerda = Expressao::Atribuicao {
                    destino: Box::new(destino),
                    valor: Box::new(calculo),
                };
                continue;
            }
            if tipo == TipoToken::Interrogacao && 1 >= minimo {
                self.avancar();
                if matches!(self.tipo(),
                    TipoToken::NovaLinha | TipoToken::PontoVirgula | TipoToken::Virgula
                        | TipoToken::FechaPar | TipoToken::FechaColchete | TipoToken::FechaChave
                        | TipoToken::Dedent | TipoToken::FimBloco | TipoToken::Fim
                        | TipoToken::Variavel | TipoToken::Constante | TipoToken::Retorne
                        | TipoToken::Devolver | TipoToken::Pare | TipoToken::Parar
                        | TipoToken::Prossiga | TipoToken::Seguir | TipoToken::Passar
                        | TipoToken::Se | TipoToken::Quando | TipoToken::Sendo | TipoToken::Senao
                        | TipoToken::Para | TipoToken::Enquanto | TipoToken::Escolha
                        | TipoToken::Avaliar | TipoToken::Tente | TipoToken::Com
                        | TipoToken::Lancar | TipoToken::Verifique | TipoToken::Incluir
                        | TipoToken::Classe | TipoToken::Funcao | TipoToken::Assinc
                        | TipoToken::Tipo | TipoToken::Estrutura | TipoToken::Enumeracao
                        | TipoToken::Global | TipoToken::Externa | TipoToken::Estatico
                        | TipoToken::Publico | TipoToken::Privado | TipoToken::Protegido
                        | TipoToken::Arroba | TipoToken::Critico | TipoToken::Seguro
                        | TipoToken::Privilegio | TipoToken::Montagem | TipoToken::Exportar
                ) {
                    esquerda = Expressao::Unaria {
                        operador: TipoToken::Interrogacao,
                        valor: Box::new(esquerda),
                    };
                } else {
                    let entao = self.expressao(0)?;
                    self.esperar(TipoToken::DoisPontos, "Esperado ':' na expressão condicional.");
                    let senao = self.expressao(1)?;
                    esquerda = Expressao::Condicional {
                        condicao: Box::new(esquerda),
                        entao: Box::new(entao),
                        senao: Box::new(senao),
                    };
                }
                continue;
            }
            if tipo == TipoToken::Menor && 16 >= minimo && self.parece_chamada_generica() {
                self.avancar();
                let mut tipos_genericos = Vec::new();
                loop {
                    tipos_genericos.push(self.caminho_tipo()?);
                    if !self.corresponde(TipoToken::Virgula) { break; }
                    self.pular_novas_linhas();
                }
                self.esperar(TipoToken::Maior, "Esperado '>' após argumentos genéricos da chamada.");
                self.esperar(TipoToken::AbrePar, "Esperado '(' após argumentos genéricos.");
                let argumentos = self.argumentos_chamada(TipoToken::FechaPar)?;
                esquerda = Expressao::Chamada {
                    alvo: Box::new(esquerda),
                    argumentos,
                    tipos_genericos,
                };
                continue;
            }
            if tipo == TipoToken::AbrePar && 16 >= minimo {
                self.avancar();
                let args = self.argumentos_chamada(TipoToken::FechaPar)?;
                esquerda = Expressao::Chamada { alvo: Box::new(esquerda), argumentos: args, tipos_genericos: Vec::new() }; continue;
            }
            if tipo == TipoToken::AbreColchete && 16 >= minimo {
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
            if tipo == TipoToken::Seta && 1 >= minimo {
                self.avancar();
                let parametros = match esquerda {
                    Expressao::Nome(nome) => vec![nome],
                    Expressao::Tupla(valores) => {
                        let mut nomes = Vec::with_capacity(valores.len());
                        for valor in valores {
                            if let Expressao::Nome(nome) = valor {
                                nomes.push(nome);
                            } else {
                                self.erro_atual("N2008", "Parâmetros de função de seta devem ser identificadores.");
                                return None;
                            }
                        }
                        nomes
                    }
                    _ => {
                        self.erro_atual("N2008", "Esperado identificador ou tupla de parâmetros antes de '->'.");
                        return None;
                    }
                };
                if self.verifica(TipoToken::AbreChave) {
                    let corpo = self.bloco()?;
                    esquerda = Expressao::LambdaBloco { parametros, corpo };
                } else {
                    let corpo = self.expressao(0)?;
                    esquerda = Expressao::Lambda { parametros, corpo: Box::new(corpo) };
                }
                continue;
            }
            if matches!(tipo, TipoToken::Ponto | TipoToken::Escopo | TipoToken::AcessoOpcional) && 16 >= minimo {
                let operador = self.avancar().tipo;
                self.pular_novas_linhas();
                let nome = self.consumir_nome_acesso("Esperado nome após operador de acesso.")?;
                esquerda = Expressao::Membro { alvo: Box::new(esquerda), operador, nome };
                continue;
            }
            let (precedencia, direita) = match tipo {
                TipoToken::Atribui | TipoToken::SomaAtribui | TipoToken::SubtraiAtribui
                | TipoToken::MultiplicaAtribui | TipoToken::DivideAtribui
                | TipoToken::DivideInteiraAtribui | TipoToken::RestoAtribui
                | TipoToken::PotenciaAtribui | TipoToken::CoalesceAtribui => (1, true),
                TipoToken::Ou | TipoToken::OuLogico => (2, false),
                TipoToken::Coalesce => (3, true),
                TipoToken::E | TipoToken::ELogico => (4, false),
                TipoToken::OuBit => (5, false),
                TipoToken::XorBit => (6, false),
                TipoToken::EBit => (7, false),
                TipoToken::Igual | TipoToken::Diferente | TipoToken::DiferenteEstrito
                | TipoToken::Iguais | TipoToken::Identico => (8, false),
                TipoToken::Menor | TipoToken::MenorIgual | TipoToken::Maior
                | TipoToken::MaiorIgual | TipoToken::Dentro | TipoToken::Fora
                | TipoToken::TipoDe | TipoToken::InstanciaDe => (9, false),
                TipoToken::Intervalo => (10, false),
                TipoToken::DeslocaEsquerda | TipoToken::DeslocaDireita
                | TipoToken::DeslocaDireitaSemSinal => (11, false),
                TipoToken::Mais | TipoToken::Menos => (12, false),
                TipoToken::Vezes | TipoToken::Divide | TipoToken::DivideInteira
                | TipoToken::Resto => (13, false),
                TipoToken::Potencia => (15, true),
                _ => break,
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
            let indentacao_continuada = self.corresponde(TipoToken::Indent);
            self.pular_novas_linhas();
            let valor = self.expressao(proximo)?;
            if indentacao_continuada {
                self.esperar(
                    TipoToken::Dedent,
                    "Indentação de continuação de expressão não foi encerrada corretamente.",
                );
            }
            let operador_composto = match op {
                TipoToken::SomaAtribui => Some(TipoToken::Mais),
                TipoToken::SubtraiAtribui => Some(TipoToken::Menos),
                TipoToken::MultiplicaAtribui => Some(TipoToken::Vezes),
                TipoToken::DivideAtribui => Some(TipoToken::Divide),
                TipoToken::DivideInteiraAtribui => Some(TipoToken::DivideInteira),
                TipoToken::RestoAtribui => Some(TipoToken::Resto),
                TipoToken::PotenciaAtribui => Some(TipoToken::Potencia),
                TipoToken::CoalesceAtribui => Some(TipoToken::Coalesce),
                _ => None,
            };
            esquerda = if op == TipoToken::Atribui {
                Expressao::Atribuicao { destino: Box::new(esquerda), valor: Box::new(valor) }
            } else if let Some(operador) = operador_composto {
                let destino = esquerda.clone();
                let calculo = Expressao::Binaria {
                    esquerda: Box::new(esquerda),
                    operador,
                    direita: Box::new(valor),
                };
                Expressao::Atribuicao { destino: Box::new(destino), valor: Box::new(calculo) }
            } else {
                Expressao::Binaria { esquerda: Box::new(esquerda), operador: op, direita: Box::new(valor) }
            };
        }
        Some(esquerda)
    }
    fn proximo_tipo_significativo(&self) -> TipoToken {
        self.tokens.iter().skip(self.atual)
            .find(|token| token.tipo != TipoToken::NovaLinha)
            .map_or(TipoToken::Fim, |token| token.tipo)
    }

    fn parece_chamada_generica(&self) -> bool {
        if !matches!(self.tokens.get(self.atual + 1).map(|token| token.tipo),
            Some(TipoToken::Identificador | TipoToken::Inteiro | TipoToken::Vazio | TipoToken::Funcao)) {
            return false;
        }
        let mut profundidade = 0usize;
        for indice in self.atual..self.tokens.len() {
            match self.tokens[indice].tipo {
                TipoToken::Menor => profundidade += 1,
                TipoToken::Maior => {
                    profundidade = profundidade.saturating_sub(1);
                    if profundidade == 0 {
                        return self.tokens.get(indice + 1)
                            .is_some_and(|proximo| proximo.tipo == TipoToken::AbrePar);
                    }
                }
                TipoToken::Fim | TipoToken::NovaLinha | TipoToken::PontoVirgula
                    if profundidade > 0 => return false,
                _ => {}
            }
        }
        false
    }
    fn argumentos_chamada(&mut self, fechamento: TipoToken) -> Option<Vec<Expressao>> {
        let mut argumentos = Vec::new();
        if self.verifica(fechamento) {
            self.avancar();
            return Some(argumentos);
        }
        loop {
            let nomeado = self.verifica(TipoToken::Identificador)
                && self.tokens.get(self.atual + 1).is_some_and(|token| {
                    matches!(token.tipo, TipoToken::Atribui | TipoToken::DoisPontos)
                });
            let argumento = if nomeado {
                let nome = self.avancar().lexema.nfc().collect::<String>();
                self.avancar();
                Expressao::ArgumentoNomeado { nome, valor: Box::new(self.expressao(0)?) }
            } else {
                self.expressao(0)?
            };
            argumentos.push(argumento);
            if !self.corresponde(TipoToken::Virgula) { break; }
            if self.verifica(fechamento) {
                self.erro_atual("N2005", "Esperado argumento após vírgula.");
                break;
            }
        }
        self.esperar(fechamento, "Esperado fechamento após argumentos.");
        Some(argumentos)
    }

    fn eh_bloco(no: &NoAst) -> bool {
        match no {
            NoAst::Atributada { declaracao, .. } => Self::eh_bloco(declaracao),
            _ => matches!(no,
                NoAst::Funcao { .. } | NoAst::Quando { .. } | NoAst::Escolha { .. }
                | NoAst::Enquanto { .. } | NoAst::Cada { .. } | NoAst::Pulso { .. }
                | NoAst::Para { .. } | NoAst::Classe { .. } | NoAst::BlocoContextual { .. } | NoAst::Tente { .. } | NoAst::Com { .. }
            ),
        }
    }
    fn inicia_item(&self) -> bool {
        matches!(self.tipo(),
            TipoToken::Variavel | TipoToken::Definir | TipoToken::Mutavel | TipoToken::Constante
            | TipoToken::Alterar | TipoToken::Funcao | TipoToken::Estatico | TipoToken::Publico | TipoToken::Privado | TipoToken::Protegido
            | TipoToken::Tipo | TipoToken::Estrutura | TipoToken::Enumeracao | TipoToken::Classe | TipoToken::Tente | TipoToken::Com | TipoToken::Lancar | TipoToken::Verifique | TipoToken::Global | TipoToken::Externa | TipoToken::Assinc | TipoToken::Arroba | TipoToken::Critico | TipoToken::Seguro | TipoToken::Privilegio | TipoToken::Montagem | TipoToken::ReservadaFutura
            | TipoToken::Exportar | TipoToken::Incluir | TipoToken::Quando | TipoToken::Se | TipoToken::Sendo | TipoToken::Escolha | TipoToken::Avaliar
            | TipoToken::Enquanto | TipoToken::Durante | TipoToken::Repetir | TipoToken::Pulso
            | TipoToken::Para | TipoToken::Cada | TipoToken::Devolver | TipoToken::Retorne | TipoToken::Ceder | TipoToken::Parar | TipoToken::Pare | TipoToken::Prossiga | TipoToken::Passar
            | TipoToken::Encerrar | TipoToken::Seguir | TipoToken::Isto | TipoToken::Lambda | TipoToken::Incrementa | TipoToken::Decrementa | TipoToken::Identificador
            | TipoToken::Inteiro | TipoToken::Decimal | TipoToken::Texto | TipoToken::TextoInterpolado | TipoToken::Caractere
            | TipoToken::Sim | TipoToken::Nao | TipoToken::Verdadeiro | TipoToken::Falso | TipoToken::Nulo | TipoToken::AbrePar
            | TipoToken::AbreColchete | TipoToken::Negacao | TipoToken::NaoBit | TipoToken::EBit | TipoToken::Interrogacao | TipoToken::Reticencias | TipoToken::Aguarda | TipoToken::Novo | TipoToken::Apagar | TipoToken::Menos
            | TipoToken::Mais
        )
    }
    fn tipo(&self) -> TipoToken { self.tokens.get(self.atual).map_or(TipoToken::Fim, |t| t.tipo) }
    fn verifica(&self, t: TipoToken) -> bool { self.tipo() == t }
    fn avancar(&mut self) -> Token { let t = self.tokens[self.atual].clone(); if t.tipo != TipoToken::Fim { self.atual += 1; } t }
    fn corresponde(&mut self, t: TipoToken) -> bool { if self.verifica(t) { self.avancar(); true } else { false } }
    fn esperar(&mut self, t: TipoToken, mensagem: &str) {
        if self.corresponde(t) {
            return;
        }
        let esperado_fechamento = matches!(
            t,
            TipoToken::FechaPar | TipoToken::FechaColchete | TipoToken::FechaChave | TipoToken::FimBloco
        );
        let atual_fechamento = matches!(
            self.tipo(),
            TipoToken::FechaPar | TipoToken::FechaColchete | TipoToken::FechaChave | TipoToken::FimBloco
        );
        if esperado_fechamento && atual_fechamento && !self.verifica(TipoToken::Fim) {
            self.erro_atual(
                "N2051",
                "Delimitador de fechamento incompatível; verifique o par de abertura e fechamento.",
            );
        } else {
            self.erro_atual("N2009", mensagem);
        }
    }
    fn erro_atual(&mut self, codigo: &'static str, mensagem: &str) {
        let intervalo = self.tokens.get(self.atual).map(|token| token.intervalo);
        self.erro_intervalo(codigo, mensagem, intervalo);
    }
    fn erro_intervalo(
        &mut self,
        codigo: &'static str,
        mensagem: &str,
        intervalo: Option<IntervaloFonte>,
    ) {
        self.diagnosticos.push(Diagnostico::novo(
            codigo,
            Severidade::Erro,
            mensagem,
            intervalo,
        ));
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
    fn consumir_nome_acesso(&mut self, mensagem: &str) -> Option<String> {
        if matches!(self.tipo(), TipoToken::De | TipoToken::Modulo | TipoToken::Biblioteca | TipoToken::Tipo) {
            Some(self.avancar().lexema.nfc().collect())
        } else {
            self.consumir_nome(mensagem)
        }
    }
    fn fechamento_sem_contexto(&self) -> bool {
        matches!(
            self.tipo(),
            TipoToken::FechaPar | TipoToken::FechaColchete | TipoToken::FechaChave | TipoToken::FimBloco
        )
    }
    fn fechamento_incompativel_com_chaves(&self) -> bool {
        matches!(
            self.tipo(),
            TipoToken::FechaPar | TipoToken::FechaColchete | TipoToken::FimBloco
        )
    }
    fn fechamento_incompativel_com_fim(&self) -> bool {
        matches!(
            self.tipo(),
            TipoToken::FechaPar | TipoToken::FechaColchete | TipoToken::FechaChave
        )
    }
    fn separador_atual(&self) -> bool { matches!(self.tipo(), TipoToken::NovaLinha | TipoToken::PontoVirgula) }
    fn separadores(&mut self) { while self.separador_atual() { self.avancar(); } }
    fn pular_novas_linhas(&mut self) { while self.corresponde(TipoToken::NovaLinha) {} }
    fn fim_item(&self) -> bool { matches!(self.tipo(), TipoToken::NovaLinha | TipoToken::PontoVirgula | TipoToken::FechaChave | TipoToken::Fim | TipoToken::FimBloco | TipoToken::Senao | TipoToken::Sendo | TipoToken::Erro | TipoToken::Sempre | TipoToken::Dedent) }
    fn sincronizar(&mut self) {
        self.sincronizar_ate(&[
            TipoToken::NovaLinha,
            TipoToken::PontoVirgula,
            TipoToken::FechaChave,
            TipoToken::FimBloco,
            TipoToken::Senao,
        ]);
    }
    /// Recupera no limite de um item sem consumir o início de uma declaração válida.
    fn sincronizar_item(&mut self) {
        self.sincronizar_ate(&[
            TipoToken::NovaLinha, TipoToken::PontoVirgula, TipoToken::FechaChave,
            TipoToken::FimBloco, TipoToken::Senao, TipoToken::Variavel, TipoToken::Definir,
            TipoToken::Mutavel, TipoToken::Constante, TipoToken::Alterar, TipoToken::Funcao,
            TipoToken::Publico, TipoToken::Privado, TipoToken::Tipo, TipoToken::Estrutura,
            TipoToken::Enumeracao, TipoToken::ReservadaFutura, TipoToken::Incluir,
            TipoToken::Quando, TipoToken::Escolha, TipoToken::Avaliar, TipoToken::Enquanto,
            TipoToken::Durante, TipoToken::Repetir, TipoToken::Pulso, TipoToken::Para,
            TipoToken::Cada, TipoToken::Devolver, TipoToken::Parar, TipoToken::Encerrar,
            TipoToken::Seguir, TipoToken::Identificador, TipoToken::Inteiro, TipoToken::Decimal,
            TipoToken::Texto, TipoToken::Caractere, TipoToken::Verdadeiro, TipoToken::Falso,
            TipoToken::Nulo, TipoToken::AbrePar, TipoToken::AbreColchete, TipoToken::Negacao,
            TipoToken::Nao, TipoToken::Menos, TipoToken::Mais,
        ]);
    }
    fn sincronizar_ate(&mut self, finais: &[TipoToken]) {
        let mut delimitadores = Vec::new();
        while !self.verifica(TipoToken::Fim) {
            let atual = self.tipo();
            if delimitadores.is_empty()
                && (finais.contains(&atual)
                    || matches!(
                        atual,
                        TipoToken::FechaPar
                            | TipoToken::FechaColchete
                            | TipoToken::FechaChave
                            | TipoToken::FimBloco
                    ))
            {
                break;
            }
            match atual {
                TipoToken::AbrePar => delimitadores.push(TipoToken::FechaPar),
                TipoToken::AbreColchete => delimitadores.push(TipoToken::FechaColchete),
                TipoToken::AbreChave => delimitadores.push(TipoToken::FechaChave),
                TipoToken::FechaPar | TipoToken::FechaColchete | TipoToken::FechaChave => {
                    if delimitadores.last() == Some(&atual) {
                        delimitadores.pop();
                    } else if let Some(posicao) =
                        delimitadores.iter().rposition(|esperado| *esperado == atual)
                    {
                        // Um fechamento cruzado invalida também os agrupamentos
                        // internos ainda abertos; não deixe a recuperação presa neles.
                        delimitadores.truncate(posicao);
                    } else {
                        // Preserve o fechamento para que o contexto chamador
                        // possa emitir o diagnóstico correto e recuperar o bloco.
                        delimitadores.clear();
                        break;
                    }
                }
                TipoToken::FimBloco => {
                    delimitadores.clear();
                    break;
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
            "funcao testar(enumeracao: inteiro)\n    devolver enumeracao\nfim",
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
            "estrutura Registro { callback: funcao(inteiro, texto): logico, callbackPadrao: funcao(funcao(inteiro): logico): vazio }",
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
        let r = analisar(&fonte("escolha resultado\n    caso Resultado::Sucesso(valor)\n        exibir(valor)\n    caso Resultado::Falha(mensagem)\n        exibir(mensagem)\n    caso _\n        exibir(0)\nfim"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        let NoAst::Escolha { casos, .. } = &r.programa.itens[0] else { panic!("AST não contém escolha"); };
        assert_eq!(casos.len(), 3);
        assert_eq!(casos[0].vinculacoes, ["valor"]);
        assert!(casos[2].enumeracao.is_none() && casos[2].variante.is_none());
    }

    #[test]
    fn reconhece_palavras_identificadores_e_operadores_maiores() {
        let r = analisar_lexico(&fonte("variavel Ωmega = 2 >= 1 // comentário\n"));
        assert!(r.diagnosticos.is_empty());
        assert_eq!(r.tokens[0].tipo, TipoToken::Variavel);
        assert_eq!(r.tokens[1].lexema, "Ωmega");
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
            "variavel unidadeVazia = ()",
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
        let r = analisar(&fonte("funcao principal(): vazio\n    variavel x: inteiro = 1 + 2 * 3\n    quando x > 2\n        devolver x\n    senao\n        devolver 0\n    fim\nfim"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.programa.itens.len(), 1);
    }
    #[test]
    fn analisa_funcao_generica_com_um_e_varios_parametros() {
        let r = analisar(&fonte(
            "funcao identidade<T>(valor: T): T\n    devolver valor\nfim\nfuncao combinar<T, U>(a: T, b: U): T\n    devolver a\nfim",
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
        let r = analisar(&fonte("funcao combinar<T, T>(a: T)\n    devolver\nfim"));
        assert!(r.diagnosticos.iter().any(|d| d.codigo == "N2014"));
    }

    #[test]
    fn preserva_visibilidade_de_funcao_na_ast() {
        let r = analisar(&fonte("publico funcao abrir()\n    devolver\nfim"));
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
        let r = analisar(&fonte("incluir Matematica como Mat\npara (variavel i = 0; i < 3; i = i + 1)\n    exibir(i)\nfim"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.programa.itens.len(), 2);
    }


    #[test]
    fn analisa_pulso_com_intervalo() {
        let r = analisar(&fonte("pulso(100)\n    encerrar\nfim"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert!(matches!(&r.programa.itens[0], NoAst::Pulso { .. }));
    }

    #[test]
    fn analisa_laco_cada_com_colecao() {
        let r = analisar(&fonte("cada item em [1, 2, 3]\n    exibir(item)\nfim"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert!(matches!(&r.programa.itens[0], NoAst::Cada { nome, .. } if nome == "item"));
    }

    #[test]
    fn condicional_sem_senao_preserva_separador_da_proxima_instrucao() {
        let r = analisar(&fonte("quando verdadeiro\n    exibir(1)\nfim\n\nvariavel depois = 2"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.programa.itens.len(), 2);
    }
    #[test]
    fn aceita_quando_senao_em_linhas_separadas() {
        let r = analisar(&fonte("quando verdadeiro\n    exibir(1)\n\nsenao\n    exibir(0)\nfim"));
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
        let r = analisar(&fonte("para (constante i = 0; i < 3; i = i + 1)\n    parar\nfim"));
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
        for caso in 0..2_048 {
            let tamanho = (caso % 257) + 1;
            let mut codigo = String::new();
            for _ in 0..tamanho {
                estado = estado
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                codigo.push_str(alfabeto[(estado as usize) % alfabeto.len()]);
            }
            let resultado = analisar(&fonte(&codigo));
            let mut ultima_posicao = 0;
            for diagnostico in resultado.diagnosticos {
                if let Some(intervalo) = diagnostico.intervalo {
                    assert!(intervalo.inicio <= intervalo.fim);
                    assert!(intervalo.fim <= codigo.len());
                    assert!(codigo.is_char_boundary(intervalo.inicio));
                    assert!(codigo.is_char_boundary(intervalo.fim));
                    assert!(intervalo.inicio >= ultima_posicao);
                    ultima_posicao = intervalo.inicio;
                }
            }
        }
    }

    #[test]
    fn parser_mantem_estabilidade_em_arquivos_extensos() {
        let mut codigo = String::with_capacity(160_000);
        for indice in 0..4_096 {
            codigo.push_str(&format!("variavel valor_{indice} = {indice}\n"));
        }

        let resultado = analisar(&fonte(&codigo));
        assert!(
            resultado.diagnosticos.is_empty(),
            "arquivo extenso válido produziu diagnósticos: {:?}",
            resultado.diagnosticos
        );
        assert_eq!(resultado.programa.itens.len(), 4_096);
    }

    #[test]
    fn parser_recupera_apos_grande_sequencia_de_delimitadores_invalidos() {
        let codigo = format!(
            "{}\nvariavel recuperada = 42",
            "}".repeat(2_048)
        );
        let resultado = analisar(&fonte(&codigo));
        assert!(
            resultado.diagnosticos.iter().any(|d| d.codigo == "N2051"),
            "fechamentos sem abertura devem ser diagnosticados"
        );
        assert!(
            resultado.programa.itens.iter().any(|item| matches!(
                item,
                NoAst::Variavel { nome, .. } if nome == "recuperada"
            )),
            "a recuperação deve preservar a declaração válida seguinte"
        );
    }
    #[test]
    fn reconhece_lista_chamada_indexacao_e_membro() {
        let r = analisar(&fonte("variavel x = sistema::somar([1, 2][0], objeto.nome)"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn reconhece_aliases_de_declaracao_e_controle() {
        let codigo = "definir a = 1\nmutavel b = 2\nmutavel c = 3\nalterar a <- 3\navaliar a\n    caso _\n        exibir(b)\nfim\ndurante falso\n    encerrar\nfim\nrepetir falso\n    seguir\nfim\ncada item em [1, 2]\n    exibir(item)\nfim";
        let r = analisar(&fonte(codigo));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.programa.itens.len(), 8);
    }

    #[test]
    fn aceita_grafias_portuguesas_sem_acentos() {
        let codigo = "funcao somar(a: inteiro, b: inteiro): inteiro\n    devolver a + b\nfim\nquando nao falso\n    exibir(1)\nsenao\n    exibir(0)\nfim\n\nenumeracao Estado { Ativo\nInativo }\npublico funcao principal()\n    exibir(Estado::Ativo)\nfim";
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
            "mutavel x = 1",
            "constante limite: inteiro = 10",
            "funcao somar(a: inteiro, b: inteiro): inteiro\n    devolver a + b\nfim",
            "publico funcao identidade<T>(valor: T): T\n    devolver valor\nfim",
            "privado funcao combinar<T, U>(a: [T], b: U): funcao(T): U\n    devolver b\nfim",
            "tipo Caminho = Modulo::Submodulo::Tipo",
            "tipo Colecao = [Resultado<inteiro>]",
            "estrutura Pessoa { nome: texto; idade: inteiro, }",
            "enumeracao Estado { Ativo\nInativo(texto, inteiro) }",
            "incluir Biblioteca::Texto como Texto",
            "quando verdadeiro\n    exibir(1)\nsenao\n    exibir(0)\nfim",
            "quando verdadeiro\n    exibir(1)\nsendo falso\n    exibir(2)\nfim",
            "escolha resultado\n    caso Resultado::Sucesso(valor)\n        exibir(valor)\n    caso _\n        exibir(0)\nfim",
            "enquanto verdadeiro\n    parar\nfim",
            "durante verdadeiro\n    seguir\nfim",
            "repetir falso\n    encerrar\nfim",
            "cada item em [1, 2, 3]\n    exibir(item)\nfim",
            "pulso(100)\n    exibir(1)\nfim",
            "para (variavel i = 0; i < 3; i = i + 1)\n    exibir(i)\nfim",
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
    fn sincroniza_formas_alternativas_e_tipos_normativos_da_gramatica() {
        let exemplos = [
            "tipo FuncaoSimples = funcao(inteiro): texto",
            "tipo FuncaoSemRetorno = funcao(inteiro)",
            "tipo FuncaoAninhada = funcao([inteiro], Resultado<texto>): [Resultado<inteiro>]",
            "tipo CaminhoGenerico = Modulo::Interno<inteiro, [texto]>",
            "tipo CaminhoQuebrado = Biblioteca\n::\nInterna::Tipo",
            "tipo AliasComSeta <- Texto",
            "estrutura Vazia {}",
            "enumeracao Resultado { Vazio, Unidade(), Valor(inteiro, texto) }",
            "incluir Modulo::Interno",
            "variavel estrutura_vazia = Pessoa {}",
            "variavel chamada_vazia = executar()",
            "variavel acesso_indice = valores[0]",
            "variavel acesso_campo = pessoa.nome",
            "variavel acesso_qualificado = Modulo::valor",
            "para (; ; )\n    parar\nfim",
            "devolver 42",
            "parar",
            "encerrar",
            "seguir",
            "incluir Biblioteca\n::\nTexto como Texto",
            "privado funcao sem_retorno()\n    passar\nfim",
            "publico funcao com_retorno(): vazio\n    devolver\nfim",
            "para (variavel i = 0; i < 2; i = i + 1)\n    exibir(i)\nfim",
            "para (i = 0; ; i = i + 1)\n    parar\nfim",
            "para (; verdadeiro; )\n    parar\nfim",
            "quando verdadeiro\n    exibir(1)\nsenao\n    exibir(0)\nfim",
            "escolha valor\n    caso Resultado::Vazio\n        passar\nfim",
            "avaliar valor\n    caso Resultado::Unico(item)\n        exibir(item)\nfim",
            "variavel agrupada = (1)",
            "variavel tupla = (1, 2, 3,)",
            "variavel acesso = modulo::objeto.campo[0](1)",
        ];

        for codigo in exemplos {
            let resultado = analisar(&fonte(codigo));
            assert!(
                resultado.diagnosticos.is_empty(),
                "forma normativa deveria ser aceita: {codigo:?}; diagnósticos: {:?}",
                resultado.diagnosticos
            );
        }
    }

    #[test]
    fn cobre_alternativas_e_producoes_gramaticais_completas() {
        let exemplos = [
            "",
            "variavel anotada: inteiro = 1",
            "constante seta <- 2",
            "privado funcao sem_retorno()\n    passar\nfim",
            "funcao bloco_fim()\n    retorne\nfim",
            "tipo Alias = Texto",
            "tipo AliasSeta <- Texto",
            "tipo Lista = [inteiro]",
            "tipo Generico = Resultado<inteiro>",
            "tipo Funcao = funcao(inteiro, texto)",
            "tipo FuncaoComRetorno = funcao(inteiro): texto",
            "estrutura Vazia {}",
            "estrutura Membros { a: inteiro, b: texto; c: logico\n}",
            "enumeracao Estado { Pronto, Falha(), Valor(inteiro) }",
            "enumeracao Resultado { Vazio\nErro(inteiro); Texto(texto), }",
            "incluir Biblioteca",
            "incluir Biblioteca::Interna como Interna",
            "quando condicao\n    passar\nsenao\n    passar\nfim",
            "quando (Pessoa\n    passar\nfim\n)\n    passar\nfim",
            "escolha (Pessoa\n    passar\nfim\n)\n    caso _\n        passar\nfim",
            "enquanto (Pessoa\n    passar\nfim\n)\n    passar\nfim",
            "cada item em (Pessoa\n    passar\nfim\n)\n    passar\nfim",
            "quando primeiro\n    passar\nsendo segundo\n    passar\nsenao\n    passar\nfim",
            "escolha valor\n    caso Estado::Pronto\n        passar\n    caso Estado::Falha()\n        passar\n    caso Estado::Valor(codigo)\n        passar\n    caso _\n        passar\nfim",
            "avaliar valor\n    caso Estado::Pronto\n        passar\nfim",
            "enquanto condicao\n    passar\nfim",
            "durante condicao\n    passar\nfim",
            "repetir condicao\n    passar\nfim",
            "cada item em itens\n    passar\nfim",
            "pulso(10)\n    passar\nfim",
            "para (variavel i = 0; i < 2; i = i + 1)\n    passar\nfim",
            "para (i = 0; ; i = i + 1)\n    passar\nfim",
            "para (; condicao; )\n    passar\nfim",
            "para (;; )\n    passar\nfim",
            "alterar destino = valor",
            "alterar destino <- valor",
            "devolver",
            "devolver resultado",
            "parar",
            "encerrar",
            "seguir",
            "alvo = valor",
            "alvo <- valor",
            "a ou b",
            "a e b",
            "a == b",
            "a != b",
            "a < b",
            "a <= b",
            "a > b",
            "a >= b",
            "a..b",
            "a + b",
            "a - b",
            "a * b",
            "a / b",
            "a % b",
            "!a",
            "nao a",
            "-a",
            "+a",
            "a ** b",
            "executar()",
            "executar(a)",
            "executar(a, b)",
            "itens[0]",
            "objeto.campo",
            "Modulo::valor",
            "()",
            "(a)",
            "(a, b)",
            "(a, b,)",
            "[]",
            "[a, b]",
            "Pessoa {}",
            "Pessoa { nome: a, idade: b }",
            "42",
            "3.14",
            "\"texto\"",
            "'x'",
            "verdadeiro",
            "falso",
            "nulo",
        ];

        for codigo in exemplos {
            let resultado = analisar(&fonte(codigo));
            assert!(
                resultado.diagnosticos.is_empty(),
                "produção/alternativa gramatical válida rejeitada: {codigo:?}; diagnósticos: {:?}",
                resultado.diagnosticos
            );
        }
    }

    #[test]
    fn continua_listas_genericas_declaracoes_e_tipos_apos_virgula() {
        let exemplos = [
            "funcao converter<T,\nU>(valor: T): U { devolver valor }",
            "tipo Mapa = Resultado<Texto,\n[inteiro]>",
            "tipo Operacao = funcao(inteiro,\ntexto): Resultado<texto,\ninteiro>",
            "funcao aninhada<T,\nU>(valor: Resultado<T,\nU>): Resultado<U,\nT> { devolver valor }",
        ];

        for codigo in exemplos {
            let resultado = analisar(&fonte(codigo));
            assert!(
                resultado.diagnosticos.is_empty(),
                "quebra após vírgula deveria continuar a lista: {codigo:?}; diagnósticos: {:?}",
                resultado.diagnosticos
            );
        }
    }

    #[test]
    fn cobre_combinacoes_restantes_das_producoes_gramaticais() {
        let exemplos = [
            "variavel seta <- 1",
            "mutavel anotada: inteiro <- 2",
            "constante anotada: inteiro <- 3",
            "tipo FuncaoVazia = funcao()",
            "tipo FuncaoVaziaComRetorno = funcao(): texto",
            "tipo ListaAninhada = [[Resultado<inteiro>]]",
            "tipo GenericoAninhado = Resultado<funcao(inteiro): [texto]>",
            "tipo CaminhoMultilinha = Modulo\n::Interno\n::Tipo",
            "estrutura Separadores { a: inteiro\nb: texto,\nc: logico; }",
            "enumeracao Variantes { A\nB(), C(inteiro, texto), }",
            "incluir Modulo\n::Interno como Alias",
            "funcao sem_retorno()\n    passar\nfim",
            "funcao sem_parametros(): vazio\n    passar\nfim",
            "funcao multiplos(a: inteiro, b: texto, c: [logico])\n    passar\nfim",
            "quando verdadeiro\n    passar\nfim",
            "quando falso\n    passar\nsenao quando verdadeiro\n    passar\nfim\nsenao\n    passar\nfim",
            "escolha valor\n    caso Enum::Vazio\n        passar\n    caso Enum::Unico()\n        passar\n    caso Enum::Par(a, b)\n        passar\n    caso _\n        passar\nfim",
            "para (inicial = 1; condicao; atualizacao = 2)\n    passar\nfim",
            "para (; condicao; atualizacao = 2)\n    passar\nfim",
            "para (inicial = 1; ; atualizacao = 2)\n    passar\nfim",
            "para (inicial = 1; condicao; )\n    passar\nfim",
            "para (; ; )\n    passar\nfim",
            "variavel encadeada = funcao_alvo(1, 2)[0].campo::interno",
            "variavel tupla_unitaria = (valor,)",
            "variavel construcao_multilinha = Pessoa {\n nome: \"Ana\",\n idade: 20,\n}",
            "variavel expressao_multilinha = objeto\n .campo\n ::interno",
            "variavel atribuicao_direita = primeiro <- segundo = terceiro",
            "variavel potencia_direita = 2 ** 3 ** 4",
            "variavel agrupamento_relacional = (1 < 2) e (3 > 2)",
            "variavel separadores = 1; constante outro = 2",
            "funcao bloco_com_separadores()\n    variavel a = 1\n    \n    \n    variavel b = 2\nfim",
            "funcao bloco_fim_vazio()\n    passar\nfim",
        ];

        for codigo in exemplos {
            let resultado = analisar(&fonte(codigo));
            assert!(
                resultado.diagnosticos.is_empty(),
                "combinação gramatical válida rejeitada: {codigo:?}; diagnósticos: {:?}",
                resultado.diagnosticos
            );
        }
    }

    #[test]
    fn aceita_separadores_no_inicio_e_fim_de_construcoes_de_estrutura() {
        let exemplos = [
            "variavel pessoa = Pessoa {\n nome: \"Ana\"\n}",
            "variavel pessoa = Pessoa {\n\n nome: \"Ana\",\n idade: 20;\n\n}",
            "variavel pessoa = Pessoa { ; nome: \"Ana\"; }",
        ];

        for codigo in exemplos {
            let resultado = analisar(&fonte(codigo));
            assert!(
                resultado.diagnosticos.is_empty(),
                "separadores permitidos em construção foram rejeitados: {codigo:?}; diagnósticos: {:?}",
                resultado.diagnosticos
            );
        }
    }

    #[test]
    fn rejeita_formas_invalidas_das_producoes_normativas() {
        let exemplos = [
            "variavel = 1",
            "constante limite =",
            "funcao sem_parametros(\n    devolver\nfim",
            "funcao incompleta(valor inteiro)\n    devolver\nfim",
            "tipo SemDestino =",
            "estrutura Pessoa { nome texto }",
            "enumeracao Estado {}",
            "incluir [inteiro]",
            "quando verdadeiro { exibir(1)",
            "escolha valor {\ncaso _\n    exibir(1)\n",
            "cada item [1, 2]\n    exibir(item)\nfim",
            "pulso\n    exibir(1)\nfim",
            "para (variavel i = 0; i < 3)\n    parar\nfim",
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
            "funcao vazia()\n    passar\nfim",
            "estrutura Vazia {}",
            "variavel tupla_vazia = ()",
            "variavel lista_vazia = []",
            "variavel pessoa = Pessoa {}",
            "escolha valor\n    caso _\n        passar\nfim",
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
    fn rejeita_tokens_extras_entre_padrao_e_bloco_de_escolha() {
        for codigo in [
            "escolha x\n    caso Estado::Ok extra\n        passar\nfim",
            "escolha x\n    caso Estado::Ok( valor ) inesperado\n        passar\nfim",
            "escolha x\n    caso _ 123\n        passar\nfim",
        ] {
            let resultado = analisar(&fonte(codigo));
            assert!(
                resultado.diagnosticos.iter().any(|d| d.codigo == "N2042"),
                "tokens entre padrão e bloco devem ser rejeitados: {codigo:?}; {:?}",
                resultado.diagnosticos
            );
            let Some(NoAst::Escolha { casos, .. }) = resultado.programa.itens.first() else {
                panic!("a escolha deve permanecer na AST parcial");
            };
            assert!(
                casos.is_empty(),
                "um caso com tokens extras não pode ser inserido na AST: {casos:?}"
            );
        }
    }

    #[test]
    fn padroes_invalidos_nao_contaminam_duplicidade_de_variantes() {
        let resultado = analisar(&fonte(
            "escolha x\n    caso Estado::Ok(valor, valor)\n        passar\n    caso Estado::Ok(item)\n        passar\n    caso _\n        passar\nfim"
        ));
        assert!(resultado.diagnosticos.iter().any(|d| d.codigo == "N2046"));
        assert!(
            !resultado.diagnosticos.iter().any(|d| d.codigo == "N2048"),
            "padrão inválido não deve reservar a identidade da variante: {:?}",
            resultado.diagnosticos
        );
        let Some(NoAst::Escolha { casos, .. }) = resultado.programa.itens.first() else {
            panic!("esperada escolha parcial");
        };
        assert_eq!(casos.len(), 2);
        assert_eq!(casos[0].variante.as_deref(), Some("Ok"));
        assert!(casos[1].enumeracao.is_none());
    }

    #[test]
    fn alias_avaliar_e_blocos_fim_cobrem_formas_normativas_de_escolha() {
        let codigo = "avaliar resultado\n    caso Estado::Vazio\n    exibir(0)\n    fim\n    caso Estado::Valor(item)\n       exibir(item)\n    \n    caso _\n       exibir(1)\nfim";
        let resultado = analisar(&fonte(codigo));
        assert!(
            resultado.diagnosticos.is_empty(),
            "alias avaliar e blocos indentados devem ser aceitos: {:?}",
            resultado.diagnosticos
        );
        let Some(NoAst::Escolha { casos, .. }) = resultado.programa.itens.first() else {
            panic!("avaliar deve produzir a mesma AST de escolha");
        };
        assert_eq!(casos.len(), 3);
        assert_eq!(casos[0].variante.as_deref(), Some("Vazio"));
        assert_eq!(casos[1].vinculacoes, ["item"]);
        assert!(casos[2].enumeracao.is_none() && casos[2].variante.is_none());
    }

    #[test]
    fn escolha_permite_curinga_antes_ou_depois_de_variantes_sem_repeticao() {
        for codigo in [
            "escolha x\n    caso _\n        passar\n    caso Estado::Ok\n        passar\nfim",
            "escolha x\n    caso Estado::Ok\n        passar\n    caso _\n        passar\nfim",
        ] {
            let resultado = analisar(&fonte(codigo));
            assert!(
                resultado.diagnosticos.is_empty(),
                "a posição do curinga não altera a validade sintática: {codigo:?}; {:?}",
                resultado.diagnosticos
            );
        }
    }

    #[test]
    fn aceita_todas_as_formas_validas_de_padroes_escolha() {
        let codigo = "escolha resultado\n    caso Resultado::Vazio\n       passar\n    \n    caso Resultado::Unico()\n       passar\n    \n    caso Resultado::Um(valor)\n       passar\n    \n    caso Resultado::Dois(primeiro, segundo)\n       passar\n    \n    caso _\n       passar\nfim";
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
            ("escolha x\n    caso Variante\n        passar\nfim", "N2042"),
            ("escolha x\n    caso Enum::\n        passar\nfim", "N2010"),
            ("escolha x\n    caso Enum::Variante(1)\n        passar\nfim", "N2045"),
            ("escolha x\n    caso Enum::Variante(valor, valor)\n        passar\nfim", "N2046"),
            ("escolha x\n    caso _()\n        passar\nfim", "N2042"),
            ("escolha x\n    caso Enum::Variante(valor,)\n        passar\nfim", "N2045"),
            ("escolha x\n    caso Enum::Variante::Aninhada\n        passar\nfim", "N2042"),
            ("escolha x\n    caso _\n        passar\n    caso _\n        passar\nfim", "N2043"),
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
    fn padroes_escolha_validam_limites_e_isolamento_de_vinculos() {
        let codigo = "escolha valor\n    caso A::Mesmo(item)\n       passar\n    \n    caso B::Mesmo(item)\n       passar\n    \n    caso A::Outro(primeiro, segundo)\n       passar\n    \n    caso _\n       passar\nfim";
        let resultado = analisar(&fonte(codigo));
        assert!(
            resultado.diagnosticos.is_empty(),
            "vínculos podem se repetir em casos distintos e variantes de enums diferentes não colidem: {:?}",
            resultado.diagnosticos
        );
        let Some(NoAst::Escolha { casos, .. }) = resultado.programa.itens.first() else {
            panic!("esperada instrução escolha");
        };
        assert_eq!(casos.len(), 4);
        assert_eq!(casos[0].vinculacoes, ["item"]);
        assert_eq!(casos[1].vinculacoes, ["item"]);
        assert_eq!(casos[2].vinculacoes, ["primeiro", "segundo"]);
    }

    #[test]
    fn normaliza_nomes_de_padroes_e_vinculos_antes_de_validar_duplicidade() {
        let variantes = analisar(&fonte(
            "escolha x\n    caso Estado::Café\n        passar\n    caso Estado::Cafe\\u\n        301\n    \n    {}\nfim"
        ));
        assert!(
            variantes.diagnosticos.iter().any(|d| d.codigo == "N2048"),
            "variantes equivalentes em NFC devem ser consideradas repetidas: {:?}",
            variantes.diagnosticos
        );

        let vinculacoes = analisar(&fonte(
            "escolha x\n    caso Estado::Ok(café, cafe\\u\n        301\n    \n    ) {}\nfim"
        ));
        assert!(
            vinculacoes.diagnosticos.iter().any(|d| d.codigo == "N2046"),
            "vínculos equivalentes em NFC devem ser considerados repetidos: {:?}",
            vinculacoes.diagnosticos
        );
    }

    #[test]
    fn curinga_invalido_nao_reserva_o_unico_curinga() {
        for codigo in [
            "escolha x\n    caso _()\n        passar\n    caso _\n        passar\nfim",
            "escolha x\n    caso _\n        passar\n    caso _()\n        passar\nfim",
        ] {
            let resultado = analisar(&fonte(codigo));
            assert!(
                resultado.diagnosticos.iter().any(|d| d.codigo == "N2042"),
                "payload em curinga deve ser rejeitado: {codigo:?}; {:?}",
                resultado.diagnosticos
            );
            assert!(
                !resultado.diagnosticos.iter().any(|d| d.codigo == "N2043"),
                "padrão curinga inválido não deve participar da contagem de duplicidade: {codigo:?}; {:?}",
                resultado.diagnosticos
            );
        }
    }

    #[test]
    fn escolha_rejeita_ausencia_de_casos_e_recupera_casos_sequenciais() {
        let vazio = analisar(&fonte("escolha valor\n    passar\nfim"));
        assert!(
            vazio.diagnosticos.iter().any(|d| d.codigo == "N2044"),
            "escolha vazia precisa ser rejeitada: {:#?}",
            vazio.diagnosticos
        );

        let codigo = "escolha valor\n    caso Estado::Erro(1)\n        passar\n    caso Estado::Ok(item)\n        passar\n    caso _\n        passar\nfim";
        let resultado = analisar(&fonte(codigo));
        assert!(
            resultado.diagnosticos.iter().any(|d| d.codigo == "N2045"),
            "payload inválido precisa gerar diagnóstico"
        );
        let Some(NoAst::Escolha { casos, .. }) = resultado.programa.itens.first() else {
            panic!("a escolha parcial deveria ser preservada");
        };
        assert_eq!(casos.len(), 2, "casos válidos posteriores devem ser preservados");
        assert_eq!(casos[0].variante.as_deref(), Some("Ok"));
        assert!(casos[1].enumeracao.is_none() && casos[1].variante.is_none());
    }

    #[test]
    fn padroes_escolha_rejeitam_quebras_e_caminhos_qualificados_extras() {
        for codigo in [
            "escolha x\n    caso Estado::Erro\n    () {}\nfim",
            "escolha x\n    caso Estado::Erro::Interno\n        passar\nfim",
            "escolha x\n    caso Estado::\n        passar\nfim",
            "escolha x\n    caso ::Erro\n        passar\nfim",
        ] {
            let resultado = analisar(&fonte(codigo));
            assert!(
                !resultado.diagnosticos.is_empty(),
                "padrão fora da produção Enum::Variante foi aceito: {codigo:?}"
            );
        }
    }

    #[test]
    fn padrao_invalido_nao_e_confundido_com_curinga() {
        let resultado = analisar(&fonte(
            "escolha x\n    caso VarianteInvalida\n        passar\n    caso _\n        passar\nfim"
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
            "escolha x\n    caso Resultado::Ok(valor)\n        passar\n    caso Resultado::Ok(outro)\n        passar\nfim"
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
    fn valida_precedencia_e_associatividade_completas_das_expressoes() {
        let codigo = "variavel potencia = -2 ** 3 ** 2\nvariavel produto = 2 + 3 * 4 % 5\nvariavel logica = verdadeiro ou falso e nao falso\nvariavel cadeia = objeto.metodo(1)[0]::campo\nvariavel atribuicao = a <- b = c";
        let resultado = analisar(&fonte(codigo));
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);

        let NoAst::Variavel { valor: Expressao::Unaria { valor, .. }, .. } =
            &resultado.programa.itens[0] else { panic!("unário deve envolver a potência"); };
        let Expressao::Binaria { operador: TipoToken::Potencia, direita, .. } = &**valor
            else { panic!("potência deve ter precedência sobre o unário"); };
        assert!(matches!(**direita, Expressao::Binaria { operador: TipoToken::Potencia, .. }),
            "potência deve associar à direita");

        let NoAst::Variavel { valor: Expressao::Binaria { operador: TipoToken::Mais, direita, .. }, .. } =
            &resultado.programa.itens[1] else { panic!("adição deve ser a raiz"); };
        assert!(matches!(**direita, Expressao::Binaria { operador: TipoToken::Resto, .. }),
            "multiplicação e resto devem preceder a adição");

        let NoAst::Variavel { valor: Expressao::Binaria { operador: TipoToken::Ou, direita, .. }, .. } =
            &resultado.programa.itens[2] else { panic!("OU deve ser a raiz"); };
        assert!(matches!(**direita, Expressao::Binaria { operador: TipoToken::E, .. }),
            "E deve preceder OU");

        let NoAst::Variavel { valor: Expressao::Membro { operador: TipoToken::Escopo, alvo, .. }, .. } =
            &resultado.programa.itens[3] else { panic!("acesso qualificado deve ser a raiz"); };
        assert!(matches!(**alvo, Expressao::Indexacao { .. }),
            "indexação deve integrar a cadeia pós-fixa");

        let NoAst::Variavel { valor: Expressao::Atribuicao { valor, .. }, .. } =
            &resultado.programa.itens[4] else { panic!("atribuição deve ser reconhecida"); };
        assert!(matches!(**valor, Expressao::Atribuicao { .. }),
            "atribuição deve associar à direita");
    }

    #[test]
    fn valida_limites_de_comparacao_e_intervalo_com_agrupamento() {
        for codigo in [
            "variavel a = (1 < 2) < 3",
            "variavel b = 1 < (2 < 3)",
            "variavel c = (1..2)..3",
            "variavel d = 1..(2..3)",
            "variavel independente = 1 < 2 e 3 < 4",
            "variavel f = 1..2 ou 3..4",
        ] {
            let resultado = analisar(&fonte(codigo));
            assert!(resultado.diagnosticos.is_empty(),
                "agrupamento ou expressões independentes deveriam ser aceitos: {codigo:?}: {:?}",
                resultado.diagnosticos);
        }

        for (codigo, diagnostico) in [
            ("variavel a = 1 < 2 < 3", "N2012"),
            ("variavel b = 1 <= 2 > 3", "N2012"),
            ("variavel c = 1..2..3", "N2013"),
            ("variavel d = 1..2 + 3..4", "N2013"),
        ] {
            let resultado = analisar(&fonte(codigo));
            assert!(resultado.diagnosticos.iter().any(|d| d.codigo == diagnostico),
                "encadeamento inválido deveria emitir {diagnostico}: {codigo:?}: {:?}",
                resultado.diagnosticos);
        }
    }

    #[test]
    fn recupera_operadores_sem_operando_em_expressoes_aninhadas() {
        let codigo = "funcao principal()\n    variavel quebrada = calcular(1 + , 2)\n    variavel outra = [3 * , 4]\n    variavel final = 99\nfim";
        let resultado = analisar(&fonte(codigo));
        assert!(resultado.diagnosticos.iter().any(|d| d.codigo == "N2008"),
            "operandos ausentes precisam gerar diagnóstico: {:?}", resultado.diagnosticos);
        let Some(NoAst::Funcao { corpo, .. }) = resultado.programa.itens.first() else {
            panic!("a função deve permanecer na AST parcial");
        };
        assert!(corpo.iter().any(|item|
            matches!(item, NoAst::Variavel { nome, .. } if nome == "final")),
            "a recuperação deve preservar a declaração posterior: {corpo:?}");
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
        for nivel in 0..150 {
            let indentacao = "    ".repeat(nivel);
            let indentacao_corpo = "    ".repeat(nivel + 1);
            let corpo = blocos
                .lines()
                .map(|linha| format!("{indentacao_corpo}{linha}"))
                .collect::<Vec<_>>()
                .join("\n");
            blocos = format!("{indentacao}quando verdadeiro\n{corpo}\n{indentacao}fim");
        }
        let resultado = analisar(&fonte(&blocos));
        assert!(
            resultado.diagnosticos.iter().any(|d| d.codigo == "N2049"),
            "aninhamento de blocos deveria atingir o limite: {:?}",
            resultado.diagnosticos
        );
    }


    #[test]
    fn analisa_sequencias_de_operadores_unarios() {
        let codigo = "variavel dupla = - -2\nvariavel logica = !nao falso\nvariavel mista = +-!nao 3";
        let resultado = analisar(&fonte(codigo));
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);

        let NoAst::Variavel { valor: Expressao::Unaria { operador: TipoToken::Menos, valor, .. }, .. } =
            &resultado.programa.itens[0] else { panic!("o primeiro menos deve ser o operador externo"); };
        assert!(matches!(**valor, Expressao::Unaria { operador: TipoToken::Menos, .. }));

        let NoAst::Variavel { valor: Expressao::Unaria { operador: TipoToken::Negacao, valor, .. }, .. } =
            &resultado.programa.itens[1] else { panic!("negação simbólica deve ser o operador externo"); };
        assert!(matches!(**valor, Expressao::Unaria { operador: TipoToken::Nao, .. }));

        let NoAst::Variavel { valor: Expressao::Unaria { operador: TipoToken::Mais, valor, .. }, .. } =
            &resultado.programa.itens[2] else { panic!("mais unário deve ser o operador externo"); };
        let Expressao::Unaria { operador: TipoToken::Menos, valor, .. } = &**valor
            else { panic!("menos unário deve permanecer aninhado"); };
        let Expressao::Unaria { operador: TipoToken::Negacao, valor, .. } = &**valor
            else { panic!("negação deve permanecer aninhada"); };
        assert!(matches!(**valor, Expressao::Unaria { operador: TipoToken::Nao, .. }));
    }

    #[test]
    fn valida_combinacoes_cruzadas_de_todos_os_niveis_de_precedencia() {
        let codigo = "variavel combinada = alfa <- beta ou gama e delta == epsilon < zeta..eta + theta * iota ** -kappa.metodo(1)[2]";
        let resultado = analisar(&fonte(codigo));
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);

        let NoAst::Variavel { valor: Expressao::Atribuicao { valor, .. }, .. } =
            &resultado.programa.itens[0] else { panic!("atribuição deve ser a raiz de menor precedência"); };
        let Expressao::Binaria { operador: TipoToken::Ou, direita, .. } = &**valor
            else { panic!("OU deve ficar abaixo de E e acima da atribuição"); };
        let Expressao::Binaria { operador: TipoToken::E, direita, .. } = &**direita
            else { panic!("E deve ficar abaixo da igualdade"); };
        let Expressao::Binaria { operador: TipoToken::Igual, direita, .. } = &**direita
            else { panic!("igualdade deve ficar abaixo da comparação"); };
        let Expressao::Binaria { operador: TipoToken::Menor, direita, .. } = &**direita
            else { panic!("comparação deve ficar abaixo do intervalo"); };
        let Expressao::Binaria { operador: TipoToken::Intervalo, direita, .. } = &**direita
            else { panic!("intervalo deve ficar abaixo da adição"); };
        let Expressao::Binaria { operador: TipoToken::Mais, direita, .. } = &**direita
            else { panic!("adição deve ficar abaixo da multiplicação"); };
        let Expressao::Binaria { operador: TipoToken::Vezes, direita, .. } = &**direita
            else { panic!("multiplicação deve ficar abaixo da potência"); };
        let Expressao::Binaria { operador: TipoToken::Potencia, direita, .. } = &**direita
            else { panic!("potência deve ficar abaixo dos operadores unários e pós-fixos"); };
        let Expressao::Unaria { valor, .. } = &**direita
            else { panic!("o operando da potência deve preservar o unário"); };
        let Expressao::Indexacao { alvo, .. } = &**valor
            else { panic!("indexação deve integrar a cadeia pós-fixa"); };
        assert!(matches!(**alvo, Expressao::Chamada { .. }),
            "chamada deve estar dentro da indexação");
    }

    #[test]
    fn aceita_atribuicoes_encadeadas_e_agrupadas() {
        for codigo in [
            "variavel a = esquerda <- meio = direita <- final",
            "variavel b = esquerda <- (meio = direita)",
            "variavel c = (esquerda <- meio) = direita",
            "variavel d = esquerda = (meio <- direita)",
        ] {
            let resultado = analisar(&fonte(codigo));
            assert!(resultado.diagnosticos.is_empty(),
                "atribuições encadeadas ou agrupadas deveriam ser analisadas: {codigo:?}: {:?}",
                resultado.diagnosticos);
        }
    }

    #[test]
    fn aceita_expressoes_validas_profundas_e_construcoes_posfixas_aninhadas() {
        let profunda = format!(
            "variavel profunda = {}valor{}",
            "(".repeat(120),
            ")".repeat(120)
        );
        let codigo = format!(
            "{profunda}\nvariavel composta = Pessoa {{ dados: ([1, 2], (3,)), resultado: fabrica([4])[0].campo }} "
        );
        let resultado = analisar(&fonte(&codigo));
        assert!(resultado.diagnosticos.is_empty(),
            "expressões válidas aninhadas não devem atingir o limite de segurança: {:?}",
            resultado.diagnosticos);
        assert_eq!(resultado.programa.itens.len(), 2,
            "todas as expressões válidas devem permanecer na AST");
    }

    #[test]
    fn recupera_item_malformado_sem_consumir_declaracao_seguinte() {
        for codigo in [
            "variavel quebrada = ) variavel preservada = 42",
            "variavel quebrada = )\nvariavel preservada = 42",
            "funcao principal()\n    variavel quebrada = ) variavel preservada = 42\nfim",
        ] {
            let resultado = analisar(&fonte(codigo));
            assert!(!resultado.diagnosticos.is_empty(),
                "a construção inválida deveria produzir diagnóstico: {codigo:?}");
            let preservada = resultado.programa.itens.iter().any(
                |item| matches!(item, NoAst::Variavel { nome, .. } if nome == "preservada")
            ) || resultado.programa.itens.iter().any(|item| matches!(
                item, NoAst::Funcao { corpo, .. } if corpo.iter().any(
                    |interno| matches!(interno, NoAst::Variavel { nome, .. } if nome == "preservada")
                )
            ));
            assert!(preservada,
                "a recuperação consumiu a declaração válida seguinte: {codigo:?}; AST: {:?}",
                resultado.programa);
        }
    }

    #[test]
    fn recupera_apos_erros_consecutivos_preservando_todos_os_itens_validos() {
        let codigo = "variavel a = ) variavel b = ] variavel c = 3\nfuncao depois()\n    devolver c\nfim";
        let resultado = analisar(&fonte(codigo));
        assert!(resultado.diagnosticos.len() >= 2,
            "cada região inválida deve ser diagnosticada: {:?}", resultado.diagnosticos);
        assert!(resultado.programa.itens.iter().any(
            |item| matches!(item, NoAst::Variavel { nome, .. } if nome == "c")
        ), "a declaração válida após erros consecutivos foi perdida: {:?}", resultado.programa);
        assert!(resultado.programa.itens.iter().any(
            |item| matches!(item, NoAst::Funcao { nome, .. } if nome == "depois")
        ), "a função posterior foi perdida: {:?}", resultado.programa);
    }

    #[test]
    fn recupera_erros_em_campos_listas_e_argumentos_aninhados() {
        let codigo = "funcao principal()\n    variavel pessoa = Pessoa { nome: , idade: 20 }\n    variavel valores = [1, , 3]\n    variavel chamada = calcular(1, , 3)\n    variavel aninhada = Pessoa { dados: calcular([1, , 2], 3), outro: 4 }\n    variavel depois = 99\nfim";
        let resultado = analisar(&fonte(codigo));
        assert!(
            resultado.diagnosticos.iter().any(|d| d.codigo == "N2008"),
            "erros nas expressões internas deveriam ser diagnosticados"
        );
        let Some(NoAst::Funcao { corpo, .. }) = resultado.programa.itens.first() else {
            panic!("a função deveria ser preservada na AST parcial");
        };
        assert!(
            corpo.iter().any(|item| matches!(item, NoAst::Variavel { nome, .. } if nome == "depois")),
            "o Parser deveria continuar até a declaração posterior: {corpo:?}"
        );
        assert_eq!(corpo.len(), 5, "itens válidos posteriores não podem desaparecer: {corpo:?}");
    }

    #[test]
    fn recupera_delimitadores_aninhados_sem_perder_o_fechamento_externo() {
        let codigo = "funcao principal()\n    variavel quebrada = Pessoa { campo: chamar([1, , 2], 3), outro: 4 }\n    variavel final = 7\nfim";
        let resultado = analisar(&fonte(codigo));
        assert!(
            resultado.diagnosticos.iter().any(|d| d.codigo == "N2008"),
            "o erro interno deveria ser reportado"
        );
        let Some(NoAst::Funcao { corpo, .. }) = resultado.programa.itens.first() else {
            panic!("a função deveria ser preservada");
        };
        assert!(
            corpo.iter().any(|item| matches!(item, NoAst::Variavel { nome, .. } if nome == "final")),
            "o fechamento da estrutura interna não pode consumir o bloco externo"
        );
    }


    #[test]
    fn recupera_campos_e_variantes_invalidos_sem_perder_declaracoes_sequenciais() {
        let codigo = "funcao principal()\n    estrutura Pessoa { nome: , idade: inteiro }\n    enumeracao Estado { Ativo(, texto), Inativo(inteiro) }\n    variavel depois = 1\nfim";
        let resultado = analisar(&fonte(codigo));
        assert!(
            !resultado.diagnosticos.is_empty(),
            "campos e payloads inválidos precisam gerar diagnósticos"
        );
        let Some(NoAst::Funcao { corpo, .. }) = resultado.programa.itens.first() else {
            panic!("a função deveria ser preservada");
        };
        assert!(
            corpo.iter().any(|item| matches!(item, NoAst::Estrutura { nome, campos } if nome == "Pessoa" && campos.iter().any(|(campo, _)| campo == "idade"))),
            "a estrutura deve preservar o campo válido após o campo inválido: {corpo:?}"
        );
        assert!(
            corpo.iter().any(|item| matches!(item, NoAst::Enumeracao { nome, variantes } if nome == "Estado" && variantes.iter().any(|(variante, _)| variante == "Inativo"))),
            "a enumeração deve preservar a variante válida após o payload inválido: {corpo:?}"
        );
        assert!(
            corpo.iter().any(|item| matches!(item, NoAst::Variavel { nome, .. } if nome == "depois")),
            "o Parser deve continuar após as declarações recuperadas"
        );
    }


    #[test]
    fn recupera_parametros_de_funcao_e_preserva_o_corpo() {
        let codigo = "funcao principal(a: , b: inteiro)\n    devolver b\nfim\nfuncao outra(a inteiro, b: texto)\n    passar\nfim\n\nvariavel depois = 1";
        let resultado = analisar(&fonte(codigo));
        assert!(
            resultado.diagnosticos.iter().any(|d| d.codigo == "N2010" || d.codigo == "N2005"),
            "parâmetros inválidos precisam gerar diagnóstico: {:?}",
            resultado.diagnosticos
        );
        assert_eq!(resultado.programa.itens.len(), 3, "declarações posteriores devem ser preservadas: {:?}", resultado.programa.itens);
        assert!(matches!(&resultado.programa.itens[0], NoAst::Funcao { parametros, corpo, .. }
            if parametros.iter().any(|(nome, _)| nome == "b") && !corpo.is_empty()));
        assert!(matches!(&resultado.programa.itens[2], NoAst::Variavel { nome, .. } if nome == "depois"));
    }

    #[test]
    fn rejeita_parametros_duplicados_e_genericos_malformados_sem_perder_funcao() {
        let duplicado = analisar(&fonte("funcao duplicar(a: inteiro, a: texto)\n    passar\nfim"));
        assert!(duplicado.diagnosticos.iter().any(|d| d.codigo == "N2050"));

        let generico = analisar(&fonte("funcao combinar<T,>(valor: T)\n    devolver valor\nfim"));
        assert!(generico.diagnosticos.iter().any(|d| d.codigo == "N2015"));
        assert!(matches!(generico.programa.itens.first(), Some(NoAst::Funcao { .. })));
    }


    #[test]
    fn recupera_assinatura_de_funcao_com_retorno_ou_fechamento_ausente() {
        for codigo in [
            "funcao principal():\n    exibir(1)\nfim\n\nvariavel depois = 1",
            "funcao principal(a: inteiro\n    exibir(a)\nfim\n\nvariavel depois = 1",
            "funcao principal(a: )\n    exibir(1)\nfim\n\nvariavel depois = 1",
        ] {
            let resultado = analisar(&fonte(codigo));
            assert!(
                !resultado.diagnosticos.is_empty(),
                "assinatura incompleta deveria gerar diagnóstico: {codigo:?}"
            );
            assert!(
                resultado.programa.itens.iter().any(|item| matches!(item, NoAst::Variavel { nome, .. } if nome == "depois")),
                "o Parser deveria preservar itens posteriores: {codigo:?}; {:?}",
                resultado.programa.itens
            );
        }
    }

    #[test]
    fn recupera_caso_sem_bloco_e_continua_escolha() {
        let resultado = analisar(&fonte(
            "escolha x\n    caso Resultado::Ok caso _\n        passar\nfim"
        ));
        assert!(
            resultado.diagnosticos.iter().any(|d| d.codigo == "N2007"),
            "caso sem bloco deveria ser diagnosticado"
        );
        let Some(NoAst::Escolha { casos, .. }) = resultado.programa.itens.first() else {
            panic!("a instrução escolha deveria ser preservada");
        };
        assert_eq!(casos.len(), 1, "o caso válido posterior deveria ser preservado");
        assert!(casos[0].enumeracao.is_none() && casos[0].variante.is_none());
    }


    #[test]
    fn diagnosticos_de_duplicidade_apontam_o_token_repetido() {
        fn verifica(codigo: &str, diagnostico: &str, alvo: &str, ocorrencia: usize) {
            let resultado = analisar(&fonte(codigo));
            let inicio = codigo.match_indices(alvo).nth(ocorrencia)
                .map(|(indice, _)| indice)
                .unwrap_or_else(|| panic!("ocorrência {ocorrencia} de {alvo:?} não encontrada em {codigo:?}"));
            let esperado = Some(IntervaloFonte { inicio, fim: inicio + alvo.len() });
            let encontrado = resultado.diagnosticos.iter().find(|d| d.codigo == diagnostico)
                .unwrap_or_else(|| panic!("diagnóstico {diagnostico} ausente: {:?}", resultado.diagnosticos));
            assert_eq!(
                encontrado.intervalo, esperado,
                "{diagnostico} deveria apontar para {alvo:?} na ocorrência {ocorrencia}"
            );
        }

        verifica(
            "funcao duplicar(item: inteiro, item: texto)\n    passar\nfim",
            "N2050", "item", 1
        );
        verifica(
            "funcao generica<T, T>()\n    passar\nfim",
            "N2014", "T", 1
        );
        verifica(
            "estrutura Pessoa { nome: texto, nome: inteiro }",
            "N2031", "nome", 1
        );
        verifica(
            "enumeracao Estado { Ativo, Ativo }",
            "N2034", "Ativo", 1
        );
        verifica(
            "variavel pessoa = Pessoa { nome: 1, nome: 2 }",
            "N2037", "nome", 1
        );
        verifica(
            "escolha x\n    caso _\n        passar\n    caso _\n        passar\nfim",
            "N2043", "_", 1
        );
        verifica(
            "escolha x\n    caso Resultado::Ok\n        passar\n    caso Resultado::Ok\n        passar\nfim",
            "N2048", "Resultado", 1
        );
        verifica(
            "escolha x\n    caso Resultado::Ok(item, item)\n        passar\nfim",
            "N2046", "item", 1
        );
    }



    #[test]
    fn valida_pares_de_delimitadores_e_recupera_apos_fechamento_cruzado() {
        let valido = analisar(&fonte(
            "funcao principal()\n    variavel valor = montar([1, (2, 3)], Pessoa { nome: \"Ana\" })\n    variavel final = 9\nfim",
        ));
        assert!(valido.diagnosticos.is_empty(), "delimitadores válidos foram rejeitados: {:?}", valido.diagnosticos);
        let Some(NoAst::Funcao { corpo, .. }) = valido.programa.itens.first() else {
            panic!("a função válida deveria permanecer na AST");
        };
        assert!(corpo.iter().any(|item| matches!(item, NoAst::Variavel { nome, .. } if nome == "final")));

        let quebrado = analisar(&fonte(
            "funcao principal()\n    variavel quebrada = calcular([1, 2)\n    variavel preservada = 7\nfim",
        ));
        assert!(
            quebrado.diagnosticos.iter().any(|d| d.codigo == "N2051"),
            "fechamento incompatível deveria gerar N2051: {:?}",
            quebrado.diagnosticos
        );
        let Some(NoAst::Funcao { corpo, .. }) = quebrado.programa.itens.first() else {
            panic!("a função parcial deveria ser preservada");
        };
        assert!(
            corpo.iter().any(|item| matches!(item, NoAst::Variavel { nome, .. } if nome == "preservada")),
            "a recuperação perdeu a declaração posterior: {corpo:?}"
        );
    }

    #[test]
    fn diagnostica_fechamentos_sem_abertura_e_blocos_incompativeis() {
        for codigo in [
            "variavel valor = 1)",
            "variavel valor = [1]]",
            "variavel tupla = (1, 2]",
            "variavel lista = [1, 2)",
            "estrutura Pessoa { nome: inteiro)",
            "enumeracao Estado { Ativo(inteiro]",
            "funcao principal() { exibir(1) fim",
            "funcao principal()\n exibir(1)\n}",
        ] {
            let resultado = analisar(&fonte(codigo));
            assert!(
                resultado.diagnosticos.iter().any(|d| d.codigo == "N2051"),
                "fechamento sem par ou bloco incompatível não foi diagnosticado: {codigo:?}: {:?}",
                resultado.diagnosticos
            );
        }
    }

    #[test]
    fn diagnostica_aberturas_nao_fechadas_de_parenteses_colchetes_e_blocos() {
        for codigo in [
            "variavel valor = calcular(1, 2",
            "variavel valores = [1, 2",
            "variavel tupla = (1, 2",
            "variavel chamada = calcular(1, 2",
            "variavel indice = valores[0",
            "estrutura Pessoa { nome: texto",
            "enumeracao Estado { Ativo(inteiro)",
            "escolha valor {\ncaso Resultado::Ok\n    exibir(1)\n",
            "variavel pessoa = Pessoa { nome: \"Ana\"",
            "tipo Lista = [inteiro",
            "tipo Acao = funcao(inteiro",
            "funcao principal() { exibir(1)",
            "funcao principal()\n exibir(1)",
        ] {
            let resultado = analisar(&fonte(codigo));
            assert!(
                resultado.diagnosticos.iter().any(|d| d.codigo == "N2009"),
                "abertura não fechada deveria produzir N2009: {codigo:?}: {:?}",
                resultado.diagnosticos
            );
        }
    }

    #[test]
    fn aceita_blocos_aninhados_com_terminadores_fim() {
        for codigo in [
            "funcao principal()\n    quando verdadeiro\n        exibir(1)\n    fim\nfim",
            "funcao principal()\n quando verdadeiro\n  enquanto verdadeiro\n   exibir(1)\n  fim\n fim\nfim",
            "funcao principal()\n    quando verdadeiro\n        exibir(1)\n    fim\n    variavel depois = 2\nfim",
        ] {
            let resultado = analisar(&fonte(codigo));
            assert!(
                resultado.diagnosticos.is_empty(),
                "blocos aninhados válidos foram rejeitados: {codigo:?}: {:?}",
                resultado.diagnosticos
            );
        }
    }

    #[test]
    fn diagnosticos_sintaticos_tem_codigo_severidade_mensagem_e_intervalo_validos() {
        let casos = [
            "variavel = 1",
            "funcao f(a inteiro)\n    passar\nfim",
            "estrutura Pessoa { nome: }",
            "enumeracao Estado { Ativo(, texto) }",
            "escolha valor\n    caso Estado::Erro(1)\n        passar\nfim",
            "variavel resultado = 1 < 2 < 3",
            "funcao f() {",
        ];
        for codigo in casos {
            let resultado = analisar(&fonte(codigo));
            assert!(!resultado.diagnosticos.is_empty(),
                "entrada inválida sem diagnóstico: {codigo:?}");
            for diagnostico in &resultado.diagnosticos {
                assert!(diagnostico.codigo.starts_with('N'),
                    "código instável ou fora do padrão: {:?}", diagnostico);
                assert_eq!(diagnostico.severidade, Severidade::Erro,
                    "erro sintático deve ter severidade Erro: {:?}", diagnostico);
                assert!(!diagnostico.mensagem.trim().is_empty(),
                    "mensagem vazia: {:?}", diagnostico);
                let intervalo = diagnostico.intervalo
                    .unwrap_or_else(|| panic!("diagnóstico sem localização: {diagnostico:?}"));
                assert!(intervalo.inicio <= intervalo.fim && intervalo.fim <= codigo.len(),
                    "intervalo fora da fonte: {diagnostico:?}; fonte={codigo:?}");
                assert!(codigo.is_char_boundary(intervalo.inicio)
                    && codigo.is_char_boundary(intervalo.fim),
                    "intervalo divide um caractere UTF-8: {diagnostico:?}; fonte={codigo:?}");
            }
        }
    }

    #[test]
    fn diagnosticos_combinados_sao_ordenados_pela_posicao_na_fonte() {
        let codigo = "variavel a = )\nvariavel b = §";
        let resultado = analisar(&fonte(codigo));
        assert!(resultado.diagnosticos.iter().any(|d| d.codigo.starts_with("N1")),
            "o erro léxico posterior deveria ser preservado: {:?}", resultado.diagnosticos);
        assert!(resultado.diagnosticos.iter().any(|d| d.codigo.starts_with("N2")),
            "o erro sintático anterior deveria ser preservado: {:?}", resultado.diagnosticos);
        let posicoes: Vec<usize> = resultado.diagnosticos.iter()
            .map(|d| d.intervalo.expect("diagnóstico deve ter intervalo").inicio)
            .collect();
        assert!(posicoes.windows(2).all(|par| par[0] <= par[1]),
            "diagnósticos não estão em ordem de origem: {:?}", resultado.diagnosticos);
    }

    #[test]
    fn diagnostico_de_fechamento_ausente_aponta_para_o_fim_da_fonte() {
        let codigo = "funcao principal() {";
        let resultado = analisar(&fonte(codigo));
        let fechamento = resultado.diagnosticos.iter()
            .find(|d| d.codigo == "N2009")
            .expect("fechamento ausente deveria emitir N2009");
        assert_eq!(fechamento.intervalo,
            Some(IntervaloFonte { inicio: codigo.len(), fim: codigo.len() }),
            "espera no EOF deve usar intervalo vazio na posição final");
    }

    #[test]
    fn emite_todos_os_codigos_de_diagnostico_sintatico_documentados() {
        let casos = [
            ("N2001", "variavel a = 1 variavel b = 2"),
            ("N2002", "publico variavel a = 1"),
            ("N2003", "variavel a 1"),
            ("N2004", "funcao f\n    passar\nfim"),
            ("N2005", "funcao f(a inteiro)\n    passar\nfim"),
            ("N2006", "tipo A inteiro"),
            ("N2007", "funcao f() exibir(1)"),
            ("N2008", "variavel a ="),
            ("N2009", "funcao f() {"),
            ("N2010", "variavel = 1"),
            ("N2011", "variavel tentar = 1"),
            ("N2012", "variavel a = 1 < 2 < 3"),
            ("N2013", "variavel a = 1..2..3"),
            ("N2014", "funcao f<T, T>()\n    passar\nfim"),
            ("N2015", "funcao f<>()\n    passar\nfim"),
            ("N2016", "tentar"),
            ("N2030", "estrutura Pessoa nome: texto }"),
            ("N2031", "estrutura Pessoa { nome: texto, nome: inteiro }"),
            ("N2032", "estrutura Pessoa { nome: texto idade: inteiro }"),
            ("N2033", "enumeracao Estado Ativo }"),
            ("N2034", "enumeracao Estado { Ativo, Ativo }"),
            ("N2035", "enumeracao Estado { Ativo Inativo }"),
            ("N2036", "enumeracao Estado {}"),
            ("N2037", "variavel pessoa = Pessoa { nome: 1, nome: 2 }"),
            ("N2038", "variavel pessoa = Pessoa { nome: 1 idade: 2 }"),
            ("N2040", "escolha valor caso _\n    passar\nfim"),
            ("N2041", "escolha valor\n    outro {}\nfim"),
            ("N2042", "escolha valor\n    caso Variante\n        passar\nfim"),
            ("N2043", "escolha valor\n    caso _\n        passar\n    caso _\n        passar\nfim"),
            ("N2044", "escolha valor\n    passar\nfim"),
            ("N2045", "escolha valor\n    caso Estado::Erro(1)\n        passar\nfim"),
            ("N2046", "escolha valor\n    caso Estado::Erro(item, item)\n        passar\nfim"),
            ("N2047", "alterar total"),
            ("N2048", "escolha valor\n    caso Estado::Erro\n        passar\n    caso Estado::Erro\n        passar\nfim"),
            ("N2050", "funcao f(item: inteiro, item: texto)\n    passar\nfim"),
            ("N2051", "variavel valor = 1)"),
        ];

        for (codigo, exemplo) in casos {
            let resultado = analisar(&fonte(exemplo));
            assert!(
                resultado.diagnosticos.iter().any(|d| d.codigo == codigo),
                "{codigo} não foi emitido para {exemplo:?}: {:?}",
                resultado.diagnosticos
            );
        }

        let profundo = format!(
            "variavel aninhada = {}1{}",
            "(".repeat(300),
            ")".repeat(300)
        );
        let resultado = analisar(&fonte(&profundo));
        assert!(
            resultado.diagnosticos.iter().any(|d| d.codigo == "N2049"),
            "N2049 não foi emitido para aninhamento excessivo"
        );
    }


    #[test]
    fn separadores_repetidos_e_finais_sao_aceitos_em_todos_os_blocos() {
        let exemplos = [
            "\n;\nvariavel a = 1;;\n\nvariavel b = 2;\n",
            "funcao principal()\n    variavel a = 1\n    \n    \n    \n    variavel b = 2\nfim",
            "funcao principal()\n    ;; variavel a = 1;;\n\n    variavel b = 2;;\nfim",
            "estrutura Pessoa { ; nome: texto;; idade: inteiro; }",
            "enumeracao Estado { ; Ativo;; Inativo; }",
        ];
        for codigo in exemplos {
            let resultado = analisar(&fonte(codigo));
            assert!(
                resultado.diagnosticos.is_empty(),
                "separadores repetidos, linhas em branco ou separador final foram rejeitados: {codigo:?}: {:?}",
                resultado.diagnosticos
            );
        }
    }

    #[test]
    fn quebras_em_parenteses_e_colchetes_nao_separam_elementos() {
        let codigo = "variavel chamada = calcular(\n 1,\n\n 2\n)\nvariavel lista = [\n 3,\n\n 4\n]\nvariavel tupla = (\n 5,\n 6\n)";
        let resultado = analisar(&fonte(codigo));
        assert!(
            resultado.diagnosticos.is_empty(),
            "quebras internas de agrupamento não devem separar instruções: {:?}",
            resultado.diagnosticos
        );
        assert_eq!(resultado.programa.itens.len(), 3);
        assert!(matches!(
            &resultado.programa.itens[0],
            NoAst::Variavel { valor: Expressao::Chamada { argumentos, .. }, .. }
                if argumentos.len() == 2
        ));
        assert!(matches!(
            &resultado.programa.itens[1],
            NoAst::Variavel { valor: Expressao::Lista(valores), .. }
                if valores.len() == 2
        ));
        assert!(matches!(
            &resultado.programa.itens[2],
            NoAst::Variavel { valor: Expressao::Tupla(valores), .. }
                if valores.len() == 2
        ));
    }

    #[test]
    fn ponto_e_virgula_nao_e_ignorado_dentro_de_agrupamentos() {
        for codigo in [
            "variavel chamada = calcular(1; 2)",
            "variavel lista = [1; 2]",
            "variavel tupla = (1; 2)",
        ] {
            let resultado = analisar(&fonte(codigo));
            assert!(
                !resultado.diagnosticos.is_empty(),
                "ponto e vírgula dentro de agrupamento foi aceito indevidamente: {codigo:?}"
            );
        }
    }

    #[test]
    fn quebras_antes_de_operadores_de_continuacao_preservam_a_expressao() {
        let operadores = [
            "+", "-", "*", "/", "%", "**", "..", "<", "<=", ">", ">=",
            "==", "!=", "e", "ou", "=", "<-",
        ];
        for operador in operadores {
            let codigo = format!("variavel resultado = 1\n\n {operador} 2");
            let resultado = analisar(&fonte(&codigo));
            assert!(
                resultado.diagnosticos.is_empty(),
                "quebra antes do operador {operador:?} encerrou a expressão: {:?}",
                resultado.diagnosticos
            );
        }
    }

    #[test]
    fn quebras_depois_de_operadores_de_continuacao_preservam_a_expressao() {
        let operadores = [
            "+", "-", "*", "/", "%", "**", "..", "<", "<=", ">", ">=",
            "==", "!=", "e", "ou", "=", "<-",
        ];
        for operador in operadores {
            let codigo = format!("variavel resultado = 1 {operador}\n\n 2");
            let resultado = analisar(&fonte(&codigo));
            assert!(
                resultado.diagnosticos.is_empty(),
                "quebra depois do operador {operador:?} encerrou a expressão: {:?}",
                resultado.diagnosticos
            );
        }
    }

    #[test]
    fn quebras_antes_de_posfixos_e_membros_continuam_a_expressao() {
        for codigo in [
            "variavel chamada = funcao_alvo\n(1, 2)",
            "variavel indice = valores\n[0]",
            "variavel membro = objeto\n.campo",
            "variavel qualificado = objeto\n::interno",
        ] {
            let resultado = analisar(&fonte(codigo));
            assert!(
                resultado.diagnosticos.is_empty(),
                "quebra antes de construção pós-fixa deveria continuar a expressão: {codigo:?}: {:?}",
                resultado.diagnosticos
            );
        }
    }

    #[test]
    fn quebra_antes_de_identificador_separa_instrucoes_independentes() {
        let resultado = analisar(&fonte(
            "variavel primeiro = 1\n\nvariavel segundo = 2"
        ));
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
        assert_eq!(resultado.programa.itens.len(), 2);
        assert!(matches!(&resultado.programa.itens[0], NoAst::Variavel { nome, .. } if nome == "primeiro"));
        assert!(matches!(&resultado.programa.itens[1], NoAst::Variavel { nome, .. } if nome == "segundo"));
    }

    #[test]
    fn quebra_antes_de_senao_preserva_a_associacao_com_quando() {
        let resultado = analisar(&fonte(
            "quando verdadeiro\n    exibir(1)\n\n\nsenao\n    exibir(2)\nfim"
        ));
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
        assert!(matches!(
            resultado.programa.itens.first(),
            Some(NoAst::Quando { entao, senao, .. }) if entao.len() == 1 && senao.len() == 1
        ));
    }

    #[test]
    fn indentacao_define_niveis_e_fim_fecha_blocos() {
        let codigo = "funcao principal()\n    variavel primeiro = 1\n    se sim\n        variavel segundo = 2\n    fim\nfim";
        let valido = analisar(&fonte(codigo));
        assert!(
            valido.diagnosticos.is_empty(),
            "blocos indentados e encerrados com 'fim' devem ser aceitos: {:?}",
            valido.diagnosticos
        );
        let Some(NoAst::Funcao { corpo, .. }) = valido.programa.itens.first() else {
            panic!("esperada função principal");
        };
        let Some(NoAst::Quando { entao, .. }) = corpo.iter().find(|no| matches!(no, NoAst::Quando { .. })) else {
            panic!("esperado condicional dentro da função");
        };
        assert!(entao.iter().any(|no| matches!(
            no, NoAst::Variavel { nome, .. } if nome == "segundo"
        )));

        let sem_indentacao = analisar(&fonte(
            "funcao principal()\nvariavel valor = 1\nfim"
        ));
        assert!(
            sem_indentacao.diagnosticos.iter().any(|d| d.codigo == "N2009"),
            "bloco não vazio sem indentação deve ser rejeitado"
        );
        let sem_fechamento = analisar(&fonte(
            "funcao principal()\n    variavel primeiro = 1\n"
        ));
        assert!(
            sem_fechamento.diagnosticos.iter().any(|d| d.codigo == "N2009"),
            "bloco sem 'fim' deve continuar inválido mesmo com dedentação"
        );
        let vazio_sem_fim = analisar(&fonte("funcao vazia()\n"));
        assert!(
            vazio_sem_fim.diagnosticos.iter().any(|d| d.codigo == "N2009"),
            "bloco vazio também precisa do terminador 'fim'"
        );
    }

    #[test]
    fn condicional_indentado_exige_terminador_fim() {
        let invalido = analisar(&fonte("se sim\n    passar\n"));
        assert!(
            invalido.diagnosticos.iter().any(|d| d.codigo == "N2009"),
            "condicional indentado sem 'fim' precisa ser rejeitado"
        );
        let valido = analisar(&fonte("se sim { passar }"));
        assert!(
            valido.diagnosticos.is_empty(),
            "a forma delimitada por chaves continua compatível: {:?}",
            valido.diagnosticos
        );
        let aninhado = analisar(&fonte(
            "funcao principal()\n    se sim\n    passar\nfim\n\nfim"
        ));
        assert!(
            aninhado.diagnosticos.is_empty(),
            "o 'fim' da função externa não pode ser consumido pelo condicional com chaves: {:?}",
            aninhado.diagnosticos
        );
    }

    #[test]
    fn ponto_e_virgula_nao_substitui_quebra_antes_de_senao() {
        let resultado = analisar(&fonte(
            "quando verdadeiro\n    passar\nfim\n;\nsenao\n    passar\nfim"
        ));
        assert!(
            !resultado.diagnosticos.is_empty(),
            "ponto e vírgula não deve ser tratado como continuação de 'senao'"
        );
    }


    #[test]
    fn analisa_campos_de_estrutura_com_todas_as_formas_de_tipo() {
        let codigo = "estrutura Registro {\n simples: inteiro,\n qualificado: utilitarios::Identidade,\n lista: [texto],\n generico: Mapa<texto, [inteiro]>,\n callback: funcao(inteiro, texto): logico,\n aninhado: funcao(funcao(inteiro): logico): vazio,\n}";
        let resultado = analisar(&fonte(codigo));
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
        let Some(NoAst::Estrutura { campos, .. }) = resultado.programa.itens.first() else {
            panic!("a declaração de estrutura deveria produzir AST");
        };
        assert_eq!(campos.len(), 6);
        assert_eq!(campos[1].1, "utilitarios::Identidade");
        assert_eq!(campos[2].1, "[texto]");
        assert_eq!(campos[3].1, "Mapa<texto,[inteiro]>");
        assert_eq!(campos[4].1, "funcao(inteiro,texto):logico");
        assert_eq!(campos[5].1, "funcao(funcao(inteiro):logico):vazio");
    }

    #[test]
    fn analisa_variantes_com_payloads_unitarios_e_multiplos() {
        let codigo = "enumeracao Resultado {\n Vazio,\n Texto(texto),\n Par(inteiro, texto),\n Complexo(Mapa<texto, [inteiro]>, funcao(inteiro): logico),\n}";
        let resultado = analisar(&fonte(codigo));
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
        let Some(NoAst::Enumeracao { variantes, .. }) = resultado.programa.itens.first() else {
            panic!("a enumeração deveria produzir AST");
        };
        assert_eq!(variantes.len(), 4);
        assert!(variantes[0].1.is_empty());
        assert_eq!(variantes[1].1, ["texto"]);
        assert_eq!(variantes[2].1, ["inteiro", "texto"]);
        assert_eq!(variantes[3].1, ["Mapa<texto,[inteiro]>", "funcao(inteiro):logico"]);
    }

    #[test]
    fn analisa_construcoes_de_estrutura_aninhadas_e_preserva_campos() {
        let codigo = r#"variavel pessoa = Pessoa {
 nome: "Ana",
 endereco: Endereco { cidade: "Sao Paulo", numero: 10 },
 ativo: verdadeiro,
}"#;
        let resultado = analisar(&fonte(codigo));
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
        let NoAst::Variavel { valor: Expressao::ConstrucaoEstrutura { nome, campos }, .. } =
            &resultado.programa.itens[0] else { panic!("construção deveria produzir AST"); };
        assert_eq!(nome, "Pessoa");
        assert_eq!(campos.len(), 3);
        assert!(matches!(&campos[1].1, Expressao::ConstrucaoEstrutura { nome, campos }
            if nome == "Endereco" && campos.len() == 2));
    }

    #[test]
    fn aceita_payload_de_enum_vazio_entre_parenteses() {
        let resultado = analisar(&fonte("enumeracao Estado { Ativo() }"));
        assert!(resultado.diagnosticos.is_empty(), "{:?}", resultado.diagnosticos);
        assert!(matches!(
            resultado.programa.itens.first(),
            Some(NoAst::Enumeracao { variantes, .. }) if variantes.iter().any(|(nome, tipos)| nome == "Ativo" && tipos.is_empty())
        ));
    }

    #[test]
    fn campo_sem_dois_pontos_nao_e_inserido_na_ast() {
        let resultado = analisar(&fonte(
            "estrutura Pessoa { nome texto, idade: inteiro }"
        ));
        assert!(
            resultado.diagnosticos.iter().any(|d| d.codigo == "N2032"),
            "campo sem ':' precisa gerar diagnóstico"
        );
        let Some(NoAst::Estrutura { campos, .. }) = resultado.programa.itens.first() else {
            panic!("estrutura deveria ser preservada");
        };
        assert_eq!(campos, &[("idade".into(), "inteiro".into())]);
    }

    #[test]
    fn campo_sem_dois_pontos_em_construcao_nao_e_inserido_na_ast() {
        let resultado = analisar(&fonte(
            r#"variavel pessoa = Pessoa { nome "Ana", idade: 20 }"#
        ));
        assert!(
            resultado.diagnosticos.iter().any(|d| d.codigo == "N2038"),
            "campo malformado precisa gerar diagnóstico"
        );
        let Some(NoAst::Variavel { valor: Expressao::ConstrucaoEstrutura { campos, .. }, .. }) =
            resultado.programa.itens.first() else {
                panic!("construção parcial deveria ser preservada");
            };
        assert_eq!(campos, &[("idade".into(), Expressao::Literal("20".into()))]);
    }

    #[test]
    fn campos_e_variantes_normalizados_em_unicode_detectam_duplicidade() {
        let estrutura = analisar(&fonte(
            "estrutura Pessoa { ação: texto, ação: inteiro }"
        ));
        assert!(estrutura.diagnosticos.iter().any(|d| d.codigo == "N2031"));

        let enumeracao = analisar(&fonte(
            "enumeracao Estado { Ação, Ação }"
        ));
        assert!(enumeracao.diagnosticos.iter().any(|d| d.codigo == "N2034"));
    }

    #[test]
    fn recupera_estrutura_e_enumeracao_malformadas_sem_perder_declaracoes_sequenciais() {
        let codigo = "estrutura Pessoa { nome texto, idade: inteiro }\nenumeracao Estado { Ativo, Inativo(inteiro,), Pronto(inteiro) }\nvariavel depois = 1";
        let resultado = analisar(&fonte(codigo));
        assert!(resultado.diagnosticos.iter().any(|d| d.codigo == "N2032"));
        assert!(resultado.diagnosticos.iter().any(|d| d.codigo == "N2035"));
        assert!(resultado.programa.itens.iter().any(|item|
            matches!(item, NoAst::Estrutura { campos, .. } if campos.iter().any(|(nome, _)| nome == "idade"))));
        assert!(resultado.programa.itens.iter().any(|item|
            matches!(item, NoAst::Enumeracao { variantes, .. } if variantes.iter().any(|(nome, _)| nome == "Pronto"))));
        assert!(resultado.programa.itens.iter().any(|item|
            matches!(item, NoAst::Variavel { nome, .. } if nome == "depois")));
    }

    #[test]
    fn analisa_sintaxe_principal_atual_e_recursos_nativos() {
        let codigo = r#"
incluir biblioteca matematica como mat

variavel ativo: logico = sim
variavel dobrar = lambda x: x * 2
variavel somar = (a, b) -> a + b
variavel quadrado = x -> x ** 2

funcao saudar(nome: texto = "Visitante") -> vazio
    se ativo == sim
        exibir(nome)
    sendo nao ativo
        exibir("Inativo")
    senao
        passar
    fim
fim

assinc funcao carregar() -> texto
    variavel resultado = aguarda buscarDados()
    retorne resultado
fim

classe Pessoa herda Humano
    funcao apresentar() -> vazio
        exibir(isto.nome)
    fim
fim

funcao percorrer()
    para numero dentro faixa(0, 10)
        se numero == 5
            prossiga
        fim
        exibir(numero)
    fim
fim

funcao escolher(opcao: inteiro)
    escolha opcao
        caso 1
            exibir("Um")
        caso 2
            exibir("Dois")
        padrao
            passar
    fim
fim
"#;
        let resultado = analisar(&fonte(codigo));
        assert!(resultado.diagnosticos.is_empty(), "{:#?}", resultado.diagnosticos);
        assert!(resultado.programa.itens.iter().any(|no| matches!(no, NoAst::Classe { .. })));
        assert!(resultado.programa.itens.iter().any(|no| matches!(no, NoAst::Funcao { assincrona: true, .. })));
    }

    #[test]
    fn analisa_operadores_de_baixo_nivel_e_condicional_ternaria() {
        let codigo = r#"
funcao operar(a: inteiro, b: inteiro, idade: inteiro)
    variavel bits = (a & b) | (a ^ b) << 2
    variavel opcional = valor?.campo ?? valorPadrao
    variavel faixa = 1..10
    variavel condicao = idade >= 18 ? sim : nao
    variavel objeto = novo Pessoa()
    variavel numero = 1
    numero =+ 2
    numero++
    variavel dentro = numero dentro faixa
    retorne bits
fim
"#;
        let resultado = analisar(&fonte(codigo));
        assert!(resultado.diagnosticos.is_empty(), "{:#?}", resultado.diagnosticos);
    }

    #[test]
    fn analisa_tratamento_de_erros_escopo_e_recursos() {
        let codigo = r#"
funcao tratar()
    tente
        variavel numero = inteiro("abc")
    erro ValorInvalido
        exibir("Valor incorreto")
    sempre
        exibir("Finalizado")
    fim

    com abrir("dados.txt", "leitura") como arquivo
        exibir(arquivo)
    fim

    verifique sim, "Condicao invalida"
    lancar erro "Operacao invalida"
    global contador
    externa valor
fim
"#;
        let resultado = analisar(&fonte(codigo));
        assert!(resultado.diagnosticos.is_empty(), "{:#?}", resultado.diagnosticos);
        let funcao = resultado.programa.itens.first().expect("função");
        let NoAst::Funcao { corpo, .. } = funcao else { panic!("esperada função") };
        assert!(corpo.iter().any(|no| matches!(no, NoAst::Tente { .. })));
        assert!(corpo.iter().any(|no| matches!(no, NoAst::Com { .. })));
        assert!(corpo.iter().any(|no| matches!(no, NoAst::Verifique { .. })));
        assert!(corpo.iter().any(|no| matches!(no, NoAst::Lancar(_))));
    }

    #[test]
    fn analisa_atributos_e_contextos_de_codigo_nativo() {
        let codigo = r#"
@alinhamento(8)
@representacao("C")
estrutura Cabecalho { codigo: u32 }

critico:
    passar
fim

privilegio sistema:
    passar
fim

montagem x86_64<u32>:
    passar
fim
"#;
        let resultado = analisar(&fonte(codigo));
        assert!(resultado.diagnosticos.is_empty(), "{:#?}", resultado.diagnosticos);
        assert!(resultado.programa.itens.iter().any(|no| matches!(no, NoAst::Atributada { .. })));
        assert_eq!(
            resultado.programa.itens.iter().filter(|no| matches!(no, NoAst::BlocoContextual { .. })).count(),
            3
        );
        assert!(resultado.programa.itens.iter().any(|no| matches!(
            no,
            NoAst::BlocoContextual { contexto, tipos_genericos, .. }
                if contexto == "montagem" && tipos_genericos == &["u32"]
        )));
    }

    #[test]
    fn parser_cobre_membros_de_classe_importacao_por_origem_e_tipos_nativos() {
        let codigo = r#"
incluir raiz de matematica como raiz

classe Pessoa
    publico funcao construtor(nome: texto)
        isto.nome = nome
    fim

    protegido estatico funcao contar() -> u32
        retorne 1
    fim
fim

variavel endereco: ponteiroBruto<u8> = obter()
variavel dados: vetor<u8, 16> = criar()
variavel referencia = ponteiro.de(&numero)
variavel reserva = memoria.reservar(4096, 8)?
variavel leitura = memoria.ler<u32>(endereco)
variavel saudacao = "Ola, {nome}"
criarUsuario(nome = "Ana", idade = 25)
"#;
        let resultado = analisar(&fonte(codigo));
        assert!(
            resultado.diagnosticos.is_empty(),
            "construções normativas deveriam ser aceitas: {:#?}",
            resultado.diagnosticos
        );
        assert!(matches!(
            resultado.programa.itens.first(),
            Some(NoAst::Incluir { caminho, apelido: Some(apelido) })
                if caminho == "matematica::raiz" && apelido == "raiz"
        ));
        let Some(NoAst::Classe { corpo, .. }) = resultado.programa.itens.get(1) else {
            panic!("esperada classe");
        };
        assert!(corpo.iter().any(|no| matches!(
            no,
            NoAst::Funcao { nome, visibilidade: Some(Visibilidade::Publico), .. }
                if nome == "construtor"
        )));
        assert!(corpo.iter().any(|no| matches!(
            no,
            NoAst::Funcao { nome, estatica: true, visibilidade: Some(Visibilidade::Protegido), .. }
                if nome == "contar"
        )));
        assert!(resultado.programa.expressoes_localizadas.iter().any(|localizado| matches!(
            &localizado.expressao,
            Expressao::Chamada { tipos_genericos, .. } if tipos_genericos == &["u32"]
        )));
        assert!(resultado.programa.expressoes_localizadas.iter().any(|localizado| matches!(
            &localizado.expressao,
            Expressao::Chamada { argumentos, .. }
                if argumentos.iter().any(|argumento| matches!(argumento, Expressao::ArgumentoNomeado { nome, .. } if nome == "nome"))
        )));
        assert!(resultado.programa.expressoes_localizadas.iter().any(|localizado|
            matches!(&localizado.expressao, Expressao::Literal(valor) if valor.contains("{nome}"))
        ));
    }


    #[test]
    fn blocos_de_instrucao_exigem_indentacao_e_fim() {
        let valido = analisar(&fonte(
            "funcao principal()\\n    se verdadeiro\\n        passar\\n    fim\\nfim"
        ));
        assert!(
            valido.diagnosticos.is_empty(),
            "blocos corretamente indentados devem ser aceitos: {:#?}",
            valido.diagnosticos
        );

        for codigo in [
            "funcao principal() { passar }",
            "se verdadeiro { passar }",
            "enquanto verdadeiro { passar }",
            "classe Pessoa { funcao iniciar() { passar } }",
        ] {
            let resultado = analisar(&fonte(codigo));
            assert!(
                resultado.diagnosticos.iter().any(|d| d.codigo == "N2009"),
                "bloco com chaves não pode substituir indentação: {codigo:?}: {:#?}",
                resultado.diagnosticos
            );
        }

        let indentacao_superior = analisar(&fonte("    variavel fora = 1"));
        assert!(
            indentacao_superior.diagnosticos.iter().any(|d| d.codigo == "N2009"),
            "indentação não pode iniciar no escopo superior"
        );

        let recuperacao = analisar(&fonte(
            "funcao quebrada()\\nvariavel depois = 1\\nfim\\nvariavel fora = 2"
        ));
        assert!(recuperacao.programa.itens.iter().any(
            |no| matches!(no, NoAst::Variavel { nome, .. } if nome == "depois")
        ));
        assert!(recuperacao.programa.itens.iter().any(
            |no| matches!(no, NoAst::Variavel { nome, .. } if nome == "fora")
        ));

        for codigo in [
            "funcao principal()\\npassar\\nfim",
            "funcao principal()\\n    passar\\n",
            "funcao vazia()\\nfim",
        ] {
            let resultado = analisar(&fonte(codigo));
            assert!(
                resultado.diagnosticos.iter().any(|d| d.codigo == "N2009"),
                "bloco sem indentação ou sem terminador deve ser rejeitado: {codigo:?}: {:#?}",
                resultado.diagnosticos
            );
        }
    }


    #[test]
    fn analisa_construcoes_de_baixo_nivel_da_sintaxe2() {
        let codigo = r#"
@representacao(C)
@alinhamento(8)
estrutura Cabecalho:
    codigo: u32
    tamanho: u16
    flags: u16
fim

@convencao(C)
externa funcao escreverByte(valor: u8) -> vazio

funcao iniciarKernel() -> vazio:
    memoria.inicializar()
    passar
fim

funcao tratarTeclado(evento: interrupcao.Evento):
    variavel tecla = porta.ler8(0x60)
    passar
fim

funcao montar() -> vazio:
    variavel resultado = montagem x86_64<u64>(
        entradas: [10, 20],
        saidas: [resultado],
        efeitos: [memoria, registrador],
        instrucoes: [registrador.ler(0), registrador.escrever(1, 2)]
    )
    passar
fim

enumeracao EstadoKernel:
    Inicializando
    Pronto
    Falha(inteiro)
fim
"#;
        let resultado = analisar(&fonte(codigo));
        assert!(
            resultado.diagnosticos.is_empty(),
            "construções da sintaxe 2 precisam gerar AST sem erros: {:#?}",
            resultado.diagnosticos
        );
        assert!(resultado.programa.itens.iter().any(|no| matches!(
            no,
            NoAst::Atributada { declaracao, .. }
                if matches!(declaracao.as_ref(), NoAst::Estrutura { nome, campos }
                    if nome == "Cabecalho" && campos.len() == 3)
        )));
        assert!(resultado.programa.itens.iter().any(|no| matches!(
            no,
            NoAst::Atributada { atributos, declaracao }
                if atributos.iter().any(|atributo| atributo.nome == "convencao")
                    && matches!(declaracao.as_ref(), NoAst::Atributada { atributos, declaracao }
                        if atributos.iter().any(|atributo| atributo.nome == "externa")
                            && matches!(declaracao.as_ref(), NoAst::Funcao { nome, corpo, .. }
                                if nome == "escreverByte" && corpo.is_empty()))
        )));
        assert!(resultado.programa.itens.iter().any(|no| matches!(
            no,
            NoAst::Enumeracao { nome, variantes }
                if nome == "EstadoKernel" && variantes.len() == 3
        )));
        assert!(resultado.programa.expressoes_localizadas.iter().any(|localizado| matches!(
            &localizado.expressao,
            Expressao::Chamada { alvo, tipos_genericos, argumentos }
                if matches!(alvo.as_ref(), Expressao::Nome(nome) if nome == "montagem::x86_64")
                    && tipos_genericos == &["u64"]
                    && argumentos.len() == 4
        )));
    }

    #[test]
    fn aceita_bloco_indentado_apos_tipos_genericos_e_tuplas() {
        let codigo = r#"
funcao envolver<T>(valor: T): Resultado<Opcional<T>, texto>
    devolver Resultado::Sucesso(Opcional::Algum(valor))
fim

funcao aplicar<T>(valor: T, operacao: funcao(T): Opcional<T>): Resultado<Opcional<T>, texto>
    devolver Resultado::Sucesso(operacao(valor))
fim

funcao inverter(par: Tupla<inteiro64, inteiro64>): Tupla<inteiro64, inteiro64>
    devolver (par[1], par[0])
fim
"#;
        let resultado = analisar(&fonte(codigo));
        assert!(
            resultado.diagnosticos.is_empty(),
            "tipos compostos não podem consumir a quebra que inicia o bloco: {:#?}",
            resultado.diagnosticos
        );
    }

    #[test]
    fn limite_de_aninhamento_corresponde_a_gramatica_normativa() {
        let dentro = format!("{}verdadeiro", "!".repeat(200));
        let resultado_dentro = analisar(&fonte(&dentro));
        assert!(
            !resultado_dentro.diagnosticos.iter().any(|d| d.codigo == "N2049"),
            "200 níveis devem respeitar o limite normativo de 256: {:?}",
            resultado_dentro.diagnosticos
        );

        let acima = format!("{}verdadeiro", "!".repeat(260));
        let resultado_acima = analisar(&fonte(&acima));
        assert!(
            resultado_acima.diagnosticos.iter().any(|d| d.codigo == "N2049"),
            "260 níveis devem produzir N2049"
        );
    }

}

}

pub use ast::{
    AliasTipoAst, CampoAst, CasoEscolha, CasoEscolhaLocalizado, Expressao, ExpressaoLocalizada,
    NoAst, NoLocalizado, OperadorAst, PadraoAst, ParametroAst, Programa, ResultadoSintatico,
    TipoAst, TipoLocalizado, VarianteAst, Visibilidade,
};
pub use lexer::{ResultadoLexico, TipoToken, Token, analisar_lexico};

pub use implementacao::*;
