//! Alocador de blocos com identificadores geracionais e acesso verificado.
//!
//! A implementação não expõe ponteiros nem referências que sobrevivam a uma
//! operação do alocador. Cada acesso valida o identificador e os limites.

/// Identificador estável de uma alocação enquanto ela estiver ativa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IdentificadorAlocacao {
    indice: usize,
    geracao: u64,
}

/// Falhas possíveis nas operações do alocador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroAlocador {
    /// O limite de bytes do alocador seria excedido.
    LimiteExcedido,
    /// O identificador não existe mais ou pertence a outra alocação.
    IdentificadorInvalido,
    /// O intervalo solicitado ultrapassa o tamanho do bloco.
    AcessoForaDosLimites,
    /// O cálculo de tamanho excedeu a capacidade de usize.
    Overflow,
}

#[derive(Debug)]
struct Bloco {
    geracao: u64,
    dados: Option<Vec<u8>>,
}

/// Alocador seguro de blocos para dados gerenciados pelo runtime.
#[derive(Debug)]
pub struct Alocador {
    blocos: Vec<Bloco>,
    livres: Vec<usize>,
    bytes_ativos: usize,
    limite_bytes: usize,
}

impl Default for Alocador {
    fn default() -> Self {
        Self::novo(usize::MAX)
    }
}

impl Alocador {
    /// Cria um alocador com limite máximo de bytes ativos.
    #[must_use]
    pub fn novo(limite_bytes: usize) -> Self {
        Self {
            blocos: Vec::new(),
            livres: Vec::new(),
            bytes_ativos: 0,
            limite_bytes,
        }
    }

    /// Retorna o total de bytes atualmente alocados.
    #[must_use]
    pub fn bytes_alocados(&self) -> usize {
        self.bytes_ativos
    }

    /// Retorna a quantidade de blocos ativos.
    #[must_use]
    pub fn blocos_ativos(&self) -> usize {
        self.blocos.iter().filter(|bloco| bloco.dados.is_some()).count()
    }

    /// Aloca um bloco preenchido com zeros e retorna seu identificador.
    pub fn alocar(&mut self, tamanho: usize) -> Result<IdentificadorAlocacao, ErroAlocador> {
        let novo_total = self.bytes_ativos.checked_add(tamanho).ok_or(ErroAlocador::Overflow)?;
        if novo_total > self.limite_bytes {
            return Err(ErroAlocador::LimiteExcedido);
        }

        let dados = vec![0; tamanho];
        let identificador = if let Some(indice) = self.livres.pop() {
            let bloco = &mut self.blocos[indice];
            bloco.dados = Some(dados);
            IdentificadorAlocacao { indice, geracao: bloco.geracao }
        } else {
            let indice = self.blocos.len();
            self.blocos.push(Bloco { geracao: 1, dados: Some(dados) });
            IdentificadorAlocacao { indice, geracao: 1 }
        };
        self.bytes_ativos = novo_total;
        Ok(identificador)
    }

    /// Lê uma faixa do bloco sem permitir acesso fora dos limites.
    pub fn ler(
        &self,
        identificador: IdentificadorAlocacao,
        inicio: usize,
        tamanho: usize,
    ) -> Result<&[u8], ErroAlocador> {
        let bloco = self.validar(identificador)?;
        let dados = bloco.dados.as_ref().ok_or(ErroAlocador::IdentificadorInvalido)?;
        let fim = inicio.checked_add(tamanho).ok_or(ErroAlocador::Overflow)?;
        dados.get(inicio..fim).ok_or(ErroAlocador::AcessoForaDosLimites)
    }

    /// Escreve bytes em uma faixa validada do bloco.
    pub fn escrever(
        &mut self,
        identificador: IdentificadorAlocacao,
        inicio: usize,
        origem: &[u8],
    ) -> Result<(), ErroAlocador> {
        let bloco = self.validar_mutavel(identificador)?;
        let dados = bloco.dados.as_mut().ok_or(ErroAlocador::IdentificadorInvalido)?;
        let fim = inicio.checked_add(origem.len()).ok_or(ErroAlocador::Overflow)?;
        let destino = dados.get_mut(inicio..fim).ok_or(ErroAlocador::AcessoForaDosLimites)?;
        destino.copy_from_slice(origem);
        Ok(())
    }

    /// Redimensiona um bloco; em caso de falha, o bloco original é preservado.
    pub fn redimensionar(
        &mut self,
        identificador: IdentificadorAlocacao,
        novo_tamanho: usize,
    ) -> Result<IdentificadorAlocacao, ErroAlocador> {
        let dados_atuais = self.validar(identificador)?
            .dados.as_ref()
            .ok_or(ErroAlocador::IdentificadorInvalido)?;
        let tamanho_antigo = dados_atuais.len();
        let novo_total = self.bytes_ativos
            .checked_sub(tamanho_antigo)
            .and_then(|total| total.checked_add(novo_tamanho))
            .ok_or(ErroAlocador::Overflow)?;
        if novo_total > self.limite_bytes {
            return Err(ErroAlocador::LimiteExcedido);
        }

        let total_temporario = self.bytes_ativos
            .checked_add(novo_tamanho)
            .ok_or(ErroAlocador::Overflow)?;
        let dados_atuais = self.ler(identificador, 0, tamanho_antigo)?.to_vec();
        let mut dados_novos = vec![0; novo_tamanho];
        let copiar = tamanho_antigo.min(novo_tamanho);
        dados_novos[..copiar].copy_from_slice(&dados_atuais[..copiar]);

        let novo = if let Some(indice) = self.livres.pop() {
            let bloco = &mut self.blocos[indice];
            bloco.dados = Some(dados_novos);
            IdentificadorAlocacao { indice, geracao: bloco.geracao }
        } else {
            let indice = self.blocos.len();
            self.blocos.push(Bloco { geracao: 1, dados: Some(dados_novos) });
            IdentificadorAlocacao { indice, geracao: 1 }
        };
        self.bytes_ativos = total_temporario;
        self.liberar(identificador)?;
        Ok(novo)
    }

    /// Libera um bloco e invalida permanentemente seu identificador anterior.
    pub fn liberar(
        &mut self,
        identificador: IdentificadorAlocacao,
    ) -> Result<(), ErroAlocador> {
        let bloco = self.validar_mutavel(identificador)?;
        let dados = bloco.dados.take().ok_or(ErroAlocador::IdentificadorInvalido)?;
        self.bytes_ativos -= dados.len();
        if let Some(proxima_geracao) = bloco.geracao.checked_add(1) {
            bloco.geracao = proxima_geracao;
            self.livres.push(identificador.indice);
        } else {
            bloco.geracao = 0;
        }
        Ok(())
    }

    /// Libera todos os blocos ativos e invalida todos os identificadores.
    pub fn limpar(&mut self) {
        self.livres.clear();
        self.bytes_ativos = 0;
        for (indice, bloco) in self.blocos.iter_mut().enumerate() {
            if bloco.dados.take().is_some() {
                if let Some(proxima_geracao) = bloco.geracao.checked_add(1) {
                    bloco.geracao = proxima_geracao;
                    self.livres.push(indice);
                } else {
                    bloco.geracao = 0;
                }
            }
        }
    }

    fn validar(&self, identificador: IdentificadorAlocacao) -> Result<&Bloco, ErroAlocador> {
        let bloco = self.blocos.get(identificador.indice)
            .ok_or(ErroAlocador::IdentificadorInvalido)?;
        if bloco.geracao != identificador.geracao || bloco.dados.is_none() {
            return Err(ErroAlocador::IdentificadorInvalido);
        }
        Ok(bloco)
    }

    fn validar_mutavel(
        &mut self,
        identificador: IdentificadorAlocacao,
    ) -> Result<&mut Bloco, ErroAlocador> {
        let bloco = self.blocos.get_mut(identificador.indice)
            .ok_or(ErroAlocador::IdentificadorInvalido)?;
        if bloco.geracao != identificador.geracao || bloco.dados.is_none() {
            return Err(ErroAlocador::IdentificadorInvalido);
        }
        Ok(bloco)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn aloca_le_escreve_e_libera_bloco() {
        let mut alocador = Alocador::novo(32);
        let bloco = alocador.alocar(8).unwrap();
        alocador.escrever(bloco, 2, &[1, 2, 3]).unwrap();
        assert_eq!(alocador.ler(bloco, 2, 3), Ok(&[1, 2, 3][..]));
        assert_eq!(alocador.bytes_alocados(), 8);
        alocador.liberar(bloco).unwrap();
        assert_eq!(alocador.bytes_alocados(), 0);
        assert_eq!(alocador.ler(bloco, 0, 1), Err(ErroAlocador::IdentificadorInvalido));
    }

    #[test]
    fn rejeita_limite_e_intervalos_invalidos() {
        let mut alocador = Alocador::novo(4);
        let bloco = alocador.alocar(4).unwrap();
        assert_eq!(alocador.alocar(1), Err(ErroAlocador::LimiteExcedido));
        assert_eq!(alocador.ler(bloco, 3, 2), Err(ErroAlocador::AcessoForaDosLimites));
        assert_eq!(alocador.escrever(bloco, usize::MAX, &[1]), Err(ErroAlocador::Overflow));
    }

    #[test]
    fn reutilizacao_nao_reativa_identificador_antigo() {
        let mut alocador = Alocador::novo(16);
        let antigo = alocador.alocar(4).unwrap();
        alocador.liberar(antigo).unwrap();
        let novo = alocador.alocar(4).unwrap();
        assert_ne!(antigo, novo);
        assert_eq!(alocador.ler(antigo, 0, 1), Err(ErroAlocador::IdentificadorInvalido));
        assert_eq!(alocador.ler(novo, 0, 1), Ok(&[0][..]));
    }

    #[test]
    fn redimensionamento_preserva_conteudo_e_invalida_handle_anterior() {
        let mut alocador = Alocador::novo(16);
        let antigo = alocador.alocar(3).unwrap();
        alocador.escrever(antigo, 0, &[7, 8, 9]).unwrap();
        let novo = alocador.redimensionar(antigo, 5).unwrap();
        assert_eq!(alocador.ler(novo, 0, 5), Ok(&[7, 8, 9, 0, 0][..]));
        assert_eq!(alocador.ler(antigo, 0, 1), Err(ErroAlocador::IdentificadorInvalido));
    }

    #[test]
    fn redimensiona_sem_ultrapassar_temporariamente_o_limite() {
        let mut alocador = Alocador::novo(4);
        let antigo = alocador.alocar(4).unwrap();
        alocador.escrever(antigo, 0, &[4, 3, 2, 1]).unwrap();
        let novo = alocador.redimensionar(antigo, 4).unwrap();
        assert_eq!(alocador.bytes_alocados(), 4);
        assert_eq!(alocador.ler(novo, 0, 4), Ok(&[4, 3, 2, 1][..]));
    }

    #[test]
    fn falha_de_redimensionamento_preserva_bloco_original() {
        let mut alocador = Alocador::novo(4);
        let bloco = alocador.alocar(4).unwrap();
        alocador.escrever(bloco, 0, &[1, 2, 3, 4]).unwrap();
        assert_eq!(alocador.redimensionar(bloco, 5), Err(ErroAlocador::LimiteExcedido));
        assert_eq!(alocador.ler(bloco, 0, 4), Ok(&[1, 2, 3, 4][..]));
    }

    #[test]
    fn limpar_invalida_handles_ativos() {
        let mut alocador = Alocador::novo(8);
        let bloco = alocador.alocar(8).unwrap();
        alocador.limpar();
        assert_eq!(alocador.bytes_alocados(), 0);
        assert_eq!(alocador.blocos_ativos(), 0);
        assert_eq!(alocador.liberar(bloco), Err(ErroAlocador::IdentificadorInvalido));
    }
}
