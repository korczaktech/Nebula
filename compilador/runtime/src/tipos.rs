//! Representações tipadas e layout de valores da linguagem Nebula.
//!
//! Este módulo centraliza a validação de valores compostos e o cálculo de
//! layout independente do backend. Tipos dinâmicos usam handles/referências
//! gerenciadas; nenhum layout expõe ponteiros crus à linguagem.

use std::collections::{BTreeMap, HashSet};

/// Tipo estático usado para validar valores e calcular sua representação.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TipoRuntime {
    /// Ausência de retorno.
    Vazio,
    /// Valor lógico de um byte.
    Logico,
    /// Inteiro com largura e sinal definidos.
    Inteiro {
        /// Largura em bits: 8, 16, 32, 64 ou 128.
        bits: u16,
        /// Indica representação com sinal.
        assinado: bool,
    },
    /// Ponto flutuante IEEE-754.
    Decimal {
        /// Largura em bits: 32 ou 64.
        bits: u16,
    },
    /// Inteiro sem sinal de oito bits.
    Byte,
    /// Escalar Unicode.
    Caractere,
    /// Texto UTF-8 gerenciado.
    Texto,
    /// Sequência homogênea gerenciada.
    Lista(Box<TipoRuntime>),
    /// Agregado posicional de tipos possivelmente diferentes.
    Tupla(Vec<TipoRuntime>),
    /// Agregado nomeado com campos em ordem declarada.
    Estrutura {
        /// Nome nominal da estrutura.
        nome: String,
        /// Campos declarados e seus tipos.
        campos: Vec<(String, TipoRuntime)>,
    },
    /// Tipo algébrico com variantes e payloads posicionais.
    Enumeracao {
        /// Nome nominal da enumeração.
        nome: String,
        /// Variantes declaradas.
        variantes: Vec<VarianteRuntime>,
    },
    /// Valor que pode estar presente ou ausente.
    Opcional(Box<TipoRuntime>),
    /// Resultado de sucesso ou falha com tipos independentes.
    Resultado(Box<TipoRuntime>, Box<TipoRuntime>),
    /// Ponteiro de código com assinatura fixa na ABI x86-64 inicial.
    Funcao(Vec<TipoRuntime>, Box<TipoRuntime>),
}

/// Variante de uma enumeração e os tipos de seu payload posicional.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VarianteRuntime {
    /// Identificador da variante.
    pub nome: String,
    /// Tipos dos valores carregados pela variante.
    pub payload: Vec<TipoRuntime>,
}

/// Layout físico calculado para um tipo no ABI inicial de 64 bits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutRuntime {
    /// Tamanho total em bytes, incluindo padding final.
    pub tamanho: u64,
    /// Alinhamento requerido em bytes.
    pub alinhamento: u64,
    /// Layout dos campos, na ordem física.
    pub campos: Vec<CampoLayoutRuntime>,
    /// Tamanho da tag discriminante em bytes.
    pub tamanho_tag: u64,
}

/// Deslocamento e layout de um campo de estrutura ou tupla.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampoLayoutRuntime {
    /// Nome do campo ou índice textual de posição.
    pub nome: String,
    /// Deslocamento do início do campo em relação ao agregado.
    pub deslocamento: u64,
    /// Tamanho do campo em bytes.
    pub tamanho: u64,
    /// Alinhamento do campo em bytes.
    pub alinhamento: u64,
}

/// Valor validável contra um tipo estático da Nebula.
#[derive(Clone, Debug, PartialEq)]
pub enum ValorTipado {
    /// Valor sem conteúdo.
    Vazio,
    /// Valor lógico.
    Logico(bool),
    /// Inteiro com sinal.
    Inteiro(i128),
    /// Inteiro sem sinal.
    InteiroSemSinal(u128),
    /// Número decimal.
    Decimal(f64),
    /// Byte.
    Byte(u8),
    /// Caractere Unicode.
    Caractere(char),
    /// Texto UTF-8.
    Texto(String),
    /// Elementos de uma lista.
    Lista(Vec<ValorTipado>),
    /// Elementos de uma tupla.
    Tupla(Vec<ValorTipado>),
    /// Campos nomeados de uma estrutura.
    Estrutura {
        /// Nome nominal da estrutura.
        nome: String,
        /// Valores indexados pelos nomes dos campos.
        campos: BTreeMap<String, ValorTipado>,
    },
    /// Variante ativa de uma enumeração.
    Enumeracao {
        /// Nome nominal da enumeração.
        nome: String,
        /// Nome da variante ativa.
        variante: String,
        /// Valores carregados pela variante.
        payload: Vec<ValorTipado>,
    },
    /// Valor opcional presente ou ausente.
    Opcional(Option<Box<ValorTipado>>),
    /// Resultado bem-sucedido.
    Sucesso(Box<ValorTipado>),
    /// Resultado com falha.
    Falha(Box<ValorTipado>),
}

/// Valida recursivamente a declaração de um tipo antes de usá-lo no runtime.
///
/// Além das larguras numéricas, verifica nomes e unicidade dos campos e
/// variantes. A validação não depende de o tipo já possuir algum valor.
const LIMITE_PROFUNDIDADE_TIPO: usize = 256;

/// Valida recursivamente a declaração de um tipo antes de usá-lo no runtime.
///
/// Além das larguras numéricas, verifica nomes e unicidade dos campos e
/// variantes. A profundidade é limitada para rejeitar tipos adversariais sem
/// permitir estouro de pilha durante validação ou cálculo de layout.
pub fn validar_tipo(tipo: &TipoRuntime) -> Result<(), String> {
    validar_tipo_interno(tipo, 0)
}

fn validar_tipo_interno(tipo: &TipoRuntime, profundidade: usize) -> Result<(), String> {
    if profundidade >= LIMITE_PROFUNDIDADE_TIPO {
        return Err(format!(
            "Tipo excede o limite de profundidade ({LIMITE_PROFUNDIDADE_TIPO})."
        ));
    }
    let proxima = profundidade + 1;
    match tipo {
        TipoRuntime::Inteiro { bits, .. } if !matches!(bits, 8 | 16 | 32 | 64 | 128) => {
            Err(format!("Largura inteira inválida: {bits}."))
        }
        TipoRuntime::Decimal { bits } if !matches!(bits, 32 | 64) => {
            Err(format!("Largura decimal inválida: {bits}."))
        }
        TipoRuntime::Lista(elemento) | TipoRuntime::Opcional(elemento) => {
            validar_tipo_interno(elemento, proxima)
        }
        TipoRuntime::Tupla(tipos) => {
            for tipo in tipos {
                validar_tipo_interno(tipo, proxima)?;
            }
            Ok(())
        }
        TipoRuntime::Estrutura { nome, campos } => {
            if nome.trim().is_empty() {
                return Err("Estrutura precisa ter um nome não vazio.".into());
            }
            let mut nomes = HashSet::new();
            for (campo, tipo) in campos {
                if campo.trim().is_empty() {
                    return Err(format!("A estrutura '{nome}' possui campo sem nome."));
                }
                if !nomes.insert(campo) {
                    return Err(format!(
                        "A estrutura '{nome}' possui campos duplicados: {campo}."
                    ));
                }
                validar_tipo_interno(tipo, proxima)?;
            }
            Ok(())
        }
        TipoRuntime::Enumeracao { nome, variantes } => {
            if nome.trim().is_empty() {
                return Err("Enumeração precisa ter um nome não vazio.".into());
            }
            if variantes.is_empty() {
                return Err(format!("Enumeração '{nome}' precisa ter variantes."));
            }
            let mut nomes = HashSet::new();
            for variante in variantes {
                if variante.nome.trim().is_empty() {
                    return Err(format!("Enumeração '{nome}' possui variante sem nome."));
                }
                if !nomes.insert(&variante.nome) {
                    return Err(format!(
                        "Variante duplicada em '{nome}': {}.",
                        variante.nome
                    ));
                }
                for tipo in &variante.payload {
                    validar_tipo_interno(tipo, proxima)?;
                }
            }
            Ok(())
        }
        TipoRuntime::Resultado(sucesso, falha) => {
            validar_tipo_interno(sucesso, proxima)?;
            validar_tipo_interno(falha, proxima)
        }
        TipoRuntime::Funcao(parametros, retorno) => {
            for parametro in parametros {
                if matches!(parametro, TipoRuntime::Vazio) {
                    return Err("Parâmetro de função não pode ter tipo vazio.".into());
                }
                validar_tipo_interno(parametro, proxima)?;
            }
            validar_tipo_interno(retorno, proxima)
        }
        _ => Ok(()),
    }
}

/// Calcula o layout de um tipo, rejeitando ciclos por valor e larguras inválidas.
pub fn calcular_layout(tipo: &TipoRuntime) -> Result<LayoutRuntime, String> {
    validar_tipo(tipo)?;
    calcular_layout_interno(tipo, &mut HashSet::new())
}

fn alinhar(valor: u64, alinhamento: u64) -> Result<u64, String> {
    if alinhamento == 0 || !alinhamento.is_power_of_two() {
        return Err("Alinhamento inválido no layout do runtime.".into());
    }
    valor
        .checked_add(alinhamento - 1)
        .map(|v| v & !(alinhamento - 1))
        .ok_or_else(|| "Overflow ao calcular layout do tipo.".into())
}

fn combinar_campos<'a>(
    campos: impl IntoIterator<Item = (String, &'a TipoRuntime)>,
    pilha: &mut HashSet<String>,
) -> Result<LayoutRuntime, String> {
    let mut deslocamento = 0u64;
    let mut alinhamento_maximo = 1u64;
    let mut layouts = Vec::new();
    for (nome, tipo) in campos {
        let layout = calcular_layout_interno(tipo, pilha)?;
        deslocamento = alinhar(deslocamento, layout.alinhamento)?;
        layouts.push(CampoLayoutRuntime {
            nome,
            deslocamento,
            tamanho: layout.tamanho,
            alinhamento: layout.alinhamento,
        });
        deslocamento = deslocamento
            .checked_add(layout.tamanho)
            .ok_or_else(|| "Overflow no tamanho de estrutura/tupla.".to_owned())?;
        alinhamento_maximo = alinhamento_maximo.max(layout.alinhamento);
    }
    let tamanho = alinhar(deslocamento, alinhamento_maximo)?;
    Ok(LayoutRuntime {
        tamanho,
        alinhamento: alinhamento_maximo,
        campos: layouts,
        tamanho_tag: 0,
    })
}

fn calcular_layout_interno(
    tipo: &TipoRuntime,
    pilha: &mut HashSet<String>,
) -> Result<LayoutRuntime, String> {
    let escalar = |tamanho, alinhamento| LayoutRuntime {
        tamanho,
        alinhamento,
        campos: Vec::new(),
        tamanho_tag: 0,
    };
    match tipo {
        TipoRuntime::Vazio => Ok(escalar(0, 1)),
        TipoRuntime::Logico | TipoRuntime::Byte => Ok(escalar(1, 1)),
        TipoRuntime::Inteiro { bits, .. } => {
            if !matches!(bits, 8 | 16 | 32 | 64 | 128) {
                return Err(format!("Largura inteira inválida: {bits}."));
            }
            let tamanho = u64::from(*bits) / 8;
            Ok(escalar(tamanho, tamanho.min(16)))
        }
        TipoRuntime::Decimal { bits } => {
            if !matches!(bits, 32 | 64) {
                return Err(format!("Largura decimal inválida: {bits}."));
            }
            let tamanho = u64::from(*bits) / 8;
            Ok(escalar(tamanho, tamanho))
        }
        TipoRuntime::Caractere => Ok(escalar(4, 4)),
        TipoRuntime::Texto | TipoRuntime::Lista(_) => Ok(escalar(16, 8)),
        TipoRuntime::Funcao(_, _) => Ok(escalar(8, 8)),
        TipoRuntime::Tupla(tipos) => {
            let campos = tipos.iter().enumerate().map(|(i, t)| (i.to_string(), t));
            combinar_campos(campos, pilha)
        }
        TipoRuntime::Estrutura { nome, campos } => {
            let mut nomes_campos = HashSet::new();
            if campos.iter().any(|(campo, _)| !nomes_campos.insert(campo)) {
                return Err(format!("A estrutura '{nome}' possui campos duplicados."));
            }
            if !pilha.insert(nome.clone()) {
                return Err(format!("Tipo recursivo por valor sem indireção: {nome}."));
            }
            let resultado = combinar_campos(campos.iter().map(|(n, t)| (n.clone(), t)), pilha);
            pilha.remove(nome);
            resultado
        }
        TipoRuntime::Opcional(interno) => {
            let payload = calcular_layout_interno(interno, pilha)?;
            let deslocamento = alinhar(1, payload.alinhamento)?;
            let tamanho = alinhar(
                deslocamento
                    .checked_add(payload.tamanho)
                    .ok_or_else(|| "Overflow no layout de Opcional.".to_owned())?,
                payload.alinhamento.max(1),
            )?;
            Ok(LayoutRuntime {
                tamanho,
                alinhamento: payload.alinhamento.max(1),
                campos: vec![CampoLayoutRuntime {
                    nome: "valor".into(),
                    deslocamento,
                    tamanho: payload.tamanho,
                    alinhamento: payload.alinhamento,
                }],
                tamanho_tag: 1,
            })
        }
        TipoRuntime::Resultado(sucesso, falha) => {
            let ok = calcular_layout_interno(sucesso, pilha)?;
            let err = calcular_layout_interno(falha, pilha)?;
            let alinhamento = ok.alinhamento.max(err.alinhamento).max(1);
            let payload = alinhar(ok.tamanho.max(err.tamanho), alinhamento)?;
            let deslocamento = alinhar(1, alinhamento)?;
            let tamanho = alinhar(
                deslocamento
                    .checked_add(payload)
                    .ok_or_else(|| "Overflow no layout de Resultado.".to_owned())?,
                alinhamento,
            )?;
            Ok(LayoutRuntime {
                tamanho,
                alinhamento,
                campos: vec![CampoLayoutRuntime {
                    nome: "payload".into(),
                    deslocamento,
                    tamanho: payload,
                    alinhamento,
                }],
                tamanho_tag: 1,
            })
        }
        TipoRuntime::Enumeracao { nome, variantes } => {
            if variantes.is_empty() {
                return Err(format!("Enumeração '{nome}' precisa ter variantes."));
            }
            if variantes.len() > u32::MAX as usize {
                return Err(format!(
                    "Enumeração '{nome}' excede o limite de variantes do ABI."
                ));
            }
            if !pilha.insert(nome.clone()) {
                return Err(format!("Tipo recursivo por valor sem indireção: {nome}."));
            }
            let resultado = (|| {
                let mut nomes = HashSet::new();
                let mut payload_maximo = 0u64;
                let mut alinhamento_payload = 1u64;
                for variante in variantes {
                    if !nomes.insert(&variante.nome) {
                        return Err(format!(
                            "Variante duplicada em '{nome}': {}.",
                            variante.nome
                        ));
                    }
                    let layout = combinar_campos(
                        variante
                            .payload
                            .iter()
                            .enumerate()
                            .map(|(i, t)| (i.to_string(), t)),
                        pilha,
                    )?;
                    payload_maximo = payload_maximo.max(layout.tamanho);
                    alinhamento_payload = alinhamento_payload.max(layout.alinhamento);
                }
                let tamanho_tag = if variantes.len() <= 256 {
                    1
                } else if variantes.len() <= 65_536 {
                    2
                } else {
                    4
                };
                let alinhamento = alinhamento_payload.max(tamanho_tag);
                let deslocamento = alinhar(tamanho_tag, alinhamento_payload)?;
                let tamanho = alinhar(
                    deslocamento
                        .checked_add(payload_maximo)
                        .ok_or_else(|| "Overflow no layout de enumeração.".to_owned())?,
                    alinhamento,
                )?;
                Ok(LayoutRuntime {
                    tamanho,
                    alinhamento,
                    tamanho_tag,
                    campos: vec![CampoLayoutRuntime {
                        nome: "payload".into(),
                        deslocamento,
                        tamanho: payload_maximo,
                        alinhamento: alinhamento_payload,
                    }],
                })
            })();
            pilha.remove(nome);
            resultado
        }
    }
}

/// Verifica se um valor satisfaz integralmente seu tipo estático.
pub fn validar_valor(tipo: &TipoRuntime, valor: &ValorTipado) -> Result<(), String> {
    validar_tipo(tipo)?;
    match (tipo, valor) {
        (TipoRuntime::Vazio, ValorTipado::Vazio)
        | (TipoRuntime::Logico, ValorTipado::Logico(_))
        | (TipoRuntime::Byte, ValorTipado::Byte(_))
        | (TipoRuntime::Caractere, ValorTipado::Caractere(_))
        | (TipoRuntime::Texto, ValorTipado::Texto(_)) => Ok(()),
        (
            TipoRuntime::Inteiro {
                bits,
                assinado: true,
            },
            ValorTipado::Inteiro(v),
        ) => validar_inteiro_assinado(*bits, *v),
        (
            TipoRuntime::Inteiro {
                bits,
                assinado: false,
            },
            ValorTipado::InteiroSemSinal(v),
        ) => validar_inteiro_sem_sinal(*bits, *v),
        (TipoRuntime::Decimal { bits: 32 }, ValorTipado::Decimal(v))
            if !v.is_finite() || (*v as f32).is_finite() =>
        {
            Ok(())
        }
        (TipoRuntime::Decimal { bits: 64 }, ValorTipado::Decimal(_)) => Ok(()),
        (TipoRuntime::Lista(elemento), ValorTipado::Lista(itens)) => {
            for item in itens {
                validar_valor(elemento, item)?;
            }
            Ok(())
        }
        (TipoRuntime::Tupla(tipos), ValorTipado::Tupla(valores))
            if tipos.len() == valores.len() =>
        {
            for (tipo, valor) in tipos.iter().zip(valores) {
                validar_valor(tipo, valor)?;
            }
            Ok(())
        }
        (
            TipoRuntime::Estrutura { nome, campos },
            ValorTipado::Estrutura {
                nome: recebido,
                campos: valores,
            },
        ) if nome == recebido => {
            if valores.len() != campos.len() {
                return Err(format!(
                    "A estrutura '{nome}' exige {} campo(s), recebeu {}.",
                    campos.len(),
                    valores.len()
                ));
            }
            for (campo, tipo) in campos {
                let valor = valores
                    .get(campo)
                    .ok_or_else(|| format!("Campo obrigatório ausente em '{nome}': {campo}."))?;
                validar_valor(tipo, valor)?;
            }
            if let Some(desconhecido) = valores
                .keys()
                .find(|n| !campos.iter().any(|(c, _)| c == *n))
            {
                return Err(format!("Campo desconhecido em '{nome}': {desconhecido}."));
            }
            Ok(())
        }
        (
            TipoRuntime::Enumeracao { nome, variantes },
            ValorTipado::Enumeracao {
                nome: recebido,
                variante,
                payload,
            },
        ) if nome == recebido => {
            let definicao = variantes
                .iter()
                .find(|v| v.nome == *variante)
                .ok_or_else(|| format!("Variante '{variante}' não existe em '{nome}'."))?;
            if definicao.payload.len() != payload.len() {
                return Err(format!(
                    "Variante '{variante}' espera {} valor(es), recebeu {}.",
                    definicao.payload.len(),
                    payload.len()
                ));
            }
            for (tipo, valor) in definicao.payload.iter().zip(payload) {
                validar_valor(tipo, valor)?;
            }
            Ok(())
        }
        (TipoRuntime::Opcional(interno), ValorTipado::Opcional(Some(valor))) => {
            validar_valor(interno, valor)
        }
        (TipoRuntime::Opcional(_), ValorTipado::Opcional(None)) => Ok(()),
        (TipoRuntime::Resultado(ok, _), ValorTipado::Sucesso(valor)) => validar_valor(ok, valor),
        (TipoRuntime::Resultado(_, err), ValorTipado::Falha(valor)) => validar_valor(err, valor),
        _ => Err(format!(
            "Valor incompatível com o tipo informado: {tipo:?}."
        )),
    }
}

fn validar_inteiro_assinado(bits: u16, valor: i128) -> Result<(), String> {
    if !matches!(bits, 8 | 16 | 32 | 64 | 128) {
        return Err(format!("Largura inteira inválida: {bits}."));
    }
    let valido = bits == 128 || {
        let limite = 1i128 << (bits - 1);
        valor >= -limite && valor < limite
    };
    if valido {
        Ok(())
    } else {
        Err(format!("Inteiro {valor} fora da faixa de {bits} bits."))
    }
}

fn validar_inteiro_sem_sinal(bits: u16, valor: u128) -> Result<(), String> {
    if !matches!(bits, 8 | 16 | 32 | 64 | 128) {
        return Err(format!("Largura inteira inválida: {bits}."));
    }
    let valido = bits == 128 || valor < (1u128 << bits);
    if valido {
        Ok(())
    } else {
        Err(format!(
            "Inteiro sem sinal {valor} fora da faixa de {bits} bits."
        ))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn valida_declaracoes_de_tipos_recursivamente() {
        let tipo = TipoRuntime::Resultado(
            Box::new(TipoRuntime::Opcional(Box::new(TipoRuntime::Inteiro {
                bits: 64,
                assinado: true,
            }))),
            Box::new(TipoRuntime::Lista(Box::new(TipoRuntime::Texto))),
        );
        assert!(validar_tipo(&tipo).is_ok());
    }

    #[test]
    fn rejeita_larguras_invalidas_em_tipos_aninhados() {
        let tipo = TipoRuntime::Lista(Box::new(TipoRuntime::Decimal { bits: 16 }));
        assert!(
            validar_tipo(&tipo)
                .unwrap_err()
                .contains("Largura decimal inválida")
        );
        assert!(calcular_layout(&tipo).is_err());
    }

    #[test]
    fn rejeita_campos_e_variantes_sem_nome_ou_duplicados() {
        let campos_duplicados = TipoRuntime::Estrutura {
            nome: "Pessoa".into(),
            campos: vec![
                ("nome".into(), TipoRuntime::Texto),
                ("nome".into(), TipoRuntime::Texto),
            ],
        };
        assert!(validar_tipo(&campos_duplicados).is_err());

        let campo_sem_nome = TipoRuntime::Estrutura {
            nome: "Pessoa".into(),
            campos: vec![(" ".into(), TipoRuntime::Texto)],
        };
        assert!(validar_tipo(&campo_sem_nome).is_err());

        let variantes_duplicadas = TipoRuntime::Enumeracao {
            nome: "Estado".into(),
            variantes: vec![
                VarianteRuntime {
                    nome: "Ativo".into(),
                    payload: vec![],
                },
                VarianteRuntime {
                    nome: "Ativo".into(),
                    payload: vec![],
                },
            ],
        };
        assert!(validar_tipo(&variantes_duplicadas).is_err());
    }

    #[test]
    fn rejeita_enumeracao_vazia_e_nome_nominal_vazio() {
        assert!(
            validar_tipo(&TipoRuntime::Enumeracao {
                nome: "Estado".into(),
                variantes: vec![],
            })
            .is_err()
        );
        assert!(
            validar_tipo(&TipoRuntime::Estrutura {
                nome: " ".into(),
                campos: vec![],
            })
            .is_err()
        );
    }

    #[test]
    fn rejeita_valor_quando_o_tipo_aninhado_e_invalido() {
        let tipo = TipoRuntime::Opcional(Box::new(TipoRuntime::Inteiro {
            bits: 7,
            assinado: false,
        }));
        let valor = ValorTipado::Opcional(None);
        assert!(validar_valor(&tipo, &valor).is_err());
    }

    #[test]
    fn valida_limites_de_inteiros_assinados_e_sem_sinal() {
        assert!(
            validar_valor(
                &TipoRuntime::Inteiro {
                    bits: 8,
                    assinado: true
                },
                &ValorTipado::Inteiro(-128)
            )
            .is_ok()
        );
        assert!(
            validar_valor(
                &TipoRuntime::Inteiro {
                    bits: 8,
                    assinado: true
                },
                &ValorTipado::Inteiro(128)
            )
            .is_err()
        );
        assert!(
            validar_valor(
                &TipoRuntime::Inteiro {
                    bits: 16,
                    assinado: false
                },
                &ValorTipado::InteiroSemSinal(65_535)
            )
            .is_ok()
        );
        assert!(
            validar_valor(
                &TipoRuntime::Inteiro {
                    bits: 16,
                    assinado: false
                },
                &ValorTipado::InteiroSemSinal(65_536)
            )
            .is_err()
        );
        assert!(
            validar_valor(
                &TipoRuntime::Inteiro {
                    bits: 128,
                    assinado: false
                },
                &ValorTipado::InteiroSemSinal(u128::MAX)
            )
            .is_ok()
        );
    }

    #[test]
    fn valida_estrutura_com_campos_exatos() {
        let tipo = TipoRuntime::Estrutura {
            nome: "Pessoa".into(),
            campos: vec![
                ("nome".into(), TipoRuntime::Texto),
                (
                    "idade".into(),
                    TipoRuntime::Inteiro {
                        bits: 64,
                        assinado: true,
                    },
                ),
            ],
        };
        let valor = ValorTipado::Estrutura {
            nome: "Pessoa".into(),
            campos: BTreeMap::from([
                ("nome".into(), ValorTipado::Texto("Ana".into())),
                ("idade".into(), ValorTipado::Inteiro(30)),
            ]),
        };
        assert!(validar_valor(&tipo, &valor).is_ok());
    }

    #[test]
    fn rejeita_campo_ausente_e_campo_desconhecido() {
        let tipo = TipoRuntime::Estrutura {
            nome: "Ponto".into(),
            campos: vec![(
                "x".into(),
                TipoRuntime::Inteiro {
                    bits: 32,
                    assinado: true,
                },
            )],
        };
        let ausente = ValorTipado::Estrutura {
            nome: "Ponto".into(),
            campos: BTreeMap::new(),
        };
        assert!(validar_valor(&tipo, &ausente).is_err());
        let extra = ValorTipado::Estrutura {
            nome: "Ponto".into(),
            campos: BTreeMap::from([
                ("x".into(), ValorTipado::Inteiro(1)),
                ("y".into(), ValorTipado::Inteiro(2)),
            ]),
        };
        assert!(validar_valor(&tipo, &extra).is_err());
    }

    #[test]
    fn valida_variantes_opcionais_e_resultados() {
        let estado = TipoRuntime::Enumeracao {
            nome: "Estado".into(),
            variantes: vec![
                VarianteRuntime {
                    nome: "Ativo".into(),
                    payload: vec![],
                },
                VarianteRuntime {
                    nome: "Erro".into(),
                    payload: vec![TipoRuntime::Texto],
                },
            ],
        };
        assert!(
            validar_valor(
                &estado,
                &ValorTipado::Enumeracao {
                    nome: "Estado".into(),
                    variante: "Erro".into(),
                    payload: vec![ValorTipado::Texto("falhou".into())]
                }
            )
            .is_ok()
        );
        assert!(
            validar_valor(
                &TipoRuntime::Opcional(Box::new(TipoRuntime::Texto)),
                &ValorTipado::Opcional(None)
            )
            .is_ok()
        );
        assert!(
            validar_valor(
                &TipoRuntime::Resultado(
                    Box::new(TipoRuntime::Texto),
                    Box::new(TipoRuntime::Inteiro {
                        bits: 32,
                        assinado: true
                    })
                ),
                &ValorTipado::Falha(Box::new(ValorTipado::Inteiro(7)))
            )
            .is_ok()
        );
    }

    #[test]
    fn decimais_preservam_valores_especiais_ieee754() {
        for valor in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                validar_valor(
                    &TipoRuntime::Decimal { bits: 32 },
                    &ValorTipado::Decimal(valor)
                )
                .is_ok()
            );
            assert!(
                validar_valor(
                    &TipoRuntime::Decimal { bits: 64 },
                    &ValorTipado::Decimal(valor)
                )
                .is_ok()
            );
        }
    }

    #[test]
    fn decimal32_rejeita_valor_finito_fora_da_faixa() {
        assert!(
            validar_valor(
                &TipoRuntime::Decimal { bits: 32 },
                &ValorTipado::Decimal(f64::MAX)
            )
            .is_err()
        );
    }

    #[test]
    fn calcula_layout_de_tupla_estrutura_opcional_e_resultado() {
        let tipo = TipoRuntime::Estrutura {
            nome: "Ponto".into(),
            campos: vec![
                (
                    "x".into(),
                    TipoRuntime::Inteiro {
                        bits: 8,
                        assinado: true,
                    },
                ),
                (
                    "y".into(),
                    TipoRuntime::Inteiro {
                        bits: 64,
                        assinado: true,
                    },
                ),
            ],
        };
        let layout = calcular_layout(&tipo).unwrap();
        assert_eq!(layout.campos[0].deslocamento, 0);
        assert_eq!(layout.campos[1].deslocamento, 8);
        assert_eq!(layout.alinhamento, 8);
        assert_eq!(layout.tamanho, 16);
        assert!(
            calcular_layout(&TipoRuntime::Opcional(Box::new(TipoRuntime::Inteiro {
                bits: 32,
                assinado: true
            })))
            .unwrap()
            .tamanho
                >= 5
        );
        assert!(
            calcular_layout(&TipoRuntime::Resultado(
                Box::new(TipoRuntime::Texto),
                Box::new(TipoRuntime::Byte)
            ))
            .unwrap()
            .tamanho
                >= 17
        );
        assert_eq!(
            calcular_layout(&TipoRuntime::Tupla(vec![]))
                .unwrap()
                .tamanho,
            0
        );
    }

    #[test]
    fn layout_de_enumeracao_escolhe_tag_pelo_numero_de_variantes() {
        let criar = |quantidade: usize| TipoRuntime::Enumeracao {
            nome: "Estado".into(),
            variantes: (0..quantidade)
                .map(|indice| VarianteRuntime {
                    nome: format!("Variante{indice}"),
                    payload: vec![],
                })
                .collect(),
        };

        let limite_u8 = calcular_layout(&criar(256)).unwrap();
        assert_eq!(limite_u8.tamanho_tag, 1);
        assert_eq!(limite_u8.tamanho, 1);

        let limite_u16 = calcular_layout(&criar(257)).unwrap();
        assert_eq!(limite_u16.tamanho_tag, 2);
        assert_eq!(limite_u16.tamanho, 2);
    }

    #[test]
    fn layout_de_enumeracao_alinha_payload_ao_maior_requisito() {
        let tipo = TipoRuntime::Enumeracao {
            nome: "Evento".into(),
            variantes: vec![
                VarianteRuntime {
                    nome: "Vazio".into(),
                    payload: vec![],
                },
                VarianteRuntime {
                    nome: "Numero".into(),
                    payload: vec![TipoRuntime::Inteiro {
                        bits: 64,
                        assinado: true,
                    }],
                },
            ],
        };

        let layout = calcular_layout(&tipo).unwrap();
        assert_eq!(layout.tamanho_tag, 1);
        assert_eq!(layout.alinhamento, 8);
        assert_eq!(layout.campos[0].deslocamento, 8);
        assert_eq!(layout.campos[0].tamanho, 8);
        assert_eq!(layout.tamanho, 16);
    }

    #[test]
    fn rejeita_tipos_com_profundidade_excessiva() {
        let mut tipo = TipoRuntime::Inteiro {
            bits: 64,
            assinado: true,
        };
        for _ in 0..LIMITE_PROFUNDIDADE_TIPO {
            tipo = TipoRuntime::Opcional(Box::new(tipo));
        }
        let erro = validar_tipo(&tipo).unwrap_err();
        assert!(erro.contains("limite de profundidade"), "{erro}");
        assert!(calcular_layout(&tipo).is_err());
    }

    #[test]
    fn rejeita_recursao_de_enumeracao_por_valor() {
        let tipo = TipoRuntime::Enumeracao {
            nome: "Lista".into(),
            variantes: vec![
                VarianteRuntime {
                    nome: "Fim".into(),
                    payload: vec![],
                },
                VarianteRuntime {
                    nome: "Proximo".into(),
                    payload: vec![TipoRuntime::Enumeracao {
                        nome: "Lista".into(),
                        variantes: vec![VarianteRuntime {
                            nome: "Fim".into(),
                            payload: vec![],
                        }],
                    }],
                },
            ],
        };
        let erro = calcular_layout(&tipo).unwrap_err();
        assert!(erro.contains("recursivo por valor"), "{erro}");
    }

    #[test]
    fn rejeita_recursao_de_estrutura_por_valor() {
        let tipo = TipoRuntime::Estrutura {
            nome: "No".into(),
            campos: vec![(
                "proximo".into(),
                TipoRuntime::Estrutura {
                    nome: "No".into(),
                    campos: vec![],
                },
            )],
        };
        assert!(calcular_layout(&tipo).is_err());
    }
    #[test]
    fn layout_de_funcao_e_ponteiro_de_codigo() {
        let tipo = TipoRuntime::Funcao(
            vec![TipoRuntime::Inteiro {
                bits: 64,
                assinado: true,
            }],
            Box::new(TipoRuntime::Logico),
        );
        let layout = calcular_layout(&tipo).unwrap();
        assert_eq!(layout.tamanho, 8);
        assert_eq!(layout.alinhamento, 8);
    }
}
