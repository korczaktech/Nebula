//! Representação semântica dos operadores sintáticos.
use crate::lexer::TipoToken;
/// Operadores reconhecidos pela gramática da Nebula.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperadorAst {
    Atribuicao, Igual, Diferente, Menor, MenorIgual, Maior, MaiorIgual,
    Adicao, Subtracao, Multiplicacao, Divisao, Resto, Potencia, E, Ou,
    Negacao, Intervalo, AcessoMembro, AcessoEscopo,
}
impl TryFrom<TipoToken> for OperadorAst {
    type Error = TipoToken;
    fn try_from(token: TipoToken) -> Result<Self, Self::Error> {
        match token {
            TipoToken::Atribui => Ok(Self::Atribuicao), TipoToken::Igual => Ok(Self::Igual),
            TipoToken::Diferente => Ok(Self::Diferente), TipoToken::Menor => Ok(Self::Menor),
            TipoToken::MenorIgual => Ok(Self::MenorIgual), TipoToken::Maior => Ok(Self::Maior),
            TipoToken::MaiorIgual => Ok(Self::MaiorIgual), TipoToken::Mais => Ok(Self::Adicao),
            TipoToken::Menos => Ok(Self::Subtracao), TipoToken::Vezes => Ok(Self::Multiplicacao),
            TipoToken::Divide => Ok(Self::Divisao), TipoToken::Resto => Ok(Self::Resto),
            TipoToken::Potencia => Ok(Self::Potencia), TipoToken::E => Ok(Self::E),
            TipoToken::Ou => Ok(Self::Ou), TipoToken::Nao | TipoToken::Negacao => Ok(Self::Negacao),
            TipoToken::Intervalo => Ok(Self::Intervalo), TipoToken::Ponto => Ok(Self::AcessoMembro),
            TipoToken::Escopo => Ok(Self::AcessoEscopo), outro => Err(outro),
        }
    }
}
