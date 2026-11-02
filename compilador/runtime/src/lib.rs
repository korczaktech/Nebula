//! Runtime seguro da Nebula: valores, escopos e memória gerenciada.
//!
//! O runtime usa apenas abstrações seguras de Rust. Valores compostos podem
//! compartilhar listas protegidas por locks, permitindo uso entre tarefas
//! sem expor ponteiros ou referências inválidas à linguagem.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;

/// Valor dinâmico utilizado pelo runtime da Nebula.
#[derive(Clone, Debug)]
pub enum Valor {
    /// Valor ausente de retorno.
    Vazio,
    /// Ausência explícita de valor.
    Nulo,
    /// Valor lógico.
    Logico(bool),
    /// Inteiro assinado de 128 bits.
    Inteiro(i128),
    /// Número decimal de 64 bits.
    Decimal(f64),
    /// Texto UTF-8.
    Texto(String),
    /// Caractere Unicode.
    Caractere(char),
    /// Lista compartilhada e protegida contra acesso concorrente.
    Lista(Arc<RwLock<Vec<Valor>>>),
    /// Intervalo inteiro inclusivo.
    Intervalo {
        /// Limite inferior inclusivo.
        inicio: i128,
        /// Limite superior inclusivo.
        fim: i128,
    },
    /// Intervalo decimal inclusivo.
    IntervaloDecimal {
        /// Limite inferior inclusivo.
        inicio: f64,
        /// Limite superior inclusivo.
        fim: f64,
    },
}

impl PartialEq for Valor {
    fn eq(&self, outro: &Self) -> bool {
        match (self, outro) {
            (Self::Vazio, Self::Vazio) | (Self::Nulo, Self::Nulo) => true,
            (Self::Logico(a), Self::Logico(b)) => a == b,
            (Self::Inteiro(a), Self::Inteiro(b)) => a == b,
            (Self::Decimal(a), Self::Decimal(b)) => a == b,
            (Self::Texto(a), Self::Texto(b)) => a == b,
            (Self::Caractere(a), Self::Caractere(b)) => a == b,
            (
                Self::Intervalo {
                    inicio: ai,
                    fim: af,
                },
                Self::Intervalo {
                    inicio: bi,
                    fim: bf,
                },
            ) => ai == bi && af == bf,
            (
                Self::IntervaloDecimal {
                    inicio: ai,
                    fim: af,
                },
                Self::IntervaloDecimal {
                    inicio: bi,
                    fim: bf,
                },
            ) => ai == bi && af == bf,
            (Self::Lista(a), Self::Lista(b)) => match (a.read(), b.read()) {
                (Ok(a), Ok(b)) => *a == *b,
                _ => false,
            },
            _ => false,
        }
    }
}

/// Operação inteira sujeita a verificação de overflow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperacaoInteira {
    /// Soma.
    Soma,
    /// Subtração.
    Subtracao,
    /// Multiplicação.
    Multiplicacao,
    /// Divisão.
    Divisao,
    /// Resto.
    Resto,
    /// Potência com expoente não negativo.
    Potencia,
}

/// Funções matemáticas escalares disponíveis no runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FuncaoMatematica {
    /// Raiz quadrada.
    Raiz,
    /// Seno em radianos.
    Seno,
    /// Cosseno em radianos.
    Cosseno,
    /// Tangente em radianos.
    Tangente,
    /// Logaritmo natural.
    Logaritmo,
    /// Exponencial natural.
    Exponencial,
    /// Arredonda para baixo.
    Piso,
    /// Arredonda para cima.
    Teto,
    /// Arredonda para o inteiro mais próximo.
    Arredondar,
    /// Valor absoluto.
    Absoluto,
}

/// Operação decimal com validação de resultados não finitos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperacaoDecimal {
    /// Soma.
    Soma,
    /// Subtração.
    Subtracao,
    /// Multiplicação.
    Multiplicacao,
    /// Divisão.
    Divisao,
    /// Resto.
    Resto,
    /// Potência real.
    Potencia,
}

/// Erros recuperáveis do runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErroRuntime {
    /// O identificador não existe em nenhum escopo ativo.
    NomeNaoEncontrado(String),
    /// O identificador já existe no escopo atual.
    NomeDuplicado(String),
    /// Tentativa de alterar uma variável constante.
    VariavelImutavel(String),
    /// Tentativa de remover o escopo global.
    EscopoGlobal,
    /// Operação solicitada para um tipo incompatível.
    TipoIncompativel(&'static str),
    /// Índice fora dos limites da lista.
    IndiceForaDosLimites {
        /// Índice solicitado.
        indice: usize,
        /// Quantidade de elementos disponíveis.
        tamanho: usize,
    },
    /// Lock de uma estrutura compartilhada foi envenenado.
    MemoriaCorrompida,
    /// A operação inteira excedeu a faixa de i128.
    Overflow,
    /// Divisão ou resto por zero.
    DivisaoPorZero,
    /// Expoente negativo não é aceito para potência inteira.
    ExpoenteNegativo,
    /// Uma tarefa terminou com panic ou não pôde ser iniciada.
    FalhaTarefa,
    /// O canal foi encerrado antes da operação.
    CanalFechado,
    /// Limites de um intervalo de texto são inválidos.
    IntervaloInvalido,
    /// Texto não representa um número válido.
    TextoNumericoInvalido,
    /// Operação decimal produziu NaN ou infinito.
    ResultadoNaoFinito,
    /// Falha em uma operação de entrada/saída.
    FalhaIo(String),
    /// O relógio do sistema não está disponível antes da época Unix.
    RelogioIndisponivel,
    /// O argumento está fora do domínio matemático da função.
    DominioMatematico,
    /// A tarefa observou um pedido cooperativo de cancelamento.
    TarefaCancelada,
    /// O contexto de execução já foi finalizado.
    RuntimeFinalizado,
}

/// Resultado padrão das operações do runtime.
pub type Resultado<T> = Result<T, ErroRuntime>;

/// Inteiro atômico com operações seguras para compartilhamento entre threads.
pub struct AtomoI64 {
    valor: AtomicI64,
}

impl AtomoI64 {
    /// Cria um átomo com valor inicial.
    #[must_use]
    pub const fn novo(valor: i64) -> Self {
        Self {
            valor: AtomicI64::new(valor),
        }
    }

    /// Carrega o valor com ordenação Acquire.
    #[must_use]
    pub fn carregar(&self) -> i64 {
        self.valor.load(Ordering::Acquire)
    }

    /// Armazena um valor com ordenação Release.
    pub fn armazenar(&self, valor: i64) {
        self.valor.store(valor, Ordering::Release);
    }

    /// Soma um valor e retorna o valor anterior, com ordenação AcqRel.
    pub fn adicionar(&self, valor: i64) -> i64 {
        self.valor.fetch_add(valor, Ordering::AcqRel)
    }

    /// Substitui o valor somente se ele corresponder ao esperado.
    pub fn comparar_trocar(&self, esperado: i64, novo: i64) -> Result<i64, i64> {
        self.valor
            .compare_exchange(esperado, novo, Ordering::AcqRel, Ordering::Acquire)
    }
}

/// Sinal compartilhado de cancelamento cooperativo.
#[derive(Clone, Debug, Default)]
pub struct TokenCancelamento {
    cancelado: Arc<AtomicBool>,
}

impl TokenCancelamento {
    /// Cria um token ainda não cancelado.
    #[must_use]
    pub fn novo() -> Self {
        Self::default()
    }

    /// Solicita cancelamento aos participantes que compartilham o token.
    pub fn cancelar(&self) {
        self.cancelado.store(true, Ordering::Release);
    }

    /// Informa se o cancelamento foi solicitado.
    #[must_use]
    pub fn cancelado(&self) -> bool {
        self.cancelado.load(Ordering::Acquire)
    }

    /// Retorna erro se o cancelamento já tiver sido solicitado.
    pub fn verificar(&self) -> Resultado<()> {
        if self.cancelado() {
            Err(ErroRuntime::TarefaCancelada)
        } else {
            Ok(())
        }
    }
}

/// Tarefa executada em uma thread do sistema.
pub struct Tarefa<T> {
    identificador: Option<JoinHandle<T>>,
}

impl<T: Send + 'static> Tarefa<T> {
    /// Inicia uma tarefa independente.
    pub fn iniciar<F>(funcao: F) -> Resultado<Self>
    where
        F: FnOnce() -> T + Send + 'static,
    {
        let identificador = std::thread::Builder::new()
            .spawn(funcao)
            .map_err(|_| ErroRuntime::FalhaTarefa)?;
        Ok(Self {
            identificador: Some(identificador),
        })
    }

    /// Inicia uma tarefa com cancelamento cooperativo.
    ///
    /// A tarefa deve consultar periodicamente o token recebido; o runtime
    /// não interrompe threads à força.
    pub fn iniciar_cancelavel<F>(funcao: F) -> Resultado<(Self, TokenCancelamento)>
    where
        F: FnOnce(TokenCancelamento) -> T + Send + 'static,
    {
        let token = TokenCancelamento::novo();
        let token_tarefa = token.clone();
        let identificador = std::thread::Builder::new()
            .spawn(move || funcao(token_tarefa))
            .map_err(|_| ErroRuntime::FalhaTarefa)?;
        Ok((
            Self {
                identificador: Some(identificador),
            },
            token,
        ))
    }

    /// Aguarda a tarefa e retorna seu resultado.
    pub fn aguardar(mut self) -> Resultado<T> {
        self.identificador
            .take()
            .expect("tarefa só pode ser aguardada uma vez")
            .join()
            .map_err(|_| ErroRuntime::FalhaTarefa)
    }
}

/// Canal concorrente com fila segura entre threads.
pub struct Canal<T> {
    remetente: Sender<T>,
    receptor: Arc<Mutex<Receiver<T>>>,
}

impl<T> Canal<T> {
    /// Cria um canal com um remetente e um receptor compartilhados.
    #[must_use]
    pub fn novo() -> Self {
        let (remetente, receptor) = mpsc::channel();
        Self {
            remetente,
            receptor: Arc::new(Mutex::new(receptor)),
        }
    }

    /// Cria outro handle para o mesmo canal.
    #[must_use]
    pub fn clonar(&self) -> Self {
        Self {
            remetente: self.remetente.clone(),
            receptor: Arc::clone(&self.receptor),
        }
    }

    /// Envia um valor para o canal.
    pub fn enviar(&self, valor: T) -> Resultado<()> {
        self.remetente
            .send(valor)
            .map_err(|_| ErroRuntime::CanalFechado)
    }

    /// Aguarda e recebe o próximo valor disponível.
    pub fn receber(&self) -> Resultado<T> {
        self.receptor
            .lock()
            .map_err(|_| ErroRuntime::MemoriaCorrompida)?
            .recv()
            .map_err(|_| ErroRuntime::CanalFechado)
    }
}

#[derive(Clone, Debug)]
struct Celula {
    valor: Valor,
    mutavel: bool,
}

/// Memória de execução com escopos léxicos e sombreamento.
#[derive(Clone, Debug)]
pub struct Memoria {
    escopos: Vec<HashMap<String, Celula>>,
}

impl Default for Memoria {
    fn default() -> Self {
        Self::nova()
    }
}

impl Memoria {
    /// Cria a memória com o escopo global aberto.
    #[must_use]
    pub fn nova() -> Self {
        Self {
            escopos: vec![HashMap::new()],
        }
    }

    /// Retorna a quantidade de escopos ativos, incluindo o global.
    #[must_use]
    pub fn quantidade_escopos(&self) -> usize {
        self.escopos.len()
    }

    /// Retorna uma cópia dos valores declarados no escopo global.
    #[must_use]
    pub fn globais(&self) -> HashMap<String, Valor> {
        self.escopos[0]
            .iter()
            .map(|(nome, celula)| (nome.clone(), celula.valor.clone()))
            .collect()
    }

    /// Abre um novo escopo léxico.
    pub fn abrir_escopo(&mut self) {
        self.escopos.push(HashMap::new());
    }

    /// Fecha o escopo atual sem permitir remover o escopo global.
    pub fn fechar_escopo(&mut self) -> Resultado<()> {
        if self.escopos.len() == 1 {
            return Err(ErroRuntime::EscopoGlobal);
        }
        self.escopos.pop();
        Ok(())
    }

    /// Declara uma variável no escopo atual.
    pub fn declarar(
        &mut self,
        nome: impl Into<String>,
        valor: Valor,
        mutavel: bool,
    ) -> Resultado<()> {
        let nome = nome.into();
        let escopo = self
            .escopos
            .last_mut()
            .expect("escopo global sempre existe");
        if escopo.contains_key(&nome) {
            return Err(ErroRuntime::NomeDuplicado(nome));
        }
        escopo.insert(nome, Celula { valor, mutavel });
        Ok(())
    }

    /// Obtém uma cópia do valor visível, respeitando sombreamento.
    pub fn obter(&self, nome: &str) -> Resultado<Valor> {
        self.escopos
            .iter()
            .rev()
            .find_map(|escopo| escopo.get(nome))
            .map(|celula| celula.valor.clone())
            .ok_or_else(|| ErroRuntime::NomeNaoEncontrado(nome.to_owned()))
    }

    /// Atribui um novo valor à declaração visível mais próxima.
    pub fn atribuir(&mut self, nome: &str, valor: Valor) -> Resultado<()> {
        for escopo in self.escopos.iter_mut().rev() {
            if let Some(celula) = escopo.get_mut(nome) {
                if !celula.mutavel {
                    return Err(ErroRuntime::VariavelImutavel(nome.to_owned()));
                }
                celula.valor = valor;
                return Ok(());
            }
        }
        Err(ErroRuntime::NomeNaoEncontrado(nome.to_owned()))
    }

    /// Atualiza um elemento de uma lista vinculada a uma variável mutável.
    pub fn definir_elemento_lista(
        &mut self,
        nome: &str,
        indice: usize,
        valor: Valor,
    ) -> Resultado<()> {
        let celula = self
            .escopos
            .iter_mut()
            .rev()
            .find_map(|escopo| escopo.get_mut(nome))
            .ok_or_else(|| ErroRuntime::NomeNaoEncontrado(nome.to_owned()))?;

        if !celula.mutavel {
            return Err(ErroRuntime::VariavelImutavel(nome.to_owned()));
        }

        let Valor::Lista(lista) = &celula.valor else {
            return Err(ErroRuntime::TipoIncompativel(
                "a variável não contém uma lista",
            ));
        };

        let mut itens = lista.write().map_err(|_| ErroRuntime::MemoriaCorrompida)?;
        let tamanho = itens.len();
        let destino = itens
            .get_mut(indice)
            .ok_or(ErroRuntime::IndiceForaDosLimites { indice, tamanho })?;
        *destino = valor;
        Ok(())
    }

    /// Adiciona um elemento ao final de uma lista mutável.
    pub fn adicionar_elemento_lista(&mut self, nome: &str, valor: Valor) -> Resultado<()> {
        let celula = self
            .escopos
            .iter_mut()
            .rev()
            .find_map(|escopo| escopo.get_mut(nome))
            .ok_or_else(|| ErroRuntime::NomeNaoEncontrado(nome.to_owned()))?;
        if !celula.mutavel {
            return Err(ErroRuntime::VariavelImutavel(nome.to_owned()));
        }
        let Valor::Lista(lista) = &celula.valor else {
            return Err(ErroRuntime::TipoIncompativel(
                "a variável não contém uma lista",
            ));
        };
        lista
            .write()
            .map_err(|_| ErroRuntime::MemoriaCorrompida)?
            .push(valor);
        Ok(())
    }

    /// Remove e retorna um elemento de uma lista mutável.
    pub fn remover_elemento_lista(&mut self, nome: &str, indice: usize) -> Resultado<Valor> {
        let celula = self
            .escopos
            .iter_mut()
            .rev()
            .find_map(|escopo| escopo.get_mut(nome))
            .ok_or_else(|| ErroRuntime::NomeNaoEncontrado(nome.to_owned()))?;
        if !celula.mutavel {
            return Err(ErroRuntime::VariavelImutavel(nome.to_owned()));
        }
        let Valor::Lista(lista) = &celula.valor else {
            return Err(ErroRuntime::TipoIncompativel(
                "a variável não contém uma lista",
            ));
        };
        let mut itens = lista.write().map_err(|_| ErroRuntime::MemoriaCorrompida)?;
        let tamanho = itens.len();
        if indice >= tamanho {
            return Err(ErroRuntime::IndiceForaDosLimites { indice, tamanho });
        }
        Ok(itens.remove(indice))
    }

    /// Lê um elemento de lista sem expor referências internas.
    pub fn elemento_lista(&self, nome: &str, indice: usize) -> Resultado<Valor> {
        let valor = self.obter(nome)?;
        let Valor::Lista(lista) = valor else {
            return Err(ErroRuntime::TipoIncompativel(
                "a variável não contém uma lista",
            ));
        };
        let itens = lista.read().map_err(|_| ErroRuntime::MemoriaCorrompida)?;
        itens
            .get(indice)
            .cloned()
            .ok_or(ErroRuntime::IndiceForaDosLimites {
                indice,
                tamanho: itens.len(),
            })
    }

    /// Retorna a quantidade de elementos de uma lista.
    pub fn comprimento_lista(&self, nome: &str) -> Resultado<usize> {
        let valor = self.obter(nome)?;
        let Valor::Lista(lista) = valor else {
            return Err(ErroRuntime::TipoIncompativel(
                "a variável não contém uma lista",
            ));
        };
        let itens = lista.read().map_err(|_| ErroRuntime::MemoriaCorrompida)?;
        Ok(itens.len())
    }
}

/// Retorna a quantidade de caracteres Unicode de um texto, não seus bytes.
#[must_use]
pub fn comprimento_texto(texto: &str) -> usize {
    texto.chars().count()
}

/// Obtém o caractere Unicode na posição informada.
pub fn caractere_texto(texto: &str, indice: usize) -> Resultado<char> {
    texto
        .chars()
        .nth(indice)
        .ok_or(ErroRuntime::IndiceForaDosLimites {
            indice,
            tamanho: comprimento_texto(texto),
        })
}

/// Retorna um trecho de texto usando posições de caracteres Unicode.
pub fn subtexto(texto: &str, inicio: usize, fim: usize) -> Resultado<String> {
    let tamanho = comprimento_texto(texto);
    if inicio > fim || fim > tamanho {
        return Err(ErroRuntime::IntervaloInvalido);
    }
    Ok(texto.chars().skip(inicio).take(fim - inicio).collect())
}

/// Verifica se um texto contém outro texto.
#[must_use]
pub fn texto_contem(texto: &str, trecho: &str) -> bool {
    texto.contains(trecho)
}

/// Remove espaços em branco Unicode do início e do fim do texto.
#[must_use]
pub fn texto_aparar(texto: &str) -> String {
    texto.trim().to_owned()
}

/// Verifica se o texto começa com o prefixo informado.
#[must_use]
pub fn texto_comeca_com(texto: &str, prefixo: &str) -> bool {
    texto.starts_with(prefixo)
}

/// Verifica se o texto termina com o sufixo informado.
#[must_use]
pub fn texto_termina_com(texto: &str, sufixo: &str) -> bool {
    texto.ends_with(sufixo)
}

/// Substitui todas as ocorrências literais de um trecho.
#[must_use]
pub fn texto_substituir(texto: &str, alvo: &str, substituto: &str) -> String {
    texto.replace(alvo, substituto)
}

/// Converte texto para maiúsculas segundo as regras Unicode.
#[must_use]
pub fn texto_maiusculas(texto: &str) -> String {
    texto.to_uppercase()
}

/// Converte texto para minúsculas segundo as regras Unicode.
#[must_use]
pub fn texto_minusculas(texto: &str) -> String {
    texto.to_lowercase()
}

/// Converte texto decimal em inteiro assinado de 128 bits.
pub fn texto_para_inteiro(texto: &str) -> Resultado<i128> {
    texto
        .trim()
        .parse()
        .map_err(|_| ErroRuntime::TextoNumericoInvalido)
}

/// Converte texto decimal em número de ponto flutuante finito.
pub fn texto_para_decimal(texto: &str) -> Resultado<f64> {
    let valor = texto
        .trim()
        .parse::<f64>()
        .map_err(|_| ErroRuntime::TextoNumericoInvalido)?;
    if !valor.is_finite() {
        return Err(ErroRuntime::ResultadoNaoFinito);
    }
    Ok(valor)
}

/// Executa uma função matemática e valida domínio e resultado finito.
pub fn calcular_matematica(funcao: FuncaoMatematica, valor: f64) -> Resultado<f64> {
    let resultado = match funcao {
        FuncaoMatematica::Raiz if valor < 0.0 => {
            return Err(ErroRuntime::DominioMatematico);
        }
        FuncaoMatematica::Raiz => valor.sqrt(),
        FuncaoMatematica::Seno => valor.sin(),
        FuncaoMatematica::Cosseno => valor.cos(),
        FuncaoMatematica::Tangente => valor.tan(),
        FuncaoMatematica::Logaritmo if valor <= 0.0 => {
            return Err(ErroRuntime::DominioMatematico);
        }
        FuncaoMatematica::Logaritmo => valor.ln(),
        FuncaoMatematica::Exponencial => valor.exp(),
        FuncaoMatematica::Piso => valor.floor(),
        FuncaoMatematica::Teto => valor.ceil(),
        FuncaoMatematica::Arredondar => valor.round(),
        FuncaoMatematica::Absoluto => valor.abs(),
    };
    if resultado.is_finite() {
        Ok(resultado)
    } else {
        Err(ErroRuntime::ResultadoNaoFinito)
    }
}

/// Executa operação decimal e rejeita divisão por zero e resultados não finitos.
pub fn operar_decimal(operacao: OperacaoDecimal, esquerda: f64, direita: f64) -> Resultado<f64> {
    let valor = match operacao {
        OperacaoDecimal::Soma => esquerda + direita,
        OperacaoDecimal::Subtracao => esquerda - direita,
        OperacaoDecimal::Multiplicacao => esquerda * direita,
        OperacaoDecimal::Divisao if direita == 0.0 => {
            return Err(ErroRuntime::DivisaoPorZero);
        }
        OperacaoDecimal::Divisao => esquerda / direita,
        OperacaoDecimal::Resto if direita == 0.0 => {
            return Err(ErroRuntime::DivisaoPorZero);
        }
        OperacaoDecimal::Resto => esquerda % direita,
        OperacaoDecimal::Potencia => esquerda.powf(direita),
    };
    if valor.is_finite() {
        Ok(valor)
    } else {
        Err(ErroRuntime::ResultadoNaoFinito)
    }
}

/// Formata inteiro sem perda de precisão.
#[must_use]
pub fn inteiro_para_texto(valor: i128) -> String {
    valor.to_string()
}

/// Lê um arquivo de texto UTF-8.
pub fn ler_arquivo(caminho: &str) -> Resultado<String> {
    std::fs::read_to_string(caminho).map_err(|erro| ErroRuntime::FalhaIo(erro.to_string()))
}

/// Escreve texto em um arquivo, substituindo seu conteúdo anterior.
pub fn escrever_arquivo(caminho: &str, conteudo: &str) -> Resultado<()> {
    std::fs::write(caminho, conteudo).map_err(|erro| ErroRuntime::FalhaIo(erro.to_string()))
}

/// Verifica se o caminho aponta para um arquivo regular existente.
#[must_use]
pub fn arquivo_existe(caminho: &str) -> bool {
    std::path::Path::new(caminho).is_file()
}

/// Cria um diretório e seus diretórios pais ausentes.
pub fn criar_diretorio(caminho: &str) -> Resultado<()> {
    std::fs::create_dir_all(caminho).map_err(|erro| ErroRuntime::FalhaIo(erro.to_string()))
}

/// Remove um arquivo regular.
pub fn remover_arquivo(caminho: &str) -> Resultado<()> {
    std::fs::remove_file(caminho).map_err(|erro| ErroRuntime::FalhaIo(erro.to_string()))
}

/// Copia um arquivo e retorna a quantidade de bytes copiados.
pub fn copiar_arquivo(origem: &str, destino: &str) -> Resultado<u64> {
    std::fs::copy(origem, destino).map_err(|erro| ErroRuntime::FalhaIo(erro.to_string()))
}

/// Move ou renomeia um arquivo.
pub fn mover_arquivo(origem: &str, destino: &str) -> Resultado<()> {
    std::fs::rename(origem, destino).map_err(|erro| ErroRuntime::FalhaIo(erro.to_string()))
}

/// Verifica se o caminho aponta para um diretório existente.
#[must_use]
pub fn diretorio_existe(caminho: &str) -> bool {
    std::path::Path::new(caminho).is_dir()
}

/// Lista os nomes das entradas de um diretório em ordem determinística.
pub fn listar_diretorio(caminho: &str) -> Resultado<Vec<String>> {
    let entradas =
        std::fs::read_dir(caminho).map_err(|erro| ErroRuntime::FalhaIo(erro.to_string()))?;
    let mut nomes = entradas
        .map(|entrada| {
            entrada
                .map(|item| item.file_name().to_string_lossy().into_owned())
                .map_err(|erro| ErroRuntime::FalhaIo(erro.to_string()))
        })
        .collect::<Resultado<Vec<_>>>()?;
    nomes.sort();
    Ok(nomes)
}

/// Retorna o diretório de trabalho atual.
pub fn diretorio_atual() -> Resultado<String> {
    std::env::current_dir()
        .map(|caminho| caminho.to_string_lossy().into_owned())
        .map_err(|erro| ErroRuntime::FalhaIo(erro.to_string()))
}

/// Lê uma variável de ambiente, preservando ausência como None.
#[must_use]
pub fn variavel_ambiente(nome: &str) -> Option<String> {
    std::env::var_os(nome).map(|valor| valor.to_string_lossy().into_owned())
}

/// Retorna os argumentos do processo como texto UTF-8 com substituição segura.
#[must_use]
pub fn argumentos_processo() -> Vec<String> {
    std::env::args_os()
        .map(|argumento| argumento.to_string_lossy().into_owned())
        .collect()
}

/// Identifica o sistema operacional e a arquitetura da execução atual.
#[must_use]
pub fn plataforma() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

/// Retorna o tempo Unix atual em milissegundos.
pub fn tempo_unix_milisegundos() -> Resultado<u128> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duracao| duracao.as_millis())
        .map_err(|_| ErroRuntime::RelogioIndisponivel)
}

/// Suspende a thread atual pelo número de milissegundos informado.
pub fn dormir_milisegundos(milisegundos: u64) {
    std::thread::sleep(std::time::Duration::from_millis(milisegundos));
}

/// Executa uma operação inteira com overflow e divisão por zero verificados.
pub fn operar_inteiro(operacao: OperacaoInteira, esquerda: i128, direita: i128) -> Resultado<i128> {
    match operacao {
        OperacaoInteira::Soma => esquerda.checked_add(direita),
        OperacaoInteira::Subtracao => esquerda.checked_sub(direita),
        OperacaoInteira::Multiplicacao => esquerda.checked_mul(direita),
        OperacaoInteira::Divisao => {
            if direita == 0 {
                return Err(ErroRuntime::DivisaoPorZero);
            }
            esquerda.checked_div(direita)
        }
        OperacaoInteira::Resto => {
            if direita == 0 {
                return Err(ErroRuntime::DivisaoPorZero);
            }
            esquerda.checked_rem(direita)
        }
        OperacaoInteira::Potencia => {
            let expoente = u32::try_from(direita).map_err(|_| ErroRuntime::ExpoenteNegativo)?;
            esquerda.checked_pow(expoente)
        }
    }
    .ok_or(ErroRuntime::Overflow)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn escopos_sombreiam_e_restauram_variaveis() {
        let mut memoria = Memoria::nova();
        memoria.declarar("valor", Valor::Inteiro(10), true).unwrap();
        memoria.abrir_escopo();
        memoria.declarar("valor", Valor::Inteiro(20), true).unwrap();
        assert_eq!(memoria.obter("valor"), Ok(Valor::Inteiro(20)));
        memoria.fechar_escopo().unwrap();
        assert_eq!(memoria.obter("valor"), Ok(Valor::Inteiro(10)));
    }

    #[test]
    fn impede_reatribuicao_de_constante() {
        let mut memoria = Memoria::nova();
        memoria
            .declarar("limite", Valor::Inteiro(9), false)
            .unwrap();
        assert_eq!(
            memoria.atribuir("limite", Valor::Inteiro(10)),
            Err(ErroRuntime::VariavelImutavel("limite".into()))
        );
    }

    #[test]
    fn impede_declaracao_duplicada_no_mesmo_escopo() {
        let mut memoria = Memoria::nova();
        memoria.declarar("x", Valor::Inteiro(1), true).unwrap();
        assert_eq!(
            memoria.declarar("x", Valor::Inteiro(2), true),
            Err(ErroRuntime::NomeDuplicado("x".into()))
        );
    }

    #[test]
    fn nao_fecha_escopo_global() {
        let mut memoria = Memoria::nova();
        assert_eq!(memoria.fechar_escopo(), Err(ErroRuntime::EscopoGlobal));
        assert_eq!(memoria.quantidade_escopos(), 1);
    }

    #[test]
    fn listas_compartilhadas_podem_ser_lidas_e_alteradas() {
        let lista = Valor::Lista(Arc::new(RwLock::new(vec![
            Valor::Inteiro(1),
            Valor::Inteiro(2),
        ])));
        let mut memoria = Memoria::nova();
        memoria.declarar("itens", lista, true).unwrap();
        memoria
            .definir_elemento_lista("itens", 1, Valor::Inteiro(7))
            .unwrap();
        assert_eq!(memoria.elemento_lista("itens", 1), Ok(Valor::Inteiro(7)));
        assert_eq!(memoria.comprimento_lista("itens"), Ok(2));
    }

    #[test]
    fn lista_adiciona_e_remove_elementos() {
        let lista = Valor::Lista(Arc::new(RwLock::new(vec![Valor::Inteiro(1)])));
        let mut memoria = Memoria::nova();
        memoria.declarar("itens", lista, true).unwrap();
        memoria
            .adicionar_elemento_lista("itens", Valor::Inteiro(2))
            .unwrap();
        assert_eq!(memoria.comprimento_lista("itens"), Ok(2));
        assert_eq!(
            memoria.remover_elemento_lista("itens", 0),
            Ok(Valor::Inteiro(1))
        );
        assert_eq!(memoria.comprimento_lista("itens"), Ok(1));
    }

    #[test]
    fn lista_diretorio_em_ordem_deterministica() {
        let raiz = std::env::temp_dir().join(format!("nebula-listar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        std::fs::write(raiz.join("zeta.txt"), "").unwrap();
        std::fs::write(raiz.join("alfa.txt"), "").unwrap();
        let itens = listar_diretorio(raiz.to_str().unwrap()).unwrap();
        assert_eq!(itens, vec!["alfa.txt", "zeta.txt"]);
        assert!(diretorio_existe(raiz.to_str().unwrap()));
        std::fs::remove_dir_all(raiz).unwrap();
    }

    #[test]
    fn atomo_sincroniza_incrementos_concorrentes() {
        let atomo = Arc::new(AtomoI64::novo(0));
        let mut threads = Vec::new();
        for _ in 0..4 {
            let compartilhado = Arc::clone(&atomo);
            threads.push(std::thread::spawn(move || {
                for _ in 0..1000 {
                    compartilhado.adicionar(1);
                }
            }));
        }
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(atomo.carregar(), 4000);
        assert_eq!(atomo.comparar_trocar(4000, 1), Ok(4000));
        assert_eq!(atomo.carregar(), 1);
    }

    #[test]
    fn lista_compartilhada_e_segura_entre_threads() {
        let lista = Valor::Lista(Arc::new(RwLock::new(vec![Valor::Inteiro(1)])));
        let mut memoria = Memoria::nova();
        memoria.declarar("itens", lista, true).unwrap();
        let mut outra_memoria = memoria.clone();
        let tarefa = std::thread::spawn(move || {
            outra_memoria
                .definir_elemento_lista("itens", 0, Valor::Inteiro(9))
                .unwrap();
        });
        tarefa.join().unwrap();
        assert_eq!(memoria.elemento_lista("itens", 0), Ok(Valor::Inteiro(9)));
    }

    #[test]
    fn cancelamento_cooperativo_e_compartilhado() {
        let (tarefa, token) = Tarefa::iniciar_cancelavel(|token| {
            while !token.cancelado() {
                std::thread::yield_now();
            }
            token.verificar()
        })
        .unwrap();
        token.cancelar();
        assert!(token.cancelado());
        assert_eq!(tarefa.aguardar(), Ok(Err(ErroRuntime::TarefaCancelada)));
    }

    #[test]
    fn tarefas_e_canais_sincronizam_threads() {
        let canal = Canal::novo();
        let emissor = canal.clonar();
        let tarefa = Tarefa::iniciar(move || emissor.enviar(42).unwrap()).unwrap();
        assert_eq!(canal.receber(), Ok(42));
        tarefa.aguardar().unwrap();
    }

    #[test]
    fn captura_panico_de_tarefa() {
        let tarefa = Tarefa::iniciar(|| panic!("falha intencional")).unwrap();
        assert_eq!(tarefa.aguardar(), Err(ErroRuntime::FalhaTarefa));
    }

    #[test]
    fn converte_texto_numerico_com_validacao() {
        assert_eq!(texto_para_inteiro(" -42 "), Ok(-42));
        assert_eq!(texto_para_decimal("3.5"), Ok(3.5));
        assert_eq!(
            texto_para_inteiro("abc"),
            Err(ErroRuntime::TextoNumericoInvalido)
        );
        assert_eq!(
            texto_para_decimal("NaN"),
            Err(ErroRuntime::ResultadoNaoFinito)
        );
        assert_eq!(inteiro_para_texto(i128::MIN), i128::MIN.to_string());
    }

    #[test]
    fn funcoes_matematicas_validam_dominio() {
        assert_eq!(calcular_matematica(FuncaoMatematica::Raiz, 9.0), Ok(3.0));
        assert_eq!(
            calcular_matematica(FuncaoMatematica::Raiz, -1.0),
            Err(ErroRuntime::DominioMatematico)
        );
        assert_eq!(
            calcular_matematica(FuncaoMatematica::Logaritmo, 0.0),
            Err(ErroRuntime::DominioMatematico)
        );
    }

    #[test]
    fn operacoes_decimais_validam_resultados() {
        assert_eq!(operar_decimal(OperacaoDecimal::Soma, 1.5, 2.0), Ok(3.5));
        assert_eq!(
            operar_decimal(OperacaoDecimal::Divisao, 1.0, 0.0),
            Err(ErroRuntime::DivisaoPorZero)
        );
        assert_eq!(
            operar_decimal(OperacaoDecimal::Potencia, -1.0, 0.5),
            Err(ErroRuntime::ResultadoNaoFinito)
        );
    }

    #[test]
    fn oferece_tempo_unix_e_suspensao() {
        assert!(tempo_unix_milisegundos().unwrap() > 0);
        dormir_milisegundos(0);
    }

    #[test]
    fn opera_arquivos_de_texto() {
        let caminho =
            std::env::temp_dir().join(format!("nebula-runtime-{}.txt", std::process::id()));
        let caminho = caminho.to_string_lossy().into_owned();
        escrever_arquivo(&caminho, "Nebula").unwrap();
        assert!(arquivo_existe(&caminho));
        assert_eq!(ler_arquivo(&caminho), Ok("Nebula".into()));
        std::fs::remove_file(&caminho).unwrap();
        assert!(!arquivo_existe(&caminho));
    }

    #[test]
    fn operacoes_de_texto_unicode_sao_deterministicas() {
        assert_eq!(texto_aparar("  olá 🌌  "), "olá 🌌");
        assert!(texto_comeca_com("Nebula", "Neb"));
        assert!(texto_termina_com("Nebula", "ula"));
        assert_eq!(texto_substituir("a-b-a", "a", "x"), "x-b-x");
        assert!(!texto_comeca_com("Nebula", "neb"));
    }

    #[test]
    fn transforma_caixa_de_texto_com_unicode() {
        assert_eq!(texto_maiusculas("ação"), "AÇÃO");
        assert_eq!(texto_maiusculas("straße"), "STRASSE");
        assert_eq!(texto_minusculas("NEBULA"), "nebula");
    }

    #[test]
    fn texto_recorta_por_caracteres_unicode() {
        assert_eq!(subtexto("ação 🌌", 0, 4), Ok("ação".into()));
        assert!(subtexto("ação", 3, 2).is_err());
        assert!(texto_contem("Nebula", "bul"));
    }

    #[test]
    fn rejeita_indice_invalido() {
        let lista = Valor::Lista(Arc::new(RwLock::new(vec![Valor::Inteiro(1)])));
        let mut memoria = Memoria::nova();
        memoria.declarar("itens", lista, true).unwrap();
        assert_eq!(
            memoria.elemento_lista("itens", 2),
            Err(ErroRuntime::IndiceForaDosLimites {
                indice: 2,
                tamanho: 1
            })
        );
    }

    #[test]
    fn texto_conta_caracteres_unicode() {
        assert_eq!(comprimento_texto("ação 🌌"), 6);
        assert_eq!(caractere_texto("ação 🌌", 5), Ok('🌌'));
    }

    #[test]
    fn aritmetica_detecta_overflow_e_divisao_por_zero() {
        assert_eq!(
            operar_inteiro(OperacaoInteira::Soma, i128::MAX, 1),
            Err(ErroRuntime::Overflow)
        );
        assert_eq!(
            operar_inteiro(OperacaoInteira::Divisao, 10, 0),
            Err(ErroRuntime::DivisaoPorZero)
        );
        assert_eq!(operar_inteiro(OperacaoInteira::Potencia, 2, 10), Ok(1024));
    }

    #[test]
    fn lista_em_variavel_constante_nao_pode_ser_alterada() {
        let lista = Valor::Lista(Arc::new(RwLock::new(vec![Valor::Inteiro(1)])));
        let mut memoria = Memoria::nova();
        memoria.declarar("fixa", lista, false).unwrap();
        assert_eq!(
            memoria.definir_elemento_lista("fixa", 0, Valor::Inteiro(2)),
            Err(ErroRuntime::VariavelImutavel("fixa".into()))
        );
    }
}



/// Estado do ciclo de vida de uma instância do runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoRuntime {
    /// Instância aceita operações e novos finalizadores.
    Ativo,
    /// Finalizadores foram executados; a instância não pode ser reativada.
    Finalizado,
}

type AcaoFinalizacao = Box<dyn FnOnce() -> Resultado<()> + Send + 'static>;

/// Contexto de execução que centraliza memória e encerramento determinístico.
///
/// Finalizadores são executados em ordem LIFO, como pilhas de escopo. Mesmo
/// quando um finalizador falha, os demais são executados; o primeiro erro é
/// retornado ao chamador. A finalização é idempotente.
pub struct ContextoRuntime {
    memoria: Memoria,
    finalizadores: Vec<AcaoFinalizacao>,
    estado: EstadoRuntime,
}

impl Default for ContextoRuntime {
    fn default() -> Self {
        Self::novo()
    }
}

impl ContextoRuntime {
    /// Cria um contexto ativo com memória global independente.
    #[must_use]
    pub fn novo() -> Self {
        Self {
            memoria: Memoria::nova(),
            finalizadores: Vec::new(),
            estado: EstadoRuntime::Ativo,
        }
    }

    /// Consulta o estado atual do contexto.
    #[must_use]
    pub fn estado(&self) -> EstadoRuntime {
        self.estado
    }

    /// Obtém acesso compartilhado à memória do contexto.
    #[must_use]
    pub fn memoria(&self) -> &Memoria {
        &self.memoria
    }

    /// Obtém acesso mutável à memória do contexto enquanto ele estiver ativo.
    pub fn memoria_mutavel(&mut self) -> Resultado<&mut Memoria> {
        if self.estado != EstadoRuntime::Ativo {
            return Err(ErroRuntime::RuntimeFinalizado);
        }
        Ok(&mut self.memoria)
    }

    /// Registra uma ação de encerramento, executada em ordem inversa.
    pub fn registrar_finalizador<F>(&mut self, acao: F) -> Resultado<()>
    where
        F: FnOnce() -> Resultado<()> + Send + 'static,
    {
        if self.estado != EstadoRuntime::Ativo {
            return Err(ErroRuntime::RuntimeFinalizado);
        }
        self.finalizadores.push(Box::new(acao));
        Ok(())
    }

    /// Executa todos os finalizadores em ordem LIFO.
    ///
    /// A função continua a limpeza após falhas e retorna o primeiro erro
    /// encontrado. Chamadas subsequentes não repetem ações já executadas.
    pub fn finalizar(&mut self) -> Resultado<()> {
        if self.estado == EstadoRuntime::Finalizado {
            return Ok(());
        }
        self.estado = EstadoRuntime::Finalizado;
        let mut primeiro_erro = None;
        while let Some(finalizador) = self.finalizadores.pop() {
            if let Err(erro) = finalizador() {
                if primeiro_erro.is_none() {
                    primeiro_erro = Some(erro);
                }
            }
        }
        primeiro_erro.map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod testes_ciclo_vida {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn contexto_finaliza_em_ordem_lifo_e_apenas_uma_vez() {
        let ordem = Arc::new(Mutex::new(Vec::new()));
        let mut contexto = ContextoRuntime::novo();
        for valor in 1..=3 {
            let ordem = Arc::clone(&ordem);
            contexto
                .registrar_finalizador(move || {
                    ordem.lock().unwrap().push(valor);
                    Ok(())
                })
                .unwrap();
        }
        contexto.finalizar().unwrap();
        contexto.finalizar().unwrap();
        assert_eq!(*ordem.lock().unwrap(), vec![3, 2, 1]);
        assert_eq!(contexto.estado(), EstadoRuntime::Finalizado);
    }

    #[test]
    fn contexto_continua_finalizacao_apos_erro() {
        let ordem = Arc::new(Mutex::new(Vec::new()));
        let mut contexto = ContextoRuntime::novo();
        let ordem_mais_antiga = Arc::clone(&ordem);
        contexto
            .registrar_finalizador(move || {
                ordem_mais_antiga.lock().unwrap().push(1);
                Ok(())
            })
            .unwrap();
        let ordem_com_falha = Arc::clone(&ordem);
        contexto
            .registrar_finalizador(move || {
                ordem_com_falha.lock().unwrap().push(2);
                Err(ErroRuntime::FalhaTarefa)
            })
            .unwrap();
        let ordem_mais_nova = Arc::clone(&ordem);
        contexto
            .registrar_finalizador(move || {
                ordem_mais_nova.lock().unwrap().push(3);
                Ok(())
            })
            .unwrap();

        assert_eq!(contexto.finalizar(), Err(ErroRuntime::FalhaTarefa));
        assert_eq!(*ordem.lock().unwrap(), vec![3, 2, 1]);
        assert_eq!(contexto.finalizar(), Ok(()));
    }

    #[test]
    fn contexto_rejeita_registro_e_acesso_mutavel_apos_finalizacao() {
        let mut contexto = ContextoRuntime::novo();
        contexto.finalizar().unwrap();
        assert_eq!(
            contexto.registrar_finalizador(|| Ok(())),
            Err(ErroRuntime::RuntimeFinalizado)
        );
        assert!(matches!(
            contexto.memoria_mutavel(),
            Err(ErroRuntime::RuntimeFinalizado)
        ));
    }
}
