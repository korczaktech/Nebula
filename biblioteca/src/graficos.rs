//! Rasterização RGBA 2D em memória, sem dependência de GPU ou janela gráfica.

/// Cor RGBA de oito bits por canal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CorRgba {
    /// Canal vermelho.
    pub vermelho: u8,
    /// Canal verde.
    pub verde: u8,
    /// Canal azul.
    pub azul: u8,
    /// Canal alfa.
    pub alfa: u8,
}

impl CorRgba {
    /// Cria uma cor opaca a partir dos canais RGB.
    #[must_use]
    pub const fn opaca(vermelho: u8, verde: u8, azul: u8) -> Self {
        Self {
            vermelho,
            verde,
            azul,
            alfa: 255,
        }
    }

    /// Cria uma cor com todos os quatro canais.
    #[must_use]
    pub const fn rgba(vermelho: u8, verde: u8, azul: u8, alfa: u8) -> Self {
        Self {
            vermelho,
            verde,
            azul,
            alfa,
        }
    }
}

/// Ponto inteiro no plano da imagem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ponto2D {
    /// Coordenada horizontal.
    pub x: i32,
    /// Coordenada vertical.
    pub y: i32,
}

/// Retângulo inteiro usado em operações de desenho.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Retangulo2D {
    /// Coordenada horizontal inicial.
    pub x: i32,
    /// Coordenada vertical inicial.
    pub y: i32,
    /// Largura positiva em pixels.
    pub largura: u32,
    /// Altura positiva em pixels.
    pub altura: u32,
}

/// Imagem RGBA em memória com limite explícito de pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImagemRgba {
    largura: u32,
    altura: u32,
    pixels: Vec<CorRgba>,
}

impl ImagemRgba {
    /// Cria uma imagem limitada pelo máximo de pixels definido pelo chamador.
    pub fn nova(largura: u32, altura: u32, limite_pixels: usize) -> Result<Self, String> {
        let total = usize::try_from(largura)
            .ok()
            .and_then(|w| usize::try_from(altura).ok().and_then(|h| w.checked_mul(h)))
            .ok_or("Dimensões da imagem excedem o limite de memória.")?;
        if largura == 0 || altura == 0 || total > limite_pixels {
            return Err("Dimensões inválidas ou limite de pixels excedido.".to_owned());
        }
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(total)
            .map_err(|_| "Memória insuficiente para a imagem.")?;
        pixels.resize(total, CorRgba::default());
        Ok(Self {
            largura,
            altura,
            pixels,
        })
    }

    /// Retorna largura e altura em pixels.
    #[must_use]
    pub fn dimensoes(&self) -> (u32, u32) {
        (self.largura, self.altura)
    }

    /// Preenche todos os pixels com uma cor.
    pub fn preencher(&mut self, cor: CorRgba) {
        self.pixels.fill(cor);
    }

    /// Lê um pixel; retorna None quando as coordenadas estão fora da imagem.
    #[must_use]
    pub fn obter_pixel(&self, x: u32, y: u32) -> Option<CorRgba> {
        if x >= self.largura || y >= self.altura {
            return None;
        }
        let indice = usize::try_from(y).ok()? * usize::try_from(self.largura).ok()?
            + usize::try_from(x).ok()?;
        self.pixels.get(indice).copied()
    }

    /// Define um pixel e informa se as coordenadas eram válidas.
    pub fn definir_pixel(&mut self, x: u32, y: u32, cor: CorRgba) -> bool {
        if x >= self.largura || y >= self.altura {
            return false;
        }
        let indice = y as usize * self.largura as usize + x as usize;
        self.pixels[indice] = cor;
        true
    }

    /// Preenche um retângulo, recortando automaticamente a área à imagem.
    pub fn preencher_retangulo(&mut self, retangulo: Retangulo2D, cor: CorRgba) {
        if retangulo.largura == 0 || retangulo.altura == 0 {
            return;
        }
        let x_inicio = i64::from(retangulo.x).max(0).min(i64::from(self.largura));
        let y_inicio = i64::from(retangulo.y).max(0).min(i64::from(self.altura));
        let x_fim = (i64::from(retangulo.x) + i64::from(retangulo.largura))
            .max(0)
            .min(i64::from(self.largura));
        let y_fim = (i64::from(retangulo.y) + i64::from(retangulo.altura))
            .max(0)
            .min(i64::from(self.altura));
        for y in y_inicio..y_fim {
            let linha = y as usize * self.largura as usize;
            for x in x_inicio..x_fim {
                self.pixels[linha + x as usize] = cor;
            }
        }
    }

    /// Desenha uma linha por Bresenham; rejeita segmentos acima de um milhão de passos.
    pub fn desenhar_linha(
        &mut self,
        inicio: Ponto2D,
        fim: Ponto2D,
        cor: CorRgba,
    ) -> Result<(), String> {
        let mut x = i64::from(inicio.x);
        let mut y = i64::from(inicio.y);
        let x_fim = i64::from(fim.x);
        let y_fim = i64::from(fim.y);
        let dx = (x_fim - x).abs();
        let dy = (y_fim - y).abs();
        if dx.max(dy) > 1_000_000 {
            return Err("Segmento excede o limite de passos da rasterização.".to_owned());
        }
        let sx = if x < x_fim { 1 } else { -1 };
        let sy = if y < y_fim { 1 } else { -1 };
        let mut erro = dx - dy;
        loop {
            if x >= 0 && y >= 0 && x < i64::from(self.largura) && y < i64::from(self.altura) {
                let indice = y as usize * self.largura as usize + x as usize;
                self.pixels[indice] = cor;
            }
            if x == x_fim && y == y_fim {
                break;
            }
            let dobro = erro * 2;
            if dobro > -dy {
                erro -= dy;
                x += sx;
            }
            if dobro < dx {
                erro += dx;
                y += sy;
            }
        }
        Ok(())
    }

    /// Exporta os pixels em ordem RGBA, linha a linha.
    #[must_use]
    pub fn bytes_rgba(&self) -> Result<Vec<u8>, String> {
        let capacidade = self
            .pixels
            .len()
            .checked_mul(4)
            .ok_or("Tamanho de buffer RGBA excede o limite.")?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(capacidade)
            .map_err(|_| "Memória insuficiente para exportar a imagem.")?;
        for pixel in &self.pixels {
            bytes.extend_from_slice(&[pixel.vermelho, pixel.verde, pixel.azul, pixel.alfa]);
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn preenche_retangulo_com_recorte() {
        let mut imagem = ImagemRgba::nova(3, 2, 6).unwrap();
        let vermelho = CorRgba::opaca(255, 0, 0);
        imagem.preencher_retangulo(
            Retangulo2D {
                x: -1,
                y: 0,
                largura: 3,
                altura: 2,
            },
            vermelho,
        );
        assert_eq!(imagem.obter_pixel(0, 0), Some(vermelho));
        assert_eq!(imagem.obter_pixel(1, 1), Some(vermelho));
        assert_eq!(imagem.obter_pixel(2, 1), Some(CorRgba::default()));
    }

    #[test]
    fn rasteriza_linha_diagonal() {
        let mut imagem = ImagemRgba::nova(3, 3, 9).unwrap();
        let branco = CorRgba::opaca(255, 255, 255);
        imagem
            .desenhar_linha(Ponto2D { x: 0, y: 0 }, Ponto2D { x: 2, y: 2 }, branco)
            .unwrap();
        assert_eq!(imagem.obter_pixel(0, 0), Some(branco));
        assert_eq!(imagem.obter_pixel(1, 1), Some(branco));
        assert_eq!(imagem.obter_pixel(2, 2), Some(branco));
    }

    #[test]
    fn rejeita_imagem_acima_do_limite() {
        assert!(ImagemRgba::nova(10, 10, 99).is_err());
    }
}
