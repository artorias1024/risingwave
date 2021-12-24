use std::sync::Arc;

use futures::{SinkExt, StreamExt};
use risingwave_common::util::addr::get_host_port;
use risingwave_pb::common::HostAddress;
use risingwave_pb::data::data_type::TypeName;
use risingwave_pb::data::DataType;
use risingwave_pb::plan::ColumnDesc;
use risingwave_pb::stream_plan::stream_node::Node;
use risingwave_pb::stream_plan::*;
use risingwave_pb::stream_service::*;
use risingwave_source::MemSourceManager;

use super::*;
use crate::stream::env::StreamTaskEnv;
use crate::stream::SimpleTableManager;
use crate::stream_op::{Barrier, Message, Mutation};

const PORT: i32 = 2333;

fn helper_make_local_actor(fragment_id: u32) -> ActorInfo {
    ActorInfo {
        fragment_id,
        host: Some(HostAddress {
            host: "127.0.0.1".into(),
            port: PORT,
        }),
    }
}

/// This test creates stream plan protos and feed them into `StreamManager`.
/// There are 5 actors in total, where:
/// * 1 = mock source
/// * 3 = pipe with RR dispatcher
/// * 7, 11 = pipe after dispatcher
/// * 13 = pipe merger
/// * 233 = mock sink
///
/// ```plain
///            /--- 7  ---\
/// 1 --- 3 ---            --- 13 --- 233
///            \--- 11 ---/
/// ```
#[tokio::test]
async fn test_stream_proto() {
    let socket_addr = get_host_port(&format!("127.0.0.1:{}", PORT)).unwrap();
    let stream_manager = StreamManager::new(socket_addr, None);
    let info = [1, 3, 7, 11, 13, 233]
        .iter()
        .cloned()
        .map(helper_make_local_actor)
        .collect::<Vec<_>>();
    stream_manager
        .update_actor_info(BroadcastActorInfoTableRequest { info })
        .unwrap();

    stream_manager
        .update_fragment(&[
            // create 0 -> (1) -> 3
            StreamFragment {
                fragment_id: 1,
                nodes: Some(StreamNode {
                    node: Some(Node::ProjectNode(ProjectNode::default())),
                    input: vec![StreamNode {
                        node: Some(Node::MergeNode(MergeNode {
                            upstream_fragment_id: vec![0],
                            input_column_descs: vec![ColumnDesc {
                                column_type: Some(DataType {
                                    type_name: TypeName::Int32 as i32,
                                    ..Default::default()
                                }),
                                ..Default::default()
                            }],
                        })),
                        input: vec![],
                        pk_indices: vec![],
                    }],
                    pk_indices: vec![],
                }),
                dispatcher: Some(Dispatcher {
                    r#type: dispatcher::DispatcherType::Hash as i32,
                    column_idx: 0,
                }),
                downstream_fragment_id: vec![3],
            },
            // create 1 -> (3) -> 7, 11
            StreamFragment {
                fragment_id: 3,
                nodes: Some(StreamNode {
                    node: Some(Node::ProjectNode(ProjectNode::default())),
                    input: vec![StreamNode {
                        node: Some(Node::MergeNode(MergeNode {
                            upstream_fragment_id: vec![1],
                            input_column_descs: vec![ColumnDesc {
                                column_type: Some(DataType {
                                    type_name: TypeName::Int32 as i32,
                                    ..Default::default()
                                }),
                                ..Default::default()
                            }],
                        })),
                        input: vec![],
                        pk_indices: vec![],
                    }],
                    pk_indices: vec![],
                }),
                dispatcher: Some(Dispatcher {
                    r#type: dispatcher::DispatcherType::Hash as i32,
                    column_idx: 0,
                }),
                downstream_fragment_id: vec![7, 11],
            },
            // create 3 -> (7) -> 13
            StreamFragment {
                fragment_id: 7,
                nodes: Some(StreamNode {
                    node: Some(Node::ProjectNode(ProjectNode::default())),
                    input: vec![StreamNode {
                        node: Some(Node::MergeNode(MergeNode {
                            upstream_fragment_id: vec![3],
                            input_column_descs: vec![ColumnDesc {
                                column_type: Some(DataType {
                                    type_name: TypeName::Int32 as i32,
                                    ..Default::default()
                                }),
                                ..Default::default()
                            }],
                        })),
                        input: vec![],
                        pk_indices: vec![],
                    }],
                    pk_indices: vec![],
                }),
                dispatcher: Some(Dispatcher {
                    r#type: dispatcher::DispatcherType::Hash as i32,
                    column_idx: 0,
                }),
                downstream_fragment_id: vec![13],
            },
            // create 3 -> (11) -> 13
            StreamFragment {
                fragment_id: 11,
                nodes: Some(StreamNode {
                    node: Some(Node::ProjectNode(ProjectNode::default())),
                    input: vec![StreamNode {
                        node: Some(Node::MergeNode(MergeNode {
                            upstream_fragment_id: vec![3],
                            input_column_descs: vec![ColumnDesc {
                                column_type: Some(DataType {
                                    type_name: TypeName::Int32 as i32,
                                    ..Default::default()
                                }),
                                ..Default::default()
                            }],
                        })),
                        input: vec![],
                        pk_indices: vec![],
                    }],
                    pk_indices: vec![],
                }),
                dispatcher: Some(Dispatcher {
                    r#type: dispatcher::DispatcherType::Simple as i32,
                    column_idx: 0,
                }),
                downstream_fragment_id: vec![13],
            },
            // create 7, 11 -> (13) -> 233
            StreamFragment {
                fragment_id: 13,
                nodes: Some(StreamNode {
                    node: Some(Node::ProjectNode(ProjectNode::default())),
                    input: vec![StreamNode {
                        node: Some(Node::MergeNode(MergeNode {
                            upstream_fragment_id: vec![7, 11],
                            input_column_descs: vec![ColumnDesc {
                                column_type: Some(DataType {
                                    type_name: TypeName::Int32 as i32,
                                    ..Default::default()
                                }),
                                ..Default::default()
                            }],
                        })),
                        input: vec![],
                        pk_indices: vec![],
                    }],
                    pk_indices: vec![],
                }),
                dispatcher: Some(Dispatcher {
                    r#type: dispatcher::DispatcherType::Simple as i32,
                    column_idx: 0,
                }),
                downstream_fragment_id: vec![233],
            },
        ])
        .unwrap();

    let env = StreamTaskEnv::new(
        Arc::new(SimpleTableManager::new()),
        Arc::new(MemSourceManager::new()),
        std::net::SocketAddr::V4("127.0.0.1:5688".parse().unwrap()),
    );
    stream_manager
        .build_fragment(&[1, 3, 7, 11, 13], env)
        .unwrap();

    let mut source = stream_manager.take_source();
    let mut sink = stream_manager.take_sink((13, 233));

    let consumer = tokio::spawn(async move {
        for _epoch in 0..100 {
            assert!(matches!(
                sink.next().await.unwrap(),
                Message::Barrier(Barrier {
                    epoch: _,
                    mutation: Mutation::Nothing
                })
            ));
        }
        assert!(matches!(
            sink.next().await.unwrap(),
            Message::Barrier(Barrier {
                epoch: 0,
                mutation: Mutation::Stop
            })
        ));
    });

    let timeout = tokio::time::Duration::from_millis(10);

    for epoch in 0..100 {
        tokio::time::timeout(
            timeout,
            source.send(Message::Barrier(Barrier {
                epoch,
                ..Barrier::default()
            })),
        )
        .await
        .expect("timeout while sending barrier message")
        .unwrap();
    }

    tokio::time::timeout(
        timeout,
        source.send(Message::Barrier(Barrier {
            epoch: 0,
            mutation: Mutation::Stop,
        })),
    )
    .await
    .expect("timeout while sending terminate message")
    .unwrap();

    tokio::time::timeout(timeout, consumer)
        .await
        .expect("timeout while waiting for sink")
        .unwrap();

    tokio::time::timeout(timeout, stream_manager.wait_all())
        .await
        .expect("timeout while waiting for processor stop")
        .unwrap();
}
