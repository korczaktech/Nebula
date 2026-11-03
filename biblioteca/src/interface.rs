//! Primitivas de interface textual independentes de terminal específico.

/// Retângulo em coordenadas de células da tela.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetanguloTela {
    /// Coluna inicial, começando em zero.
    pub x: usize,
    /// Linha inicial, começando em zero.
    pub y: usize,
    /// Quantidade de colunas.
    pub largura: usize,
    /// Quantidade de linhas.
    pub altura: usize,
}

/// Buffer de caracteres que pode ser renderizado em terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TelaTexto {
    largura: usize,
    altura: usize,
    celulas: Vec<char>,
}

impl TelaTexto {
    /// Cria uma tela com limite de um milhão de células.
    pub fn nova(largura: usize, altura: usize) -> Result<Self, String> {
        let total = largura
            .checked_mul(altura)
            .ok_or("Dimensões da tela excedem o limite.")?;
        if largura == 0 || altura == 0 || total > 1_000_000 {
            return Err(
                "A tela deve ter dimensões positivas e até um milhão de células.".to_owned(),
            );
        }
        let mut celulas = Vec::new();
        celulas
            .try_reserve_exact(total)
            .map_err(|_| "Memória insuficiente para criar a tela.")?;
        celulas.resize(total, ' ');
        Ok(Self {
            largura,
            altura,
            celulas,
        })
    }

    /// Retorna largura e altura em células.
    #[must_use]
    pub fn dimensoes(&self) -> (usize, usize) {
        (self.largura, self.altura)
    }

    /// Limpa todas as células com o caractere informado.
    pub fn limpar(&mut self, caractere: char) {
        let caractere = if caractere.is_control() {
            ' '
        } else {
            caractere
        };
        self.celulas.fill(caractere);
    }

    /// Desenha texto com quebra de linha e recorte automático nas bordas.
    /// Caracteres de controle são substituídos por espaços.
    pub fn desenhar_texto(&mut self, x: usize, y: usize, texto: &str) -> usize {
        let mut coluna = x;
        let mut linha = y;
        let mut desenhados = 0;
        for caractere in texto.chars() {
            if caractere == '\n' {
                coluna = x;
                linha = linha.saturating_add(1);
                continue;
            }
            if linha >= self.altura {
                break;
            }
            if coluna < self.largura {
                let indice = linha * self.largura + coluna;
                self.celulas[indice] = if caractere.is_control() {
                    ' '
                } else {
                    caractere
                };
                desenhados += 1;
            }
            coluna = coluna.saturating_add(1);
        }
        desenhados
    }

    /// Desenha a borda de um retângulo usando um único caractere.
    pub fn desenhar_caixa(&mut self, retangulo: RetanguloTela, borda: char) {
        if retangulo.largura == 0 || retangulo.altura == 0 || borda.is_control() {
            return;
        }
        let largura = self.largura;
        let altura = self.altura;
        if retangulo.x >= largura || retangulo.y >= altura {
            return;
        }
        let ultimo_x = retangulo.x.saturating_add(retangulo.largura - 1).min(largura - 1);
        let ultimo_y = retangulo.y.saturating_add(retangulo.altura - 1).min(altura - 1);
        for x in retangulo.x..=ultimo_x {
            self.definir(x, retangulo.y, borda);
            if retangulo.altura > 1 {
                self.definir(x, ultimo_y, borda);
            }
        }
        for y in retangulo.y..=ultimo_y {
            self.definir(retangulo.x, y, borda);
            if retangulo.largura > 1 {
                self.definir(ultimo_x, y, borda);
            }
        }
    }

    /// Lê a célula nas coordenadas informadas.
    #[must_use]
    pub fn obter(&self, x: usize, y: usize) -> Option<char> {
        if x >= self.largura || y >= self.altura {
            return None;
        }
        self.celulas.get(y * self.largura + x).copied()
    }

    /// Converte o buffer em linhas de texto, sem sequências ANSI.
    #[must_use]
    pub fn renderizar(&self) -> String {
        let mut saida = String::with_capacity(self.celulas.len() + self.altura.saturating_sub(1));
        for linha in 0..self.altura {
            if linha > 0 {
                saida.push('\n');
            }
            let inicio = linha * self.largura;
            for caractere in &self.celulas[inicio..inicio + self.largura] {
                saida.push(*caractere);
            }
        }
        saida
    }

    /// Renderiza a tela usando comandos ANSI para limpar e posicionar o cursor.
    #[must_use]
    pub fn renderizar_ansi(&self) -> String {
        format!("\x1b[2J\x1b[H{}", self.renderizar())
    }

    fn definir(&mut self, x: usize, y: usize, caractere: char) {
        if x < self.largura && y < self.altura {
            self.celulas[y * self.largura + x] = caractere;
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn desenha_texto_e_aplica_recorte() {
        let mut tela = TelaTexto::nova(5, 2).unwrap();
        assert_eq!(tela.desenhar_texto(3, 0, "abc\nx"), 3);
        assert_eq!(tela.obter(3, 0), Some('a'));
        assert_eq!(tela.obter(4, 0), Some('b'));
        assert_eq!(tela.obter(3, 1), Some('x'));
    }

    #[test]
    fn desenha_borda_sem_escrever_fora_da_tela() {
        let mut tela = TelaTexto::nova(4, 3).unwrap();
        tela.desenhar_caixa(
            RetanguloTela {
                x: 2,
                y: 1,
                largura: 8,
                altura: 4,
            },
            '#',
        );
        assert_eq!(tela.obter(2, 1), Some('#'));
        assert_eq!(tela.obter(3, 2), Some('#'));
        assert_eq!(tela.obter(0, 0), Some(' '));
    }

    #[test]
    fn rejeita_dimensoes_excessivas() {
        assert!(TelaTexto::nova(usize::MAX, 2).is_err());
        assert!(TelaTexto::nova(0, 1).is_err());
    }
}
