//! Fachada modular da biblioteca padrão Nebula.
//!
//! Este crate organiza APIs implementadas no runtime Rust. Um módulo só é
//! exportado quando oferece operações reais; módulos planejados sem API
//! funcional não são publicados como se estivessem prontos.

/// Serialização e análise de dados JSON.
pub mod dados;

/// Tipos e operações essenciais compartilhados pelos módulos.
pub mod nucleo {
    pub use nebula_runtime::{ErroRuntime, Resultado, Valor};
}

/// Operações matemáticas verificadas.
pub mod matematica {
    pub use nebula_runtime::{
        FuncaoMatematica, OperacaoDecimal, OperacaoInteira, calcular_matematica, operar_decimal,
        operar_inteiro,
    };
}

/// Manipulação de texto Unicode.
pub mod texto {
    pub use nebula_runtime::{
        caractere_texto, comprimento_texto, inteiro_para_texto, subtexto, texto_aparar,
        texto_comeca_com, texto_contem, texto_maiusculas, texto_minusculas, texto_para_decimal,
        texto_para_inteiro, texto_substituir, texto_termina_com,
    };
}

/// Coleções dinâmicas e memória lexical.
pub mod colecoes {
    pub use nebula_runtime::{Memoria, Valor};
}

/// Operações reais de arquivos e diretórios.
pub mod arquivos {
    pub use nebula_runtime::{
        arquivo_existe, copiar_arquivo, criar_diretorio, diretorio_atual, diretorio_existe,
        escrever_arquivo, ler_arquivo, listar_diretorio, mover_arquivo, remover_arquivo,
    };
}

/// Relógio e suspensão do processo.
pub mod tempo {
    pub use nebula_runtime::{dormir_milisegundos, tempo_unix_milisegundos};
}

/// Informações do processo e ambiente.
pub mod sistema {
    pub use nebula_runtime::{argumentos_processo, plataforma, variavel_ambiente};
}

/// Alocação controlada e identificadores geracionais.
pub mod memoria {
    pub use nebula_runtime::{Alocador, ErroAlocador, IdentificadorAlocacao};
}

/// Primitivas concorrentes seguras.
pub mod paralelo {
    pub use nebula_runtime::{AtomoI64, Canal, Tarefa, TokenCancelamento};
}

/// Funções criptográficas SHA-256, HMAC e comparação segura.
pub mod seguranca {
    pub use nebula_seguranca::{comparar_seguro, hmac_sha256_hex, sha256_hex};
}

/// Cliente TCP síncrono com limite explícito de leitura.
pub mod rede {
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpStream, ToSocketAddrs};
    use std::time::Duration;

    /// Erro de transporte TCP.
    #[derive(Debug)]
    pub enum ErroRede {
        /// Nenhum endereço foi resolvido.
        EnderecoInvalido,
        /// Erro de socket ou de transferência.
        Io(std::io::Error),
        /// A resposta excedeu o limite solicitado.
        LimiteResposta,
    }

    /// Conexão TCP com timeout de leitura e escrita.
    pub struct ConexaoTcp {
        fluxo: TcpStream,
        limite_leitura: usize,
    }

    impl ConexaoTcp {
        /// Conecta ao primeiro endereço resolvido dentro do timeout.
        pub fn conectar<A: ToSocketAddrs>(
            endereco: A,
            timeout: Duration,
        ) -> Result<Self, ErroRede> {
            let enderecos = endereco.to_socket_addrs().map_err(ErroRede::Io)?;
            let mut ultimo = None;
            for endereco in enderecos {
                match TcpStream::connect_timeout(&endereco, timeout) {
                    Ok(fluxo) => {
                        fluxo
                            .set_read_timeout(Some(timeout))
                            .map_err(ErroRede::Io)?;
                        fluxo
                            .set_write_timeout(Some(timeout))
                            .map_err(ErroRede::Io)?;
                        return Ok(Self {
                            fluxo,
                            limite_leitura: 1024 * 1024,
                        });
                    }
                    Err(erro) => ultimo = Some(erro),
                }
            }
            Err(ultimo.map_or(ErroRede::EnderecoInvalido, ErroRede::Io))
        }

        /// Configura o máximo de bytes aceitos por chamada de recebimento.
        pub fn definir_limite_leitura(&mut self, limite: usize) {
            self.limite_leitura = limite;
        }

        /// Envia todos os bytes fornecidos.
        pub fn enviar(&mut self, dados: &[u8]) -> Result<(), ErroRede> {
            self.fluxo.write_all(dados).map_err(ErroRede::Io)
        }

        /// Recebe até o limite configurado ou até o encerramento remoto.
        pub fn receber(&mut self) -> Result<Vec<u8>, ErroRede> {
            let mut dados = Vec::new();
            let mut buffer = [0_u8; 8192];
            loop {
                match self.fluxo.read(&mut buffer) {
                    Ok(0) => return Ok(dados),
                    Ok(lidos) => {
                        if dados.len().saturating_add(lidos) > self.limite_leitura {
                            return Err(ErroRede::LimiteResposta);
                        }
                        dados.extend_from_slice(&buffer[..lidos]);
                    }
                    Err(erro)
                        if erro.kind() == std::io::ErrorKind::WouldBlock
                            || erro.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        return Ok(dados);
                    }
                    Err(erro) => return Err(ErroRede::Io(erro)),
                }
            }
        }

        /// Encerra explicitamente os dois sentidos da conexão.
        pub fn fechar(self) -> Result<(), ErroRede> {
            self.fluxo.shutdown(Shutdown::Both).map_err(ErroRede::Io)
        }
    }
}

#[cfg(test)]
mod testes {
    #[test]
    fn modulos_reais_expoem_funcoes_do_runtime() {
        assert_eq!(super::texto::comprimento_texto("Nebula 🪐"), 8);
        assert_eq!(
            super::matematica::operar_inteiro(super::matematica::OperacaoInteira::Soma, 20, 22)
                .expect("soma"),
            42
        );
        assert_eq!(
            super::nucleo::Valor::Inteiro(7),
            super::nucleo::Valor::Inteiro(7)
        );
    }

    #[test]
    fn memoria_e_tempo_estao_expostos_por_apis_tipadas() {
        let mut memoria = super::colecoes::Memoria::nova();
        memoria
            .declarar("x", super::colecoes::Valor::Inteiro(3), false)
            .expect("declaração");
        assert_eq!(
            memoria.obter("x").expect("leitura"),
            super::colecoes::Valor::Inteiro(3)
        );
        assert!(super::tempo::tempo_unix_milisegundos().is_ok());
    }
}
