//! Tipos públicos que representam os tokens produzidos pelo Lexer.

use nebula_diagnosticos::Diagnostico;
use nebula_fonte::IntervaloFonte;

/// Categorias de tokens produzidas pelo Lexer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TipoToken {
    Fim, NovaLinha, Identificador, Inteiro, Decimal, Texto, Caractere,
    Variavel, Definir, Mutavel, Alterar, Constante, Funcao, Quando, Senao, Escolha, Avaliar, Caso,
    Para, Cada, Em, Pulso, Enquanto, Durante, Repetir, Devolver,
    Incluir, Como, Verdadeiro, Falso, Nulo, FimBloco, Parar, Encerrar, Seguir,
    E, Ou, Nao, Publico, Privado, Tipo, Estrutura, Enumeracao, ReservadaFutura,
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
