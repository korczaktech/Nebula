//! Tipos compartilhados para arquivos e posições de origem da Nebula.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

/// Arquivo-fonte carregado em memória.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArquivoFonte {
    caminho: PathBuf,
    conteudo: String,
}

/// Intervalo de bytes dentro de um arquivo-fonte, com início inclusivo e fim exclusivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntervaloFonte {
    /// Offset inicial em bytes.
    pub inicio: usize,
    /// Offset final em bytes (exclusivo).
    pub fim: usize,
}

impl IntervaloFonte {
    /// Cria um intervalo se os limites forem ordenados.
    #[must_use]
    pub fn novo(inicio: usize, fim: usize) -> Option<Self> {
        (inicio <= fim).then_some(Self { inicio, fim })
    }

    /// Retorna o tamanho em bytes.
    #[must_use]
    pub fn tamanho(self) -> usize {
        self.fim - self.inicio
    }

    /// Indica se o intervalo não contém bytes.
    #[must_use]
    pub fn vazio(self) -> bool {
        self.inicio == self.fim
    }
}

impl ArquivoFonte {
    /// Cria um arquivo-fonte em memória.
    pub fn novo(caminho: impl Into<PathBuf>, conteudo: impl Into<String>) -> Self {
        Self {
            caminho: caminho.into(),
            conteudo: conteudo.into(),
        }
    }

    /// Caminho associado ao arquivo.
    #[must_use]
    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    /// Conteúdo UTF-8 do arquivo.
    #[must_use]
    pub fn conteudo(&self) -> &str {
        &self.conteudo
    }

    /// Retorna o trecho de um intervalo válido em limites UTF-8.
    #[must_use]
    pub fn trecho(&self, intervalo: IntervaloFonte) -> Option<&str> {
        self.conteudo.get(intervalo.inicio..intervalo.fim)
    }

    /// Converte um offset em bytes para linha e coluna (base 1).
    #[must_use]
    pub fn linha_coluna(&self, offset: usize) -> Option<(usize, usize)> {
        if offset > self.conteudo.len() || !self.conteudo.is_char_boundary(offset) {
            return None;
        }

        let prefixo = &self.conteudo[..offset];
        let linha = prefixo.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let inicio_linha = prefixo.rfind('\n').map_or(0, |pos| pos + 1);
        let coluna = self.conteudo[inicio_linha..offset].chars().count() + 1;
        Some((linha, coluna))
    }
}

#[cfg(test)]
mod testes {
    use super::{ArquivoFonte, IntervaloFonte};

    #[test]
    fn calcula_linha_e_coluna_com_unicode() {
        let fonte = ArquivoFonte::novo("teste.nbx", "ação\nNebula");
        let offset = "ação\nNe".len();
        assert_eq!(fonte.linha_coluna(offset), Some((2, 3)));
    }

    #[test]
    fn rejeita_offset_no_meio_de_caractere_utf8() {
        let fonte = ArquivoFonte::novo("teste.nbx", "á");
        assert_eq!(fonte.linha_coluna(1), None);
    }

    #[test]
    fn extrai_intervalo_valido() {
        let fonte = ArquivoFonte::novo("teste.nbx", "Nebula");
        let intervalo = IntervaloFonte::novo(0, 3).expect("intervalo ordenado");
        assert_eq!(fonte.trecho(intervalo), Some("Neb"));
    }

    #[test]
    fn rejeita_intervalo_invertido() {
        assert!(IntervaloFonte::novo(4, 2).is_none());
    }
}
