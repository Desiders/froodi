use froodi_compile_core::{
    compile, DependencyRequest, Diagnostic, ExecutionKind, Graph, Origin, Registration, RegistrationId, RequestMode, ScopeKey, Target,
    ValueSource,
};

const APP: ScopeKey = ScopeKey {
    priority: 1,
    name: "app",
    skipped_by_default: false,
};
const REQUEST: ScopeKey = ScopeKey {
    priority: 3,
    name: "request",
    skipped_by_default: false,
};

use RequestMode::{Shared, Transient};

fn reg(key: &'static str, scope: ScopeKey, deps: &[(&'static str, RequestMode)]) -> Registration<&'static str> {
    Registration {
        key,
        type_name: key,
        requests: deps
            .iter()
            .map(|&(target, mode)| DependencyRequest {
                target: Target::Key(target),
                mode,
                type_name: target,
            })
            .collect(),
        scope,
        cache_provides: true,
        finalizer: None,
        execution: ExecutionKind::Sync,
        source: ValueSource::Factory,
        replaces: false,
        origin: None,
    }
}

fn graph(registrations: Vec<Registration<&'static str>>) -> Graph<&'static str> {
    let mut graph = Graph::new(vec![REQUEST, APP]);
    for registration in registrations {
        graph.push(registration);
    }
    graph
}

fn ids(raw: &[u32]) -> Vec<RegistrationId> {
    raw.iter().copied().map(RegistrationId).collect()
}

#[test]
fn orders_dependencies_before_dependents() {
    let compiled = compile(graph(vec![
        reg("Service", REQUEST, &[("Repository", Shared)]),
        reg("Repository", REQUEST, &[("Database", Shared)]),
        reg("Database", APP, &[("Config", Shared)]),
        reg("Config", APP, &[]),
    ]))
    .unwrap();

    assert_eq!(compiled.order(), ids(&[3, 2, 1, 0]));
}

#[test]
fn assigns_owning_scope_from_sorted_hierarchy() {
    let compiled = compile(graph(vec![reg("Handler", REQUEST, &[]), reg("Config", APP, &[])])).unwrap();

    assert_eq!(compiled.scopes(), &[APP, REQUEST]);
    assert_eq!(compiled.node(RegistrationId(0)).scope.index(), 1);
    assert_eq!(compiled.node(RegistrationId(1)).scope.index(), 0);
}

#[test]
fn keeps_inject_and_inject_transient_edges_distinct() {
    let compiled = compile(graph(vec![reg("A", APP, &[("B", Shared), ("B", Transient)]), reg("B", APP, &[])])).unwrap();

    let edges: Vec<_> = compiled
        .node(RegistrationId(0))
        .edges
        .iter()
        .map(|edge| (edge.target, edge.mode))
        .collect();
    assert_eq!(edges, vec![(RegistrationId(1), Shared), (RegistrationId(1), Transient)]);
}

#[test]
fn looks_up_registration_by_binding_key() {
    let compiled = compile(graph(vec![reg("A", APP, &[]), reg("B", APP, &[])])).unwrap();

    assert_eq!(compiled.lookup(&"B"), Some(RegistrationId(1)));
    assert_eq!(compiled.lookup(&"C"), None);
}

fn steps(path: &[froodi_compile_core::PathStep]) -> Vec<&'static str> {
    path.iter().map(|step| step.type_name).collect()
}

#[test]
fn reports_missing_binding_with_path_from_root() {
    let err = compile(graph(vec![
        reg("Service", REQUEST, &[("Repository", Shared)]),
        reg("Repository", REQUEST, &[("Database", Shared)]),
        reg("Database", APP, &[("Config", Shared)]),
    ]))
    .unwrap_err();

    match err.0.as_slice() {
        [Diagnostic::MissingBinding { missing, path }] => {
            assert_eq!(*missing, "Config");
            assert_eq!(steps(path), vec!["Service", "Repository", "Database"]);
        }
        other => panic!("unexpected diagnostics: {other:?}"),
    }
}

#[test]
fn renders_missing_binding_as_dependency_tree() {
    let err = compile(graph(vec![
        reg("Service", REQUEST, &[("Repository", Shared)]),
        reg("Repository", REQUEST, &[("Database", Shared)]),
        reg("Database", APP, &[("Config", Shared)]),
    ]))
    .unwrap_err();

    assert_eq!(
        err.to_string(),
        "error: cannot construct Service\n\
         \nService\
         \n└── Repository\
         \n    └── Database\
         \n        └── Config\
         \n            no registration found"
    );
}

#[test]
fn reports_each_duplicated_binding_once() {
    let err = compile(graph(vec![
        reg("A", APP, &[]),
        reg("B", APP, &[]),
        reg("A", REQUEST, &[]),
        reg("A", APP, &[]),
    ]))
    .unwrap_err();

    match err.0.as_slice() {
        [Diagnostic::Duplicate { type_name, origins }] => {
            assert_eq!(*type_name, "A");
            assert_eq!(origins.len(), 3);
        }
        other => panic!("unexpected diagnostics: {other:?}"),
    }
}

#[test]
fn renders_duplicate_with_registration_origins() {
    let origin = |expr, line| {
        Some(Origin {
            expr,
            file: "src/app.rs",
            line,
            column: 9,
        })
    };
    let mut first = reg("Config", APP, &[]);
    first.origin = origin("load_config", 10);
    let mut second = reg("Config", APP, &[]);
    second.origin = origin("instance(config)", 20);

    let err = compile(graph(vec![first, second])).unwrap_err();

    assert_eq!(
        err.to_string(),
        "error: Config is provided by 2 registrations\
         \n  - `load_config` at src/app.rs:10:9\
         \n  - `instance(config)` at src/app.rs:20:9"
    );
}

#[test]
fn reports_one_cycle_per_loop_through_any_request_mode() {
    // Froodi rejects cycles over every dependency, transient ones included (`Registry::detect_cyclic_dependencies`).
    let err = compile(graph(vec![
        reg("A", APP, &[("B", Shared)]),
        reg("B", APP, &[("C", Transient)]),
        reg("C", APP, &[("A", Shared)]),
        reg("D", APP, &[("A", Shared)]),
    ]))
    .unwrap_err();

    match err.0.as_slice() {
        [Diagnostic::Cycle { path }] => assert_eq!(steps(path), vec!["A", "B", "C", "A"]),
        other => panic!("unexpected diagnostics: {other:?}"),
    }
}

#[test]
fn renders_cycle_as_dependency_tree() {
    let err = compile(graph(vec![
        reg("A", APP, &[("B", Shared)]),
        reg("B", APP, &[("C", Shared)]),
        reg("C", APP, &[("A", Shared)]),
    ]))
    .unwrap_err();

    assert_eq!(err.to_string(), "error: dependency cycle\n\nA\n└── B\n    └── C\n        └── A");
}

#[test]
fn reports_dependency_on_narrower_scope() {
    // Froodi: a dependency must live in an equal or wider scope (`Registry::detect_unreachable_scopes`).
    let err = compile(graph(vec![reg("Wide", APP, &[("Narrow", Shared)]), reg("Narrow", REQUEST, &[])])).unwrap_err();

    match err.0.as_slice() {
        [Diagnostic::ScopeViolation {
            dependent,
            dependent_scope,
            dependency,
            dependency_scope,
        }] => {
            assert_eq!((dependent.type_name, *dependent_scope), ("Wide", APP));
            assert_eq!((dependency.type_name, *dependency_scope), ("Narrow", REQUEST));
        }
        other => panic!("unexpected diagnostics: {other:?}"),
    }
}

#[test]
fn allows_dependency_on_wider_or_equal_scope() {
    assert!(compile(graph(vec![
        reg("Narrow", REQUEST, &[("Wide", Shared), ("Peer", Shared)]),
        reg("Wide", APP, &[]),
        reg("Peer", REQUEST, &[])
    ]))
    .is_ok());
}

#[test]
fn renders_scope_violation_with_both_scopes() {
    let err = compile(graph(vec![reg("Wide", APP, &[("Narrow", Shared)]), reg("Narrow", REQUEST, &[])])).unwrap_err();

    assert_eq!(
        err.to_string(),
        "error: Wide (scope app, priority 1) depends on Narrow (scope request, priority 3), which is a narrower scope \
         and can never be resolved from it; a dependency must live in an equal or wider scope"
    );
}

#[test]
fn reports_scope_outside_hierarchy() {
    let step = ScopeKey {
        priority: 5,
        name: "step",
        skipped_by_default: false,
    };
    let err = compile(graph(vec![reg("A", step, &[])])).unwrap_err();

    assert_eq!(
        err.to_string(),
        "error: A is registered in scope step, priority 5, which is not part of the registry's scope hierarchy"
    );
}

#[test]
fn computes_transitive_reachability() {
    let compiled = compile(graph(vec![
        reg("A", APP, &[("B", Shared)]),
        reg("B", APP, &[("C", Transient)]),
        reg("C", APP, &[]),
        reg("D", APP, &[]),
    ]))
    .unwrap();

    assert_eq!(compiled.reachable(RegistrationId(0)).collect::<Vec<_>>(), ids(&[1, 2]));
    assert_eq!(compiled.reachable(RegistrationId(2)).count(), 0);
    assert!(compiled.reaches(RegistrationId(0), RegistrationId(2)));
    assert!(!compiled.reaches(RegistrationId(0), RegistrationId(3)));
}

#[test]
fn carries_registration_metadata_independent_of_scope() {
    let mut uncached = reg("Uncached", APP, &[]);
    uncached.cache_provides = false;
    uncached.finalizer = Some(ExecutionKind::Async);
    uncached.execution = ExecutionKind::Async;
    uncached.source = ValueSource::Instance;

    let compiled = compile(graph(vec![uncached, reg("Cached", APP, &[])])).unwrap();

    let node = compiled.node(RegistrationId(0));
    assert_eq!(node.type_name, "Uncached");
    assert_eq!(node.scope, compiled.node(RegistrationId(1)).scope);
    assert!(!node.cache_provides);
    assert!(compiled.node(RegistrationId(1)).cache_provides);
    assert_eq!(node.finalizer, Some(ExecutionKind::Async));
    assert_eq!(node.execution, ExecutionKind::Async);
    assert_eq!(node.source, ValueSource::Instance);
}

fn with_id_request(mut registration: Registration<&'static str>, target: u32) -> Registration<&'static str> {
    registration.requests.push(DependencyRequest {
        target: Target::Id(RegistrationId(target)),
        mode: Shared,
        type_name: "B",
    });
    registration
}

#[test]
fn accepts_edges_already_resolved_by_rustc() {
    let compiled = compile(graph(vec![reg("B", APP, &[]), with_id_request(reg("A", APP, &[]), 0)])).unwrap();

    assert_eq!(compiled.node(RegistrationId(1)).edges[0].target, RegistrationId(0));
}

#[test]
#[should_panic(expected = "request targets a registration outside the graph")]
fn rejects_frontend_edge_outside_the_graph() {
    let _ = compile(graph(vec![with_id_request(reg("A", APP, &[]), 7)]));
}

#[test]
fn collects_every_diagnostic_in_a_stable_order() {
    let err = compile(graph(vec![
        reg("A", APP, &[("Missing", Shared)]),
        reg("B", APP, &[("C", Shared)]),
        reg("C", APP, &[("B", Shared)]),
        reg("A", APP, &[]),
    ]))
    .unwrap_err();

    let kinds: Vec<_> = err
        .0
        .iter()
        .map(|diagnostic| match diagnostic {
            Diagnostic::Duplicate { .. } => "duplicate",
            Diagnostic::MissingBinding { .. } => "missing",
            Diagnostic::Cycle { .. } => "cycle",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, vec!["duplicate", "missing", "cycle"]);
}

#[test]
fn compiles_the_same_graph_to_the_same_output() {
    let make = || {
        graph(vec![
            reg("A", APP, &[("B", Shared), ("C", Shared)]),
            reg("B", APP, &[("D", Shared)]),
            reg("C", APP, &[("D", Shared)]),
            reg("D", APP, &[]),
        ])
    };

    assert_eq!(compile(make()).unwrap(), compile(make()).unwrap());
    assert_eq!(compile(make()).unwrap().order(), ids(&[3, 1, 2, 0]));
}

#[test]
fn records_custom_resolver_requests_without_resolving_them() {
    let compiled = compile(graph(vec![
        reg("A", APP, &[("MapInject<Update>", RequestMode::Resolver), ("B", Shared)]),
        reg("B", APP, &[]),
    ]))
    .unwrap();

    let edges: Vec<_> = compiled.node(RegistrationId(0)).edges.iter().map(|edge| edge.target).collect();
    assert_eq!(edges, ids(&[1]));
}

#[test]
fn exposes_nodes_in_registration_order() {
    let compiled = compile(graph(vec![reg("A", APP, &[]), reg("B", REQUEST, &[])])).unwrap();

    let names: Vec<_> = compiled.nodes().iter().map(|node| node.type_name).collect();
    assert_eq!(names, vec!["A", "B"]);
}

fn boundary(key: &'static str, scope: ScopeKey) -> Registration<&'static str> {
    let mut registration = reg(key, scope, &[(key, Shared)]);
    registration.source = ValueSource::Runtime;
    registration
}

#[test]
fn links_a_runtime_boundary_to_the_real_provider() {
    let compiled = compile(graph(vec![
        boundary("Plugin", APP),
        reg("Host", APP, &[("Plugin", Shared)]),
        reg("Plugin", APP, &[]),
    ]))
    .unwrap();

    assert_eq!(compiled.node(RegistrationId(0)).edges[0].target, RegistrationId(2));
    assert_eq!(compiled.node(RegistrationId(1)).edges[0].target, RegistrationId(2));
    assert_eq!(compiled.lookup(&"Plugin"), Some(RegistrationId(2)));
}

#[test]
fn reports_a_runtime_boundary_nothing_provides() {
    let err = compile(graph(vec![boundary("Plugin", APP)])).unwrap_err();

    assert!(matches!(err.0.as_slice(), [Diagnostic::MissingBinding { missing: "Plugin", .. }]));
}

fn replacement(key: &'static str, scope: ScopeKey) -> Registration<&'static str> {
    let mut registration = reg(key, scope, &[]);
    registration.replaces = true;
    registration
}

#[test]
fn an_explicit_replacement_takes_over_its_key_and_the_edges_to_it() {
    let compiled = compile(graph(vec![
        reg("Config", APP, &[]),
        with_id_request(reg("Database", APP, &[]), 0),
        replacement("Config", APP),
    ]))
    .unwrap();

    assert_eq!(compiled.lookup(&"Config"), Some(RegistrationId(2)));
    assert_eq!(compiled.node(RegistrationId(1)).edges[0].target, RegistrationId(2));
    assert_eq!(compiled.node(RegistrationId(0)).replaced_by, Some(RegistrationId(2)));
}

#[test]
fn renders_where_each_step_of_a_path_is_registered() {
    let mut a = reg("A", APP, &[("B", Shared)]);
    a.origin = Some(Origin {
        expr: "make_a",
        file: "src/app.rs",
        line: 3,
        column: 17,
    });
    let err = compile(graph(vec![a, reg("B", APP, &[("A", Shared)])])).unwrap_err();

    assert_eq!(
        err.to_string(),
        "error: dependency cycle\n\nA  [`make_a` at src/app.rs:3:17]\n└── B\n    └── A  [`make_a` at src/app.rs:3:17]"
    );
}
