//! Lexer completo da sintaxe Nebula.
use super::token::{ResultadoLexico, TipoToken, Token};
use nebula_diagnosticos::{Diagnostico, Severidade};
use nebula_fonte::{ArquivoFonte, IntervaloFonte};
use unicode_ident::{is_xid_continue, is_xid_start};
use unicode_normalization::UnicodeNormalization;

#[must_use]
pub fn analisar_lexico(fonte: &ArquivoFonte) -> ResultadoLexico {
    Lexer::novo(fonte.conteudo()).executar()
}

struct Lexer<'a> {
    fonte: &'a str,
    bytes: &'a [u8],
    atual: usize,
    inicio: usize,
    tokens: Vec<Token>,
    diagnosticos: Vec<Diagnostico>,
    indentacoes: Vec<usize>,
    delimitadores: Vec<(u8, usize)>,
    inicio_linha: bool,
}

impl<'a> Lexer<'a> {
    fn novo(fonte: &'a str) -> Self {
        Self {
            fonte,
            bytes: fonte.as_bytes(),
            atual: 0,
            inicio: 0,
            tokens: Vec::new(),
            diagnosticos: Vec::new(),
            indentacoes: vec![0],
            delimitadores: Vec::new(),
            inicio_linha: true,
        }
    }
    fn executar(mut self) -> ResultadoLexico {
        while !self.fim() {
            if self.inicio_linha && !self.suprime_indentacao() {
                self.processar_indentacao();
                if self.fim() {
                    break;
                }
            }
            self.inicio = self.atual;
            self.escanear_token();
        }
        let aberturas = std::mem::take(&mut self.delimitadores);
        for (abertura, posicao) in aberturas {
            let nome = match abertura {
                b'(' => "parêntese",
                b'[' => "colchete",
                _ => "chave",
            };
            self.erro(
                "N1011",
                &format!("Delimitador {nome} sem fechamento."),
                posicao,
                posicao + 1,
            );
        }
        while self.indentacoes.len() > 1 {
            self.indentacoes.pop();
            self.adicionar(TipoToken::Dedent, self.atual, self.atual);
        }
        self.adicionar(TipoToken::Fim, self.atual, self.atual);
        ResultadoLexico {
            tokens: self.tokens,
            diagnosticos: self.diagnosticos,
        }
    }
    fn fim(&self) -> bool {
        self.atual >= self.bytes.len()
    }
    fn byte(&self, n: usize) -> Option<u8> {
        self.bytes.get(self.atual + n).copied()
    }
    fn caractere(&self) -> Option<char> {
        self.fonte.get(self.atual..)?.chars().next()
    }
    fn avancar(&mut self) -> u8 {
        let b = self.bytes[self.atual];
        self.atual += 1;
        b
    }
    fn corresponde(&mut self, b: u8) -> bool {
        if self.byte(0) == Some(b) {
            self.atual += 1;
            true
        } else {
            false
        }
    }
    fn adicionar(&mut self, tipo: TipoToken, inicio: usize, fim: usize) {
        self.tokens.push(Token {
            tipo,
            lexema: self.fonte.get(inicio..fim).unwrap_or("").to_owned(),
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
    fn suprime_indentacao(&self) -> bool {
        !self.delimitadores.is_empty()
    }
    fn suprime_quebras(&self) -> bool {
        self.delimitadores
            .iter()
            .any(|(b, _)| matches!(b, b'(' | b'['))
    }
    fn processar_indentacao(&mut self) {
        let inicio = self.atual;
        let mut coluna = 0usize;
        let mut tab = false;
        while matches!(self.byte(0), Some(b' ' | b'\t')) {
            if self.avancar() == b'\t' {
                tab = true;
                coluna += 4;
            } else {
                coluna += 1;
            }
        }
        if self.fim()
            || matches!(self.byte(0), Some(b'\n' | b'\r'))
            || self.byte(0) == Some(b'#')
            || (self.byte(0) == Some(b'/') && self.byte(1) == Some(b'/'))
        {
            self.inicio_linha = false;
            return;
        }
        if tab {
            self.erro(
                "N1012",
                "Use espaços para indentar; tabulações tornam a indentação ambígua.",
                inicio,
                self.atual,
            );
        }
        let atual = *self.indentacoes.last().unwrap_or(&0);
        match coluna.cmp(&atual) {
            std::cmp::Ordering::Greater => {
                self.indentacoes.push(coluna);
                self.adicionar(TipoToken::Indent, inicio, self.atual);
            }
            std::cmp::Ordering::Less => {
                while self.indentacoes.len() > 1 && coluna < *self.indentacoes.last().unwrap_or(&0)
                {
                    self.indentacoes.pop();
                    self.adicionar(TipoToken::Dedent, inicio, self.atual);
                }
                if coluna != *self.indentacoes.last().unwrap_or(&0) {
                    self.erro(
                        "N1013",
                        "A indentação não corresponde a um nível anterior.",
                        inicio,
                        self.atual,
                    );
                    self.indentacoes.push(coluna);
                    self.adicionar(TipoToken::Indent, inicio, self.atual);
                }
            }
            std::cmp::Ordering::Equal => {}
        }
        self.inicio_linha = false;
    }
    fn escanear_token(&mut self) {
        let b = self.avancar();
        match b {
            b' ' | b'\t' => {}
            b'\n' => self.quebra_linha(self.inicio, self.atual),
            b'\r' => {
                if self.corresponde(b'\n') {
                    self.quebra_linha(self.inicio, self.atual);
                } else {
                    self.erro(
                        "N1001",
                        "Quebra CR isolada; use LF ou CRLF.",
                        self.inicio,
                        self.atual,
                    );
                }
            }
            b'#' => self.comentario_linha(),
            b'/' if self.byte(0) == Some(b'/') && self.byte(1) == Some(b'=') => {
                self.atual += 2;
                self.simples(TipoToken::DivideInteiraAtribui);
            }
            b'/' if self.corresponde(b'/') => self.comentario_linha(),
            b'/' if self.corresponde(b'*') => self.comentario_bloco(),
            b'"' => self.literal_texto(),
            b'\'' => self.literal_texto_simples(),
            b'\x60' => self.literal_interpolado(),
            b'0'..=b'9' => self.numero(),
            b'(' => self.abrir(b'(', TipoToken::AbrePar),
            b'[' => self.abrir(b'[', TipoToken::AbreColchete),
            b'{' => self.abrir(b'{', TipoToken::AbreChave),
            b')' => self.fechar(b'(', TipoToken::FechaPar),
            b']' => self.fechar(b'[', TipoToken::FechaColchete),
            b'}' => self.fechar(b'{', TipoToken::FechaChave),
            b',' => self.simples(TipoToken::Virgula),
            b';' => self.simples(TipoToken::PontoVirgula),
            b':' if self.corresponde(b':') => self.simples(TipoToken::Escopo),
            b':' => self.simples(TipoToken::DoisPontos),
            b'.' if self.corresponde(b'.') && self.corresponde(b'.') => {
                self.simples(TipoToken::Reticencias)
            }
            b'.' if self.fonte.get(self.inicio..self.atual) == Some("..") => {
                self.simples(TipoToken::Intervalo)
            }
            b'.' => self.simples(TipoToken::Ponto),
            b'@' => self.simples(TipoToken::Arroba),
            b'+' => self.operador_composto(
                b'+',
                TipoToken::Incrementa,
                b'=',
                TipoToken::SomaAtribui,
                TipoToken::Mais,
            ),
            b'-' => {
                if self.corresponde(b'>') {
                    self.simples(TipoToken::Seta);
                } else {
                    self.operador_composto(
                        b'-',
                        TipoToken::Decrementa,
                        b'=',
                        TipoToken::SubtraiAtribui,
                        TipoToken::Menos,
                    );
                }
            }
            b'*' => {
                if self.corresponde(b'*') {
                    if self.corresponde(b'=') {
                        self.simples(TipoToken::PotenciaAtribui);
                    } else {
                        self.simples(TipoToken::Potencia);
                    }
                } else if self.corresponde(b'=') {
                    self.simples(TipoToken::MultiplicaAtribui);
                } else {
                    self.simples(TipoToken::Vezes);
                }
            }
            b'/' => {
                if self.corresponde(b'=') {
                    self.simples(TipoToken::DivideAtribui);
                } else {
                    self.simples(TipoToken::Divide);
                }
            }
            b'\\' => {
                if self.corresponde(b'\\') {
                    if self.corresponde(b'=') {
                        self.simples(TipoToken::DivideInteiraAtribui);
                    } else {
                        self.simples(TipoToken::DivideInteira);
                    }
                } else {
                    self.erro(
                        "N1003",
                        "Barra invertida isolada não é um operador.",
                        self.inicio,
                        self.atual,
                    );
                }
            }
            b'%' => {
                if self.corresponde(b'=') {
                    self.simples(TipoToken::RestoAtribui);
                } else {
                    self.simples(TipoToken::Resto);
                }
            }
            b'=' => self.operador_igual(),
            b'!' => {
                if self.corresponde(b'=') {
                    if self.corresponde(b'=') {
                        self.simples(TipoToken::DiferenteEstrito);
                    } else {
                        self.simples(TipoToken::Diferente);
                    }
                } else {
                    self.simples(TipoToken::Negacao);
                }
            }
            b'<' => {
                if self.corresponde(b'=') {
                    self.simples(TipoToken::MenorIgual);
                } else if self.corresponde(b'-') {
                    self.simples(TipoToken::Atribui);
                } else if self.corresponde(b'<') {
                    self.simples(TipoToken::DeslocaEsquerda);
                } else {
                    self.simples(TipoToken::Menor);
                }
            }
            b'>' => {
                if self.corresponde(b'>') {
                    if self.corresponde(b'>') {
                        self.simples(TipoToken::DeslocaDireitaSemSinal);
                    } else {
                        self.simples(TipoToken::DeslocaDireita);
                    }
                } else if self.corresponde(b'=') {
                    self.simples(TipoToken::MaiorIgual);
                } else {
                    self.simples(TipoToken::Maior);
                }
            }
            b'&' => {
                if self.corresponde(b'&') {
                    self.simples(TipoToken::ELogico);
                } else {
                    self.simples(TipoToken::EBit);
                }
            }
            b'|' => {
                if self.corresponde(b'|') {
                    self.simples(TipoToken::OuLogico);
                } else {
                    self.simples(TipoToken::OuBit);
                }
            }
            b'^' => self.simples(TipoToken::XorBit),
            b'~' => self.simples(TipoToken::NaoBit),
            b'?' => self.operador_interrogacao(),
            _ => {
                self.atual -= 1;
                if let Some(c) = self.caractere() {
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
    fn quebra_linha(&mut self, inicio: usize, fim: usize) {
        if !self.suprime_quebras() {
            self.adicionar(TipoToken::NovaLinha, inicio, fim);
        }
        self.inicio_linha = true;
    }
    fn comentario_linha(&mut self) {
        while !self.fim() && !matches!(self.byte(0), Some(b'\n' | b'\r')) {
            if let Some(c) = self.caractere() {
                self.atual += c.len_utf8();
            } else {
                break;
            }
        }
    }
    fn comentario_bloco(&mut self) {
        let mut fechado = false;
        while !self.fim() {
            if self.byte(0) == Some(b'*') && self.byte(1) == Some(b'/') {
                self.atual += 2;
                fechado = true;
                break;
            }
            if self.byte(0) == Some(b'\n') {
                let p = self.atual;
                self.atual += 1;
                self.quebra_linha(p, self.atual);
            } else if self.byte(0) == Some(b'\r') {
                let p = self.atual;
                self.atual += 1;
                if self.corresponde(b'\n') {
                    self.quebra_linha(p, self.atual);
                } else {
                    self.erro("N1001", "Quebra CR isolada; use LF ou CRLF.", p, self.atual);
                }
            } else if let Some(c) = self.caractere() {
                self.atual += c.len_utf8();
            } else {
                break;
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
    fn abrir(&mut self, b: u8, tipo: TipoToken) {
        self.delimitadores.push((b, self.inicio));
        self.simples(tipo);
    }
    fn fechar(&mut self, esperado: u8, tipo: TipoToken) {
        if let Some((aberto, _)) = self.delimitadores.pop() {
            if aberto != esperado {
                self.erro(
                    "N1014",
                    "Delimitadores fechados em ordem incorreta.",
                    self.inicio,
                    self.atual,
                );
            }
        } else {
            self.erro(
                "N1015",
                "Delimitador de fechamento sem abertura correspondente.",
                self.inicio,
                self.atual,
            );
        }
        self.simples(tipo);
    }
    fn operador_composto(
        &mut self,
        segundo: u8,
        tipo_segundo: TipoToken,
        igual: u8,
        tipo_igual: TipoToken,
        simples: TipoToken,
    ) {
        if self.corresponde(segundo) {
            self.simples(tipo_segundo);
        } else if self.corresponde(igual) {
            self.simples(tipo_igual);
        } else {
            self.simples(simples);
        }
    }
    fn operador_igual(&mut self) {
        if self.corresponde(b'=') {
            self.simples(TipoToken::Igual);
        } else if self.corresponde(b'+') {
            self.simples(TipoToken::SomaAtribui);
        } else if self.corresponde(b'-') {
            self.simples(TipoToken::SubtraiAtribui);
        } else if self.corresponde(b'*') {
            if self.corresponde(b'*') {
                self.simples(TipoToken::PotenciaAtribui);
            } else {
                self.simples(TipoToken::MultiplicaAtribui);
            }
        } else if self.corresponde(b'/') {
            self.simples(TipoToken::DivideAtribui);
        } else if self.corresponde(b'\\') {
            if self.corresponde(b'\\') {
                self.simples(TipoToken::DivideInteiraAtribui);
            } else {
                self.erro(
                    "N1003",
                    "Operador de atribuição de divisão inteira incompleto.",
                    self.inicio,
                    self.atual,
                );
            }
        } else if self.corresponde(b'%') {
            self.simples(TipoToken::RestoAtribui);
        } else {
            self.simples(TipoToken::Atribui);
        }
    }
    fn operador_interrogacao(&mut self) {
        if self.corresponde(b'?') {
            if self.corresponde(b'=') {
                self.simples(TipoToken::CoalesceAtribui);
            } else {
                self.simples(TipoToken::Coalesce);
            }
        } else if self.corresponde(b'.') {
            self.simples(TipoToken::AcessoOpcional);
        } else {
            self.simples(TipoToken::Interrogacao);
        }
    }
    fn simples(&mut self, tipo: TipoToken) {
        self.adicionar(tipo, self.inicio, self.atual);
    }

    fn identificador(&mut self) {
        while let Some(c) = self.caractere() {
            if c == '_' || is_xid_continue(c) {
                self.atual += c.len_utf8();
            } else {
                break;
            }
        }
        let palavra = self.fonte[self.inicio..self.atual]
            .nfc()
            .collect::<String>();
        let tipo = match palavra.as_str() {
            "variavel" => TipoToken::Variavel,
            "constante" => TipoToken::Constante,
            "funcao" | "função" => TipoToken::Funcao,
            "classe" => TipoToken::Classe,
            "herda" => TipoToken::Herda,
            "se" => TipoToken::Se,
            "sendo" => TipoToken::Sendo,
            "senao" | "senão" => TipoToken::Senao,
            "para" => TipoToken::Para,
            "dentro" => TipoToken::Dentro,
            "enquanto" => TipoToken::Enquanto,
            "escolha" => TipoToken::Escolha,
            "caso" => TipoToken::Caso,
            "padrao" => TipoToken::Padrao,
            "pare" => TipoToken::Pare,
            "prossiga" => TipoToken::Prossiga,
            "passar" => TipoToken::Passar,
            "retorne" => TipoToken::Retorne,
            "lambda" => TipoToken::Lambda,
            "ceder" => TipoToken::Ceder,
            "incluir" => TipoToken::Incluir,
            "biblioteca" => TipoToken::Biblioteca,
            "modulo" => TipoToken::Modulo,
            "de" => TipoToken::De,
            "como" => TipoToken::Como,
            "tente" => TipoToken::Tente,
            "erro" => TipoToken::Erro,
            "sempre" => TipoToken::Sempre,
            "lancar" => TipoToken::Lancar,
            "verifique" => TipoToken::Verifique,
            "com" => TipoToken::Com,
            "assinc" => TipoToken::Assinc,
            "aguarda" => TipoToken::Aguarda,
            "global" => TipoToken::Global,
            "externa" => TipoToken::Externa,
            "isto" => TipoToken::Isto,
            "novo" => TipoToken::Novo,
            "apagar" => TipoToken::Apagar,
            "identico" => TipoToken::Identico,
            "nao" | "não" => TipoToken::Nao,
            "fora" => TipoToken::Fora,
            "e" => TipoToken::E,
            "ou" => TipoToken::Ou,
            "sim" => TipoToken::Sim,
            "verdadeiro" => TipoToken::Verdadeiro,
            "falso" => TipoToken::Falso,
            "nulo" => TipoToken::Nulo,
            "iguais" => TipoToken::Iguais,
            "tipoDe" => TipoToken::TipoDe,
            "instanciaDe" => TipoToken::InstanciaDe,
            "definir" => TipoToken::Definir,
            "exportar" => TipoToken::Exportar,
            "construtor" => TipoToken::Construtor,
            "estatico" => TipoToken::Estatico,
            "privado" => TipoToken::Privado,
            "publico" | "público" => TipoToken::Publico,
            "protegido" => TipoToken::Protegido,
            "critico" => TipoToken::Critico,
            "privilegio" => TipoToken::Privilegio,
            "montagem" => TipoToken::Montagem,
            "tipo" => TipoToken::Tipo,
            "estrutura" => TipoToken::Estrutura,
            "enumeracao" | "enumeração" => TipoToken::Enumeracao,
            "mutavel" | "mutável" => TipoToken::Mutavel,
            "alterar" => TipoToken::Alterar,
            "quando" => TipoToken::Quando,
            "avaliar" => TipoToken::Avaliar,
            "cada" => TipoToken::Cada,
            "em" => TipoToken::Em,
            "pulso" => TipoToken::Pulso,
            "durante" => TipoToken::Durante,
            "repetir" => TipoToken::Repetir,
            "devolver" => TipoToken::Devolver,
            "fim" => TipoToken::FimBloco,
            "parar" => TipoToken::Parar,
            "encerrar" => TipoToken::Encerrar,
            "seguir" => TipoToken::Seguir,
            "tentar" | "capturar" | "finalmente" => TipoToken::ReservadaFutura,
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
            let inicio = self.atual;
            while self
                .byte(0)
                .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
            {
                self.atual += 1;
            }
            let trecho = &self.fonte[inicio..self.atual];
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
        let bruto = &self.fonte[self.inicio..self.atual];
        let valido = bruto
            .split(['.', 'e', 'E', '+', '-'])
            .filter(|p| !p.is_empty())
            .all(|p| {
                !p.starts_with('_')
                    && !p.ends_with('_')
                    && !p.contains("__")
                    && p.chars().all(|c| c.is_ascii_digit() || c == '_')
            });
        if !valido {
            self.erro(
                "N1005",
                "Separador numérico em posição inválida.",
                self.inicio,
                self.atual,
            );
        }
        self.adicionar(tipo, self.inicio, self.atual);
    }

    fn literal_texto(&mut self) {
        let triplo = self.byte(0) == Some(b'"') && self.byte(1) == Some(b'"');
        if triplo {
            self.atual += 2;
        }
        let mut fechado = false;
        while !self.fim() {
            if triplo
                && self.byte(0) == Some(b'"')
                && self.byte(1) == Some(b'"')
                && self.byte(2) == Some(b'"')
            {
                self.atual += 3;
                fechado = true;
                break;
            }
            if !triplo && self.byte(0) == Some(b'"') {
                self.atual += 1;
                fechado = true;
                break;
            }
            if !triplo && matches!(self.byte(0), Some(b'\n' | b'\r')) {
                self.erro(
                    "N1006",
                    "Texto simples não pode atravessar uma quebra física.",
                    self.inicio,
                    self.atual,
                );
                break;
            }
            if self.byte(0) == Some(b'\\') {
                let p = self.atual;
                self.atual += 1;
                if !self.escape_valido() {
                    self.erro("N1007", "Escape inválido em literal.", p, self.atual);
                }
            } else if let Some(c) = self.caractere() {
                self.atual += c.len_utf8();
            } else {
                break;
            }
        }
        if !fechado {
            self.erro(
                "N1008",
                "Literal de texto não terminado.",
                self.inicio,
                self.atual,
            );
        }
        self.adicionar(TipoToken::Texto, self.inicio, self.atual);
    }
    fn literal_texto_simples(&mut self) {
        let mut fechado = false;
        while !self.fim() {
            if self.byte(0) == Some(b'\'') {
                self.atual += 1;
                fechado = true;
                break;
            }
            if matches!(self.byte(0), Some(b'\n' | b'\r')) {
                self.erro(
                    "N1006",
                    "Texto simples não pode atravessar uma quebra física.",
                    self.inicio,
                    self.atual,
                );
                break;
            }
            if self.byte(0) == Some(b'\\') {
                let posicao = self.atual;
                self.atual += 1;
                if !self.escape_valido() {
                    self.erro("N1007", "Escape inválido em literal.", posicao, self.atual);
                }
            } else if let Some(caractere) = self.caractere() {
                self.atual += caractere.len_utf8();
            } else {
                break;
            }
        }
        if !fechado {
            self.erro(
                "N1008",
                "Literal de texto não terminado.",
                self.inicio,
                self.atual,
            );
        }
        self.adicionar(TipoToken::Texto, self.inicio, self.atual);
    }

    fn literal_interpolado(&mut self) {
        let mut fechado = false;
        while !self.fim() {
            if self.byte(0) == Some(b'\x60') {
                self.atual += 1;
                fechado = true;
                break;
            }
            if matches!(self.byte(0), Some(b'\n' | b'\r')) {
                break;
            }
            if self.byte(0) == Some(b'\\') {
                let p = self.atual;
                self.atual += 1;
                if !self.escape_valido() {
                    self.erro(
                        "N1007",
                        "Escape inválido em texto interpolado.",
                        p,
                        self.atual,
                    );
                }
            } else if let Some(c) = self.caractere() {
                self.atual += c.len_utf8();
            } else {
                break;
            }
        }
        if !fechado {
            self.erro(
                "N1008",
                "Texto interpolado não terminado.",
                self.inicio,
                self.atual,
            );
        }
        self.adicionar(TipoToken::TextoInterpolado, self.inicio, self.atual);
    }
    fn escape_valido(&mut self) -> bool {
        if self.fim() || matches!(self.byte(0), Some(b'\n' | b'\r')) {
            return false;
        }
        match self.avancar() {
            b'\\' | b'"' | b'\'' | b'n' | b'r' | b't' | b'0' | b'\x60' | b'$' => true,
            b'u' if self.corresponde(b'{') => {
                let inicio = self.atual;
                while self.byte(0).is_some_and(|b| b.is_ascii_hexdigit()) {
                    self.atual += 1;
                }
                let digitos = self.fonte.get(inicio..self.atual).unwrap_or("");
                let escalar = u32::from_str_radix(digitos, 16)
                    .ok()
                    .and_then(char::from_u32);
                self.corresponde(b'}') && escalar.is_some() && !digitos.is_empty()
            }
            _ => false,
        }
    }
}
