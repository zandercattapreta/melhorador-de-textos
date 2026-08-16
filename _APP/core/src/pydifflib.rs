// ==============================================================================
// SCRIPT: pydifflib.rs (txtmelhorator-core)
// DESCRIÇÃO: ratio() do difflib.SequenceMatcher (autojunk=False) em Rust
// CHAMADO POR: cleanup.rs (deduplicação de páginas escaneadas)
// CONTRATO (RESPOSTA ESPERADA): mesmo valor (f64) do CPython para os mesmos inputs
// ==============================================================================

//! Implementação fiel do algoritmo de `difflib` do CPython (sem junk):
//! find_longest_match + get_matching_blocks + ratio. A deduplicação de
//! páginas usa `ratio() >= 0.55`; qualquer divergência numérica poderia
//! remover a página errada de um livro, então a paridade aqui é crítica.

use std::collections::HashMap;

/// ratio() = 2*M / (len(a)+len(b)), M = soma dos blocos casados.
pub fn ratio(a: &str, b: &str) -> f64 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    // b2j: posição de cada char em b (equivalente ao __chain_b sem autojunk).
    let mut b2j: HashMap<char, Vec<usize>> = HashMap::new();
    for (j, &c) in b.iter().enumerate() {
        b2j.entry(c).or_default().push(j);
    }

    let matched: usize = matching_blocks(&a, &b, &b2j).iter().map(|&(_, _, n)| n).sum();
    2.0 * matched as f64 / (a.len() + b.len()) as f64
}

/// Porta de SequenceMatcher.find_longest_match (sem isjunk/autojunk).
/// j2len como Vec indexado por j (mesma semântica do dict do CPython,
/// sem custo de hashing — necessário p/ páginas de milhares de chars).
fn longest_match(
    a: &[char],
    b_len: usize,
    b2j: &HashMap<char, Vec<usize>>,
    alo: usize,
    ahi: usize,
    blo: usize,
    bhi: usize,
) -> (usize, usize, usize) {
    let (mut besti, mut bestj, mut bestsize) = (alo, blo, 0usize);
    // j2len[j]: comprimento da melhor cadeia terminando em j (0 = ausente).
    let mut j2len: Vec<usize> = vec![0; b_len + 1];
    let mut newj2len: Vec<usize> = vec![0; b_len + 1];
    let mut touched: Vec<usize> = Vec::new();
    let mut new_touched: Vec<usize> = Vec::new();
    for i in alo..ahi {
        if let Some(indices) = b2j.get(&a[i]) {
            for &j in indices {
                if j < blo {
                    continue;
                }
                if j >= bhi {
                    break;
                }
                let k = if j > 0 { j2len[j - 1] } else { 0 } + 1;
                newj2len[j] = k;
                new_touched.push(j);
                if k > bestsize {
                    besti = i + 1 - k;
                    bestj = j + 1 - k;
                    bestsize = k;
                }
            }
        }
        // Troca os buffers zerando só as posições tocadas (O(toques)).
        for &j in &touched {
            j2len[j] = 0;
        }
        std::mem::swap(&mut j2len, &mut newj2len);
        std::mem::swap(&mut touched, &mut new_touched);
        new_touched.clear();
    }
    (besti, bestj, bestsize)
}

/// Porta de get_matching_blocks (sem o merge final de blocos adjacentes —
/// o merge não altera a SOMA dos tamanhos, que é o que ratio usa).
fn matching_blocks(
    a: &[char],
    b: &[char],
    b2j: &HashMap<char, Vec<usize>>,
) -> Vec<(usize, usize, usize)> {
    let mut queue = vec![(0usize, a.len(), 0usize, b.len())];
    let mut blocks = Vec::new();
    while let Some((alo, ahi, blo, bhi)) = queue.pop() {
        let (i, j, k) = longest_match(a, b.len(), b2j, alo, ahi, blo, bhi);
        if k > 0 {
            blocks.push((i, j, k));
            if alo < i && blo < j {
                queue.push((alo, i, blo, j));
            }
            if i + k < ahi && j + k < bhi {
                queue.push((i + k, ahi, j + k, bhi));
            }
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    // Valores de referência gerados no CPython 3.12:
    //   SequenceMatcher(None, a, b, autojunk=False).ratio()

    #[test]
    fn identicos() {
        assert_eq!(ratio("abcdef", "abcdef"), 1.0);
    }

    #[test]
    fn disjuntos() {
        assert_eq!(ratio("abc", "xyz"), 0.0);
    }

    #[test]
    fn caso_classico() {
        // CPython: 0.75
        let r = ratio("abcd", "bcde");
        assert!((r - 0.75).abs() < 1e-12, "r={r}");
    }

    #[test]
    fn frase_parecida() {
        // CPython: ratio("uma pagina de teste", "uma pagina de texto") = 0.8947368421052632
        let r = ratio("uma pagina de teste", "uma pagina de texto");
        assert!((r - 0.8947368421052632).abs() < 1e-12, "r={r}");
    }

    #[test]
    fn vazios() {
        assert_eq!(ratio("", ""), 1.0);
        assert_eq!(ratio("a", ""), 0.0);
    }
}
