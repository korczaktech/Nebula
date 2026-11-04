//! Varredura do código-fonte e reconhecimento dos tokens Nebula.

use nebula_diagnosticos::{Diagnostico, Severidade};
use nebula_fonte::{ArquivoFonte, IntervaloFonte};
use unicode_ident::{is_xid_continue, is_xid_start};
use unicode_normalization::UnicodeNormalization;

use super::token::{ResultadoLexico, TipoToken, Token};

/// Analisa lexicalmente uma fonte Nebula.
///
/// A varredura preserva os lexemas originais e registra intervalos em bytes
/// com início inclusivo e fim exclusivo. Erros léxicos são acumulados para
/// permitir que a análise continue sempre que houver recuperação segura.
#[must_use]
pub fn analisar_lexico(fonte: &ArquivoFonte) -> ResultadoLexico {
    Lexer::novo(fonte.conteudo()).executar()
}

struct Lexer<'a> {
    texto: &'a str,
    bytes: &'a [u8],
    inicio: usize,
    atual: usize,
    delimitadores: Vec<u8>,
    tokens: Vec<Token>,
    diagnosticos: Vec<Diagnostico>,
}

impl<'a> Lexer<'a> {
    fn novo(texto: &'a str) -> Self {
        Self {
            texto,
            bytes: texto.as_bytes(),
            inicio: 0,
            atual: 0,
            delimitadores: Vec::new(),
            tokens: Vec::new(),
            diagnosticos: Vec::new(),
        }
    }

    fn executar(mut self) -> ResultadoLexico {
        while !self.fim() {
            self.inicio = self.atual;
            self.escanear();
        }
        self.adicionar(TipoToken::Fim, self.atual, self.atual);
        ResultadoLexico {
            tokens: self.tokens,
            diagnosticos: self.diagnosticos,
        }
    }

    fn continuacao_ativa(&self) -> bool {
        !self.delimitadores.is_empty()
    }

    fn fim(&self) -> bool {
        self.atual >= self.bytes.len()
    }
    fn byte(&self, deslocamento: usize) -> Option<u8> {
        self.bytes.get(self.atual + deslocamento).copied()
    }
    fn avancar(&mut self) -> u8 {
        let b = self.bytes[self.atual];
        self.atual += 1;
        b
    }
    fn corresponde(&mut self, esperado: u8) -> bool {
        if self.byte(0) == Some(esperado) {
            self.atual += 1;
            true
        } else {
            false
        }
    }
    fn adicionar(&mut self, tipo: TipoToken, inicio: usize, fim: usize) {
        self.tokens.push(Token {
            tipo,
            lexema: self.texto.get(inicio..fim).unwrap_or("").to_owned(),
            intervalo: IntervaloFonte { inicio, fim },
        });
    }
    fn erro(&mut self, codigo: &'static str, mensagem: &str, inicio: usize, fim: usize) {
        self.diagnosticos.push(Diagnostico::novo(
            codigo,
            Severidade::Erro,
            mensagem,
            Some(IntervaloFonte { inicio, fim }),
        ));
    }
    fn caractere_atual(&self) -> Option<char> {
        self.texto.get(self.atual..)?.chars().next()
    }
    fn escanear(&mut self) {
        let b = self.avancar();
        match b {
            b' ' | b'\t' => {}
            b'\n' => {
                if !self.continuacao_ativa() {
                    self.adicionar(TipoToken::NovaLinha, self.inicio, self.atual);
                }
            }
            b'\r' => {
                if !self.corresponde(b'\n') {
                    self.erro(
                        "N1001",
                        "Quebra CR isolada; use LF ou CRLF.",
                        self.inicio,
                        self.atual,
                    );
                } else if !self.continuacao_ativa() {
                    self.adicionar(TipoToken::NovaLinha, self.inicio, self.atual);
                }
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
                    if self.byte(0) == Some(b'\n') && !self.continuacao_ativa() {
                        let pos = self.atual;
                        self.atual += 1;
                        self.adicionar(TipoToken::NovaLinha, pos, self.atual);
                    } else if self.byte(0) == Some(b'\r')
                        && self.byte(1) == Some(b'\n')
                        && !self.continuacao_ativa()
                    {
                        let pos = self.atual;
                        self.atual += 2;
                        self.adicionar(TipoToken::NovaLinha, pos, self.atual);
                    } else if self.byte(0) == Some(b'\r') && self.byte(1) != Some(b'\n') {
                        let pos = self.atual;
                        self.atual += 1;
                        self.erro(
                            "N1001",
                            "Quebra CR isolada; use LF ou CRLF.",
                            pos,
                            self.atual,
                        );
                    } else {
                        self.atual += 1;
                    }
                }
                if !fechado {
                    self.erro(
                        "N1002",
                        "Comentário de bloco não terminado.",
                        self.inicio,
                        self.atual,
                    );
                }
            }
            b'"' => self.literal_texto(),
            b'\'' => self.literal_caractere(),
            b'0'..=b'9' => self.numero(),
            b'(' => {
                self.delimitadores.push(b'(');
                self.simples(TipoToken::AbrePar);
            }
            b')' => {
                if self.delimitadores.last() == Some(&b'(') {
                    self.delimitadores.pop();
                }
                self.simples(TipoToken::FechaPar);
            }
            b'{' => self.simples(TipoToken::AbreChave),
            b'}' => self.simples(TipoToken::FechaChave),
            b'[' => {
                self.delimitadores.push(b'[');
                self.simples(TipoToken::AbreColchete);
            }
            b']' => {
                if self.delimitadores.last() == Some(&b'[') {
                    self.delimitadores.pop();
                }
                self.simples(TipoToken::FechaColchete);
            }
            b',' => self.simples(TipoToken::Virgula),
            b';' => self.simples(TipoToken::PontoVirgula),
            b':' if self.corresponde(b':') => self.simples(TipoToken::Escopo),
            b':' => self.simples(TipoToken::DoisPontos),
            b'.' if self.corresponde(b'.') => self.simples(TipoToken::Intervalo),
            b'.' => self.simples(TipoToken::Ponto),
            b'+' => self.simples(TipoToken::Mais),
            b'-' => self.simples(TipoToken::Menos),
            b'*' if self.corresponde(b'*') => self.simples(TipoToken::Potencia),
            b'*' => self.simples(TipoToken::Vezes),
            b'%' => self.simples(TipoToken::Resto),
            b'/' => self.simples(TipoToken::Divide),
            b'=' if self.corresponde(b'=') => self.simples(TipoToken::Igual),
            b'=' => self.simples(TipoToken::Atribui),
            b'!' if self.corresponde(b'=') => self.simples(TipoToken::Diferente),
            b'!' => self.simples(TipoToken::Negacao),
            b'<' if self.corresponde(b'=') => self.simples(TipoToken::MenorIgual),
            b'<' if self.corresponde(b'-') => self.simples(TipoToken::Atribui),
            b'<' => self.simples(TipoToken::Menor),
            b'>' if self.corresponde(b'=') => self.simples(TipoToken::MaiorIgual),
            b'>' => self.simples(TipoToken::Maior),
            _ => {
                self.atual -= 1;
                if let Some(c) = self.caractere_atual() {
                    if c == '_' || is_xid_start(c) {
                        self.identificador();
                    } else {
                        self.atual += c.len_utf8();
                        self.erro(
                            "N1003",
                            &format!("Caractere inesperado: {c}."),
                            self.inicio,
                            self.atual,
                        );
                    }
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
                self.erro(
                    "N1001",
                    "Quebra CR isolada; use LF ou CRLF.",
                    pos,
                    self.atual,
                );
            } else {
                self.atual += 1;
            }
        }
    }
    fn simples(&mut self, tipo: TipoToken) {
        self.adicionar(tipo, self.inicio, self.atual);
    }
    fn identificador(&mut self) {
        while let Some(c) = self.caractere_atual() {
            if c == '_' || is_xid_continue(c) {
                self.atual += c.len_utf8();
            } else {
                break;
            }
        }
        let palavra_original = &self.texto[self.inicio..self.atual];
        let palavra = palavra_original.nfc().collect::<String>();
        let tipo = match palavra.as_str() {
            "variavel" => TipoToken::Variavel,
            "definir" => TipoToken::Definir,
            "mutavel" | "mutável" => TipoToken::Mutavel,
            "alterar" => TipoToken::Alterar,
            "constante" => TipoToken::Constante,
            "funcao" | "função" => TipoToken::Funcao,
            "quando" => TipoToken::Quando,
            "escolha" => TipoToken::Escolha,
            "avaliar" => TipoToken::Avaliar,
            "caso" => TipoToken::Caso,
            "senao" | "senão" => TipoToken::Senao,
            "para" => TipoToken::Para,
            "cada" => TipoToken::Cada,
            "em" => TipoToken::Em,
            "pulso" => TipoToken::Pulso,
            "enquanto" => TipoToken::Enquanto,
            "durante" => TipoToken::Durante,
            "repetir" => TipoToken::Repetir,
            "devolver" => TipoToken::Devolver,
            "incluir" => TipoToken::Incluir,
            "como" => TipoToken::Como,
            "verdadeiro" => TipoToken::Verdadeiro,
            "falso" => TipoToken::Falso,
            "nulo" => TipoToken::Nulo,
            "fim" => TipoToken::FimBloco,
            "parar" => TipoToken::Parar,
            "encerrar" => TipoToken::Encerrar,
            "seguir" => TipoToken::Seguir,
            "e" => TipoToken::E,
            "ou" => TipoToken::Ou,
            "nao" | "não" => TipoToken::Nao,
            "publico" | "público" => TipoToken::Publico,
            "privado" => TipoToken::Privado,
            "tipo" => TipoToken::Tipo,
            "estrutura" => TipoToken::Estrutura,
            "enumeracao" | "enumeração" => TipoToken::Enumeracao,
            "tentar" | "capturar" | "finalmente" | "assinc" => TipoToken::ReservadaFutura,
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
                if b == b'_' || digito_valido || b.is_ascii_alphanumeric() {
                    self.atual += 1;
                } else {
                    break;
                }
            }
            let trecho = &self.texto[inicio_digitos..self.atual];
            let valido = !trecho.is_empty()
                && !trecho.starts_with('_')
                && !trecho.ends_with('_')
                && !trecho.contains("__")
                && trecho
                    .chars()
                    .filter(|c| *c != '_')
                    .all(|c| c.is_digit(radix));
            if !valido {
                self.erro(
                    "N1005",
                    "Literal inteiro em base não decimal malformado.",
                    self.inicio,
                    self.atual,
                );
            }
            self.adicionar(TipoToken::Inteiro, self.inicio, self.atual);
            return;
        }
        while self
            .byte(0)
            .is_some_and(|b| b.is_ascii_digit() || b == b'_')
        {
            self.atual += 1;
        }
        if self.byte(0) == Some(b'.') && self.byte(1).is_some_and(|b| b.is_ascii_digit()) {
            tipo = TipoToken::Decimal;
            self.atual += 1;
            while self
                .byte(0)
                .is_some_and(|b| b.is_ascii_digit() || b == b'_')
            {
                self.atual += 1;
            }
        }
        if matches!(self.byte(0), Some(b'e' | b'E')) {
            tipo = TipoToken::Decimal;
            self.atual += 1;
            if matches!(self.byte(0), Some(b'+' | b'-')) {
                self.atual += 1;
            }
            let antes = self.atual;
            while self
                .byte(0)
                .is_some_and(|b| b.is_ascii_digit() || b == b'_')
            {
                self.atual += 1;
            }
            if antes == self.atual {
                self.erro(
                    "N1004",
                    "Expoente numérico sem dígitos.",
                    self.inicio,
                    self.atual,
                );
            }
        }
        let bruto = &self.texto[self.inicio..self.atual];
        let grupos_validos = bruto
            .split(['.', 'e', 'E', '+', '-'])
            .filter(|grupo| !grupo.is_empty())
            .all(|grupo| {
                !grupo.starts_with('_')
                    && !grupo.ends_with('_')
                    && !grupo.contains("__")
                    && grupo.chars().all(|c| c.is_ascii_digit() || c == '_')
            });
        if !grupos_validos {
            self.erro(
                "N1005",
                "Separador numérico '_' em posição inválida.",
                self.inicio,
                self.atual,
            );
        }
        self.adicionar(tipo, self.inicio, self.atual);
    }
    fn literal_texto(&mut self) {
        let mut fechado = false;
        let mut interrompido_por_quebra = false;
        while !self.fim() {
            match self.avancar() {
                b'"' => {
                    fechado = true;
                    break;
                }
                b'\n' | b'\r' => {
                    self.atual -= 1;
                    self.erro(
                        "N1006",
                        "Texto não pode atravessar uma quebra de linha.",
                        self.inicio,
                        self.atual + 1,
                    );
                    interrompido_por_quebra = true;
                    break;
                }
                b'\\' => {
                    if !self.escape_valido() {
                        self.erro(
                            "N1007",
                            "Escape inválido em literal.",
                            self.atual.saturating_sub(2),
                            self.atual,
                        );
                    }
                }
                _ => {}
            }
        }
        if !fechado && !interrompido_por_quebra {
            self.erro(
                "N1008",
                "Literal de texto não terminado.",
                self.inicio,
                self.atual,
            );
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
            if matches!(self.byte(0), Some(b'\n' | b'\r')) {
                break;
            }
            if self.byte(0) == Some(b'\\') {
                self.atual += 1;
                if !self.escape_valido() {
                    self.erro(
                        "N1007",
                        "Escape inválido em literal.",
                        self.atual.saturating_sub(2),
                        self.atual,
                    );
                }
            } else if let Some(caractere) = self.caractere_atual() {
                self.atual += caractere.len_utf8();
            } else {
                break;
            }
            unidades += 1;
        }
        if !fechado || unidades != 1 {
            self.erro(
                "N1009",
                "Literal de caractere deve conter exatamente um caractere e ser terminado.",
                self.inicio,
                self.atual,
            );
        }
        self.adicionar(TipoToken::Caractere, self.inicio, self.atual);
    }
    fn escape_valido(&mut self) -> bool {
        if self.fim() || matches!(self.byte(0), Some(b'\n' | b'\r')) {
            return false;
        }
        match self.avancar() {
            b'\\' | b'"' | b'\'' | b'n' | b'r' | b't' | b'0' => true,
            b'u' if self.corresponde(b'{') => {
                let inicio = self.atual;
                while self.byte(0).is_some_and(|b| b.is_ascii_hexdigit()) {
                    self.atual += 1;
                }
                let digitos = self.texto.get(inicio..self.atual).unwrap_or("");
                let escalar = u32::from_str_radix(digitos, 16)
                    .ok()
                    .and_then(char::from_u32);
                self.corresponde(b'}') && escalar.is_some()
            }
            _ => false,
        }
    }
}
