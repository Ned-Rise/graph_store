use std::marker::PhantomData;

use actix::{Actor, Context, Handler, Message};
use rusqlite::{params, Connection};

use crate::{GraphEdge, GraphMutation, GraphNode, GraphNodeIndex, StoreError};

const DEFAULT_GRAPH_LOG_PATH: &str = "data/graph_store/log";

pub enum MutationsLogStoreMessage<N, E, I>
where
    N: GraphNode + Unpin + 'static,
    E: GraphEdge + Unpin + 'static,
    I: GraphNodeIndex + From<N> + Unpin + 'static,
{
    Add(GraphMutation<N, E, I>),
    Commit(GraphMutation<N, E, I>),
}

impl<N, E, I> Message for MutationsLogStoreMessage<N, E, I>
where
    N: GraphNode + Unpin + 'static,
    E: GraphEdge + Unpin + 'static,
    I: GraphNodeIndex + From<N> + Unpin + 'static,
{
    type Result = Result<usize, StoreError>;
}

pub struct MutationsLogStore<N, E, I>
where
    N: GraphNode + Unpin + 'static,
    E: GraphEdge + Unpin + 'static,
    I: GraphNodeIndex + From<N> + Unpin + 'static,
{
    conn: Connection,
    phantom_n: PhantomData<N>,
    phantom_e: PhantomData<E>,
    phantom_i: PhantomData<I>,
}

impl<N, E, I> MutationsLogStore<N, E, I>
where
    N: GraphNode + Unpin + 'static,
    E: GraphEdge + Unpin + 'static,
    I: GraphNodeIndex + From<N> + Unpin + 'static,
{
    pub fn new(store_path: Option<String>) -> Self {
        let mutations_log_path = match store_path {
            Some(path) => format!("{}/log.sqlite", path),
            None => DEFAULT_GRAPH_LOG_PATH.to_string(),
        };

        let conn = Connection::open(mutations_log_path).unwrap();
        Self::init_log_table(&conn).unwrap();

        Self {
            conn,
            phantom_n: PhantomData,
            phantom_e: PhantomData,
            phantom_i: PhantomData,
        }
    }

    fn init_log_table(conn: &Connection) -> Result<(), rusqlite::Error> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS mutations_log (
            id CHAR(32) PRIMARY KEY NOT NULL UNIQUE,
            mutation BLOB NOT NULL,
            committed BOOLEAN NOT NULL
        )",
            (),
        )?;
        Ok(())
    }
}

impl<N, E, I> Actor for MutationsLogStore<N, E, I>
where
    N: GraphNode + Unpin + 'static,
    E: GraphEdge + Unpin + 'static,
    I: GraphNodeIndex + From<N> + Unpin + 'static,
{
    type Context = Context<Self>;
}

impl<N, E, I> Handler<MutationsLogStoreMessage<N, E, I>>
    for MutationsLogStore<N, E, I>
where
    N: GraphNode + Unpin + 'static,
    E: GraphEdge + Unpin + 'static,
    I: GraphNodeIndex + From<N> + Unpin + 'static,
{
    type Result = Result<usize, StoreError>;

    fn handle(
        &mut self,
        msg: MutationsLogStoreMessage<N, E, I>,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        match msg {
            MutationsLogStoreMessage::Add(graph_mutation) => {
                let hash = graph_mutation.get_hash();
                let mutation: Vec<u8> = graph_mutation.try_into()?;

                match self.conn.execute(
                    "INSERT INTO mutations_log (id, mutation, committed)
                        VALUES (?1, ?2, ?3)",
                    params![hash, mutation, false],
                ) {
                    Ok(n) => Ok(n),
                    Err(err) => {
                        log::error!(
                            "[MutationsLogStore.handle<Add>] Error: {}",
                            err
                        );
                        Err(StoreError::WriteLogError(err.to_string()))
                    }
                }
            }
            MutationsLogStoreMessage::Commit(graph_mutation) => {
                let hash = graph_mutation.get_hash();
                let mutation: Vec<u8> = graph_mutation.try_into()?;

                match self.conn.execute(
                    "INSERT INTO mutations_log (id, mutation, committed)
                        VALUES (?1, ?2, ?3)
                     ON CONFLICT(id) DO UPDATE SET committed = true
                        WHERE id = ?1",
                    params![hash, mutation, true],
                ) {
                    Ok(n) => Ok(n),
                    Err(err) => {
                        log::error!("Error: {}", err);
                        Err(StoreError::WriteLogError(err.to_string()))
                    }
                }
            }
        }
    }
}