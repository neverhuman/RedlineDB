use super::BuildParams;
use super::build;
use super::support::{init_random_graph, permutation};

#[test]
fn permutation_is_a_permutation() {
    let p = permutation(100, 42);
    let mut sorted = p.clone();
    sorted.sort();
    assert_eq!(sorted, (0..100).collect::<Vec<_>>());
}

#[test]
fn init_random_graph_no_self_loops() {
    let g = init_random_graph(20, 5, 7);
    for (i, ns) in g.iter().enumerate() {
        assert!(!ns.contains(&(i as u32)));
        assert!(ns.len() <= 5);
    }
}

#[test]
fn empty_input() {
    let (entry, neigh) = build(
        2,
        &[],
        BuildParams {
            max_degree: 4,
            search_list_size: 8,
            alpha: 1.2,
            seed: 1,
        },
    );
    assert_eq!(entry, 0);
    assert!(neigh.is_empty());
}

#[test]
fn small_build_yields_bounded_degree() {
    let vs: Vec<Vec<f32>> = (0..50)
        .map(|i| vec![i as f32 * 0.1, (i as f32 * 0.7).sin()])
        .collect();
    let (_, neigh) = build(
        2,
        &vs,
        BuildParams {
            max_degree: 8,
            search_list_size: 32,
            alpha: 1.2,
            seed: 1,
        },
    );
    for ns in &neigh {
        assert!(ns.len() <= 8, "out-degree exceeded R");
    }
}
