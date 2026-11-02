//! Lexer, AST e Parser do subconjunto inicial da linguagem Nebula.

#![forbid(unsafe_code)]
#![allow(missing_docs)]
#![rustfmt::skip]

use nebula_diagnosticos::{Diagnostico, Severidade};
use nebula_fonte::{ArquivoFonte, IntervaloFonte};
use unicode_ident::{is_xid_continue, is_xid_start};
use unicode_normalization::UnicodeNormalization;

/// Categorias de tokens produzidas pelo Lexer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TipoToken {
    Fim, NovaLinha, Identificador, Inteiro, Decimal, Texto, Caractere,
    Variavel, Constante, Funcao, Quando, Senao, Para, Enquanto, Devolver,
    Incluir, Como, Verdadeiro, Falso, Nulo, FimBloco, Parar, Seguir,
    E, Ou, Nao, Publico, Privado, Tipo, ReservadaFutura,
    AbrePar, FechaPar, AbreChave, FechaChave, AbreColchete, FechaColchete,
    Virgula, PontoVirgula, DoisPontos, Ponto,
    Atribui, Mais, Menos, Vezes, Divide, Resto, Potencia,
    Igual, Diferente, Menor, MenorIgual, Maior, MaiorIgual,
    Negacao, Intervalo, Escopo,
}

/// Token com texto original e intervalo de bytes na fonte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub tipo: TipoToken,
    pub lexema: String,
    pub intervalo: IntervaloFonte,
}

/// Resultado léxico: tokens e diagnósticos; erros não interrompem a varredura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultadoLexico {
    pub tokens: Vec<Token>,
    pub diagnosticos: Vec<Diagnostico>,
}

/// Analisa lexicalmente uma fonte Nebula.
#[must_use]
pub fn analisar_lexico(fonte: &ArquivoFonte) -> ResultadoLexico {
    Lexer::novo(fonte.conteudo()).executar()
}

struct Lexer<'a> {
    texto: &'a str,
    bytes: &'a [u8],
    inicio: usize,
    atual: usize,
    profundidade_continuacao: usize,
    tokens: Vec<Token>,
    diagnosticos: Vec<Diagnostico>,
}

impl<'a> Lexer<'a> {
    fn novo(texto: &'a str) -> Self {
        Self { texto, bytes: texto.as_bytes(), inicio: 0, atual: 0, profundidade_continuacao: 0, tokens: Vec::new(), diagnosticos: Vec::new() }
    }

    fn executar(mut self) -> ResultadoLexico {
        while !self.fim() {
            self.inicio = self.atual;
            self.escanear();
        }
        self.adicionar(TipoToken::Fim, self.atual, self.atual);
        ResultadoLexico { tokens: self.tokens, diagnosticos: self.diagnosticos }
    }

    fn fim(&self) -> bool { self.atual >= self.bytes.len() }
    fn byte(&self, deslocamento: usize) -> Option<u8> { self.bytes.get(self.atual + deslocamento).copied() }
    fn avancar(&mut self) -> u8 { let b = self.bytes[self.atual]; self.atual += 1; b }
    fn corresponde(&mut self, esperado: u8) -> bool {
        if self.byte(0) == Some(esperado) { self.atual += 1; true } else { false }
    }
    fn adicionar(&mut self, tipo: TipoToken, inicio: usize, fim: usize) {
        self.tokens.push(Token { tipo, lexema: self.texto.get(inicio..fim).unwrap_or("").to_owned(), intervalo: IntervaloFonte { inicio, fim } });
    }
    fn erro(&mut self, codigo: &'static str, mensagem: &str, inicio: usize, fim: usize) {
        self.diagnosticos.push(Diagnostico::novo(codigo, Severidade::Erro, mensagem, Some(IntervaloFonte { inicio, fim })));
    }
    fn caractere_atual(&self) -> Option<char> { self.texto.get(self.atual..)?.chars().next() }
    fn escanear(&mut self) {
        let b = self.avancar();
        match b {
            b' ' | b'\t' => {},
            b'\n' => { if self.profundidade_continuacao == 0 { self.adicionar(TipoToken::NovaLinha, self.inicio, self.atual); } },
            b'\r' => {
                if self.corresponde(b'\n') { if self.profundidade_continuacao == 0 { self.adicionar(TipoToken::NovaLinha, self.inicio, self.atual); } }
                else { self.erro("N1001", "Quebra CR isolada; use LF ou CRLF.", self.inicio, self.atual); }
            }
            b'/' if self.corresponde(b'/') => self.comentario_linha(),
            b'/' if self.corresponde(b'*') => {
                let mut fechado = false;
                while !self.fim() {
                    if self.byte(0) == Some(b'*') && self.byte(1) == Some(b'/') {
                        self.atual += 2;
                        fechado = true;
                        break;
                    }
                    if self.byte(0) == Some(b'\n') && self.profundidade_continuacao == 0 {
                        let pos = self.atual;
                        self.atual += 1;
                        self.adicionar(TipoToken::NovaLinha, pos, self.atual);
                    } else if self.byte(0) == Some(b'\r') && self.byte(1) == Some(b'\n') && self.profundidade_continuacao == 0 {
                        let pos = self.atual;
                        self.atual += 2;
                        self.adicionar(TipoToken::NovaLinha, pos, self.atual);
                    } else if self.byte(0) == Some(b'\r') && self.byte(1) != Some(b'\n') {
                        let pos = self.atual;
                        self.atual += 1;
                        self.erro("N1001", "Quebra CR isolada; use LF ou CRLF.", pos, self.atual);
                    } else {
                        self.atual += 1;
                    }
                }
                if !fechado { self.erro("N1002", "Comentário de bloco não terminado.", self.inicio, self.atual); }
            }
            b'"' => self.literal_texto(),
            b'\'' => self.literal_caractere(),
            b'0'..=b'9' => self.numero(),
            b'(' => { self.profundidade_continuacao += 1; self.simples(TipoToken::AbrePar); }, b')' => { self.profundidade_continuacao = self.profundidade_continuacao.saturating_sub(1); self.simples(TipoToken::FechaPar); },
            b'{' => self.simples(TipoToken::AbreChave), b'}' => self.simples(TipoToken::FechaChave),
            b'[' => { self.profundidade_continuacao += 1; self.simples(TipoToken::AbreColchete); }, b']' => { self.profundidade_continuacao = self.profundidade_continuacao.saturating_sub(1); self.simples(TipoToken::FechaColchete); },
            b',' => self.simples(TipoToken::Virgula), b';' => self.simples(TipoToken::PontoVirgula),
            b':' if self.corresponde(b':') => self.simples(TipoToken::Escopo),
            b':' => self.simples(TipoToken::DoisPontos),
            b'.' if self.corresponde(b'.') => self.simples(TipoToken::Intervalo),
            b'.' => self.simples(TipoToken::Ponto),
            b'+' => self.simples(TipoToken::Mais), b'-' => self.simples(TipoToken::Menos),
            b'*' if self.corresponde(b'*') => self.simples(TipoToken::Potencia),
            b'*' => self.simples(TipoToken::Vezes), b'%' => self.simples(TipoToken::Resto),
            b'/' => self.simples(TipoToken::Divide),
            b'=' if self.corresponde(b'=') => self.simples(TipoToken::Igual),
            b'=' => self.simples(TipoToken::Atribui),
            b'!' if self.corresponde(b'=') => self.simples(TipoToken::Diferente),
            b'!' => self.simples(TipoToken::Negacao),
            b'<' if self.corresponde(b'=') => self.simples(TipoToken::MenorIgual),
            b'<' => self.simples(TipoToken::Menor),
            b'>' if self.corresponde(b'=') => self.simples(TipoToken::MaiorIgual),
            b'>' => self.simples(TipoToken::Maior),
            _ => {
                self.atual -= 1;
                if let Some(c) = self.caractere_atual() {
                    if c == '_' || is_xid_start(c) { self.identificador(); }
                    else { self.atual += c.len_utf8(); self.erro("N1003", &format!("Caractere inesperado: {c}."), self.inicio, self.atual); }
                }
            }
        }
    }
    fn comentario_linha(&mut self) {
        while !self.fim() && self.byte(0) != Some(b'\n') {
            if self.byte(0) == Some(b'\r') && self.byte(1) == Some(b'\n') {
                break;
            }
            if self.byte(0) == Some(b'\r') {
                let pos = self.atual;
                self.atual += 1;
                self.erro("N1001", "Quebra CR isolada; use LF ou CRLF.", pos, self.atual);
            } else {
                self.atual += 1;
            }
        }
    }
    fn simples(&mut self, tipo: TipoToken) { self.adicionar(tipo, self.inicio, self.atual); }
    fn identificador(&mut self) {
        while let Some(c) = self.caractere_atual() {
            if c == '_' || is_xid_continue(c) { self.atual += c.len_utf8(); } else { break; }
        }
        let palavra_original = &self.texto[self.inicio..self.atual];
        let palavra = palavra_original.nfc().collect::<String>();
        let tipo = match palavra.as_str() {
            "variavel" => TipoToken::Variavel, "constante" => TipoToken::Constante,
            "funcao" => TipoToken::Funcao, "quando" => TipoToken::Quando,
            "senao" => TipoToken::Senao, "para" => TipoToken::Para,
            "enquanto" => TipoToken::Enquanto, "devolver" => TipoToken::Devolver,
            "incluir" => TipoToken::Incluir, "como" => TipoToken::Como,
            "verdadeiro" => TipoToken::Verdadeiro, "falso" => TipoToken::Falso,
            "nulo" => TipoToken::Nulo, "fim" => TipoToken::FimBloco,
            "parar" => TipoToken::Parar, "seguir" => TipoToken::Seguir,
            "e" => TipoToken::E, "ou" => TipoToken::Ou, "nao" => TipoToken::Nao,
            "publico" => TipoToken::Publico, "privado" => TipoToken::Privado,
            "tipo" => TipoToken::Tipo,
            "cada" | "tentar" | "capturar" | "finalmente" | "assinc" | "aguardar" | "estrutura" | "enumeracao" => TipoToken::ReservadaFutura,
            _ => TipoToken::Identificador,
        };
        self.adicionar(tipo, self.inicio, self.atual);
    }
    fn numero(&mut self) {
        let mut tipo = TipoToken::Inteiro;
        let base = match (self.bytes.get(self.inicio).copied(), self.byte(0)) {
            (Some(b'0'), Some(b'x' | b'X')) => Some(16u32),
            (Some(b'0'), Some(b'b' | b'B')) => Some(2u32),
            (Some(b'0'), Some(b'o' | b'O')) => Some(8u32),
            _ => None,
        };
        if let Some(radix) = base {
            self.atual += 1;
            let inicio_digitos = self.atual;
            while let Some(b) = self.byte(0) {
                let digito_valido = match radix {
                    2 => matches!(b, b'0'..=b'1'),
                    8 => matches!(b, b'0'..=b'7'),
                    16 => b.is_ascii_hexdigit(),
                    _ => false,
                };
                if b == b'_' || digito_valido { self.atual += 1; }
                else if b.is_ascii_alphanumeric() { self.atual += 1; }
                else { break; }
            }
            let trecho = &self.texto[inicio_digitos..self.atual];
            let valido = !trecho.is_empty()
                && !trecho.starts_with('_') && !trecho.ends_with('_')
                && !trecho.contains("__")
                && trecho.chars().filter(|c| *c != '_').all(|c| c.to_digit(radix).is_some());
            if !valido { self.erro("N1005", "Literal inteiro em base não decimal malformado.", self.inicio, self.atual); }
            self.adicionar(TipoToken::Inteiro, self.inicio, self.atual);
            return;
        }
        while self.byte(0).is_some_and(|b| b.is_ascii_digit() || b == b'_') { self.atual += 1; }
        if self.byte(0) == Some(b'.') && self.byte(1).is_some_and(|b| b.is_ascii_digit()) {
            tipo = TipoToken::Decimal; self.atual += 1;
            while self.byte(0).is_some_and(|b| b.is_ascii_digit() || b == b'_') { self.atual += 1; }
        }
        if matches!(self.byte(0), Some(b'e' | b'E')) {
            tipo = TipoToken::Decimal; self.atual += 1;
            if matches!(self.byte(0), Some(b'+' | b'-')) { self.atual += 1; }
            let antes = self.atual;
            while self.byte(0).is_some_and(|b| b.is_ascii_digit() || b == b'_') { self.atual += 1; }
            if antes == self.atual { self.erro("N1004", "Expoente numérico sem dígitos.", self.inicio, self.atual); }
        }
        let bruto = &self.texto[self.inicio..self.atual];
        let grupos_validos = bruto
            .split(|c| matches!(c, '.' | 'e' | 'E' | '+' | '-'))
            .filter(|grupo| !grupo.is_empty())
            .all(|grupo| {
                !grupo.starts_with('_')
                    && !grupo.ends_with('_')
                    && !grupo.contains("__")
                    && grupo.chars().all(|c| c.is_ascii_digit() || c == '_')
            });
        if !grupos_validos {
            self.erro("N1005", "Separador numérico '_' em posição inválida.", self.inicio, self.atual);
        }
        self.adicionar(tipo, self.inicio, self.atual);
    }
    fn literal_texto(&mut self) {
        let mut fechado = false;
        while !self.fim() {
            match self.avancar() {
                b'"' => { fechado = true; break; }
                b'\n' | b'\r' => { self.atual -= 1; self.erro("N1006", "Texto não pode atravessar uma quebra de linha.", self.inicio, self.atual + 1); break; }
                b'\\' => { if !self.escape_valido() { self.erro("N1007", "Escape inválido em literal.", self.atual.saturating_sub(2), self.atual); } }
                _ => {}
            }
        }
        if !fechado && !self.diagnosticos.iter().any(|d| d.codigo == "N1006" && d.intervalo.is_some_and(|i| i.inicio == self.inicio)) {
            self.erro("N1008", "Literal de texto não terminado.", self.inicio, self.atual);
        }
        self.adicionar(TipoToken::Texto, self.inicio, self.atual);
    }
    fn literal_caractere(&mut self) {
        let mut unidades = 0usize;
        let mut fechado = false;
        while !self.fim() {
            if self.byte(0) == Some(b'\'') {
                self.atual += 1;
                fechado = true;
                break;
            }
            if matches!(self.byte(0), Some(b'\n' | b'\r')) { break; }
            if self.byte(0) == Some(b'\\') {
                self.atual += 1;
                if !self.escape_valido() {
                    self.erro("N1007", "Escape inválido em literal.", self.atual.saturating_sub(2), self.atual);
                }
            } else if let Some(caractere) = self.caractere_atual() {
                self.atual += caractere.len_utf8();
            } else { break; }
            unidades += 1;
        }
        if !fechado || unidades != 1 {
            self.erro("N1009", "Literal de caractere deve conter exatamente um caractere e ser terminado.", self.inicio, self.atual);
        }
        self.adicionar(TipoToken::Caractere, self.inicio, self.atual);
    }
    fn escape_valido(&mut self) -> bool {
        if self.fim() { return false; }
        match self.avancar() {
            b'\\' | b'"' | b'\'' | b'n' | b'r' | b't' | b'0' => true,
            b'u' if self.corresponde(b'{') => {
                let inicio = self.atual;
                while self.byte(0).is_some_and(|b| b.is_ascii_hexdigit()) { self.atual += 1; }
                let digitos = self.texto.get(inicio..self.atual).unwrap_or("");
                let escalar = u32::from_str_radix(digitos, 16).ok().and_then(char::from_u32);
                self.corresponde(b'}') && escalar.is_some()
            }
            _ => false,
        }
    }
}

/// Tipos de nós da AST, independentes de detalhes de formatação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expressao {
    Nome(String), Literal(String), Lista(Vec<Expressao>),
    Unaria { operador: TipoToken, valor: Box<Expressao> },
    Binaria { esquerda: Box<Expressao>, operador: TipoToken, direita: Box<Expressao> },
    Atribuicao { destino: Box<Expressao>, valor: Box<Expressao> },
    Chamada { alvo: Box<Expressao>, argumentos: Vec<Expressao> },
    Indexacao { alvo: Box<Expressao>, indice: Box<Expressao> },
    Membro { alvo: Box<Expressao>, operador: TipoToken, nome: String },
}
/// Visibilidade declarada para uma função.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibilidade { Publico, Privado }
/// Declarações e instruções representadas na AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoAst {
    Variavel { nome: String, tipo: Option<String>, valor: Expressao, constante: bool },
    Funcao { visibilidade: Option<Visibilidade>, nome: String, parametros: Vec<(String, String)>, retorno: Option<String>, corpo: Vec<NoAst> },
    Tipo { nome: String, destino: String },
    Incluir { caminho: String, apelido: Option<String> },
    Expressao(Expressao), Quando { condicao: Expressao, entao: Vec<NoAst>, senao: Vec<NoAst> },
    Enquanto { condicao: Expressao, corpo: Vec<NoAst> },
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
    let mut programa = parser.programa();
    let mut diagnosticos = lexico.diagnosticos;
    diagnosticos.extend(parser.diagnosticos);
    // Evita descartar o programa parcial: erros léxicos e sintáticos coexistem.
    programa.itens.shrink_to_fit();
    ResultadoSintatico { programa, diagnosticos }
}

struct Parser { tokens: Vec<Token>, atual: usize, diagnosticos: Vec<Diagnostico> }
impl Parser {
    fn novo(tokens: Vec<Token>) -> Self { Self { tokens, atual: 0, diagnosticos: Vec::new() } }
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
            TipoToken::Variavel => self.declaracao_variavel(false),
            TipoToken::Constante => self.declaracao_variavel(true),
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
            TipoToken::Incluir => self.incluir(),
            TipoToken::Quando => self.quando(),
            TipoToken::Enquanto => self.enquanto(),
            TipoToken::Para => self.para(),
            TipoToken::Devolver => { self.avancar(); let e = if self.fim_item() { None } else { self.expressao(0) }; Some(NoAst::Devolver(e)) }
            TipoToken::Parar => { self.avancar(); Some(NoAst::Parar) }
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
        Some(NoAst::Funcao { visibilidade, nome, parametros, retorno, corpo })
    }
    fn tipo_alias(&mut self) -> Option<NoAst> {
        self.avancar(); let nome = self.consumir_nome("Esperado nome após 'tipo'.")?;
        if !self.corresponde(TipoToken::Atribui) { self.erro_atual("N2006", "Esperado '=' na declaração de tipo."); return None; }
        self.pular_novas_linhas();
        Some(NoAst::Tipo { nome, destino: self.caminho_tipo()? })
    }
    fn incluir(&mut self) -> Option<NoAst> {
        self.avancar(); let caminho = self.caminho_tipo()?;
        let apelido = if self.corresponde(TipoToken::Como) { self.consumir_nome("Esperado apelido após 'como'.") } else { None };
        Some(NoAst::Incluir { caminho, apelido })
    }
    fn quando(&mut self) -> Option<NoAst> {
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
    fn enquanto(&mut self) -> Option<NoAst> {
        self.avancar(); let condicao = self.expressao(0)?; let corpo = self.bloco()?;
        Some(NoAst::Enquanto { condicao, corpo })
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
        if self.corresponde(TipoToken::AbreColchete) {
            let interno = self.caminho_tipo()?;
            self.esperar(TipoToken::FechaColchete, "Esperado ']' após tipo de lista.");
            return Some(format!("[{interno}]"));
        }
        let mut nome = self.consumir_nome("Esperado nome de tipo ou módulo.")?;
        while self.corresponde(TipoToken::Escopo) {
            nome.push_str("::");
            nome.push_str(&self.consumir_nome("Esperado nome após '::'.")?);
        }
        Some(nome)
    }
    fn expressao(&mut self, minimo: u8) -> Option<Expressao> {
        let mut comparacao_relacional = false;
        let mut intervalo_visto = false;
        let mut esquerda = match self.tipo() {
            TipoToken::Negacao | TipoToken::Nao | TipoToken::Menos | TipoToken::Mais => {
                let op = self.avancar().tipo; self.pular_novas_linhas(); let valor = self.expressao(9)?;
                Expressao::Unaria { operador: op, valor: Box::new(valor) }
            }
            TipoToken::Identificador => Expressao::Nome(self.avancar().lexema.nfc().collect()),
            TipoToken::Inteiro | TipoToken::Decimal | TipoToken::Texto | TipoToken::Caractere | TipoToken::Verdadeiro | TipoToken::Falso | TipoToken::Nulo => Expressao::Literal(self.avancar().lexema),
            TipoToken::AbrePar => { self.avancar(); let e = self.expressao(0)?; self.esperar(TipoToken::FechaPar, "Esperado ')' após expressão."); e }
            TipoToken::AbreColchete => {
                self.avancar(); let mut valores = Vec::new();
                if !self.verifica(TipoToken::FechaColchete) {
                    loop { valores.push(self.expressao(0)?); if !self.corresponde(TipoToken::Virgula) { break; } }
                }
                self.esperar(TipoToken::FechaColchete, "Esperado ']' após lista."); Expressao::Lista(valores)
            }
            TipoToken::ReservadaFutura => {
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
                self.avancar(); let mut args = Vec::new();
                if !self.verifica(TipoToken::FechaPar) { loop { args.push(self.expressao(0)?); if !self.corresponde(TipoToken::Virgula) { break; } } }
                self.esperar(TipoToken::FechaPar, "Esperado ')' após argumentos.");
                esquerda = Expressao::Chamada { alvo: Box::new(esquerda), argumentos: args }; continue;
            }
            if tipo == TipoToken::AbreColchete && 14 >= minimo {
                self.avancar(); let indice = self.expressao(0)?; self.esperar(TipoToken::FechaColchete, "Esperado ']' após índice.");
                esquerda = Expressao::Indexacao { alvo: Box::new(esquerda), indice: Box::new(indice) }; continue;
            }
            if matches!(tipo, TipoToken::Ponto | TipoToken::Escopo) && 14 >= minimo {
                let operador = self.avancar().tipo;
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
        } else if self.verifica(TipoToken::ReservadaFutura) {
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
        let r = analisar_lexico(&fonte("variavel a = 1\r\nvariavel b = 2"));
        while !self.verifica(TipoToken::Fim) && !self.separador_atual() && !matches!(self.tipo(), TipoToken::FechaChave | TipoToken::FimBloco | TipoToken::Senao) { self.avancar(); }
    }
}

#[cfg(test)]
        let r = analisar_lexico(&fonte("variavel a = 1\rvariavel b = 2"));
    use super::*;
    fn fonte(codigo: &str) -> ArquivoFonte { ArquivoFonte::novo("teste.nbx", codigo) }

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
        let r = analisar(&fonte("variavel soma = 1\\n\\n + 2"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
    #[test]
    fn permite_continuacao_de_expressao_apos_operador() {
        let r = analisar(&fonte("variavel soma = 1 +\n 2"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
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
    fn preserva_visibilidade_de_funcao_na_ast() {
        let r = analisar(&fonte("publico funcao abrir() { devolver }"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert!(matches!(&r.programa.itens[0], NoAst::Funcao { visibilidade: Some(Visibilidade::Publico), .. }));
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
    fn analisa_laco_para_e_inclusao_de_modulo() {
        let r = analisar(&fonte("incluir Matemática como Mat\npara (variavel i = 0; i < 3; i = i + 1) { exibir(i) }"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
        assert_eq!(r.programa.itens.len(), 2);
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
        let r = analisar(&fonte("variavel cada = 1"));
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
    fn reconhece_lista_chamada_indexacao_e_membro() {
        let r = analisar(&fonte("variavel x = sistema::somar([1, 2][0], objeto.nome)"));
        assert!(r.diagnosticos.is_empty(), "{:?}", r.diagnosticos);
    }
}
