//! Integração entre o sistema de tipos semântico e os layouts do runtime.
//!
//! A conversão usa as declarações reais da AST para resolver estruturas e
//! enumerações nominais. Tipos sem representação física definida são
//! rejeitados explicitamente, em vez de receberem um layout fictício.

use nebula_runtime::{LayoutRuntime, TipoRuntime, VarianteRuntime, calcular_layout};
use nebula_semantica::Tipo;
use nebula_sintaxe::{NoAst, Programa};
use std::collections::HashSet;

/// Converte um tipo semântico em sua representação física do runtime.
///
/// Estruturas e enumerações são resolvidas pelas declarações do programa.
/// Ciclos por valor, parâmetros genéricos não instanciados e tipos sem ABI
/// definido retornam erro.
pub fn converter_tipo(programa: &Programa, tipo: &Tipo) -> Result<TipoRuntime, String> {
    converter_tipo_interno(programa, tipo, &mut HashSet::new())
}

/// Calcula tamanho, alinhamento e deslocamentos para um tipo semântico.
pub fn calcular_layout_tipo(programa: &Programa, tipo: &Tipo) -> Result<LayoutRuntime, String> {
    let representacao = converter_tipo(programa, tipo)?;
    calcular_layout(&representacao)
}

fn converter_tipo_interno(
    programa: &Programa,
    tipo: &Tipo,
    ativos: &mut HashSet<String>,
) -> Result<TipoRuntime, String> {
    let converter =
        |tipo: &Tipo, ativos: &mut HashSet<String>| converter_tipo_interno(programa, tipo, ativos);
    Ok(match tipo {
        Tipo::Vazio => TipoRuntime::Vazio,
        Tipo::Logico => TipoRuntime::Logico,
        Tipo::Inteiro | Tipo::Inteiro64 => TipoRuntime::Inteiro {
            bits: 64,
            assinado: true,
        },
        Tipo::Inteiro8 => TipoRuntime::Inteiro {
            bits: 8,
            assinado: true,
        },
        Tipo::Inteiro16 => TipoRuntime::Inteiro {
            bits: 16,
            assinado: true,
        },
        Tipo::Inteiro32 => TipoRuntime::Inteiro {
            bits: 32,
            assinado: true,
        },
        Tipo::Inteiro128 => TipoRuntime::Inteiro {
            bits: 128,
            assinado: true,
        },
        Tipo::InteiroU8 => TipoRuntime::Inteiro {
            bits: 8,
            assinado: false,
        },
        Tipo::InteiroU16 => TipoRuntime::Inteiro {
            bits: 16,
            assinado: false,
        },
        Tipo::InteiroU32 => TipoRuntime::Inteiro {
            bits: 32,
            assinado: false,
        },
        Tipo::InteiroU64 => TipoRuntime::Inteiro {
            bits: 64,
            assinado: false,
        },
        Tipo::InteiroU128 => TipoRuntime::Inteiro {
            bits: 128,
            assinado: false,
        },
        Tipo::Decimal | Tipo::Decimal64 => TipoRuntime::Decimal { bits: 64 },
        Tipo::Decimal32 => TipoRuntime::Decimal { bits: 32 },
        Tipo::Byte => TipoRuntime::Byte,
        Tipo::Caractere => TipoRuntime::Caractere,
        Tipo::Texto => TipoRuntime::Texto,
        Tipo::Lista(elemento) => TipoRuntime::Lista(Box::new(converter(elemento, ativos)?)),
        Tipo::Tupla(elementos) => TipoRuntime::Tupla(
            elementos
                .iter()
                .map(|t| converter(t, ativos))
                .collect::<Result<_, _>>()?,
        ),
        Tipo::Opcional(elemento) => TipoRuntime::Opcional(Box::new(converter(elemento, ativos)?)),
        Tipo::Resultado(sucesso, falha) => TipoRuntime::Resultado(
            Box::new(converter(sucesso, ativos)?),
            Box::new(converter(falha, ativos)?),
        ),
        Tipo::Intervalo(elemento) => TipoRuntime::Tupla(vec![
            converter(elemento, ativos)?,
            converter(elemento, ativos)?,
        ]),
        Tipo::Estrutura(nome) => {
            if !ativos.insert(nome.clone()) {
                return Err(format!("Tipo recursivo por valor sem indireção: {nome}."));
            }
            let declaracao = programa
                .itens
                .iter()
                .find_map(|item| match item {
                    NoAst::Estrutura {
                        nome: declarado,
                        campos,
                    } if declarado == nome => Some(campos),
                    _ => None,
                })
                .ok_or_else(|| {
                    format!("Declaração da estrutura '{nome}' não encontrada na AST.")
                })?;
            let campos = declaracao
                .iter()
                .map(|(campo, tipo)| {
                    let tipo = resolver_tipo_declarado(programa, tipo, ativos)?;
                    Ok((campo.clone(), tipo))
                })
                .collect::<Result<Vec<_>, String>>()?;
            ativos.remove(nome);
            TipoRuntime::Estrutura {
                nome: nome.clone(),
                campos,
            }
        }
        Tipo::Enumeracao(nome) => {
            if !ativos.insert(nome.clone()) {
                return Err(format!("Tipo recursivo por valor sem indireção: {nome}."));
            }
            let declaracao = programa
                .itens
                .iter()
                .find_map(|item| match item {
                    NoAst::Enumeracao {
                        nome: declarado,
                        variantes,
                    } if declarado == nome => Some(variantes),
                    _ => None,
                })
                .ok_or_else(|| {
                    format!("Declaração da enumeração '{nome}' não encontrada na AST.")
                })?;
            let variantes = declaracao
                .iter()
                .map(|(variante, payload)| {
                    let payload = payload
                        .iter()
                        .map(|tipo| resolver_tipo_declarado(programa, tipo, ativos))
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(VarianteRuntime {
                        nome: variante.clone(),
                        payload,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            ativos.remove(nome);
            TipoRuntime::Enumeracao {
                nome: nome.clone(),
                variantes,
            }
        }
        Tipo::Funcao(parametros, retorno) => TipoRuntime::Funcao(
            parametros
                .iter()
                .map(|tipo| converter(tipo, ativos))
                .collect::<Result<_, _>>()?,
            Box::new(converter(retorno, ativos)?),
        ),
        Tipo::Parametro(nome) => {
            return Err(format!(
                "Parâmetro genérico '{nome}' precisa ser instanciado antes do cálculo de layout."
            ));
        }
        Tipo::Desconhecido => {
            return Err("Não é possível calcular layout de um tipo desconhecido.".into());
        }
    })
}

fn resolver_tipo_declarado(
    programa: &Programa,
    nome: &str,
    ativos: &mut HashSet<String>,
) -> Result<TipoRuntime, String> {
    let tipo = match nome {
        "vazio" => Tipo::Vazio,
        "logico" | "lógico" => Tipo::Logico,
        "inteiro" => Tipo::Inteiro,
        "inteiro8" => Tipo::Inteiro8,
        "inteiro16" => Tipo::Inteiro16,
        "inteiro32" => Tipo::Inteiro32,
        "inteiro64" => Tipo::Inteiro64,
        "inteiro128" => Tipo::Inteiro128,
        "inteiro_u8" => Tipo::InteiroU8,
        "inteiro_u16" => Tipo::InteiroU16,
        "inteiro_u32" => Tipo::InteiroU32,
        "inteiro_u64" => Tipo::InteiroU64,
        "inteiro_u128" => Tipo::InteiroU128,
        "decimal" | "decimal64" => Tipo::Decimal64,
        "decimal32" => Tipo::Decimal32,
        "texto" => Tipo::Texto,
        "caractere" => Tipo::Caractere,
        "byte" => Tipo::Byte,
        _ if nome.starts_with('[') && nome.ends_with(']') => {
            let interno = resolver_tipo_declarado(programa, &nome[1..nome.len() - 1], ativos)?;
            return Ok(TipoRuntime::Lista(Box::new(interno)));
        }
        _ => {
            if let Some(destino) = programa.itens.iter().find_map(|item| match item {
                NoAst::Tipo {
                    nome: declarado,
                    destino,
                } if declarado == nome => Some(destino.as_str()),
                _ => None,
            }) {
                let chave = format!("alias:{nome}");
                if !ativos.insert(chave.clone()) {
                    return Err(format!("Ciclo no alias de tipo '{nome}'."));
                }
                let resultado = resolver_tipo_declarado(programa, destino, ativos);
                ativos.remove(&chave);
                return resultado;
            }
            programa
                .itens
                .iter()
                .find_map(|item| match item {
                    NoAst::Estrutura {
                        nome: declarado, ..
                    } if declarado == nome => Some(Tipo::Estrutura(nome.into())),
                    NoAst::Enumeracao {
                        nome: declarado, ..
                    } if declarado == nome => Some(Tipo::Enumeracao(nome.into())),
                    _ => None,
                })
                .ok_or_else(|| format!("Tipo '{nome}' não possui declaração física conhecida."))?
        }
    };
    converter_tipo_interno(programa, &tipo, ativos)
}

#[cfg(test)]
mod testes {
    use super::*;
    use nebula_fonte::ArquivoFonte;
    use nebula_sintaxe::analisar;

    fn programa(codigo: &str) -> Programa {
        let fonte = ArquivoFonte::novo("layout.nbx", codigo);
        let resultado = analisar(&fonte);
        assert!(
            resultado.diagnosticos.is_empty(),
            "{:?}",
            resultado.diagnosticos
        );
        resultado.programa
    }

    #[test]
    fn converte_tipos_primitivos_e_algebricos() {
        let ast = programa("");
        let tipo = Tipo::Resultado(
            Box::new(Tipo::Opcional(Box::new(Tipo::InteiroU32))),
            Box::new(Tipo::Lista(Box::new(Tipo::Texto))),
        );
        let runtime = converter_tipo(&ast, &tipo).unwrap();
        assert!(matches!(runtime, TipoRuntime::Resultado(_, _)));
        assert!(calcular_layout_tipo(&ast, &tipo).unwrap().tamanho > 0);
    }

    #[test]
    fn calcula_layout_de_estrutura_declarada() {
        let ast = programa("estrutura Pessoa { idade: inteiro8, ativo: logico, saldo: decimal64 }");
        let tipo = Tipo::Estrutura("Pessoa".into());
        let layout = calcular_layout_tipo(&ast, &tipo).unwrap();
        assert_eq!(layout.alinhamento, 8);
        assert_eq!(
            layout
                .campos
                .iter()
                .map(|c| c.deslocamento)
                .collect::<Vec<_>>(),
            vec![0, 1, 8]
        );
        assert_eq!(layout.tamanho, 16);
    }

    #[test]
    fn calcula_layout_de_enum_declarada() {
        let ast = programa("enumeracao Estado { Inativo, Ativo, Falha(texto) }");
        let tipo = Tipo::Enumeracao("Estado".into());
        let layout = calcular_layout_tipo(&ast, &tipo).unwrap();
        assert_eq!(layout.tamanho_tag, 1);
        assert_eq!(layout.alinhamento, 8);
        assert_eq!(layout.tamanho, 24);
    }

    #[test]
    fn rejeita_tipo_nominal_sem_declaracao_na_ast() {
        let ast = programa("");
        assert!(calcular_layout_tipo(&ast, &Tipo::Estrutura("Ausente".into())).is_err());
    }

    #[test]
    fn calcula_layout_de_tipo_de_funcao() {
        let ast = programa("");
        let tipo = Tipo::Funcao(vec![Tipo::Inteiro], Box::new(Tipo::Logico));
        let layout = calcular_layout_tipo(&ast, &tipo).unwrap();
        assert_eq!(layout.tamanho, 8);
        assert_eq!(layout.alinhamento, 8);
    }

}
