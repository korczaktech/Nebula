//! Backend nativo C++17 da Nebula.
#![forbid(unsafe_code)]
#[rustfmt::skip]
mod implementacao {
use nebula_ir::{ExpressaoIr, FuncaoIr, InstrucaoIr, OperadorIr, ProgramaIr, ValorIr};
use nebula_semantica::Tipo;
use std::collections::HashMap;
use std::fmt::Write;

/// Gera uma unidade C++17 para os recursos suportados do MVP.
pub fn gerar_cpp(programa: &ProgramaIr) -> Result<String, String> {
    if !programa.modulos.is_empty() {
        return Err("O backend nativo ainda não liga módulos externos.".into());
    }
    let assinaturas = programa.funcoes.iter().map(|(nome, f)| {
        (nome.clone(), (f.parametros.iter().map(|(_, t)| t.clone()).collect(), f.retorno.clone()))
    }).collect();
    let mut g = Gerador { codigo: String::new(), nivel: 0, variaveis: HashMap::new(), assinaturas, retorno: None };
    g.l("#include <cstdint>");
    g.l("#include <cmath>");
    g.l("#include <iostream>");
    g.l("#include <string>");
    g.l("#include <vector>");
    g.l("");
    g.l("template <typename T> static void nb_escrever(const T& valor) { std::cout << std::boolalpha << valor; }");
    g.l("template <typename T> static void nb_escrever(const std::vector<T>& valores) { std::cout << '['; for (std::size_t i = 0; i < valores.size(); ++i) { if (i) std::cout << \", \"; nb_escrever(valores[i]); } std::cout << ']'; }");
    g.l("template <typename T> static void nb_exibir(const T& valor) { nb_escrever(valor); std::cout << '\\n'; }");
    g.l("static std::string nb_ler_texto() { std::string valor; std::getline(std::cin, valor); return valor; }");
    g.l("");
    let mut nomes: Vec<_> = programa.funcoes.keys().collect();
    nomes.sort();
    for nome in &nomes {
        let f = &programa.funcoes[*nome];
        g.l(&format!("{} {}({});", tipo_cpp(&f.retorno)?, nome_cpp(nome), parametros_cpp(&f.parametros)?));
    }
    if !nomes.is_empty() { g.l(""); }
    for nome in nomes {
        g.funcao(&programa.funcoes[nome])?;
        g.l("");
    }
    if !programa.instrucoes.is_empty() || programa.funcoes.contains_key("principal") {
        g.l("int main() {");
        g.nivel += 1;
        g.bloco(&programa.instrucoes)?;
        if let Some(principal) = programa.funcoes.get("principal") {
            if principal.retorno == Tipo::Vazio {
                g.l("nb_principal();");
                g.l("return 0;");
            } else {
                g.l("return static_cast<int>(nb_principal());");
            }
        } else {
            g.l("return 0;");
        }
        g.nivel -= 1;
        g.l("}");
    } else {
        return Err("Declare a função principal ou instruções globais.".into());
    }
    Ok(g.codigo)
}

fn tipo_cpp(tipo: &Tipo) -> Result<String, String> {
    match tipo {
        Tipo::Vazio => Ok("void".into()),
        Tipo::Logico => Ok("bool".into()),
        Tipo::Inteiro | Tipo::Inteiro64 => Ok("std::int64_t".into()),
        Tipo::Inteiro8 => Ok("std::int8_t".into()),
        Tipo::Inteiro16 => Ok("std::int16_t".into()),
        Tipo::Inteiro32 => Ok("std::int32_t".into()),
        Tipo::Byte => Ok("std::uint8_t".into()),
        Tipo::Decimal | Tipo::Decimal64 => Ok("double".into()),
        Tipo::Decimal32 => Ok("float".into()),
        Tipo::Texto => Ok("std::string".into()),
        Tipo::Caractere => Ok("char".into()),
        Tipo::Lista(elemento) => Ok(format!("std::vector<{}>", tipo_cpp(elemento)?)),
        Tipo::Inteiro128 => Err("inteiro128 ainda não possui ABI nativa estável.".into()),
        _ => Err(format!("Tipo ainda não suportado pelo backend nativo: {tipo:?}")),
    }
}

fn nome_cpp(nome: &str) -> String {
    let mut saida = String::from("nb_");
    for c in nome.chars() {
        if c.is_ascii_alphanumeric() { saida.push(c); }
        else { let _ = write!(saida, "_u{:x}_", c as u32); }
    }
    saida
}

fn parametros_cpp(parametros: &[(String, Tipo)]) -> Result<String, String> {
    if parametros.is_empty() { return Ok("void".into()); }
    parametros.iter().map(|(nome, tipo)| {
        Ok(format!("{} {}", tipo_cpp(tipo)?, nome_cpp(nome)))
    }).collect::<Result<Vec<_>, String>>().map(|itens| itens.join(", "))
}

struct Gerador {
    codigo: String,
    nivel: usize,
    variaveis: HashMap<String, Tipo>,
    assinaturas: HashMap<String, (Vec<Tipo>, Tipo)>,
    retorno: Option<Tipo>,
}

impl Gerador {
    fn l(&mut self, linha: &str) {
        for _ in 0..self.nivel { self.codigo.push_str("    "); }
        self.codigo.push_str(linha);
        self.codigo.push('\n');
    }

    fn funcao(&mut self, funcao: &FuncaoIr) -> Result<(), String> {
        let variaveis_anteriores = self.variaveis.clone();
        let retorno_anterior = self.retorno.clone();
        self.variaveis.clear();
        for (nome, tipo) in &funcao.parametros { self.variaveis.insert(nome.clone(), tipo.clone()); }
        self.retorno = Some(funcao.retorno.clone());
        self.l(&format!("{} {}({}) {{", tipo_cpp(&funcao.retorno)?, nome_cpp(&funcao.nome), parametros_cpp(&funcao.parametros)?));
        self.nivel += 1;
        self.bloco(&funcao.corpo)?;
        self.nivel -= 1;
        self.l("}");
        self.variaveis = variaveis_anteriores;
        self.retorno = retorno_anterior;
        Ok(())
    }

    fn bloco(&mut self, instrucoes: &[InstrucaoIr]) -> Result<(), String> {
        let variaveis_anteriores = self.variaveis.clone();
        for instrucao in instrucoes { self.instrucao(instrucao)?; }
        self.variaveis = variaveis_anteriores;
        Ok(())
    }

    fn instrucao(&mut self, instrucao: &InstrucaoIr) -> Result<(), String> {
        match instrucao {
            InstrucaoIr::Declarar { nome, tipo, valor, .. } => {
                let valor_cpp = self.expr(valor, Some(tipo))?;
                self.l(&format!("{} {} = {};", tipo_cpp(tipo)?, nome_cpp(nome), valor_cpp));
                self.variaveis.insert(nome.clone(), tipo.clone());
            }
            InstrucaoIr::Expressao(expressao) => {
                let valor = self.expr(expressao, None)?;
                self.l(&format!("{valor};"));
            }
            InstrucaoIr::Se { condicao, entao, senao } => {
                let condicao_cpp = self.expr(condicao, None)?;
                self.l(&format!("if ({condicao_cpp}) {{"));
                self.nivel += 1;
                self.bloco(entao)?;
                self.nivel -= 1;
                if senao.is_empty() { self.l("}"); }
                else {
                    self.l("} else {");
                    self.nivel += 1;
                    self.bloco(senao)?;
                    self.nivel -= 1;
                    self.l("}");
                }
            }
            InstrucaoIr::Enquanto { condicao, corpo } => {
                let condicao_cpp = self.expr(condicao, None)?;
                self.l(&format!("while ({condicao_cpp}) {{"));
                self.nivel += 1;
                self.bloco(corpo)?;
                self.nivel -= 1;
                self.l("}");
            }
            InstrucaoIr::Para { inicial, condicao, atualizacao, corpo } => {
                let variaveis_anteriores = self.variaveis.clone();
                let inicial_cpp = match inicial.as_deref() {
                    Some(InstrucaoIr::Declarar { nome, tipo, valor, .. }) => {
                        let valor_cpp = self.expr(valor, Some(tipo))?;
                        self.variaveis.insert(nome.clone(), tipo.clone());
                        format!("{} {} = {}", tipo_cpp(tipo)?, nome_cpp(nome), valor_cpp)
                    }
                    Some(InstrucaoIr::Expressao(expressao)) => self.expr(expressao, None)?,
                    None => String::new(),
                    _ => return Err("Inicializador de para não suportado.".into()),
                };
                let condicao_cpp = condicao.as_ref().map(|e| self.expr(e, None)).transpose()?.unwrap_or("true".into());
                let atualizacao_cpp = atualizacao.as_ref().map(|e| self.expr(e, None)).transpose()?.unwrap_or_default();
                self.l(&format!("for ({inicial_cpp}; {condicao_cpp}; {atualizacao_cpp}) {{"));
                self.nivel += 1;
                self.bloco(corpo)?;
                self.nivel -= 1;
                self.l("}");
                self.variaveis = variaveis_anteriores;
            }
            InstrucaoIr::Devolver(valor) => {
                let retorno_esperado = self.retorno.clone();
                let valor_cpp = valor.as_ref().map(|e| self.expr(e, retorno_esperado.as_ref())).transpose()?.unwrap_or_default();
                self.l(&format!("return{valor_cpp};"));
            }
            InstrucaoIr::Parar => self.l("break;"),
            InstrucaoIr::Seguir => self.l("continue;"),
        }
        Ok(())
    }

    fn tipo_expr(&self, expressao: &ExpressaoIr) -> Option<Tipo> {
        match expressao {
            ExpressaoIr::Constante(valor) => match valor {
                ValorIr::Inteiro(_) => Some(Tipo::Inteiro),
                ValorIr::Decimal(_) => Some(Tipo::Decimal),
                ValorIr::Logico(_) => Some(Tipo::Logico),
                ValorIr::Texto(_) => Some(Tipo::Texto),
                ValorIr::Caractere(_) => Some(Tipo::Caractere),
                ValorIr::Lista(valores) => valores.first().and_then(|v| self.tipo_valor(v)).map(|t| Tipo::Lista(Box::new(t))),
                _ => None,
            },
            ExpressaoIr::Variavel(nome) => self.variaveis.get(nome).cloned(),
            ExpressaoIr::Lista(valores) => valores.first().and_then(|v| self.tipo_expr(v)).map(|t| Tipo::Lista(Box::new(t))),
            ExpressaoIr::Unaria { valor, .. } => self.tipo_expr(valor),
            ExpressaoIr::Binaria { esquerda, operador, .. } => match operador {
                OperadorIr::Igual | OperadorIr::Diferente | OperadorIr::Menor | OperadorIr::MenorIgual | OperadorIr::Maior | OperadorIr::MaiorIgual | OperadorIr::E | OperadorIr::Ou => Some(Tipo::Logico),
                _ => self.tipo_expr(esquerda),
            },
            ExpressaoIr::Atribuicao { nome, .. } => self.variaveis.get(nome).cloned(),
            ExpressaoIr::Chamada { nome, .. } => self.assinaturas.get(nome).map(|(_, retorno)| retorno.clone()).or_else(|| if nome == "ler_texto" { Some(Tipo::Texto) } else { None }),
            ExpressaoIr::Indexacao { alvo, .. } => match self.tipo_expr(alvo) { Some(Tipo::Lista(elemento)) => Some(*elemento), _ => None },
            ExpressaoIr::ChamadaQualificada { .. } => None,
        }
    }

    fn tipo_valor(&self, valor: &ValorIr) -> Option<Tipo> {
        match valor {
            ValorIr::Inteiro(_) => Some(Tipo::Inteiro),
            ValorIr::Decimal(_) => Some(Tipo::Decimal),
            ValorIr::Logico(_) => Some(Tipo::Logico),
            ValorIr::Texto(_) => Some(Tipo::Texto),
            ValorIr::Caractere(_) => Some(Tipo::Caractere),
            ValorIr::Lista(valores) => valores.first().and_then(|v| self.tipo_valor(v)).map(|t| Tipo::Lista(Box::new(t))),
            _ => None,
        }
    }

    fn expr(&mut self, expressao: &ExpressaoIr, esperado: Option<&Tipo>) -> Result<String, String> {
        match expressao {
            ExpressaoIr::Constante(valor) => self.valor(valor, esperado),
            ExpressaoIr::Variavel(nome) => Ok(nome_cpp(nome)),
            ExpressaoIr::Lista(valores) => {
                let elemento = match esperado {
                    Some(Tipo::Lista(elemento)) => (**elemento).clone(),
                    _ => valores.first().and_then(|v| self.tipo_expr(v)).ok_or("Lista vazia exige tipo esperado.")?,
                };
                let itens = valores.iter().map(|v| self.expr(v, Some(&elemento))).collect::<Result<Vec<_>, _>>()?;
                Ok(format!("std::vector<{}>{{{}}}", tipo_cpp(&elemento)?, itens.join(", ")))
            }
            ExpressaoIr::Unaria { operador, valor } => {
                let op = match operador {
                    OperadorIr::Menos => "-",
                    OperadorIr::Mais => "+",
                    OperadorIr::Nao | OperadorIr::Negacao => "!",
                    _ => return Err("Operador unário não suportado.".into()),
                };
                Ok(format!("({op}{})", self.expr(valor, None)?))
            }
            ExpressaoIr::Binaria { esquerda, operador, direita } => {
                let a = self.expr(esquerda, None)?;
                let b = self.expr(direita, None)?;
                if *operador == OperadorIr::Potencia { return Ok(format!("std::pow({a}, {b})")); }
                let op = match operador {
                    OperadorIr::Mais => "+",
                    OperadorIr::Menos => "-",
                    OperadorIr::Vezes => "*",
                    OperadorIr::Divide => "/",
                    OperadorIr::Resto => "%",
                    OperadorIr::Igual => "==",
                    OperadorIr::Diferente => "!=",
                    OperadorIr::Menor => "<",
                    OperadorIr::MenorIgual => "<=",
                    OperadorIr::Maior => ">",
                    OperadorIr::MaiorIgual => ">=",
                    OperadorIr::E => "&&",
                    OperadorIr::Ou => "||",
                    _ => return Err("Operador binário não suportado.".into()),
                };
                Ok(format!("({a} {op} {b})"))
            }
            ExpressaoIr::Atribuicao { nome, valor } => {
                let tipo = self.variaveis.get(nome).cloned();
                Ok(format!("({} = {})", nome_cpp(nome), self.expr(valor, tipo.as_ref())?))
            }
            ExpressaoIr::Chamada { nome, argumentos } if nome == "exibir" => {
                if argumentos.len() != 1 { return Err("exibir exige exatamente um argumento.".into()); }
                Ok(format!("nb_exibir({})", self.expr(&argumentos[0], None)?))
            }
            ExpressaoIr::Chamada { nome, argumentos } if nome == "ler_texto" => {
                if !argumentos.is_empty() { return Err("ler_texto não aceita argumentos.".into()); }
                Ok("nb_ler_texto()".into())
            }
            ExpressaoIr::Chamada { nome, argumentos } => {
                let assinatura = self.assinaturas.get(nome).cloned();
                let itens = argumentos.iter().enumerate().map(|(i, arg)| {
                    self.expr(arg, assinatura.as_ref().and_then(|(params, _)| params.get(i)))
                }).collect::<Result<Vec<_>, _>>()?;
                Ok(format!("{}({})", nome_cpp(nome), itens.join(", ")))
            }
            ExpressaoIr::Indexacao { alvo, indice } => {
                Ok(format!("({}).at({})", self.expr(alvo, None)?, self.expr(indice, None)?))
            }
            ExpressaoIr::ChamadaQualificada { .. } => Err("Chamadas de módulos externos ainda não são suportadas pelo backend nativo.".into()),
        }
    }

    fn valor(&self, valor: &ValorIr, esperado: Option<&Tipo>) -> Result<String, String> {
        match valor {
            ValorIr::Inteiro(n) => Ok(n.to_string()),
            ValorIr::Decimal(n) if n.is_finite() => Ok(format!("{n:?}")),
            ValorIr::Logico(v) => Ok(v.to_string()),
            ValorIr::Texto(v) => {
                let mut bytes = String::new();
                for byte in v.as_bytes() { let _ = write!(bytes, "\\{:03o}", byte); }
                Ok(format!("std::string(\"{bytes}\")"))
            }
            ValorIr::Caractere(v) if v.is_ascii() => Ok(format!("'{}'", v.escape_default())),
            ValorIr::Caractere(_) => Err("Caractere Unicode não ASCII ainda não possui representação char portável.".into()),
            ValorIr::Lista(valores) => {
                let elemento = match esperado {
                    Some(Tipo::Lista(elemento)) => (**elemento).clone(),
                    _ => valores.first().and_then(|v| self.tipo_valor(v)).ok_or("Lista vazia exige tipo esperado.")?,
                };
                let itens = valores.iter().map(|v| self.valor(v, Some(&elemento))).collect::<Result<Vec<_>, _>>()?;
                Ok(format!("std::vector<{}>{{{}}}", tipo_cpp(&elemento)?, itens.join(", ")))
            }
            _ => Err("Literal ainda não suportado pelo backend nativo.".into()),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    fn programa() -> ProgramaIr {
        ProgramaIr {
            instrucoes: vec![],
            funcoes: HashMap::from([("principal".into(), FuncaoIr {
                nome: "principal".into(), parametros: vec![], retorno: Tipo::Vazio,
                corpo: vec![
                    InstrucaoIr::Declarar { nome: "xs".into(), tipo: Tipo::Lista(Box::new(Tipo::Inteiro)), valor: ExpressaoIr::Lista(vec![ExpressaoIr::Constante(ValorIr::Inteiro(7)), ExpressaoIr::Constante(ValorIr::Inteiro(8))]), constante: false },
                    InstrucaoIr::Expressao(ExpressaoIr::Chamada { nome: "exibir".into(), argumentos: vec![ExpressaoIr::Indexacao { alvo: Box::new(ExpressaoIr::Variavel("xs".into())), indice: Box::new(ExpressaoIr::Constante(ValorIr::Inteiro(0))) }] }),
                ],
            })]),
            modulos: HashMap::new(),
        }
    }
    #[test]
    fn gera_cpp17_com_ponto_de_entrada_e_listas() {
        let cpp = gerar_cpp(&programa()).expect("C++ válido");
        assert!(cpp.contains("int main()"));
        assert!(cpp.contains("std::vector<std::int64_t>"));
        assert!(cpp.contains(".at(0)"));
    }
    #[test]
    fn rejeita_tipo_sem_abi_estavel() {
        assert!(tipo_cpp(&Tipo::Inteiro128).is_err());
    }
}
}
pub use implementacao::gerar_cpp;
