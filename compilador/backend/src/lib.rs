//! Geração de código C11 a partir da IR da Nebula.
#![forbid(unsafe_code)]
#[rustfmt::skip]
mod implementacao {
use nebula_ir::{ExpressaoIr, FuncaoIr, InstrucaoIr, OperadorIr, ProgramaIr, ValorIr};
use nebula_semantica::Tipo;
use std::fmt::Write;

/// Gera código C11 para o subconjunto nativo suportado.
pub fn gerar_c(p: &ProgramaIr) -> Result<String, String> {
    if !p.modulos.is_empty() { return Err("O backend ainda não liga módulos externos.".into()); }
    let mut g = Gerador { s: String::new(), nivel: 0 };
    g.l("#include <stdbool.h>"); g.l("#include <stdint.h>"); g.l("#include <stdio.h>"); g.l(r#"#define nb_exibir(v) _Generic((v), char*: printf("%s\n", (v)), const char*: printf("%s\n", (v)), float: printf("%g\n", (double)(v)), double: printf("%g\n", (double)(v)), char: printf("%c\n", (int)(v)), default: printf("%lld\n", (long long)(v)))"#); g.l("");
    let mut nomes: Vec<_> = p.funcoes.keys().collect(); nomes.sort();
    for n in &nomes { let f=&p.funcoes[*n]; g.l(&format!("{} {}({});", tc(&f.retorno)?, nc(n), pc(&f.parametros)?)); }
    if !nomes.is_empty() { g.l(""); }
    for n in nomes { g.funcao(&p.funcoes[n])?; g.l(""); }
    if !p.instrucoes.is_empty() || p.funcoes.contains_key("principal") {
        g.l("int main(void) {");
        g.nivel += 1;
        g.bloco(&p.instrucoes)?;
        if let Some(principal) = p.funcoes.get("principal") {
            if principal.retorno == Tipo::Vazio {
                g.l("nb_principal();");
                g.l("return 0;");
            } else {
                g.l("return (int)nb_principal();");
            }
        } else {
            g.l("return 0;");
        }
        g.nivel -= 1;
        g.l("}");
    } else {
        return Err("Declare a função principal ou instruções globais.".into());
    }
    Ok(g.s)
}
fn tc(t:&Tipo)->Result<&'static str,String>{match t{
 Tipo::Vazio=>Ok("void"),Tipo::Logico=>Ok("bool"),Tipo::Inteiro|Tipo::Inteiro64=>Ok("int64_t"),
 Tipo::Inteiro8=>Ok("int8_t"),Tipo::Inteiro16=>Ok("int16_t"),Tipo::Inteiro32=>Ok("int32_t"),
 Tipo::Byte=>Ok("uint8_t"),Tipo::Decimal|Tipo::Decimal64=>Ok("double"),Tipo::Decimal32=>Ok("float"),
 Tipo::Texto=>Ok("const char *"),Tipo::Caractere=>Ok("char"),
 _=>Err(format!("Tipo não suportado no backend nativo: {t:?}"))}}
fn nc(n:&str)->String{let mut s="nb_".to_owned();for c in n.chars(){if c.is_ascii_alphanumeric(){s.push(c)}else{let _=write!(s,"_u{:x}_",c as u32);}}s}
fn pc(p:&[(String,Tipo)])->Result<String,String>{if p.is_empty(){return Ok("void".into())}p.iter().map(|(n,t)|Ok(format!("{} {}",tc(t)?,nc(n)))).collect::<Result<Vec<_>,_>>().map(|v|v.join(", "))}
struct Gerador{s:String,nivel:usize}
impl Gerador{
 fn l(&mut self,x:&str){for _ in 0..self.nivel{self.s.push_str("    ")}self.s.push_str(x);self.s.push('\n')}
 fn funcao(&mut self,f:&FuncaoIr)->Result<(),String>{let nome=nc(&f.nome);let ret=tc(&f.retorno)?;self.l(&format!("{ret} {nome}({}) {{",pc(&f.parametros)?));self.nivel+=1;self.bloco(&f.corpo)?;self.nivel-=1;self.l("}");Ok(())}
 fn bloco(&mut self,v:&[InstrucaoIr])->Result<(),String>{for i in v{self.inst(i)?}Ok(())}
 fn inst(&mut self, i: &InstrucaoIr) -> Result<(), String> {
    match i {
        InstrucaoIr::Declarar { nome, tipo, valor, .. } => {
            let valor = self.e(valor)?;
            self.l(&format!("{} {} = {};", tc(tipo)?, nc(nome), valor));
        }
        InstrucaoIr::Expressao(e) => {
            let valor = self.e(e)?;
            self.l(&format!("{valor};"));
        }
        InstrucaoIr::Se { condicao, entao, senao } => {
            let cond = self.e(condicao)?;
            self.l(&format!("if ({cond}) {{"));
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
            let cond = self.e(condicao)?;
            self.l(&format!("while ({cond}) {{"));
            self.nivel += 1;
            self.bloco(corpo)?;
            self.nivel -= 1;
            self.l("}");
        }
        InstrucaoIr::Para { inicial, condicao, atualizacao, corpo } => {
            let init = match inicial.as_deref() {
                Some(InstrucaoIr::Declarar { nome, tipo, valor, .. }) =>
                    format!("{} {} = {}", tc(tipo)?, nc(nome), self.e(valor)?),
                Some(InstrucaoIr::Expressao(e)) => self.e(e)?,
                None => String::new(),
                _ => return Err("Inicializador de para não suportado.".into()),
            };
            let cond = condicao.as_ref().map(|e| self.e(e)).transpose()?.unwrap_or("true".into());
            let update = atualizacao.as_ref().map(|e| self.e(e)).transpose()?.unwrap_or_default();
            self.l(&format!("for ({init}; {cond}; {update}) {{"));
            self.nivel += 1;
            self.bloco(corpo)?;
            self.nivel -= 1;
            self.l("}");
        }
        InstrucaoIr::Devolver(v) => {
            let valor = v.as_ref().map(|e| self.e(e).map(|s| format!(" {s}"))).transpose()?.unwrap_or_default();
            self.l(&format!("return{valor};"));
        }
        InstrucaoIr::Parar => self.l("break;"),
        InstrucaoIr::Seguir => self.l("continue;"),
    }
    Ok(())
 }
 #[allow(clippy::only_used_in_recursion)]
 fn e(&mut self,e:&ExpressaoIr)->Result<String,String>{match e{
 ExpressaoIr::Constante(v)=>match v{ValorIr::Inteiro(n)=>Ok(n.to_string()),ValorIr::Decimal(n)if n.is_finite()=>Ok(format!("{n:?}")),ValorIr::Logico(b)=>Ok(b.to_string()),ValorIr::Texto(s)=>{let mut bytes=String::new();for byte in s.as_bytes(){let _=write!(bytes,"\\{:03o}",byte);}Ok(format!("\"{bytes}\""))},ValorIr::Caractere(c) if c.is_ascii()=>Ok(format!("'{}'",c.escape_default())),ValorIr::Caractere(_)=>Err("Caractere Unicode não ASCII ainda não possui representação char portável no backend C11.".into()),_=>Err("Literal não suportado pelo backend.".into())},
 ExpressaoIr::Variavel(n)=>Ok(nc(n)),
 ExpressaoIr::Lista(_)|ExpressaoIr::Indexacao{..}=>Err("Listas e indexação não suportadas pelo backend nativo inicial.".into()),
 ExpressaoIr::Unaria{operador,valor}=>{let o=match operador{OperadorIr::Menos=>"-",OperadorIr::Mais=>"+",OperadorIr::Nao|OperadorIr::Negacao=>"!",_=>return Err("Operador unário não suportado.".into())};Ok(format!("({o}{})",self.e(valor)?))},
 ExpressaoIr::Binaria{esquerda,operador,direita}=>{let a=self.e(esquerda)?;let b=self.e(direita)?;let o=match operador{OperadorIr::Mais=>"+",OperadorIr::Menos=>"-",OperadorIr::Vezes=>"*",OperadorIr::Divide=>"/",OperadorIr::Resto=>"%",OperadorIr::Igual=>"==",OperadorIr::Diferente=>"!=",OperadorIr::Menor=>"<",OperadorIr::MenorIgual=>"<=",OperadorIr::Maior=>">",OperadorIr::MaiorIgual=>">=",OperadorIr::E=>"&&",OperadorIr::Ou=>"||",_=>return Err("Operador binário não suportado.".into())};Ok(format!("({a} {o} {b})"))},
 ExpressaoIr::Atribuicao{nome,valor}=>Ok(format!("({} = {})",nc(nome),self.e(valor)?)),
 ExpressaoIr::Chamada{nome,argumentos}if nome=="exibir"=>{if argumentos.len()!=1{return Err("exibir exige um argumento.".into())}let a=self.e(&argumentos[0])?;Ok(format!("nb_exibir({a})"))},
 ExpressaoIr::Chamada{nome,argumentos}=>Ok(format!("{}({})",nc(nome),argumentos.iter().map(|a|self.e(a)).collect::<Result<Vec<_>,_>>()?.join(", "))),
 ExpressaoIr::ChamadaQualificada{..}=>Err("Chamadas de módulos não suportadas pelo backend inicial.".into()),
 }}
}
#[cfg(test)]mod testes{use super::*;use std::collections::HashMap;
fn p()->ProgramaIr{ProgramaIr{instrucoes:vec![],funcoes:HashMap::from([("principal".into(),FuncaoIr{nome:"principal".into(),parametros:vec![],retorno:Tipo::Vazio,corpo:vec![InstrucaoIr::Declarar{nome:"x".into(),tipo:Tipo::Inteiro,valor:ExpressaoIr::Constante(ValorIr::Inteiro(7)),constante:false},InstrucaoIr::Expressao(ExpressaoIr::Chamada{nome:"exibir".into(),argumentos:vec![ExpressaoIr::Variavel("x".into())]})]})]),modulos:HashMap::new()}}
#[test]fn gera_c11_e_ponto_de_entrada(){let c=gerar_c(&p()).unwrap();assert!(c.contains("int main(void)"));
assert!(c.contains("nb_principal"));assert!(c.contains("printf"));assert!(c.contains("int64_t nb_x = 7;"));}
#[test]fn rejeita_tipo_sem_representacao(){assert!(tc(&Tipo::Inteiro128).is_err());}
}

}
pub use implementacao::*;
