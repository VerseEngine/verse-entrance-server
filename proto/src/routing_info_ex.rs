use crate::swarm::*;
use itertools::Itertools;
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};
use std::iter::FromIterator;

const INITIAL_TTL: u32 = 6;

pub trait RoutingInfoEx {
    fn get_min_depth(&self, session_id: impl AsRef<[u8]>) -> Option<u32>;
    fn is_known(&self, session_id: impl AsRef<[u8]>) -> bool;
    fn add_known(&self, session_id: impl AsRef<[u8]>) -> Self;
    fn to_send_data(&self, depth: u32) -> Self;
    fn get_relation_count(&self) -> usize;
    fn get_relations(&self) -> Option<&[RoutingInfo]>;
    fn set_relations(&mut self, relations: Vec<RoutingInfo>);

    fn fold<F, B>(&self, init: B, f: F) -> B
    where
        F: Fn(B, &RoutingInfo) -> B;
}
fn _to_send_data(ri: &RoutingInfo, depth: u32) -> RoutingInfo {
    let mut res = ri.clone();
    res.known_gateway_session_ids.clear();
    if depth == 0 {
        res.set_count(ri.get_relation_count() as u32);
    } else if let Some(routing_info::Relation::RoutingInfos(ris)) = res.relation {
        res.relation = Some(routing_info::Relation::RoutingInfos(RoutingInfos {
            infos: ris
                .infos
                .iter()
                .filter(|v| v.node_type() == NodeType::Normal || v.node_type() == NodeType::Gateway)
                .map(|v| _to_send_data(v, depth - 1))
                .collect(),
        }));
    }
    res
}

pub trait IntoRoutingInfoRecursiveIterator {
    fn recursive_iter_with_depth(&self) -> RoutingInfoRecursiveIteratorWithDepth;

    fn recursive_iter(&self) -> RoutingInfoRecursiveIterator {
        RoutingInfoRecursiveIterator::new(self.recursive_iter_with_depth())
    }

    fn get_known_gateway_session_ids(&self) -> Vec<SessionIdWithTtl> {
        let mut exists = HashMap::<Vec<u8>, SessionIdWithTtl>::new();
        let all = self.recursive_iter().collect::<Vec<_>>();

        if let Some(tracker_ri) = all.iter().find(|v| v.node_type() == NodeType::Tracker) {
            let Some(gateways) = tracker_ri.get_relations() else {
                warn!("unreachable");
                return vec![];
            };

            return gateways
                .iter()
                .filter_map(|v| {
                    v.session_id.as_ref().map(|session_id| SessionIdWithTtl {
                        session_id: session_id.clone(),
                        ttl: INITIAL_TTL,
                    })
                })
                .collect();
        };

        for r in all.iter() {
            for o in r.known_gateway_session_ids.iter() {
                match exists.entry(o.session_id.clone()) {
                    // 到達不能なNodeは最大値-1を採用
                    Entry::Occupied(ref mut v) => {
                        if v.get().ttl < o.ttl {
                            v.get_mut().ttl = o.ttl - 1;
                        }
                    }
                    Entry::Vacant(v) => {
                        v.insert(SessionIdWithTtl {
                            session_id: o.session_id.clone(),
                            ttl: o.ttl - 1,
                        });
                    }
                }
            }
        }

        // 実際のNodeTypeを反映
        for r in all.iter() {
            let Some(ref session_id) = r.session_id else {
                continue;
            };
            if r.node_type() == NodeType::Gateway {
                match exists.entry(session_id.clone()) {
                    Entry::Occupied(ref mut v) => {
                        v.get_mut().ttl = INITIAL_TTL;
                    }
                    Entry::Vacant(v) => {
                        v.insert(SessionIdWithTtl {
                            session_id: session_id.clone(),
                            ttl: INITIAL_TTL,
                        });
                    }
                }
            } else {
                exists.remove(session_id);
            }
        }
        exists.into_values().collect()
    }
    fn get_known_gateway_session_ids_next_ttl(&self) -> Vec<SessionIdWithTtl> {
        self.get_known_gateway_session_ids()
            .into_iter()
            .filter(|v| v.ttl > 1)
            .map(|mut v| {
                v.ttl -= 1;
                v
            })
            .collect()
    }
}

impl RoutingInfoEx for RoutingInfo {
    fn get_min_depth(&self, session_id: impl AsRef<[u8]>) -> Option<u32> {
        let session_id = session_id.as_ref();
        self.recursive_iter_with_depth()
            .filter_map(|v| {
                if v.0
                    .session_id
                    .as_ref()
                    .map_or(false, |v| (v as &[u8]).cmp(session_id).is_eq())
                {
                    Some(v.1)
                } else {
                    None
                }
            })
            .sorted_by(|a, b| a.cmp(b))
            .next()
    }
    fn is_known(&self, session_id: impl AsRef<[u8]>) -> bool {
        let session_id = session_id.as_ref();
        self.recursive_iter().any(|v| {
            v.session_id
                .as_ref()
                .map_or(false, |v| (v as &[u8]).cmp(session_id).is_eq())
        })
    }
    fn add_known(&self, session_id: impl AsRef<[u8]>) -> Self {
        let mut infos = if let Some(infos) = self.get_relations() {
            infos.to_vec()
        } else {
            Vec::new()
        };
        infos.push(RoutingInfo {
            node_type: NodeType::Normal as i32,
            session_id: Some(session_id.as_ref().to_vec()),
            ..Default::default()
        });
        let mut res = self.clone();
        res.relation = Some(routing_info::Relation::RoutingInfos(RoutingInfos { infos }));
        res
    }
    fn to_send_data(&self, depth: u32) -> Self {
        _to_send_data(self, depth)
    }
    fn get_relation_count(&self) -> usize {
        match self.relation {
            Some(routing_info::Relation::Count(v)) => v as usize,
            Some(routing_info::Relation::RoutingInfos(ref routing_infos)) => {
                routing_infos.infos.len()
            }
            _ => 0,
        }
    }
    fn get_relations(&self) -> Option<&[RoutingInfo]> {
        if let Some(routing_info::Relation::RoutingInfos(ref routing_infos)) = self.relation {
            Some(&routing_infos.infos)
        } else {
            None
        }
    }
    fn set_relations(&mut self, relations: Vec<RoutingInfo>) {
        self.relation = Some(routing_info::Relation::RoutingInfos(RoutingInfos {
            infos: relations,
        }));
    }

    fn fold<F, B>(&self, init: B, f: F) -> B
    where
        F: Fn(B, &RoutingInfo) -> B,
    {
        self.recursive_iter().fold(init, f)
    }
}
impl IntoRoutingInfoRecursiveIterator for RoutingInfo {
    fn recursive_iter_with_depth(&self) -> RoutingInfoRecursiveIteratorWithDepth {
        RoutingInfoRecursiveIteratorWithDepth::new([self])
    }
}

impl<T: AsRef<RoutingInfo>> IntoRoutingInfoRecursiveIterator for &[T] {
    fn recursive_iter_with_depth(&self) -> RoutingInfoRecursiveIteratorWithDepth {
        RoutingInfoRecursiveIteratorWithDepth::from_iter((*self).iter().map(|v| v.as_ref()))
    }
}

impl<T: AsRef<RoutingInfo>> IntoRoutingInfoRecursiveIterator for Vec<T> {
    fn recursive_iter_with_depth(&self) -> RoutingInfoRecursiveIteratorWithDepth {
        RoutingInfoRecursiveIteratorWithDepth::from_iter((*self).iter().map(|v| v.as_ref()))
    }
}

pub struct RoutingInfoRecursiveIterator<'a> {
    inner: RoutingInfoRecursiveIteratorWithDepth<'a>,
}
impl<'a> RoutingInfoRecursiveIterator<'a> {
    fn new(inner: RoutingInfoRecursiveIteratorWithDepth<'a>) -> Self {
        RoutingInfoRecursiveIterator { inner }
    }
}
impl<'a> Iterator for RoutingInfoRecursiveIterator<'a> {
    type Item = &'a RoutingInfo;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|v| v.0)
    }
}

pub struct RoutingInfoRecursiveIteratorWithDepth<'a> {
    stack: RefCell<VecDeque<(&'a RoutingInfo, u32)>>,
}
impl<'a> RoutingInfoRecursiveIteratorWithDepth<'a> {
    fn new<const N: usize>(root: [&'a RoutingInfo; N]) -> Self {
        RoutingInfoRecursiveIteratorWithDepth {
            stack: RefCell::new(VecDeque::from_iter(root.into_iter().map(|v| (v, 0)))),
        }
    }

    fn from_iter(iter: impl IntoIterator<Item = &'a RoutingInfo>) -> Self {
        RoutingInfoRecursiveIteratorWithDepth {
            stack: RefCell::new(VecDeque::from_iter(iter.into_iter().map(|v| (v, 0)))),
        }
    }
}
impl<'a> Iterator for RoutingInfoRecursiveIteratorWithDepth<'a> {
    type Item = (&'a RoutingInfo, u32);
    fn next(&mut self) -> Option<Self::Item> {
        let cur = { self.stack.borrow_mut().pop_front() };
        if let Some(cur) = cur {
            if let Some(relations) = cur.0.get_relations() {
                let mut stack = self.stack.borrow_mut();
                for c in relations.iter() {
                    stack.push_back((c, cur.1 + 1));
                }
            }
            return Some(cur);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive::*;
    use std::rc::Rc;

    fn get_sid(n: u8) -> [u8; 32] {
        let mut sid = [0u8; 32];
        sid[0] = n;
        sid
    }
    fn gs(last_connect_time: u64, last_disconnect_time: u64) -> GatewayState {
        GatewayState {
            last_connect_time,
            last_disconnect_time,
        }
    }

    fn create_test_tree() -> RoutingInfo {
        RoutingInfo {
            gateway_state: Some(gs(1, 0)),
            node_type: NodeType::Normal.into(),
            relation: Some(routing_info::Relation::RoutingInfos(RoutingInfos {
                infos: vec![
                    RoutingInfo {
                        session_id: Some(get_sid(2).into()),
                        gateway_state: Some(gs(2, 0)),
                        node_type: NodeType::Normal.into(),
                        relation: Some(routing_info::Relation::RoutingInfos(RoutingInfos {
                            infos: vec![
                                RoutingInfo {
                                    session_id: Some(get_sid(3).into()),
                                    gateway_state: Some(gs(3, 0)),
                                    node_type: NodeType::Normal.into(),
                                    ..Default::default()
                                },
                                RoutingInfo {
                                    session_id: Some(get_sid(4).into()),
                                    gateway_state: Some(gs(4, 0)),
                                    node_type: NodeType::Normal.into(),
                                    ..Default::default()
                                },
                            ],
                        })),
                        ..Default::default()
                    },
                    RoutingInfo {
                        session_id: Some(get_sid(5).into()),
                        gateway_state: Some(gs(13, 0)),
                        node_type: NodeType::Normal.into(),
                        relation: Some(routing_info::Relation::RoutingInfos(RoutingInfos {
                            infos: vec![
                                RoutingInfo {
                                    session_id: Some(get_sid(14).into()),
                                    gateway_state: Some(gs(14, 0)),
                                    node_type: NodeType::Normal.into(),
                                    ..Default::default()
                                },
                                RoutingInfo {
                                    session_id: Some(get_sid(15).into()),
                                    gateway_state: Some(gs(15, 0)),
                                    node_type: NodeType::Normal.into(),
                                    ..Default::default()
                                },
                            ],
                        })),
                        ..Default::default()
                    },
                ],
            })),
            ..Default::default()
        }
    }

    #[test]
    fn test_recursive_iter() {
        let tree = create_test_tree();
        let mut iter = tree.recursive_iter();
        assert_eq!(
            iter.next()
                .unwrap()
                .gateway_state
                .as_ref()
                .unwrap()
                .last_connect_time,
            1
        );
        assert_eq!(
            iter.next()
                .unwrap()
                .gateway_state
                .as_ref()
                .unwrap()
                .last_connect_time,
            2
        );
        assert_eq!(
            iter.next()
                .unwrap()
                .gateway_state
                .as_ref()
                .unwrap()
                .last_connect_time,
            13
        );
        assert_eq!(
            iter.next()
                .unwrap()
                .gateway_state
                .as_ref()
                .unwrap()
                .last_connect_time,
            3
        );
        assert_eq!(
            iter.next()
                .unwrap()
                .gateway_state
                .as_ref()
                .unwrap()
                .last_connect_time,
            4
        );
        assert_eq!(
            iter.next()
                .unwrap()
                .gateway_state
                .as_ref()
                .unwrap()
                .last_connect_time,
            14
        );
        assert_eq!(
            iter.next()
                .unwrap()
                .gateway_state
                .as_ref()
                .unwrap()
                .last_connect_time,
            15
        );
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }
    #[test]
    fn test_fold() {
        let tree = create_test_tree();

        assert_eq!(
            tree.fold(0, |acc, v| {
                if v.gateway_state.as_ref().unwrap().last_connect_time % 2 == 1 {
                    acc + 1
                } else {
                    acc
                }
            }),
            4
        );
        assert_eq!(
            tree.fold(0, |acc, v| {
                if v.gateway_state.as_ref().unwrap().last_connect_time % 2 == 0 {
                    acc + 1
                } else {
                    acc
                }
            }),
            3
        );
        assert_eq!(tree.fold(0, |acc, _| acc + 1), 7);
        assert_eq!(
            tree.fold(0, |acc, v| acc
                + v.gateway_state.as_ref().unwrap().last_connect_time),
            52
        );
    }
    #[test]
    fn test_recursive_iter_1() {
        let tree = create_test_tree();

        let mut counter = 0;
        tree.recursive_iter().for_each(|v| {
            if v.gateway_state.as_ref().unwrap().last_connect_time % 2 == 1 {
                counter += 1;
            }
        });
        assert_eq!(counter, 4);

        counter = 0;
        tree.recursive_iter().for_each(|v| {
            if v.gateway_state.as_ref().unwrap().last_connect_time % 2 == 0 {
                counter += 1;
            }
        });
        assert_eq!(counter, 3);
    }
    #[test]
    fn test_recursive_iter_2() {
        let tree: &[std::rc::Rc<RoutingInfo>] =
            &[Rc::new(create_test_tree()), Rc::new(create_test_tree())];
        let mut counter = 0;
        tree.recursive_iter().for_each(|v| {
            if v.gateway_state.as_ref().unwrap().last_connect_time % 2 == 1 {
                counter += 1;
            }
        });
        assert_eq!(counter, 8);
    }
    #[test]
    fn test_recursive_iter_3() {
        let tree = vec![Rc::new(create_test_tree()), Rc::new(create_test_tree())];
        let mut counter = 0;
        tree.recursive_iter().for_each(|v| {
            if v.gateway_state.as_ref().unwrap().last_connect_time % 2 == 1 {
                counter += 1;
            }
        });
        assert_eq!(counter, 8);
    }
    #[test]
    fn test_recursive_iter_with_depth() {
        let tree = vec![Rc::new(create_test_tree()), Rc::new(create_test_tree())];
        let mut iter = tree.recursive_iter_with_depth();

        for _ in 0..2 {
            let v = iter.next().unwrap();
            assert_eq!(v.1, 0);
        }
        for _ in 0..2 {
            let v = iter.next().unwrap();
            assert_eq!(v.0.session_id, Some(get_sid(2).into()));
            assert_eq!(v.1, 1);

            let v = iter.next().unwrap();
            assert_eq!(v.0.session_id, Some(get_sid(5).into()));
            assert_eq!(v.1, 1);
        }
        for _ in 0..2 {
            let v = iter.next().unwrap();
            assert_eq!(v.0.session_id, Some(get_sid(3).into()));
            assert_eq!(v.1, 2);

            let v = iter.next().unwrap();
            assert_eq!(v.0.session_id, Some(get_sid(4).into()));
            assert_eq!(v.1, 2);

            let v = iter.next().unwrap();
            assert_eq!(v.0.session_id, Some(get_sid(14).into()));
            assert_eq!(v.1, 2);

            let v = iter.next().unwrap();
            assert_eq!(v.0.session_id, Some(get_sid(15).into()));
            assert_eq!(v.1, 2);
        }
        assert_eq!(iter.next(), None);
    }
    #[test]
    fn test_get_known_gateway_session_ids() {
        let f = |session_id: Vec<u8>, ttl: u32| SessionIdWithTtl { session_id, ttl };

        let ar = vec![
            Rc::new(RoutingInfo {
                session_id: Some(vec![1]),
                node_type: NodeType::Gateway.into(),
                known_gateway_session_ids: vec![f(vec![2], 3), f(vec![3], 2)],
                ..Default::default()
            }),
            Rc::new(RoutingInfo {
                session_id: Some(vec![2]),
                node_type: NodeType::Gateway.into(),
                known_gateway_session_ids: vec![f(vec![1], 1), f(vec![3], 1)],
                ..Default::default()
            }),
            Rc::new(RoutingInfo {
                session_id: Some(vec![4]),
                node_type: NodeType::Gateway.into(),
                known_gateway_session_ids: vec![f(vec![3], 2)],
                ..Default::default()
            }),
            Rc::new(RoutingInfo {
                session_id: Some(vec![5]),
                node_type: NodeType::Normal.into(),
                known_gateway_session_ids: vec![f(vec![3], 2), f(vec![6], 2)],
                ..Default::default()
            }),
            Rc::new(RoutingInfo {
                session_id: Some(vec![6]),
                node_type: NodeType::Normal.into(),
                ..Default::default()
            }),
        ];
        let res = ar.get_known_gateway_session_ids();

        assert_eq!(res.len(), 4);
        assert_eq!(
            res.iter().find(|v| v.session_id[0] == 1).unwrap().ttl,
            INITIAL_TTL
        );
        assert_eq!(
            res.iter().find(|v| v.session_id[0] == 2).unwrap().ttl,
            INITIAL_TTL
        );
        assert_eq!(res.iter().find(|v| v.session_id[0] == 3).unwrap().ttl, 1);
        assert_eq!(
            res.iter().find(|v| v.session_id[0] == 4).unwrap().ttl,
            INITIAL_TTL
        );

        let res = ar.get_known_gateway_session_ids_next_ttl();
        assert_eq!(res.len(), 3);
        assert_eq!(
            res.iter().find(|v| v.session_id[0] == 1).unwrap().ttl,
            INITIAL_TTL - 1
        );
        assert_eq!(
            res.iter().find(|v| v.session_id[0] == 2).unwrap().ttl,
            INITIAL_TTL - 1
        );
        assert_eq!(
            res.iter().find(|v| v.session_id[0] == 4).unwrap().ttl,
            INITIAL_TTL - 1
        );

        let ar = vec![
            Rc::new(RoutingInfo {
                session_id: Some(vec![1]),
                node_type: NodeType::Gateway.into(),
                known_gateway_session_ids: vec![f(vec![2], 3), f(vec![3], 2)],
                ..Default::default()
            }),
            Rc::new(RoutingInfo {
                session_id: Some(vec![1]),
                node_type: NodeType::Tracker.into(),
                known_gateway_session_ids: vec![f(vec![2], 3), f(vec![3], 2)],
                relation: Some(routing_info::Relation::RoutingInfos(RoutingInfos {
                    infos: vec![
                        RoutingInfo {
                            session_id: Some(get_sid(100).into()),
                            node_type: NodeType::Gateway.into(),
                            ..Default::default()
                        },
                        RoutingInfo {
                            session_id: Some(get_sid(101).into()),
                            node_type: NodeType::Gateway.into(),
                            ..Default::default()
                        },
                    ],
                })),
                ..Default::default()
            }),
        ];
        let res = ar.get_known_gateway_session_ids();
        assert_eq!(res.len(), 2);
        assert_eq!(
            res.iter().find(|v| v.session_id[0] == 100).unwrap().ttl,
            INITIAL_TTL
        );
        assert_eq!(
            res.iter().find(|v| v.session_id[0] == 101).unwrap().ttl,
            INITIAL_TTL
        );
    }
    #[test]
    fn test_is_known() {
        let tree = create_test_tree();
        assert!(tree.is_known(get_sid(2)));
        assert!(tree.is_known(get_sid(5)));
        assert!(tree.is_known(get_sid(3)));
        assert!(!tree.is_known(get_sid(10)));
        assert!(!tree.is_known(get_sid(0)));

        let tree = RoutingInfo {
            session_id: Some(get_sid(1).into()),
            node_type: NodeType::Gateway.into(),
            ..Default::default()
        };
        assert!(tree.is_known(get_sid(1)));
    }
    #[test]
    fn test_add_known() {
        let f = |session_id: Vec<u8>, ttl: u32| SessionIdWithTtl { session_id, ttl };
        let tree = RoutingInfo {
            session_id: Some(get_sid(1).into()),
            node_type: NodeType::Normal.into(),
            position: Some(Position3D::from_xyz(1.0, 2.0, 3.0)),
            gateway_state: Some(gs(4, 0)),
            known_gateway_session_ids: vec![f(vec![2], 1)],
            ..Default::default()
        };
        assert!(tree.is_known(get_sid(1)));
        assert!(!tree.is_known(get_sid(2)));
        assert!(!tree.is_known(get_sid(3)));

        let tree = tree.add_known(get_sid(2));
        assert!(tree.is_known(get_sid(2)));

        let tree = tree.add_known(get_sid(10));
        assert!(tree.is_known(get_sid(10)));

        assert_eq!(tree.session_id, Some(get_sid(1).into()));
        assert_eq!(tree.node_type(), NodeType::Normal);
        assert_eq!(tree.position.unwrap(), Position3D::from_xyz(1.0, 2.0, 3.0));
        assert_eq!(tree.gateway_state.unwrap().last_connect_time, 4);
        assert_eq!(tree.known_gateway_session_ids, vec![f(vec![2], 1)]);
    }
    #[test]
    fn test_to_send_data() {
        let tree = RoutingInfo {
            ..Default::default()
        };
        assert_eq!(tree.get_relation_count(), 0);
        assert!(tree.relation.is_none());

        let tree = tree.add_known(get_sid(2));
        assert_eq!(tree.get_relation_count(), 1);

        let tree = tree.add_known(get_sid(10));
        assert_eq!(tree.get_relation_count(), 2);
        assert!(tree.has_routing_infos());
        assert!(!tree.has_count());

        let tree = tree.to_send_data(0);
        assert_eq!(tree.get_relation_count(), 2);
        assert!(!tree.has_routing_infos());
        assert!(tree.has_count());

        let tree = create_test_tree();
        let res = tree.to_send_data(1);
        assert_eq!(res.get_relations().unwrap()[0].get_relation_count(), 2);
        assert!(!res.get_relations().unwrap()[0].has_routing_infos());

        let tree = create_test_tree();
        let res = tree.to_send_data(2);
        assert_eq!(res.get_relations().unwrap()[0].get_relation_count(), 2);
        assert!(res.get_relations().unwrap()[0].has_routing_infos());
    }
    #[test]
    fn test_relation() {
        let mut tree = RoutingInfo {
            ..Default::default()
        };
        tree.set_relations(vec![
            RoutingInfo {
                session_id: Some(vec![1]),
                node_type: NodeType::Gateway.into(),
                ..Default::default()
            },
            RoutingInfo {
                session_id: Some(vec![2]),
                node_type: NodeType::Gateway.into(),
                ..Default::default()
            },
        ]);

        let ar = tree.get_relations().unwrap();
        assert_eq!(ar[0].session_id, Some(vec![1]));
        assert_eq!(ar[1].session_id, Some(vec![2]));
    }
    #[test]
    fn test_get_min_depth() {
        let target_session_id = get_sid(1);
        let tree = RoutingInfo {
            session_id: Some(get_sid(0).into()),
            relation: Some(routing_info::Relation::RoutingInfos(RoutingInfos {
                infos: vec![
                    RoutingInfo {
                        session_id: Some(get_sid(2).into()),
                        relation: Some(routing_info::Relation::RoutingInfos(RoutingInfos {
                            infos: vec![
                                RoutingInfo {
                                    session_id: Some(get_sid(2).into()),
                                    ..Default::default()
                                },
                                RoutingInfo {
                                    session_id: Some(get_sid(3).into()),
                                    relation: Some(routing_info::Relation::RoutingInfos(
                                        RoutingInfos {
                                            infos: vec![RoutingInfo {
                                                session_id: Some(get_sid(1).into()),
                                                ..Default::default()
                                            }],
                                        },
                                    )),
                                    ..Default::default()
                                },
                            ],
                        })),
                        ..Default::default()
                    },
                    RoutingInfo {
                        session_id: Some(get_sid(3).into()),
                        relation: Some(routing_info::Relation::RoutingInfos(RoutingInfos {
                            infos: vec![RoutingInfo {
                                session_id: Some(get_sid(1).into()),
                                ..Default::default()
                            }],
                        })),
                        ..Default::default()
                    },
                ],
            })),
            ..Default::default()
        };
        assert_eq!(tree.get_min_depth(&target_session_id), Some(2));
    }
}
