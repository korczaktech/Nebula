//! Coleções tipadas da biblioteca padrão Nebula.
use std::collections::{BTreeMap, VecDeque};

pub use nebula_runtime::{Memoria, Valor};

/// Mapa ordenado de chaves únicas para valores.
#[derive(Debug,Clone,PartialEq,Eq,Default)]
pub struct Mapa<K:Ord,V>(BTreeMap<K,V>);
impl<K:Ord,V> Mapa<K,V>{
    #[must_use] pub fn novo()->Self{Self(BTreeMap::new())}
    pub fn inserir(&mut self,chave:K,valor:V)->Option<V>{self.0.insert(chave,valor)}
    pub fn obter(&self,chave:&K)->Option<&V>{self.0.get(chave)}
    pub fn remover(&mut self,chave:&K)->Option<V>{self.0.remove(chave)}
    #[must_use] pub fn contem(&self,chave:&K)->bool{self.0.contains_key(chave)}
    #[must_use] pub fn comprimento(&self)->usize{self.0.len()}
    #[must_use] pub fn vazio(&self)->bool{self.0.is_empty()}
    pub fn iterar(&self)->impl Iterator<Item=(&K,&V)>{self.0.iter()}
}

/// Fila FIFO: o primeiro elemento inserido é o primeiro removido.
#[derive(Debug,Clone,PartialEq,Eq,Default)]
pub struct Fila<T>(VecDeque<T>);
impl<T> Fila<T>{
    #[must_use] pub fn nova()->Self{Self(VecDeque::new())}
    pub fn enfileirar(&mut self,v:T){self.0.push_back(v)}
    pub fn desenfileirar(&mut self)->Option<T>{self.0.pop_front()}
    pub fn frente(&self)->Option<&T>{self.0.front()}
    #[must_use] pub fn comprimento(&self)->usize{self.0.len()}
    #[must_use] pub fn vazio(&self)->bool{self.0.is_empty()}
}

/// Pilha LIFO: o último elemento inserido é o primeiro removido.
#[derive(Debug,Clone,PartialEq,Eq,Default)]
pub struct Pilha<T>(Vec<T>);
impl<T> Pilha<T>{
    #[must_use] pub fn nova()->Self{Self(Vec::new())}
    pub fn empilhar(&mut self,v:T){self.0.push(v)}
    pub fn desempilhar(&mut self)->Option<T>{self.0.pop()}
    pub fn topo(&self)->Option<&T>{self.0.last()}
    #[must_use] pub fn comprimento(&self)->usize{self.0.len()}
    #[must_use] pub fn vazio(&self)->bool{self.0.is_empty()}
}

#[cfg(test)]
mod testes{
 use super::*;
 #[test] fn mapa_insere_substitui_e_remove(){let mut m=Mapa::novo();assert_eq!(m.inserir("a",1),None);assert_eq!(m.inserir("a",2),Some(1));assert_eq!(m.obter(&"a"),Some(&2));assert_eq!(m.remover(&"a"),Some(2));assert!(m.vazio())}
 #[test] fn fila_respeita_fifo(){let mut f=Fila::nova();f.enfileirar(1);f.enfileirar(2);assert_eq!(f.frente(),Some(&1));assert_eq!(f.desenfileirar(),Some(1));assert_eq!(f.desenfileirar(),Some(2));assert_eq!(f.desenfileirar(),None)}
 #[test] fn pilha_respeita_lifo(){let mut p=Pilha::nova();p.empilhar(1);p.empilhar(2);assert_eq!(p.topo(),Some(&2));assert_eq!(p.desempilhar(),Some(2));assert_eq!(p.desempilhar(),Some(1));assert_eq!(p.desempilhar(),None)}
}
