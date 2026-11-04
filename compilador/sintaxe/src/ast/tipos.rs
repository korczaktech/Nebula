//! Tipos estruturados da AST e conversão de grafias da gramática.

/// Tipo da linguagem representado sem perder sua estrutura interna.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TipoAst {
    Nome(String),
    Caminho(Vec<String>),
    Lista(Box<TipoAst>),
    Generico {
        nome: String,
        argumentos: Vec<TipoAst>,
    },
    Tupla(Vec<TipoAst>),
    Funcao {
        parametros: Vec<TipoAst>,
        retorno: Box<TipoAst>,
    },
    Desconhecido(String),
}

impl TipoAst {
    /// Converte a representação textual produzida pelo parser em uma árvore de tipos.
    pub fn analisar(origem: &str) -> Self {
        let texto = origem.trim();
        if texto.is_empty() {
            return Self::Desconhecido(texto.to_owned());
        }

        if let Some(conteudo) = texto.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            return Self::Lista(Box::new(Self::analisar(conteudo)));
        }

        if let Some(resto) = texto.strip_prefix("funcao(") {
            if let Some(fecho) = encontrar_fecho(resto, ')') {
                let parametros = separar_nivel_superior(&resto[..fecho], ',')
                    .into_iter()
                    .filter(|parte| !parte.trim().is_empty())
                    .map(Self::analisar)
                    .collect();

                if let Some(retorno) = resto[fecho + 1..].trim().strip_prefix(':') {
                    return Self::Funcao {
                        parametros,
                        retorno: Box::new(Self::analisar(retorno)),
                    };
                }
            }
            return Self::Desconhecido(texto.to_owned());
        }

        if texto.starts_with('(') && texto.ends_with(')') {
            let partes = separar_nivel_superior(&texto[1..texto.len() - 1], ',');
            if partes.len() > 1 {
                return Self::Tupla(partes.into_iter().map(Self::analisar).collect());
            }
        }

        if texto.ends_with('>') {
            if let Some(abertura) = encontrar_abertura_generica(texto) {
                let nome = texto[..abertura].trim();
                let interior = &texto[abertura + 1..texto.len() - 1];
                let argumentos = separar_nivel_superior(interior, ',');
                if !nome.is_empty() && !argumentos.is_empty() {
                    return Self::Generico {
                        nome: nome.to_owned(),
                        argumentos: argumentos.into_iter().map(Self::analisar).collect(),
                    };
                }
            }
        }

        if texto.contains("::") {
            let partes: Vec<_> = texto.split("::").map(str::trim).collect();
            if partes.iter().all(|parte| !parte.is_empty()) {
                return Self::Caminho(partes.into_iter().map(str::to_owned).collect());
            }
        }

        if texto.chars().any(char::is_whitespace) {
            return Self::Desconhecido(texto.to_owned());
        }
        Self::Nome(texto.to_owned())
    }

    /// Devolve a grafia canônica do tipo.
    pub fn para_texto(&self) -> String {
        match self {
            Self::Nome(nome) => nome.clone(),
            Self::Caminho(partes) => partes.join("::"),
            Self::Lista(tipo) => format!("[{}]", tipo.para_texto()),
            Self::Generico { nome, argumentos } => format!(
                "{}<{}>",
                nome,
                argumentos
                    .iter()
                    .map(Self::para_texto)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Tupla(tipos) => format!(
                "({})",
                tipos
                    .iter()
                    .map(Self::para_texto)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Funcao {
                parametros,
                retorno,
            } => format!(
                "funcao({}): {}",
                parametros
                    .iter()
                    .map(Self::para_texto)
                    .collect::<Vec<_>>()
                    .join(", "),
                retorno.para_texto()
            ),
            Self::Desconhecido(origem) => origem.clone(),
        }
    }
}

fn encontrar_fecho(texto: &str, fechamento: char) -> Option<usize> {
    let mut profundidade = 1usize;
    for (indice, caractere) in texto.char_indices() {
        if caractere == '(' {
            profundidade += 1;
        }
        if caractere == fechamento {
            profundidade = profundidade.checked_sub(1)?;
            if profundidade == 0 {
                return Some(indice);
            }
        }
    }
    None
}

fn encontrar_abertura_generica(texto: &str) -> Option<usize> {
    let mut profundidade = 0usize;
    for (indice, caractere) in texto.char_indices().rev() {
        match caractere {
            '>' => profundidade += 1,
            '<' => {
                profundidade = profundidade.checked_sub(1)?;
                if profundidade == 0 {
                    return Some(indice);
                }
            }
            _ => {}
        }
    }
    None
}

fn separar_nivel_superior(texto: &str, separador: char) -> Vec<&str> {
    let mut partes = Vec::new();
    let mut inicio = 0usize;
    let (mut angulo, mut parenteses, mut colchetes) = (0usize, 0usize, 0usize);

    for (indice, caractere) in texto.char_indices() {
        match caractere {
            '<' => angulo += 1,
            '>' => angulo = angulo.saturating_sub(1),
            '(' => parenteses += 1,
            ')' => parenteses = parenteses.saturating_sub(1),
            '[' => colchetes += 1,
            ']' => colchetes = colchetes.saturating_sub(1),
            atual
                if atual == separador
                    && angulo == 0
                    && parenteses == 0
                    && colchetes == 0 =>
            {
                partes.push(texto[inicio..indice].trim());
                inicio = indice + atual.len_utf8();
            }
            _ => {}
        }
    }

    partes.push(texto[inicio..].trim());
    partes
}

#[cfg(test)]
mod testes {
    use super::TipoAst;

    #[test]
    fn analisa_tipos_aninhados_sem_dividir_separadores_internos() {
        let tipo = TipoAst::analisar("Resultado<[texto], Tupla<inteiro, logico>>");
        assert_eq!(
            tipo.para_texto(),
            "Resultado<[texto], Tupla<inteiro, logico>>"
        );
    }

    #[test]
    fn analisa_caminhos_listas_tuplas_e_funcoes() {
        assert_eq!(
            TipoAst::analisar("Modulo::Pessoa"),
            TipoAst::Caminho(vec!["Modulo".into(), "Pessoa".into()])
        );
        assert_eq!(
            TipoAst::analisar("[inteiro]"),
            TipoAst::Lista(Box::new(TipoAst::Nome("inteiro".into())))
        );
        assert_eq!(
            TipoAst::analisar("(inteiro, texto)"),
            TipoAst::Tupla(vec![
                TipoAst::Nome("inteiro".into()),
                TipoAst::Nome("texto".into())
            ])
        );
        assert_eq!(
            TipoAst::analisar("funcao(inteiro, texto): logico"),
            TipoAst::Funcao {
                parametros: vec![
                    TipoAst::Nome("inteiro".into()),
                    TipoAst::Nome("texto".into())
                ],
                retorno: Box::new(TipoAst::Nome("logico".into()))
            }
        );
    }
}
