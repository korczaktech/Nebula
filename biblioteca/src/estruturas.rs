#![rustfmt::skip]
//! Estruturas genéricas da biblioteca padrão Nebula.
use nebula_runtime::{Canal, ErroRuntime, Resultado};
use std::sync::{Arc, RwLock};

/// Estado isolado e explicitamente transferível.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Orbita<T>(T);
impl<T> Orbita<T> {
    #[must_use] pub const fn nova(valor:T)->Self{Self(valor)}
    #[must_use] pub const fn estado(&self)->&T{&self.0}
    pub fn operar<R>(&mut self,f:impl FnOnce(&mut T)->R)->R{f(&mut self.0)}
    #[must_use] pub fn finalizar(self)->T{self.0}
}

/// Vetor contíguo com operações verificadas.
#[derive(Debug,Clone,PartialEq,Eq,Default)]
pub struct Vetor<T>(Vec<T>);
impl<T> Vetor<T>{
    #[must_use] pub fn novo(itens:impl IntoIterator<Item=T>)->Self{Self(itens.into_iter().collect())}
    #[must_use] pub fn comprimento(&self)->usize{self.0.len()}
    #[must_use] pub fn vazio(&self)->bool{self.0.is_empty()}
    pub fn adicionar(&mut self,item:T){self.0.push(item)}
    pub fn obter(&self,i:usize)->Resultado<&T>{self.0.get(i).ok_or(ErroRuntime::IndiceForaDosLimites{indice:i,tamanho:self.0.len()})}
    pub fn remover(&mut self,i:usize)->Resultado<T>{if i>=self.0.len(){return Err(ErroRuntime::IndiceForaDosLimites{indice:i,tamanho:self.0.len()})} Ok(self.0.remove(i))}
    #[must_use] pub fn iterar(&self)->impl Iterator<Item=&T>{self.0.iter()}
    #[must_use] pub fn em_vetor(self)->Vec<T>{self.0}
}

/// Matriz retangular indexada por linha e coluna.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct Matriz<T>{linhas:usize,colunas:usize,dados:Vec<T>}
impl<T> Matriz<T>{
    pub fn nova(linhas:usize,colunas:usize,dados:Vec<T>)->Resultado<Self>{
        if linhas.checked_mul(colunas)!=Some(dados.len()){return Err(ErroRuntime::TipoIncompativel("dimensões da matriz não correspondem aos dados"))}
        Ok(Self{linhas,colunas,dados})
    }
    #[must_use] pub const fn dimensoes(&self)->(usize,usize){(self.linhas,self.colunas)}
    pub fn obter(&self,l:usize,c:usize)->Resultado<&T>{
        if l>=self.linhas||c>=self.colunas{return Err(ErroRuntime::IndiceForaDosLimites{indice:l.saturating_mul(self.colunas).saturating_add(c),tamanho:self.dados.len()})}
        Ok(&self.dados[l*self.colunas+c])
    }
    pub fn alterar(&mut self,l:usize,c:usize,v:T)->Resultado<T>{
        if l>=self.linhas||c>=self.colunas{return Err(ErroRuntime::IndiceForaDosLimites{indice:l.saturating_mul(self.colunas).saturating_add(c),tamanho:self.dados.len()})}
        Ok(std::mem::replace(&mut self.dados[l*self.colunas+c],v))
    }
    #[must_use] pub fn em_vetor(self)->Vec<T>{self.dados}
}

/// Pipeline ordenado com map, filter e fold.
#[derive(Debug,Clone,PartialEq,Eq,Default)]
pub struct Fluxo<T>(Vec<T>);
impl<T> Fluxo<T>{
    #[must_use] pub fn novo(itens:impl IntoIterator<Item=T>)->Self{Self(itens.into_iter().collect())}
    #[must_use] pub fn mapear<U>(&self,f:impl FnMut(&T)->U)->Fluxo<U>{Fluxo::novo(self.0.iter().map(f))}
    #[must_use] pub fn filtrar(&self,mut f:impl FnMut(&T)->bool)->Fluxo<T> where T:Clone{Fluxo::novo(self.0.iter().filter(|x|f(x)).cloned())}
    pub fn reduzir<A>(&self,inicial:A,f:impl FnMut(A,&T)->A)->A{self.0.iter().fold(inicial,f)}
    #[must_use] pub fn coletar(self)->Vec<T>{self.0}
}

/// Associação imutável entre uma chave e seu valor.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct Nexo<K,V>{chave:K,valor:V}
impl<K,V>Nexo<K,V>{
    #[must_use] pub const fn novo(chave:K,valor:V)->Self{Self{chave,valor}}
    #[must_use] pub const fn chave(&self)->&K{&self.chave}
    #[must_use] pub const fn valor(&self)->&V{&self.valor}
    #[must_use] pub fn separar(self)->(K,V){(self.chave,self.valor)}
}

/// Handle compartilhado com leitura e escrita sincronizadas.
#[derive(Debug)]
pub struct Elo<T>(Arc<RwLock<T>>);
impl<T>Elo<T>{
    #[must_use] pub fn novo(valor:T)->Self{Self(Arc::new(RwLock::new(valor)))}
    #[must_use] pub fn clonar(&self)->Self{Self(Arc::clone(&self.0))}
    pub fn ler(&self)->Resultado<T> where T:Clone{self.0.read().map(|v|v.clone()).map_err(|_|ErroRuntime::MemoriaCorrompida)}
    pub fn alterar<R>(&self,f:impl FnOnce(&mut T)->R)->Resultado<R>{let mut v=self.0.write().map_err(|_|ErroRuntime::MemoriaCorrompida)?;Ok(f(&mut v))}
    #[must_use] pub fn referencias(&self)->usize{Arc::strong_count(&self.0)}
}

/// Transformação reutilizável e segura entre tipos.
pub struct Prisma<A,B>(Arc<dyn Fn(A)->B+Send+Sync>);
impl<A,B> Clone for Prisma<A,B>{fn clone(&self)->Self{Self(Arc::clone(&self.0))}}
impl<A:'static,B:'static> Prisma<A,B>{
    #[must_use] pub fn novo(f:impl Fn(A)->B+Send+Sync+'static)->Self{Self(Arc::new(f))}
    pub fn aplicar(&self,v:A)->B{(self.0)(v)}
    #[must_use] pub fn compor<C:'static>(self,g:impl Fn(B)->C+Send+Sync+'static)->Prisma<A,C>{Prisma::novo(move|v|g(self.aplicar(v)))}
}

/// Núcleo com ciclo de vida ativo/finalizado.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum EstadoNucleo{Ativo,Finalizado}
#[derive(Debug)]
pub struct Nucleo<T>{estado:EstadoNucleo,recurso:Option<T>}
impl<T>Nucleo<T>{
    #[must_use] pub const fn novo(v:T)->Self{Self{estado:EstadoNucleo::Ativo,recurso:Some(v)}}
    #[must_use] pub const fn estado(&self)->EstadoNucleo{self.estado}
    pub fn acessar<R>(&mut self,f:impl FnOnce(&mut T)->R)->Resultado<R>{if self.estado!=EstadoNucleo::Ativo{return Err(ErroRuntime::RuntimeFinalizado)}Ok(f(self.recurso.as_mut().ok_or(ErroRuntime::RuntimeFinalizado)?))}
    pub fn finalizar(&mut self)->Resultado<T>{if self.estado!=EstadoNucleo::Ativo{return Err(ErroRuntime::RuntimeFinalizado)}self.estado=EstadoNucleo::Finalizado;self.recurso.take().ok_or(ErroRuntime::RuntimeFinalizado)}
}

/// Canal tipado para emitir e aguardar sinais entre tarefas.
pub struct Sinal<T>(Canal<T>);
impl<T> Clone for Sinal<T>{fn clone(&self)->Self{Self(self.0.clonar())}}
impl<T>Sinal<T>{
    #[must_use] pub fn novo()->Self{Self(Canal::novo())}
    pub fn emitir(&self,v:T)->Resultado<()>{self.0.enviar(v)}
    pub fn aguardar(&self)->Resultado<T>{self.0.receber()}
}

#[cfg(test)]
mod testes{
 use super::*;
 #[test] fn vetor_verifica_indices(){let mut v=Vetor::novo([3,4]);assert_eq!(*v.obter(1).unwrap(),4);assert!(v.obter(2).is_err());assert_eq!(v.remover(0).unwrap(),3)}
 #[test] fn matriz_valida_dimensoes_e_acesso(){let mut m=Matriz::nova(2,2,vec![1,2,3,4]).unwrap();assert_eq!(*m.obter(1,0).unwrap(),3);m.alterar(1,0,9).unwrap();assert_eq!(*m.obter(1,0).unwrap(),9);assert!(Matriz::nova(2,2,vec![1]).is_err())}
 #[test] fn fluxo_compoe_operacoes(){let f=Fluxo::novo([1,2,3,4]).mapear(|x|x*2).filtrar(|x|x%4==0);assert_eq!(f.reduzir(0,|a,x|a+x),12);assert_eq!(f.coletar(),vec![4,8])}
 #[test] fn nexo_preserva_associacao(){let n=Nexo::novo("a",7);assert_eq!(n.separar(),("a",7))}
 #[test] fn elo_compartilha_estado(){let a=Elo::novo(1);let b=a.clonar();b.alterar(|v|*v=8).unwrap();assert_eq!(a.ler(),Ok(8));assert_eq!(a.referencias(),2)}
 #[test] fn prisma_encadeia(){let p=Prisma::novo(|x:i32|x+2).compor(|x|x*3);assert_eq!(p.aplicar(4),18)}
 #[test] fn nucleo_finaliza_uma_vez(){let mut n=Nucleo::novo(5);assert_eq!(n.finalizar(),Ok(5));assert_eq!(n.estado(),EstadoNucleo::Finalizado);assert!(n.finalizar().is_err())}
 #[test] fn sinal_transmite_valor(){let s=Sinal::novo();let tx=s.clone();tx.emitir(42).unwrap();assert_eq!(s.aguardar(),Ok(42))}
 #[test] fn orbita_isola_estado(){let mut a=Orbita::nova(2);let b=Orbita::nova(5);a.operar(|x|*x+=1);assert_eq!(a.finalizar(),3);assert_eq!(b.estado(),&5)}
}
